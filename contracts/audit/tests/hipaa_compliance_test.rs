//! HIPAA Audit Trail Compliance Verification (issue #317).
//!
//! Maps tests directly to `docs/compliance/hipaa-mapping.md` requirements:
//!
//! * **§ 164.312(b) Audit Controls** — every PHI access produces an audit
//!   entry containing *who* (actor), *what* (action/target/result), *when*
//!   (timestamp), and *where* (segment).
//! * **Immutability** — the audit log is append-only; the hash chain and
//!   Merkle root make retroactive modification detectable.
//! * **§ 164.316(b)(2)(i) Retention** — audit records are retained for at
//!   least six years; compaction before the retention window fails.

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

use audit::contract::{AuditContract, AuditContractClient};
use audit::merkle_log::MerkleLog;
use audit::types::{AuditError, LogSegmentId, RetentionPolicy};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, Symbol};

// ── HIPAA retention constant ─────────────────────────────────────────────────

/// HIPAA § 164.316(b)(2)(i): retain documentation for six years from the
/// date of its creation or the date when it was last in effect, whichever
/// is later.  Six years at 365 days/year plus two leap-day allowances:
/// `6 × 365 × 86 400 + 2 × 86 400 = 189_388_800`.
pub const HIPAA_RETENTION_SECS: u64 = (6 * 365 + 2) * 86_400;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn seg(name: &str) -> LogSegmentId {
    LogSegmentId::new(name).unwrap()
}

fn contract_setup() -> (Env, AuditContractClient<'static>, Symbol) {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register(AuditContract, ());
    let client = AuditContractClient::new(&env, &id);
    client.initialize(&Address::generate(&env));

    let segment = Symbol::short("PHI");
    client.create_segment(&segment);
    (env, client, segment)
}

/// Simulate one PHI access: an append must occur for every read/write.
fn log_phi_access(
    client: &AuditContractClient,
    segment: &Symbol,
    who: &Address,
    what: &str,
    where_: &str,
) -> u64 {
    client.append_entry(
        segment,
        who,
        &Symbol::short(what),
        &Symbol::short(where_),
        &Symbol::short("OK"),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Every PHI access creates an audit entry
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn every_phi_access_creates_an_audit_entry() {
    let (env, client, segment) = contract_setup();
    let clinician = Address::generate(&env);
    let researcher = Address::generate(&env);

    let accesses = [
        (&clinician, "READ", "REC_1"),
        (&clinician, "READ", "REC_2"),
        (&researcher, "EXPORT", "REC_1"),
        (&clinician, "UPDATE", "REC_1"),
        (&researcher, "DENIED", "REC_3"),
    ];

    for (who, what, where_) in accesses {
        log_phi_access(&client, &segment, who, what, where_);
    }

    let entries = client.get_entries(&segment);
    assert_eq!(
        entries.len() as u64,
        accesses.len() as u64,
        "every simulated PHI access must produce exactly one audit entry"
    );
    assert_eq!(client.get_entry_count(&segment), accesses.len() as u64);
}

#[test]
fn phi_access_without_audit_append_is_detectable_by_count_gap() {
    // Counterpart property: entries are strictly sequential (1..=n), so a
    // PHI access that skipped logging manifests as a missing sequence number.
    let (env, client, segment) = contract_setup();
    let clinician = Address::generate(&env);

    let seq1 = log_phi_access(&client, &segment, &clinician, "READ", "REC_1");
    let seq2 = log_phi_access(&client, &segment, &clinician, "READ", "REC_2");
    let seq3 = log_phi_access(&client, &segment, &clinician, "READ", "REC_3");

    assert_eq!((seq1, seq2, seq3), (1, 2, 3));
    assert_eq!(client.get_entry_count(&segment), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Entries include who, what, when, where
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn phi_entries_carry_who_what_when_where() {
    let (env, client, segment) = contract_setup();
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);

    let clinician = Address::generate(&env);
    let seq = log_phi_access(&client, &segment, &clinician, "READ", "REC_9");

    let entries = client.get_entries(&segment);
    let entry = entries.get(0).unwrap();

    // who — unique actor identity
    assert_eq!(entry.actor, clinician, "entry must record who accessed");
    // what — action on a resource, with an outcome
    assert_eq!(
        entry.action,
        Symbol::short("READ"),
        "entry must record what"
    );
    assert_eq!(
        entry.target,
        Symbol::short("REC_9"),
        "entry must record which resource"
    );
    assert_eq!(entry.result, Symbol::short("OK"));
    // when — ledger timestamp
    assert_eq!(
        entry.timestamp, 1_700_000_000,
        "entry must record when the access occurred"
    );
    // where — segment partition the entry lives in (checked via segment scoping)
    assert_eq!(entry.sequence, seq);

    // The segment is "where": a second segment must not see this entry.
    let other = Symbol::short("OTHER");
    client.create_segment(&other);
    assert_eq!(
        client.get_entry_count(&other),
        0,
        "entries are scoped to their segment (where)"
    );
    assert_eq!(client.get_entry_count(&segment), 1);
}

#[test]
fn timestamps_are_recorded_from_ledger_and_monotonic() {
    let (env, client, segment) = contract_setup();
    let clinician = Address::generate(&env);

    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    log_phi_access(&client, &segment, &clinician, "READ", "REC_1");
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_060);
    log_phi_access(&client, &segment, &clinician, "READ", "REC_2");

    let entries = client.get_entries(&segment);
    let e0 = entries.get(0).unwrap();
    let e1 = entries.get(1).unwrap();

    assert_eq!(e0.timestamp, 1_700_000_000);
    assert_eq!(e1.timestamp, 1_700_000_060);
    assert!(
        e1.timestamp >= e0.timestamp,
        "audit timestamps must be non-decreasing (when)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Audit log immutability
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn contract_storage_is_append_only_after_phil_accesses() {
    let (env, client, segment) = contract_setup();
    let clinician = Address::generate(&env);

    log_phi_access(&client, &segment, &clinician, "READ", "REC_1");
    let snapshot: std::vec::Vec<_> = client.get_entries(&segment).iter().collect();

    // Run many more appends; earlier entries must be byte-identical.
    for _ in 0..10 {
        log_phi_access(&client, &segment, &clinician, "READ", "REC_X");
    }
    let after: std::vec::Vec<_> = client.get_entries(&segment).iter().collect();

    assert_eq!(snapshot[0], after[0], "historic entry must never mutate");
    assert_eq!(client.get_entry_count(&segment), 11);
}

#[test]
fn hash_chain_verifies_over_phi_accesses() {
    let mut log = MerkleLog::new(seg("hipaa-chain"));
    log.append(1000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    log.append(2000, "clinician", "record.update", "patient:42", "ok")
        .unwrap();
    log.append(3000, "researcher", "record.export", "patient:42", "denied")
        .unwrap();

    assert!(
        log.verify_chain(1, 3).is_ok(),
        "intact PHI audit chain must verify"
    );
    assert_eq!(log.get_entry(1).unwrap().prev_hash, [0u8; 32]);
}

#[test]
fn rewritten_phi_access_changes_merkle_root() {
    // An attacker who rewrites a historical entry cannot keep the same root:
    // the published root exposes the modification.
    let mut original = MerkleLog::new(seg("hipaa-root"));
    original
        .append(1000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    original
        .append(2000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    let published = original.publish_root(2000);

    let mut forged = MerkleLog::new(seg("hipaa-root"));
    forged
        .append(1000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    forged
        .append(2000, "clinician", "record.delete", "patient:42", "ok")
        .unwrap(); // tampered action

    assert_ne!(
        forged.current_root(),
        published,
        "a rewritten PHI entry must change the Merkle root"
    );
}

#[test]
fn out_of_order_timestamps_are_rejected() {
    // "When" cannot be backdated: appending an earlier timestamp fails.
    let mut log = MerkleLog::new(seg("hipaa-time"));
    log.append(2000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();

    let result = log.append(1000, "clinician", "record.read", "patient:42", "ok");
    assert!(matches!(
        result,
        Err(AuditError::OutOfOrderTimestamp { .. })
    ));
}

#[test]
fn entry_tampering_breaks_subsequent_chain_links() {
    // Rebuilding a log where one PHI entry's content differs produces a
    // divergent chain from that point forward.
    let mut honest = MerkleLog::new(seg("hipaa-link"));
    honest
        .append(1000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    honest
        .append(2000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();

    let mut tampered = MerkleLog::new(seg("hipaa-link"));
    tampered
        .append(1000, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    tampered
        .append(2000, "clinician", "record.read", "patient:43", "ok")
        .unwrap(); // victim changed

    let h2 = honest.get_entry(2).unwrap();
    let t2 = tampered.get_entry(2).unwrap();
    assert_ne!(
        h2.entry_hash, t2.entry_hash,
        "tampered entry hash must diverge — hash chain broken for auditors"
    );
    // Honest chain still verifies end-to-end.
    assert!(honest.verify_chain(1, 2).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Six-year retention requirement (§ 164.316(b)(2)(i))
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn six_year_retention_constant_matches_hipaa() {
    // Six 365-day years plus two leap-day allowances.
    assert_eq!(HIPAA_RETENTION_SECS, 189_388_800);
    assert!(HIPAA_RETENTION_SECS > 6 * 365 * 86_400);
}

#[test]
fn phi_entries_cannot_be_deleted_within_six_years() {
    let mut log = MerkleLog::new(seg("hipaa-retain"));
    let created_at: u64 = 1_000_000_000;
    log.append(created_at, "clinician", "record.read", "patient:42", "ok")
        .unwrap();

    log.set_retention(RetentionPolicy {
        segment: seg("hipaa-retain"),
        min_retention_secs: HIPAA_RETENTION_SECS,
        requires_witness_for_deletion: false,
    });

    // One second before six years: deletion must fail.
    let too_early = created_at + HIPAA_RETENTION_SECS - 1;
    let result = log.compact(1, 1, too_early, 0);
    assert!(
        matches!(result, Err(AuditError::RetentionPolicyViolation { .. })),
        "deletion before the six-year window must be rejected"
    );

    // Entry still present after the failed attempt.
    assert!(log.get_entry(1).is_ok());
}

#[test]
fn phi_entries_may_be_deleted_after_six_years() {
    let mut log = MerkleLog::new(seg("hipaa-retain-ok"));
    let created_at: u64 = 1_000_000_000;
    log.append(created_at, "clinician", "record.read", "patient:42", "ok")
        .unwrap();

    log.set_retention(RetentionPolicy {
        segment: seg("hipaa-retain-ok"),
        min_retention_secs: HIPAA_RETENTION_SECS,
        requires_witness_for_deletion: false,
    });

    let after_six_years = created_at + HIPAA_RETENTION_SECS;
    let result = log.compact(1, 1, after_six_years, 0);
    assert!(
        result.is_ok(),
        "deletion exactly at the six-year boundary must be permitted"
    );
}

#[test]
fn partial_retention_window_is_evaluated_per_entry() {
    // Older entries may pass the window while newer ones must be retained.
    let mut log = MerkleLog::new(seg("hipaa-mixed"));
    let old_ts: u64 = 1_000_000_000;
    let new_ts: u64 = old_ts + HIPAA_RETENTION_SECS + 100;

    log.append(old_ts, "clinician", "record.read", "patient:42", "ok")
        .unwrap();
    log.append(new_ts, "clinician", "record.read", "patient:43", "ok")
        .unwrap();

    log.set_retention(RetentionPolicy {
        segment: seg("hipaa-mixed"),
        min_retention_secs: HIPAA_RETENTION_SECS,
        requires_witness_for_deletion: false,
    });

    let now = new_ts + HIPAA_RETENTION_SECS - 1;
    // Range covering the recent entry must fail.
    let result = log.compact(1, 2, now, 0);
    assert!(matches!(
        result,
        Err(AuditError::RetentionPolicyViolation { .. })
    ));

    // Range covering only the old entry succeeds.
    assert!(log.compact(1, 1, now, 0).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. End-to-end HIPAA audit scenario
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn end_to_end_hipaa_audit_scenario() {
    let (env, client, segment) = contract_setup();
    let clinician = Address::generate(&env);

    // 1. PHI accesses are each audited with who/what/when/where.
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    log_phi_access(&client, &segment, &clinician, "READ", "REC_1");
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_120);
    log_phi_access(&client, &segment, &clinician, "UPDATE", "REC_1");
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_240);
    log_phi_access(&client, &segment, &clinician, "EXPORT", "REC_1");

    let entries = client.get_entries(&segment);
    assert_eq!(entries.len(), 3);
    let expected_actions = [
        Symbol::short("READ"),
        Symbol::short("UPDATE"),
        Symbol::short("EXPORT"),
    ];
    for (i, e) in entries.iter().enumerate() {
        assert_eq!(e.actor, clinician); // who
        assert_eq!(e.action, expected_actions[i]); // what
        assert!(e.timestamp > 0); // when
    }

    // 2. Mirror the entries into a hash-chained log for immutability proofs.
    let mut log = MerkleLog::new(seg("hipaa-e2e"));
    for e in entries.iter() {
        log.append(e.timestamp, "clinician", "phi.access", "patient:42", "ok")
            .unwrap();
    }
    assert!(log.verify_chain(1, 3).is_ok());
    let root = log.publish_root(1_700_000_240);
    let proof = log.inclusion_proof(2).unwrap();
    assert!(proof.verify(&root).is_ok());

    // 3. Retention: entries locked for six years.
    log.set_retention(RetentionPolicy {
        segment: seg("hipaa-e2e"),
        min_retention_secs: HIPAA_RETENTION_SECS,
        requires_witness_for_deletion: false,
    });
    let first_ts = log.get_entry(1).unwrap().timestamp;
    assert!(matches!(
        log.compact(1, 1, first_ts + HIPAA_RETENTION_SECS - 1, 0),
        Err(AuditError::RetentionPolicyViolation { .. })
    ));
}
