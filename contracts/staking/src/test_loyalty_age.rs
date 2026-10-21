//! Tests for stake-age tracking that feeds the Governor's loyalty multiplier.
//! Covers: age progression over 1 / 30 / 365 days, top-ups, partial unstake
//! (age kept) and full unstake (age reset).

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, Env,
};

use crate::{StakingContract, StakingContractClient};

const DAY: u64 = 86_400;
const START: u64 = 1_000_000;

fn setup() -> (Env, StakingContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(START);

    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);
    client.initialize(
        &Address::generate(&env),
        &stake_token.address(),
        &reward_token.address(),
        &0,
        &0,
    );

    (env, client, stake_token.address())
}

fn new_staker(env: &Env, stake_token: &Address, amount: i128) -> Address {
    let staker = Address::generate(env);
    StellarAssetClient::new(env, stake_token).mint(&staker, &amount);
    staker
}

fn advance(env: &Env, secs: u64) {
    env.ledger().with_mut(|l| l.timestamp += secs);
}

#[test]
fn test_never_staked_has_zero_age() {
    let (env, client, _) = setup();
    let who = Address::generate(&env);
    assert_eq!(client.get_stake_since(&who), 0);
    assert_eq!(client.get_stake_age(&who), 0);
}

#[test]
fn test_stake_age_progression_1_30_365_days() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 1_000);
    client.stake(&staker, &1_000);
    assert_eq!(client.get_stake_since(&staker), START);

    advance(&env, DAY);
    assert_eq!(client.get_stake_age(&staker), DAY);

    advance(&env, 29 * DAY);
    assert_eq!(client.get_stake_age(&staker), 30 * DAY);

    advance(&env, 335 * DAY);
    assert_eq!(client.get_stake_age(&staker), 365 * DAY);
}

#[test]
fn test_top_up_does_not_reset_age() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 2_000);
    client.stake(&staker, &1_000);

    advance(&env, 30 * DAY);
    client.stake(&staker, &1_000);

    assert_eq!(client.get_stake_since(&staker), START);
    assert_eq!(client.get_stake_age(&staker), 30 * DAY);
}

#[test]
fn test_partial_unstake_keeps_age() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 1_000);
    client.stake(&staker, &1_000);

    advance(&env, 30 * DAY);
    client.request_unstake(&staker, &400);

    assert_eq!(client.get_staked(&staker), 600);
    assert_eq!(client.get_stake_since(&staker), START);
    assert_eq!(client.get_stake_age(&staker), 30 * DAY);
}

#[test]
fn test_full_unstake_resets_age() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 1_000);
    client.stake(&staker, &1_000);

    advance(&env, 365 * DAY);
    client.request_unstake(&staker, &1_000);

    assert_eq!(client.get_staked(&staker), 0);
    assert_eq!(client.get_stake_since(&staker), 0);
    assert_eq!(client.get_stake_age(&staker), 0);
}

#[test]
fn test_full_unstake_in_steps_resets_age() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 1_000);
    client.stake(&staker, &1_000);

    advance(&env, 10 * DAY);
    client.request_unstake(&staker, &500);
    assert_eq!(client.get_stake_age(&staker), 10 * DAY);

    client.request_unstake(&staker, &500);
    assert_eq!(client.get_stake_age(&staker), 0);
}

#[test]
fn test_restake_after_full_unstake_starts_new_clock() {
    let (env, client, token) = setup();
    let staker = new_staker(&env, &token, 1_000);
    client.stake(&staker, &1_000);

    advance(&env, 100 * DAY);
    let req = client.request_unstake(&staker, &1_000);
    client.withdraw(&staker, &req);

    advance(&env, 5 * DAY);
    let restake_at = env.ledger().timestamp();
    client.stake(&staker, &1_000);
    assert_eq!(client.get_stake_since(&staker), restake_at);

    advance(&env, DAY);
    assert_eq!(client.get_stake_age(&staker), DAY);
}

#[test]
fn test_ages_are_tracked_per_staker() {
    let (env, client, token) = setup();
    let early = new_staker(&env, &token, 1_000);
    let late = new_staker(&env, &token, 1_000);

    client.stake(&early, &1_000);
    advance(&env, 30 * DAY);
    client.stake(&late, &1_000);
    advance(&env, DAY);

    assert_eq!(client.get_stake_age(&early), 31 * DAY);
    assert_eq!(client.get_stake_age(&late), DAY);

    // One staker leaving must not affect the other's clock.
    client.request_unstake(&late, &1_000);
    assert_eq!(client.get_stake_age(&early), 31 * DAY);
}
