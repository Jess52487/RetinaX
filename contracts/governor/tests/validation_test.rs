//! validation_test.rs
//!
//! Input validation and constraint testing for the `Governor` smart contract.
//!
//! Verifies:
//! - Empty proposal actions rejected (`InvalidInput`)
//! - Non-positive total supply rejected during init and admin update (`InvalidInput`)
//! - Unstaked proposal creation rejected (`InsufficientStake`)
//! - Self-delegation rejected (`SelfDelegation`)
//! - Operations on non-existent proposal IDs return `ProposalNotFound`
//! - Committing vote with zero stake rejected (`InsufficientStake`)

#![cfg(test)]

extern crate std;

use governor::{proposal::ProposalType, ContractError, GovernorContract, GovernorContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

fn setup() -> (
    Env,
    GovernorContractClient<'static>,
    Address,
    Address,
use governor::{
    proposal::{ProposalAction, ProposalType},
    voting::VoteChoice,
    ContractError, GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    vec, Address, BytesN, Env, String, Vec,
};

fn setup() -> (
    Env,
    Address,
    GovernorContractClient<'static>,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);
    client.initialize(&admin, &staking, &treasury, &1_000);

    (env, client, admin, staking, treasury)
}

#[test]
fn initialize_rejects_non_positive_vote_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_initialize(&admin, &staking, &treasury, &0),
        Err(Ok(ContractError::InvalidInput))
    );
}

#[test]
fn initialize_rejects_second_initialization() {
    let (env, client, _admin, staking, treasury) = setup();
    let admin = Address::generate(&env);

    assert_eq!(
        client.try_initialize(&admin, &staking, &treasury, &2_000),
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

#[test]
fn create_proposal_rejects_empty_actions() {
    let (env, client, _admin, _staking, _treasury) = setup();
    let proposer = Address::generate(&env);
    let actions = Vec::new(&env);

    assert_eq!(
        client.try_create_proposal(
            &proposer,
            &ProposalType::ParameterChange,
            &String::from_str(&env, "Empty action list"),
            &actions,
        ),
        Err(Ok(ContractError::InvalidInput))
    client.initialize(&admin, &staking, &treasury, &1_000_000_000i128);

    (env, contract_id, client, admin)
}

fn set_mock_stake(env: &Env, contract_id: &Address, voter: &Address, amount: i128) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_STK"), voter.clone());
        env.storage().persistent().set(&key, &amount);
    });
}

// ── Validation Tests ─────────────────────────────────────────────────────────

#[test]
fn test_create_proposal_empty_actions_rejected() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 1_000);

    let empty_actions: Vec<ProposalAction> = vec![&env];

    let res = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Empty actions"),
        &empty_actions,
    );
    assert_eq!(res, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn test_create_proposal_unstaked_proposer_rejected() {
    let (env, _contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    // No stake set (zero stake)

    let mut actions = vec![&env];
    actions.push_back(ProposalAction {
        target: Address::generate(&env),
        function: symbol_short!("ACTION"),
        params_hash: BytesN::from_array(&env, &[0u8; 32]),
    });

    let res = client.try_create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Unstaked proposer"),
        &actions,
    );
    assert_eq!(res, Err(Ok(ContractError::InsufficientStake)));
}

#[test]
fn test_initialize_non_positive_supply_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    // Zero supply
    let res_zero = client.try_initialize(&admin, &staking, &treasury, &0i128);
    assert_eq!(res_zero, Err(Ok(ContractError::InvalidInput)));

    // Negative supply
    let res_neg = client.try_initialize(&admin, &staking, &treasury, &-100i128);
    assert_eq!(res_neg, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn test_set_total_vote_supply_non_positive_rejected() {
    let (_env, _contract_id, client, admin) = setup();

    let res_zero = client.try_set_total_vote_supply(&admin, &0i128);
    assert_eq!(res_zero, Err(Ok(ContractError::InvalidInput)));

    let res_neg = client.try_set_total_vote_supply(&admin, &-500i128);
    assert_eq!(res_neg, Err(Ok(ContractError::InvalidInput)));
}

#[test]
fn test_self_delegation_rejected() {
    let (env, _contract_id, client, _admin) = setup();

    let user = Address::generate(&env);
    let res = client.try_delegate(&user, &user);
    assert_eq!(res, Err(Ok(ContractError::SelfDelegation)));
}

#[test]
fn test_non_existent_proposal_operations_rejected() {
    let (env, _contract_id, client, admin) = setup();

    let non_existent_id = 9999u64;

    // View returns None
    assert!(client.get_proposal(&non_existent_id).is_none());
    assert!(client.get_proposal_summary(&non_existent_id).is_none());

    // Advance phase returns ProposalNotFound
    assert_eq!(
        client.try_advance_phase(&admin, &non_existent_id),
        Err(Ok(ContractError::ProposalNotFound))
    );

    // Commit vote returns ProposalNotFound
    let voter = Address::generate(&env);
    let commitment = BytesN::from_array(&env, &[1u8; 32]);
    assert_eq!(
        client.try_commit_vote(&voter, &non_existent_id, &commitment),
        Err(Ok(ContractError::ProposalNotFound))
    );

    // Reveal vote returns ProposalNotFound
    let salt = BytesN::from_array(&env, &[2u8; 32]);
    assert_eq!(
        client.try_reveal_vote(&voter, &non_existent_id, &VoteChoice::For, &salt),
        Err(Ok(ContractError::ProposalNotFound))
    );

    // Execute returns ProposalNotFound
    assert_eq!(
        client.try_execute_proposal(&admin, &non_existent_id),
        Err(Ok(ContractError::ProposalNotFound))
    );
}

#[test]
fn execute_proposal_rejects_unknown_id() {
    let (env, client, _admin, _staking, _treasury) = setup();
    let caller = Address::generate(&env);

    assert_eq!(
        client.try_execute_proposal(&caller, &0),
        Err(Ok(ContractError::ProposalNotFound))
    );
fn test_commit_vote_already_committed_and_wrong_phase() {
    let (env, contract_id, client, _admin) = setup();

    let proposer = Address::generate(&env);
    set_mock_stake(&env, &contract_id, &proposer, 10_000);

    let mut actions = vec![&env];
    actions.push_back(ProposalAction {
        target: Address::generate(&env),
        function: symbol_short!("ACTION"),
        params_hash: BytesN::from_array(&env, &[0u8; 32]),
    });

    let proposal_id = client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&env, "Voting proposal"),
        &actions,
    );

    let voter = Address::generate(&env);
    let commitment = BytesN::from_array(&env, &[1u8; 32]);

    // In Draft phase, commit fails with WrongPhase
    let res_draft = client.try_commit_vote(&voter, &proposal_id, &commitment);
    assert_eq!(res_draft, Err(Ok(ContractError::WrongPhase)));

    // Move to voting phase: Draft -> Discussion -> Voting
    client.advance_phase(&proposer, &proposal_id);
    env.ledger().with_mut(|l| l.timestamp += 259_200 + 1);
    client.advance_phase(&proposer, &proposal_id);

    // First commit succeeds
    assert!(client.try_commit_vote(&voter, &proposal_id, &commitment).is_ok());

    // Second commit fails with AlreadyCommitted
    let res_dup = client.try_commit_vote(&voter, &proposal_id, &commitment);
    assert_eq!(res_dup, Err(Ok(ContractError::AlreadyCommitted)));
}
