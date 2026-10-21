//! access_test.rs
//!
//! Tests for authentication and authorization constraints in the `Governor` contract.
//!
//! Verifies that access-gated functions properly enforce authorization:
//! - Non-admin callers cannot execute admin-gated functions (`set_total_vote_supply`)
//! - Non-proposer callers cannot advance a proposal from `Draft` phase
//! - Unauthorized delegates cannot commit votes on behalf of delegators
//! - Re-initialization is rejected

#![cfg(test)]

extern crate std;

use governor::{ContractError, GovernorContract, GovernorContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, GovernorContractClient<'static>, Address, Address) {
use governor::{
    proposal::{ProposalAction, ProposalType},
    ContractError, GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    vec, Address, BytesN, Env, String,
};

/// Deploy and initialize the governor; return `(env, client, admin, stranger)`.
fn setup() -> (
    Env,
    Address,
    GovernorContractClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    let stranger = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);
    client.initialize(&admin, &staking, &treasury, &1_000);

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    client.initialize(&admin, &staking, &treasury, &1_000_000_000i128);

    (env, contract_id, client, admin, stranger)
}

#[test]
fn non_admin_cannot_update_total_vote_supply() {
    let (env, client, _admin, stranger) = setup();
    env.mock_all_auths_allowing_non_root_auth();

    assert_eq!(
        client.try_set_total_vote_supply(&stranger, &2_000),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn admin_can_update_total_vote_supply() {
    let (_env, client, admin, _stranger) = setup();

    assert!(client.try_set_total_vote_supply(&admin, &2_000).is_ok());
fn set_mock_stake(env: &Env, contract_id: &Address, voter: &Address, amount: i128) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_STK"), voter.clone());
        env.storage().persistent().set(&key, &amount);
    });
}

// ── Access Control Tests ─────────────────────────────────────────────────────

#[test]
fn test_set_total_vote_supply_unauthorized() {
    let (_env, _contract_id, client, _admin, stranger) = setup();

    let res = client.try_set_total_vote_supply(&stranger, &2_000_000_000i128);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_set_total_vote_supply_by_admin_succeeds() {
    let (_env, _contract_id, client, admin, _stranger) = setup();

    let res = client.try_set_total_vote_supply(&admin, &2_000_000_000i128);
    assert!(res.is_ok());
}

#[test]
fn test_reinitialize_rejected() {
    let (env, _contract_id, client, _admin, stranger) = setup();

    let staking2 = Address::generate(&env);
    let treasury2 = Address::generate(&env);

    let res = client.try_initialize(&stranger, &staking2, &treasury2, &1_000_000_000i128);
    assert_eq!(res, Err(Ok(ContractError::AlreadyInitialized)));
}

#[test]
fn test_advance_draft_phase_only_by_proposer() {
    let (env, contract_id, client, _admin, stranger) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 5_000);

    let mut actions = vec![&env];
    actions.push_back(ProposalAction {
        target: Address::generate(&env),
        function: symbol_short!("ACTION"),
        params_hash: BytesN::from_array(&env, &[1u8; 32]),
    });

    let proposal_id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Proposal"),
        &actions,
    );

    // Stranger cannot advance proposal from Draft
    let res = client.try_advance_phase(&stranger, &proposal_id);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    // Proposer can advance proposal from Draft to Discussion
    let res_proposer = client.try_advance_phase(&proposer, &proposal_id);
    assert!(res_proposer.is_ok());
}

#[test]
fn test_commit_vote_as_delegate_unauthorized() {
    let (env, contract_id, client, _admin, stranger) = setup();

    let delegator = Address::generate(&env);
    let real_delegate = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &delegator, 10_000);

    // Delegate to real_delegate
    client.delegate(&delegator, &real_delegate);

    let mut actions = vec![&env];
    actions.push_back(ProposalAction {
        target: Address::generate(&env),
        function: symbol_short!("ACTION"),
        params_hash: BytesN::from_array(&env, &[1u8; 32]),
    });

    let proposal_id = client.create_proposal(
        &delegator,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Delegation Proposal"),
        &actions,
    );

    // Move to voting phase: Draft -> Discussion -> Voting
    client.advance_phase(&delegator, &proposal_id);
    env.ledger().with_mut(|l| l.timestamp += 259_200 + 1);
    client.advance_phase(&delegator, &proposal_id);

    let commitment = BytesN::from_array(&env, &[7u8; 32]);

    // Stranger tries to commit as delegate for delegator
    let res = client.try_commit_vote_as_delegate(&stranger, &delegator, &proposal_id, &commitment);
    assert_eq!(res, Err(Ok(ContractError::NotADelegate)));
}
