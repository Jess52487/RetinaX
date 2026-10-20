#![cfg(test)]

extern crate std;

use governor::{ContractError, GovernorContract, GovernorContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, GovernorContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);
    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);
    client.initialize(&admin, &staking, &treasury, &1_000);

    (env, client, admin, stranger)
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
}
