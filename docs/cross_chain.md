# Cross-Chain Medical Records Access

## Overview
This document describes the architectural design and message passing protocol for the `cross_chain` module in the RetinaX-Org contracts ecosystem. It aims to fulfill interoperability requirements (Issue #40) by enabling foreign blockchains to safely interact with Vision records.

## Message Passing Protocol
The cross-chain bridge is designed around a trusted relayer network. Given the constraints of Soroban's lightweight execution environment, heavy zero-knowledge or light-client proofs of foreign chains are delegated. Instead, relayers—which are authenticated entities—submit validated messages.

Each message contains:
- `source_chain` (String): e.g., "ethereum", "polygon".
- `source_address` (String): e.g., "0xabc123...".
- `target_action` (Symbol): e.g., `GRANT` for granting access to records.
- `payload` (Bytes): Additional payload data for the action.

The `process_message` endpoint additionally accepts a separate `message_id: Bytes` argument (a unique identifier or hash of the message) used to prevent replay attacks.

## Identity Mapping
Because users on foreign chains use different wallet addressing formats (e.g., Ethereum 20-byte addresses vs. Stellar/Soroban 32-byte Ed25519 public keys or contract addresses), we provide an identity mapping endpoint:
```rust
fn map_identity(admin, foreign_chain, foreign_address, local_address)
```
This maps `(foreign_chain, foreign_address)` directly to the user's `local_address` on Soroban. When a cross-chain message is processed, this map is consulted so the `cross_chain` contract can act on the correct local patient's behalf.

## Access Control and Verification
- **Relayers**: Only explicitly whitelisted relayers (added via `add_relayer`) can invoke `process_message`.
- **Replay Protection**: The message ID is saved in storage (`PROCESSED_MESSAGES`) upon execution. If a relayer submits the same message twice, the transaction simply reverts.
- **Cross-Contract Authorization**: When granting access to a third-party (`GRANT`), the bridge forwards the action to the `vision_records` contract. Standard RBAC authorization applies; the patient must have delegated permissions to the bridge or the bridge must be recognized as a system administrator to manage access on their behalf.

## Clinical data sharing protocol (issue #322)

A patient can share one vision record with a grantee on a foreign chain.
The protocol lives in `contracts/cross_chain/src/clinical_share.rs`.

### Message format

`ClinicalShareMessage` (protocol version `SHARE_PROTOCOL_VERSION = 1`):

| Field          | Type           | Meaning                                                    |
|----------------|----------------|------------------------------------------------------------|
| `version`      | `u32`          | Must equal `SHARE_PROTOCOL_VERSION`.                       |
| `message_id`   | `BytesN<32>`   | Unique per message. Replay protection keys on it.          |
| `target_chain` | `String`       | Foreign chain the ciphertext is destined for.              |
| `patient`      | `Address`      | Local patient who owns the record. Must authorise the call.|
| `grantee`      | `Address`      | Principal who receives the data.                           |
| `record_id`    | `u64`          | Vision record being shared.                                |
| `nonce`        | `BytesN<12>`   | AEAD nonce. Must not be all zeros.                         |
| `payload_hash` | `BytesN<32>`   | SHA-256 of the ciphertext.                                 |
| `issued_at`    | `u64`          | UNIX seconds.                                              |
| `expires_at`   | `u64`          | UNIX seconds. Window is at most 86 400 seconds.            |

The ciphertext is sent alongside the message. It is at most
`MAX_SHARE_PAYLOAD_BYTES` (4 096) bytes and is never decrypted on-chain.

### Flow

1. The patient encrypts the record off-chain with an AEAD cipher, using a key
   agreed with the grantee.
2. The patient calls `share_clinical_record(patient, message, ciphertext, vision_contract)`.
3. The contract checks, in order:
   - the caller is `message.patient` and has authorised the call,
   - the version, size, nonce, and time window are valid,
   - `SHA-256(ciphertext) == payload_hash`,
   - `message_id` has not been used,
   - `vision_contract.check_record_access(record_id, grantee)` is not `None`.
4. On success the message ID is marked used, an audit entry is appended, and a
   `SHARE` event carrying the message and ciphertext is emitted for relayers.

Receiving chains and relayers call `verify_clinical_share(message, ciphertext)`
to check the commitment without changing state.

### Consent

Consent is checked per record, not per patient. A patient-wide grant does not
authorise sending one record to a foreign chain. The grant must name the record.

### Audit log

`get_share_audit(sequence)` and `get_share_audit_count()` expose an append-only
log. Each entry records the message ID, patient, grantee, record, target chain,
payload hash, and ledger time.

### Errors

| Error               | Cause                                                  |
|---------------------|--------------------------------------------------------|
| `Unauthorized`      | Caller is not `message.patient`.                       |
| `UnsupportedVersion`| `version` is not `SHARE_PROTOCOL_VERSION`.             |
| `InvalidMessage`    | Empty chain or ciphertext, zero nonce, bad time window.|
| `PayloadTooLarge`   | Ciphertext exceeds `MAX_SHARE_PAYLOAD_BYTES`.          |
| `PayloadMismatch`   | Ciphertext does not match `payload_hash`.              |
| `MessageExpired`    | Ledger time is past `expires_at`.                      |
| `AlreadyProcessed`  | `message_id` was already used (replay).                |
| `ConsentRequired`   | No active record-level grant for the grantee.          |
| `ExternalCallFailed`| The consent check on the vision contract failed.       |
