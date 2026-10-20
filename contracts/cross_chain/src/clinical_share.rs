//! # Cross-chain clinical data sharing protocol (issue #322)
//!
//! A patient on this chain can share one vision record with a grantee on a
//! foreign chain. The sharing flow is:
//!
//! 1. The patient encrypts the record off-chain with an AEAD cipher under a
//!    key agreed with the grantee. The ciphertext is never decrypted on-chain.
//! 2. The patient builds a [`ClinicalShareMessage`] that commits to the
//!    ciphertext with `payload_hash = SHA-256(ciphertext)`.
//! 3. `share_clinical_record` checks, in order:
//!    - the patient authorised the call and owns the message,
//!    - the message is well formed, unexpired and within the protocol version,
//!    - the ciphertext matches the committed hash,
//!    - the message ID has not been used before (replay protection),
//!    - the vision_records contract reports an active grant for this
//!      `(record_id, grantee)` pair (consent verification).
//! 4. On success the message is recorded in an append-only audit log and a
//!    `SHARE` event carrying the message and ciphertext is emitted for relayers.
//!
//! Relayers deliver the event to the target chain. The target chain verifies
//! the same commitment with [`verify_clinical_share`](crate::CrossChainContract::verify_clinical_share)
//! before decrypting.
//!
//! ## Storage key layout
//!
//! | Key tuple                    | Value                  |
//! |------------------------------|------------------------|
//! | `("SHR_MSG", message_id)`    | `true` (replay guard)  |
//! | `("SHR_AUD", sequence: u64)` | [`ShareAuditEntry`]    |
//! | `"SHR_CTR"` (instance)       | `u64` next audit index |

use soroban_sdk::{
    contracttype, symbol_short, vec, Address, Bytes, BytesN, Env, Error, IntoVal, String, Symbol,
};

use crate::events;
use crate::{CrossChainError, TTL_EXTEND_TO, TTL_THRESHOLD};

/// Protocol version this contract understands. Messages with any other
/// version are rejected.
pub const SHARE_PROTOCOL_VERSION: u32 = 1;

/// Maximum ciphertext size in bytes. Bounds event size and storage cost.
pub const MAX_SHARE_PAYLOAD_BYTES: u32 = 4_096;

/// Maximum lifetime of a message, in seconds (24 hours).
pub const MAX_SHARE_TTL_SECONDS: u64 = 86_400;

const AUDIT_COUNTER: Symbol = symbol_short!("SHR_CTR");

/// Wire format for one clinical share.
///
/// Field notes:
/// * `message_id`   — unique per message; the replay guard keys on it.
/// * `target_chain` — foreign chain identifier the ciphertext is destined for.
/// * `patient`      — local patient address that owns the record.
/// * `grantee`      — principal on the target chain who receives the data.
/// * `record_id`    — vision_records record being shared.
/// * `nonce`        — 12-byte AEAD nonce, non-zero.
/// * `payload_hash` — SHA-256 of the ciphertext.
/// * `issued_at` / `expires_at` — UNIX seconds; the window is at most
///   [`MAX_SHARE_TTL_SECONDS`] wide.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClinicalShareMessage {
    pub version: u32,
    pub message_id: BytesN<32>,
    pub target_chain: String,
    pub patient: Address,
    pub grantee: Address,
    pub record_id: u64,
    pub nonce: BytesN<12>,
    pub payload_hash: BytesN<32>,
    pub issued_at: u64,
    pub expires_at: u64,
}

/// One entry in the append-only audit log of completed shares.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShareAuditEntry {
    pub sequence: u64,
    pub message_id: BytesN<32>,
    pub patient: Address,
    pub grantee: Address,
    pub record_id: u64,
    pub target_chain: String,
    pub payload_hash: BytesN<32>,
    pub recorded_at: u64,
}

/// Mirror of `vision_records::AccessLevel`. Soroban encodes unit variants by
/// name, so this decodes the value returned by the vision_records contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ConsentLevel {
    None,
    Read,
    Write,
    Full,
    Admin,
}

/// Check a message without changing any state.
///
/// Used by relayers and by receiving chains to confirm that a ciphertext
/// matches its commitment before decrypting.
pub fn validate_message(
    env: &Env,
    message: &ClinicalShareMessage,
    ciphertext: &Bytes,
) -> Result<(), CrossChainError> {
    if message.version != SHARE_PROTOCOL_VERSION {
        return Err(CrossChainError::UnsupportedVersion);
    }
    if message.target_chain.is_empty() {
        return Err(CrossChainError::InvalidMessage);
    }
    if ciphertext.is_empty() {
        return Err(CrossChainError::InvalidMessage);
    }
    if ciphertext.len() > MAX_SHARE_PAYLOAD_BYTES {
        return Err(CrossChainError::PayloadTooLarge);
    }
    if message.nonce.to_array() == [0u8; 12] {
        return Err(CrossChainError::InvalidMessage);
    }

    match message.expires_at.checked_sub(message.issued_at) {
        Some(ttl) if ttl > 0 && ttl <= MAX_SHARE_TTL_SECONDS => {}
        _ => return Err(CrossChainError::InvalidMessage),
    }

    let now = env.ledger().timestamp();
    if now < message.issued_at {
        return Err(CrossChainError::InvalidMessage);
    }
    if now > message.expires_at {
        return Err(CrossChainError::MessageExpired);
    }

    let digest: BytesN<32> = env.crypto().sha256(ciphertext).into();
    if digest != message.payload_hash {
        return Err(CrossChainError::PayloadMismatch);
    }

    Ok(())
}

/// Validate, verify consent, record the share and emit the relay event.
pub fn share_record(
    env: &Env,
    patient: Address,
    message: ClinicalShareMessage,
    ciphertext: Bytes,
    vision_contract: Address,
) -> Result<BytesN<32>, CrossChainError> {
    patient.require_auth();
    if message.patient != patient {
        return Err(CrossChainError::Unauthorized);
    }

    validate_message(env, &message, &ciphertext)?;

    let replay_key = (symbol_short!("SHR_MSG"), message.message_id.clone());
    if env.storage().persistent().has(&replay_key) {
        return Err(CrossChainError::AlreadyProcessed);
    }

    verify_record_consent(env, &vision_contract, message.record_id, &message.grantee)?;

    env.storage().persistent().set(&replay_key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&replay_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    append_audit_entry(env, &message)?;
    events::emit_clinical_share(env, &message, ciphertext);

    Ok(message.message_id)
}

/// Ask the vision_records contract whether `grantee` may read `record_id`.
///
/// Consent is record-scoped on purpose: a patient-wide grant alone does not
/// authorise sending one record to a foreign chain.
fn verify_record_consent(
    env: &Env,
    vision_contract: &Address,
    record_id: u64,
    grantee: &Address,
) -> Result<(), CrossChainError> {
    let result = env.try_invoke_contract::<ConsentLevel, Error>(
        vision_contract,
        &Symbol::new(env, "check_record_access"),
        vec![env, record_id.into_val(env), grantee.into_val(env)],
    );

    match result {
        Ok(Ok(ConsentLevel::None)) => Err(CrossChainError::ConsentRequired),
        Ok(Ok(_)) => Ok(()),
        _ => Err(CrossChainError::ExternalCallFailed),
    }
}

fn append_audit_entry(env: &Env, message: &ClinicalShareMessage) -> Result<(), CrossChainError> {
    let sequence: u64 = env.storage().instance().get(&AUDIT_COUNTER).unwrap_or(0);
    let next = sequence
        .checked_add(1)
        .ok_or(CrossChainError::InvalidMessage)?;

    let entry = ShareAuditEntry {
        sequence,
        message_id: message.message_id.clone(),
        patient: message.patient.clone(),
        grantee: message.grantee.clone(),
        record_id: message.record_id,
        target_chain: message.target_chain.clone(),
        payload_hash: message.payload_hash.clone(),
        recorded_at: env.ledger().timestamp(),
    };

    let key = (symbol_short!("SHR_AUD"), sequence);
    env.storage().persistent().set(&key, &entry);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    env.storage().instance().set(&AUDIT_COUNTER, &next);

    Ok(())
}

/// Read one audit entry by its sequence number.
pub fn get_audit_entry(env: &Env, sequence: u64) -> Option<ShareAuditEntry> {
    env.storage()
        .persistent()
        .get(&(symbol_short!("SHR_AUD"), sequence))
}

/// Number of audit entries written so far.
pub fn audit_count(env: &Env) -> u64 {
    env.storage().instance().get(&AUDIT_COUNTER).unwrap_or(0)
}
