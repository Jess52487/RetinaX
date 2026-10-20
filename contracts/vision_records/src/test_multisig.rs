#![cfg(test)]

use crate::{ContractError, VisionRecordsContract, VisionRecordsContractClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, Vec};

fn setup_test() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_multisig_threshold_boundary_1_of_1() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone()]);

    // Admin configures 1-of-1 multisig
    client.configure_multisig(&admin, &signers, &1);

    let cfg = client.get_multisig_config().expect("Config should be set");
    assert_eq!(cfg.threshold, 1);
    assert_eq!(cfg.signers.len(), 1);

    // s1 proposes an action
    let action = symbol_short!("SET_RATE");
    let data_hash = BytesN::from_array(&env, &[1u8; 32]);
    let prop_id = client.propose_admin_action(&s1, &action, &data_hash);

    let prop = client
        .get_proposal(&prop_id)
        .expect("Proposal should exist");
    assert_eq!(prop.approvals.len(), 1);
    assert_eq!(prop.approvals.get(0).unwrap(), s1);
}

#[test]
fn test_multisig_threshold_boundary_n_of_n() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);

    // Configure 3-of-3 multisig
    client.configure_multisig(&admin, &signers, &3);

    let action = symbol_short!("UPGRADE");
    let data_hash = BytesN::from_array(&env, &[2u8; 32]);
    let prop_id = client.propose_admin_action(&s1, &action, &data_hash);

    let prop1 = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop1.approvals.len(), 1);

    // 2nd approval
    client.approve_admin_action(&s2, &prop_id);
    let prop2 = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop2.approvals.len(), 2);

    // 3rd approval completes n-of-n quorum
    client.approve_admin_action(&s3, &prop_id);
    let prop3 = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop3.approvals.len(), 3);
}

#[test]
fn test_multisig_duplicate_approval_rejection_no_double_counting() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

    client.configure_multisig(&admin, &signers, &2);

    let prop_id = client.propose_admin_action(
        &s1,
        &symbol_short!("ACT"),
        &BytesN::from_array(&env, &[3u8; 32]),
    );

    // Proposer attempting to approve again must be rejected with Unauthorized
    let res_dup_proposer = client.try_approve_admin_action(&s1, &prop_id);
    assert_eq!(
        res_dup_proposer.unwrap_err().unwrap(),
        ContractError::Unauthorized
    );

    // Signer 2 approves once
    client.approve_admin_action(&s2, &prop_id);

    // Signer 2 attempting duplicate approval must be rejected
    let res_dup_s2 = client.try_approve_admin_action(&s2, &prop_id);
    assert_eq!(
        res_dup_s2.unwrap_err().unwrap(),
        ContractError::Unauthorized
    );

    // Verify approval count did not double count
    let prop = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.approvals.len(), 2);
}

#[test]
fn test_multisig_approval_nonexistent_or_unauthorized_signer() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let stranger = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone()]);

    client.configure_multisig(&admin, &signers, &1);

    let prop_id = client.propose_admin_action(
        &s1,
        &symbol_short!("TEST"),
        &BytesN::from_array(&env, &[4u8; 32]),
    );

    // Stranger cannot approve
    let err_stranger = client.try_approve_admin_action(&stranger, &prop_id);
    assert_eq!(
        err_stranger.unwrap_err().unwrap(),
        ContractError::Unauthorized
    );

    // Approving non-existent proposal ID is rejected
    let err_nonexistent = client.try_approve_admin_action(&s1, &9999u64);
    assert_eq!(
        err_nonexistent.unwrap_err().unwrap(),
        ContractError::Unauthorized
    );
}

#[test]
fn test_multisig_signer_removal_while_proposal_pending() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone(), s3.clone()]);

    client.configure_multisig(&admin, &signers, &2);

    let prop_id = client.propose_admin_action(
        &s1,
        &symbol_short!("ACT"),
        &BytesN::from_array(&env, &[5u8; 32]),
    );

    // Admin removes s2 from the authorized signers
    let new_signers = Vec::from_array(&env, [s1.clone(), s3.clone()]);
    client.configure_multisig(&admin, &new_signers, &2);

    // Removed signer s2 attempting to approve is rejected
    let res_removed = client.try_approve_admin_action(&s2, &prop_id);
    assert_eq!(
        res_removed.unwrap_err().unwrap(),
        ContractError::Unauthorized
    );

    // Remaining valid signer s3 can approve
    let res_valid = client.try_approve_admin_action(&s3, &prop_id);
    assert!(res_valid.is_ok());

    let prop = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.approvals.len(), 2);
}

#[test]
fn test_multisig_quorum_changes_during_active_proposal() {
    let (env, client, admin) = setup_test();

    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let signers = Vec::from_array(&env, [s1.clone(), s2.clone()]);

    // Initial threshold 2
    client.configure_multisig(&admin, &signers, &2);

    let prop_id = client.propose_admin_action(
        &s1,
        &symbol_short!("ACT"),
        &BytesN::from_array(&env, &[6u8; 32]),
    );

    let prop_before = client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop_before.approvals.len(), 1);

    // Admin lowers quorum to 1
    client.configure_multisig(&admin, &signers, &1);

    let cfg_after = client.get_multisig_config().unwrap();
    assert_eq!(cfg_after.threshold, 1);
}
