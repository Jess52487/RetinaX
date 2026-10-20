# Consent Management Guide

This guide explains how patient consent is recorded, checked, revoked, and expired in RetinaX. It describes the code as it exists today, so it covers two implementations:

- **On-chain consent** in the `vision_records` contract (`contracts/vision_records/src/consent_management.rs`, with entry points in `contracts/vision_records/src/lib.rs`). This is the implementation the deployed contract enforces.
- **`ConsentManager`** in the common crate (`contracts/common/src/consent.rs`). An in-memory model with filtering helpers and ABAC attribute export. No contract calls it today.

## 🎯 Consent Overview

A consent is a grant from a **patient** to a **grantee** (for example a provider or clinic). It lasts for a fixed duration and can be revoked early. A grantee can read a patient's records only while the consent is active.

Core rules:

1. **Patient authority.** Only the patient can grant or revoke consent. Each call requires the patient's authorization.
2. **One record per pair.** There is at most one consent record for each (patient, grantee) pair. A new grant replaces the old record.
3. **Time-bound.** Every grant has an expiry time. Once it passes, the consent stops counting as active.
4. **Revocable.** A revoked record stays in storage for audit, but it no longer grants access.

## 📋 Consent Data Structure

### Consent Types

`ConsentType` is defined in `contracts/vision_records/src/consent_management.rs`:

| Variant | Meaning |
|---------|---------|
| `Treatment` | Consent for clinical treatment |
| `Sharing` | Consent to share records with another party |
| `Research` | Consent for research use |
| `Emergency` | Emergency consent label |
| `Billing` | Consent for billing access |

Note that `ConsentType` is stored on the record but is **not** checked by the access logic. Access checks test whether a consent is active, not which type it is. See [ABAC Integration](#-abac-integration).

### On-Chain Consent Record

```rust
#[contracttype]
pub struct ConsentGrant {
    pub patient: Address,
    pub grantee: Address,
    pub consent_type: ConsentType,
    pub granted_at: u64,   // ledger timestamp, seconds
    pub expires_at: u64,   // ledger timestamp, seconds
    pub revoked: bool,
}
```

The record is stored in persistent storage under the key `(CONSENT, patient, grantee)`. The `consent_key` helper builds this key.

### Off-Chain Consent Record (`ConsentManager`)

```rust
pub struct ConsentRecord {
    pub subject: String,        // patient identifier
    pub grantee: String,        // practitioner or entity identifier
    pub consent_type: ConsentType,
    pub granted_at: u64,
    pub expires_at: Option<u64>, // None means no expiry
    pub revoked: bool,
}
```

Identifiers are validated on every call:

| Field | Length (bytes) | Other rules |
|-------|----------------|-------------|
| Consent ID | 1 to 64 | Printable characters only, not all whitespace |
| Subject | 1 to 128 | Printable characters only, not all whitespace |
| Grantee | 1 to 128 | Printable characters only, not all whitespace |

## 🔄 Consent Lifecycle

```
              grant_consent(patient, grantee, type, duration)
                               │
                               ▼
                   ┌──────────────────────┐
                   │  ACTIVE              │
                   │  revoked = false     │
                   │  now < expires_at    │
                   └──────────┬───────────┘
                              │
          ┌───────────────────┼─────────────────────┐
          │                   │                     │
  revoke_consent()     now >= expires_at       grant_consent()
          │                   │                     │
          ▼                   ▼                     ▼
   ┌────────────┐      ┌────────────┐        replaces the record
   │  REVOKED   │      │  EXPIRED   │        and returns to ACTIVE
   │ revoked=true│     │ (derived)  │
   └────────────┘      └────────────┘
```

Expired and revoked records are never deleted. Expiry is derived from `expires_at` whenever the record is read.

### 1. Grant

Entry point: `grant_consent(patient, grantee, consent_type, duration_seconds)` in `contracts/vision_records/src/lib.rs`.

1. The global circuit breaker must not be paused.
2. `patient.require_auth()` must succeed.
3. `duration_seconds` must be non-zero. Zero returns `InvalidInput`.
4. `expires_at = now + duration_seconds`, using a saturating add.
5. The record is written under `(CONSENT, patient, grantee)`. It **replaces** any existing record for that pair, including a revoked one, so a new grant reinstates access.
6. The storage TTL of the entry is extended.
7. If a previous record existed, a `CST_UPD` event is emitted with the old and new expiry. A `CST_GRT` event is always emitted.

### 2. Verification

`has_active_consent(patient, grantee)` returns `true` only when all of these hold:

- A record exists for the pair.
- `revoked` is `false`.
- `expires_at > now`. At the exact expiry second the consent is already inactive.

`get_consent(patient, grantee)` returns the raw record and does not apply these checks. Callers that need an access decision must use `has_active_consent` or the ABAC checks below.

### 3. Revocation

Entry point: `revoke_consent(patient, grantee)`.

- Requires `patient.require_auth()`.
- Sets `revoked = true` and keeps the record for audit.
- If no record exists for the pair, the call still succeeds and still emits a `CST_REV` event. Monitoring code should not treat the event as proof that a record existed.

### 4. Expiry

Nothing has to be called to expire a consent. The record stops being active once `now >= expires_at`. The `CST_EXP` event type is defined in `contracts/vision_records/src/events.rs`, but no current entry point emits it. Off-chain systems that need an expiry notice must compute it from `expires_at`.

## ⏱️ TTL and Expiration Behavior

There are two separate clocks. Keep them apart when reasoning about consent:

| Clock | Where | Controls access? |
|-------|-------|------------------|
| `expires_at` on the record | Consent record | **Yes.** Access is decided by comparing it with the ledger timestamp. |
| Storage TTL on the persistent entry | Ledger storage | No. It keeps the entry alive, and `grant_consent` extends it. |

Rules:

- Consent validity is always decided by `expires_at`. Whether the storage entry still exists does not change that decision.
- `duration_seconds` must be non-zero. `validation::validate_duration` in `contracts/vision_records/src/validation.rs` also caps durations at 157,680,000 seconds (5 years). **The `grant_consent` entry point does not call that helper today**, so the entry point accepts durations above 5 years. Treat the cap as intended behavior, not current enforcement.
- `ConsentManager` uses `expires = now + ttl` when a TTL is given, and treats `None` as no expiry. Its status is `Active` only when `now < expires`.

## 🔍 Filtering by Grantee and Subject

On-chain storage has no list-by-grantee or list-by-patient function. Consent is looked up by the (patient, grantee) pair only. To find all consents for a grantee or patient on-chain, index the `CST_GRT`, `CST_UPD`, and `CST_REV` events off-chain.

`ConsentManager` provides in-memory filters:

| Function | Returns |
|----------|---------|
| `get_consent_status(id, now)` | `Active`, `Expired`, `Revoked`, or `NotGranted` |
| `get_consent_attribute(id, now)` | The `ConsentAttribute` for one record, or `None` |
| `get_active_consents_for_grantee(grantee, now)` | Active records whose grantee matches |
| `get_active_consents_for_subject(subject, now)` | Active records whose subject matches |

Both list functions validate their input and return only records whose status is `Active` at `now`. Expired and revoked records are excluded.

## 🛡️ ABAC Integration

Attribute-based access control (ABAC) adds policy checks on top of consent. Consent is necessary but not always sufficient. There are two integration points.

### A. Consent as a policy condition (`consent_required`)

In `contracts/vision_records/src/rbac.rs`, an ABAC policy can set `consent_required`. When it does, evaluation:

1. Requires both a patient and a resource ID in the request context. If either is missing, access is denied.
2. Reads the record under `(CONSENT, patient, user)`, where `user` is the requesting address.
3. Denies access if the record is missing, is revoked, or has `expires_at <= current_time`.

This check does **not** compare `consent_type` with the type the policy needs. Any unexpired, unrevoked consent satisfies it.

### B. Access level check (`check_access`)

`check_access(patient, grantee)` in `contracts/vision_records/src/lib.rs` returns `AccessLevel::None` unless all of these hold:

1. An active consent exists for the pair (`has_active_consent`).
2. An `AccessGrant` exists under `(ACCESS, patient, grantee)` and its `expires_at` is in the future.
3. `evaluate_access_policies` allows the grantee for the patient.

Only then is the grant's `level` returned.

### Where consent is checked in record reads

Record reads treat an active consent as one of several ways to get access. A caller who is the patient, a provider who created the record, or an address with `ReadAnyRecord` or `SystemAdmin` is allowed without consent. Otherwise the read falls back to `has_active_consent` and then to `check_access`.

## 🚨 Emergency Access

Emergency access is handled by separate entry points, not by consent records: `check_emergency_access` and `access_record_via_emergency` in `contracts/vision_records/src/lib.rs`, with an audit trail readable through `get_emergency_audit_trail`. The rules those functions apply are outside the scope of this guide. `ConsentType::Emergency` is only a label on a consent record and does not, by itself, grant emergency access.

## 📣 Events

| Topic | Emitted when | Notes |
|-------|--------------|-------|
| `CST_GRT` | A grant is made | Always emitted on grant |
| `CST_UPD` | A grant replaces an existing record | Carries the old and new expiry |
| `CST_REV` | `revoke_consent` is called | Emitted even if no record existed |
| `CST_EXP` | Not emitted by current code | Defined only |

Topics are `(event_name, patient, grantee)`, so indexers can filter by patient or grantee.

## 🧪 Testing

- `contracts/vision_records/tests/consent_management_test.rs`: event emission for grant, revoke, and update.
- `contracts/vision_records/src/test_consent_management_failures.rs`: failure cases, including zero-duration grants.
- `contracts/common/src/consent.rs`: unit tests for `ConsentManager` bounds, grant, revoke, and filtering.

## 🚧 Not Implemented

Earlier versions of this guide described features that do not exist in the current contracts. They are listed here so nobody relies on them:

- A `Consent` struct with `consent_id`, `permissions`, `data_types`, `purpose`, `conditions`, and `metadata` fields. The on-chain record only has the fields shown above.
- A per-patient consent index and a consent ID generator. Records are keyed by the (patient, grantee) pair.
- Per-permission checks (for example `read_medical` or `share_research`). Access uses record-level access grants and ABAC policies instead.
- Patient dashboards, provider consent views, consent analytics, and compliance report generation.

## 📚 References

- `contracts/vision_records/src/consent_management.rs`: consent types, record, and storage helpers
- `contracts/vision_records/src/rbac.rs`: ABAC policy engine and the `consent_required` condition
- `contracts/common/src/consent.rs`: `ConsentManager` and its validation rules
- [Access Control Architecture](../access-control-architecture.md)
