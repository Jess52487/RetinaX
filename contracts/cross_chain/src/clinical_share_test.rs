//! Tests for the cross-chain clinical sharing protocol (issue #322).

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

use crate::clinical_share::{
    ClinicalShareMessage, MAX_SHARE_PAYLOAD_BYTES, MAX_SHARE_TTL_SECONDS, SHARE_PROTOCOL_VERSION,
};
use crate::{CrossChainContract, CrossChainContractClient, CrossChainError};
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, testutils::Address as _, Address, Bytes,
    BytesN, Env, String,
};

/// Mirrors `vision_records::AccessLevel`; Soroban encodes unit variants by name.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockAccessLevel {
    None,
    Read,
    Write,
    Full,
    Admin,
}

/// Stands in for the vision_records contract's consent check.
#[contract]
struct MockVisionRecords;

#[contractimpl]
impl MockVisionRecords {
    pub fn set_consent(env: Env, allowed: bool) {
        env.storage()
            .instance()
            .set(&symbol_short!("CONSENT"), &allowed);
    }

    pub fn check_record_access(env: Env, record_id: u64, grantee: Address) -> MockAccessLevel {
        let _ = (record_id, grantee);
        let allowed: bool = env
            .storage()
            .instance()
            .get(&symbol_short!("CONSENT"))
            .unwrap_or(false);
        if allowed {
            MockAccessLevel::Read
        } else {
            MockAccessLevel::None
        }
    }
}

struct Fixture {
    env: Env,
    client: CrossChainContractClient<'static>,
    vision: Address,
    consent: MockVisionRecordsClient<'static>,
    patient: Address,
    grantee: Address,
    ciphertext: Bytes,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);

    let vision = env.register(MockVisionRecords, ());
    let consent = MockVisionRecordsClient::new(&env, &vision);
    consent.set_consent(&true);

    let patient = Address::generate(&env);
    let grantee = Address::generate(&env);
    let ciphertext = Bytes::from_slice(&env, &[0xAB; 64]);

    Fixture {
        env,
        client,
        vision,
        consent,
        patient,
        grantee,
        ciphertext,
    }
}

fn message(
    env: &Env,
    patient: &Address,
    grantee: &Address,
    ciphertext: &Bytes,
    id: u8,
) -> ClinicalShareMessage {
    let digest: BytesN<32> = env.crypto().sha256(ciphertext).into();
    let now = env.ledger().timestamp();
    ClinicalShareMessage {
        version: SHARE_PROTOCOL_VERSION,
        message_id: BytesN::from_array(env, &[id; 32]),
        target_chain: String::from_str(env, "ethereum"),
        patient: patient.clone(),
        grantee: grantee.clone(),
        record_id: 42,
        nonce: BytesN::from_array(env, &[1u8; 12]),
        payload_hash: digest,
        issued_at: now,
        expires_at: now + 3_600,
    }
}

#[test]
fn share_succeeds_and_writes_audit_entry() {
    let f = setup();
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 1);

    let id = f
        .client
        .share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision);

    assert_eq!(id, msg.message_id);
    assert_eq!(f.client.get_share_audit_count(), 1);

    let entry = f.client.get_share_audit(&0).unwrap();
    assert_eq!(entry.record_id, 42);
    assert_eq!(entry.patient, f.patient);
    assert_eq!(entry.grantee, f.grantee);
    assert_eq!(entry.payload_hash, msg.payload_hash);
    assert_eq!(entry.target_chain, String::from_str(&f.env, "ethereum"));

    assert!(f.client.get_share_audit(&1).is_none());
}

#[test]
fn verify_accepts_matching_ciphertext_and_rejects_tampered_bytes() {
    let f = setup();
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 2);

    f.client.verify_clinical_share(&msg, &f.ciphertext);

    let tampered = Bytes::from_slice(&f.env, &[0xCD; 64]);
    assert_eq!(
        f.client.try_verify_clinical_share(&msg, &tampered),
        Err(Ok(CrossChainError::PayloadMismatch))
    );
}

#[test]
fn share_rejects_ciphertext_that_does_not_match_commitment() {
    let f = setup();
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 3);
    let tampered = Bytes::from_slice(&f.env, &[0xCD; 64]);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &tampered, &f.vision),
        Err(Ok(CrossChainError::PayloadMismatch))
    );
    assert_eq!(f.client.get_share_audit_count(), 0);
}

#[test]
fn share_requires_active_record_consent() {
    let f = setup();
    f.consent.set_consent(&false);
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 4);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::ConsentRequired))
    );
    assert_eq!(f.client.get_share_audit_count(), 0);
}

#[test]
fn share_rejects_replayed_message() {
    let f = setup();
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 5);

    f.client
        .share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::AlreadyProcessed))
    );
    assert_eq!(f.client.get_share_audit_count(), 1);
}

#[test]
fn share_rejects_expired_message() {
    let f = setup();
    let msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 6);

    f.env
        .ledger()
        .with_mut(|li| li.timestamp = msg.expires_at + 1);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::MessageExpired))
    );
}

#[test]
fn share_rejects_caller_who_is_not_the_message_patient() {
    let f = setup();
    let other = Address::generate(&f.env);
    let msg = message(&f.env, &other, &f.grantee, &f.ciphertext, 7);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::Unauthorized))
    );
}

#[test]
fn share_rejects_zero_nonce() {
    let f = setup();
    let mut msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 8);
    msg.nonce = BytesN::from_array(&f.env, &[0u8; 12]);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::InvalidMessage))
    );
}

#[test]
fn share_rejects_unsupported_protocol_version() {
    let f = setup();
    let mut msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 9);
    msg.version = SHARE_PROTOCOL_VERSION + 1;

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::UnsupportedVersion))
    );
}

#[test]
fn share_rejects_window_longer_than_maximum_ttl() {
    let f = setup();
    let mut msg = message(&f.env, &f.patient, &f.grantee, &f.ciphertext, 10);
    msg.expires_at = msg.issued_at + MAX_SHARE_TTL_SECONDS + 1;

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &f.ciphertext, &f.vision),
        Err(Ok(CrossChainError::InvalidMessage))
    );
}

#[test]
fn share_rejects_oversized_ciphertext() {
    let f = setup();
    let mut big = Bytes::new(&f.env);
    for _ in 0..(MAX_SHARE_PAYLOAD_BYTES + 1) {
        big.push_back(0xEE);
    }
    let msg = message(&f.env, &f.patient, &f.grantee, &big, 11);

    assert_eq!(
        f.client
            .try_share_clinical_record(&f.patient, &msg, &big, &f.vision),
        Err(Ok(CrossChainError::PayloadTooLarge))
    );
}
