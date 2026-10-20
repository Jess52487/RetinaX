#![cfg(test)]

extern crate std;

use governor::{proposal::ProposalType, ContractError, GovernorContract, GovernorContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

fn setup() -> (
    Env,
    GovernorContractClient<'static>,
    Address,
    Address,
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
}
