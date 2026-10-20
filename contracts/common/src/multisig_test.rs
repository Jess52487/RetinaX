#![cfg(test)]

use super::{
    approve, configure, get_config, get_proposal, is_executable, mark_executed, propose,
    MultisigError,
};
use soroban_sdk::{contract, symbol_short, testutils::Address as _, Address, BytesN, Env, Vec};

#[contract]
struct MockContract;

fn with_contract<F, R>(env: &Env, f: F) -> R
where
    F: FnOnce() -> R,
{
    let contract_id = env.register(MockContract, ());
    env.as_contract(&contract_id, f)
}

#[test]
fn test_configure_valid() {
    let env = Env::default();
    with_contract(&env, || {
        let signers = Vec::from_array(&env, [Address::generate(&env), Address::generate(&env)]);
        let result = configure(&env, signers.clone(), 2);
        assert!(result.is_ok());

        let cfg = get_config(&env).unwrap();
        assert_eq!(cfg.threshold, 2);
        assert_eq!(cfg.signers.len(), 2);
    });
}

#[test]
fn test_configure_invalid() {
    let env = Env::default();
    with_contract(&env, || {
        let signers = Vec::from_array(&env, [Address::generate(&env)]);

        // Threshold 0
        assert_eq!(
            configure(&env, signers.clone(), 0),
            Err(MultisigError::InvalidConfig)
        );

        // Threshold > Signers
        assert_eq!(
            configure(&env, signers.clone(), 2),
            Err(MultisigError::InvalidConfig)
        );
    });
}

#[test]
fn test_propose_and_approve_quorum() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);

        configure(&env, signers, 2).unwrap();

        let action = symbol_short!("ACTION");
        let data_hash = BytesN::from_array(&env, &[0u8; 32]);

        // Proposer is s1
        let id = propose(&env, &s1, action.clone(), data_hash.clone()).unwrap();

        // approvals.len() is 1 (s1)
        assert!(!is_executable(&env, id));

        // s2 approves
        approve(&env, &s2, id).unwrap();

        // approvals.len() is 2 (s1, s2). Threshold is 2.
        assert!(is_executable(&env, id));

        // Mark as executed
        mark_executed(&env, id).unwrap();

        // Already executed
        assert_eq!(mark_executed(&env, id), Err(MultisigError::AlreadyExecuted));
    });
}

#[test]
fn test_duplicate_approval_fails() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), Address::generate(&env)]);
        configure(&env, signers, 2).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("ACT"),
            BytesN::from_array(&env, &[0u8; 32]),
        )
        .unwrap();

        // Proposer already approved
        assert_eq!(approve(&env, &s1, id), Err(MultisigError::AlreadyApproved));
    });
}

#[test]
fn test_non_signer_cannot_approve() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let stranger = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone()]);
        configure(&env, signers, 1).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("ACT"),
            BytesN::from_array(&env, &[0u8; 32]),
        )
        .unwrap();

        assert_eq!(approve(&env, &stranger, id), Err(MultisigError::NotASigner));
    });
}

#[test]
fn test_large_signer_set() {
    let env = Env::default();
    with_contract(&env, || {
        let mut signers = Vec::new(&env);
        for _ in 0..20 {
            signers.push_back(Address::generate(&env));
        }

        let threshold = 15;
        configure(&env, signers.clone(), threshold).unwrap();

        let proposer = signers.get(0).unwrap();
        let id = propose(
            &env,
            &proposer,
            symbol_short!("ACT"),
            BytesN::from_array(&env, &[0u8; 32]),
        )
        .unwrap();

        // Add 13 more approvals (total 14)
        for i in 1..14 {
            approve(&env, &signers.get(i).unwrap(), id).unwrap();
        }
        assert!(!is_executable(&env, id));

        // Add 15th approval
        approve(&env, &signers.get(14).unwrap(), id).unwrap();
        assert!(is_executable(&env, id));
    });
}

#[test]
fn test_threshold_transition() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

        // Initial threshold 2
        configure(&env, signers.clone(), 2).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("ACT"),
            BytesN::from_array(&env, &[0u8; 32]),
        )
        .unwrap();
        assert!(!is_executable(&env, id));

        // Change threshold to 1
        configure(&env, signers.clone(), 1).unwrap();

        // Now it should be executable because it has 1 approval (s1)
        assert!(is_executable(&env, id));
    });
}

#[test]
fn test_remove_signer_impact() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

        configure(&env, signers.clone(), 2).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("ACT"),
            BytesN::from_array(&env, &[0u8; 32]),
        )
        .unwrap();
        approve(&env, &s2, id).unwrap();
        assert!(is_executable(&env, id));

        // Remove s2 from signers. New config: threshold 1, signers [s1]
        let new_signers = Vec::from_array(&env, [s1.clone()]);
        configure(&env, new_signers, 1).unwrap();

        // It's STILL executable because it has 2 approvals in persistent storage,
        // and threshold is now 1.
        assert!(is_executable(&env, id));
    });
}

// ── Additional Edge Case Tests for Issue #264 ────────────────────────────────

#[test]
fn test_threshold_boundary_1_of_1() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone()]);
        configure(&env, signers, 1).expect("1-of-1 configuration should be valid");

        let id = propose(
            &env,
            &s1,
            symbol_short!("ONE_ONE"),
            BytesN::from_array(&env, &[1u8; 32]),
        )
        .expect("Proposing by single signer should succeed");

        // Single signer proposal is immediately executable because proposer counts as first approval
        assert!(is_executable(&env, id));
        let proposal = get_proposal(&env, id).expect("Proposal should exist");
        assert_eq!(proposal.approvals.len(), 1);
        assert_eq!(proposal.approvals.get(0).unwrap(), s1);

        mark_executed(&env, id).expect("Execution must succeed for 1-of-1");
        assert!(!is_executable(&env, id));
    });
}

#[test]
fn test_threshold_boundary_n_of_n() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);
        // 3-of-3 threshold: every single signer must approve
        configure(&env, signers, 3).expect("3-of-3 configuration should be valid");

        let id = propose(
            &env,
            &s1,
            symbol_short!("THREE_3"),
            BytesN::from_array(&env, &[2u8; 32]),
        )
        .expect("Proposal creation should succeed");

        // 1 of 3: not executable
        assert!(!is_executable(&env, id));
        assert_eq!(mark_executed(&env, id), Err(MultisigError::ThresholdNotMet));

        // 2 of 3: still not executable
        approve(&env, &s2, id).expect("Second approval should succeed");
        assert!(!is_executable(&env, id));
        assert_eq!(mark_executed(&env, id), Err(MultisigError::ThresholdNotMet));

        // 3 of 3: now executable
        approve(&env, &s3, id).expect("Third approval should succeed");
        assert!(is_executable(&env, id));
        mark_executed(&env, id).expect("Execution must succeed when all signers approved");
    });
}

#[test]
fn test_no_double_counting_of_approvals() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);
        configure(&env, signers, 3).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("NO_DBL"),
            BytesN::from_array(&env, &[3u8; 32]),
        )
        .unwrap();

        // Proposer double approval attempt
        assert_eq!(approve(&env, &s1, id), Err(MultisigError::AlreadyApproved));

        // Signer 2 approves once
        approve(&env, &s2, id).unwrap();
        // Signer 2 attempts second approval
        assert_eq!(approve(&env, &s2, id), Err(MultisigError::AlreadyApproved));

        let prop = get_proposal(&env, id).unwrap();
        assert_eq!(
            prop.approvals.len(),
            2,
            "Approvals length must strictly equal 2 unique signers"
        );

        // Threshold of 3 cannot be met by repeated attempts from s1 or s2
        assert!(!is_executable(&env, id));
    });
}

#[test]
fn test_approval_after_execution_and_nonexistent_proposal() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);
        configure(&env, signers, 1).unwrap();

        // Non-existent proposal ID
        assert_eq!(
            approve(&env, &s1, 9999),
            Err(MultisigError::ProposalNotFound)
        );

        // Create and execute proposal
        let id = propose(
            &env,
            &s1,
            symbol_short!("EXEC"),
            BytesN::from_array(&env, &[4u8; 32]),
        )
        .unwrap();
        mark_executed(&env, id).unwrap();

        // Approval attempt after proposal has executed
        assert_eq!(approve(&env, &s2, id), Err(MultisigError::AlreadyExecuted));
    });
}

#[test]
fn test_signer_removal_while_proposal_pending() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);
        configure(&env, signers, 2).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("PEND"),
            BytesN::from_array(&env, &[5u8; 32]),
        )
        .unwrap();

        // Remove s2 from the signer set while proposal is pending
        let updated_signers = Vec::from_array(&env, [s1.clone(), s3.clone()]);
        configure(&env, updated_signers, 2).unwrap();

        // s2 is no longer a valid signer and must be rejected
        assert_eq!(approve(&env, &s2, id), Err(MultisigError::NotASigner));

        // s3 remains an authorized signer and can approve
        approve(&env, &s3, id).expect("Active signer s3 must be allowed to approve");
        assert!(is_executable(&env, id));
    });
}

#[test]
fn test_quorum_increase_and_decrease_during_active_proposal() {
    let env = Env::default();
    with_contract(&env, || {
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);
        let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);
        // Initial quorum: 3
        configure(&env, signers.clone(), 3).unwrap();

        let id = propose(
            &env,
            &s1,
            symbol_short!("QUORUM"),
            BytesN::from_array(&env, &[6u8; 32]),
        )
        .unwrap();
        approve(&env, &s2, id).unwrap(); // Approvals: [s1, s2] (2 approvals)

        assert!(!is_executable(&env, id));

        // Decrease quorum to 2 during active proposal
        configure(&env, signers.clone(), 2).unwrap();
        // Immediately executable now that approvals count (2) >= new threshold (2)
        assert!(is_executable(&env, id));

        // Increase quorum to 3 again
        configure(&env, signers.clone(), 3).unwrap();
        // No longer executable until 3rd signer approves
        assert!(!is_executable(&env, id));

        // s3 approves -> now executable under quorum 3
        approve(&env, &s3, id).unwrap();
        assert!(is_executable(&env, id));
        mark_executed(&env, id).unwrap();
    });
}
