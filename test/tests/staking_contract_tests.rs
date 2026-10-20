//! End-to-end staking contract coverage for the public lifecycle.

extern crate std;

use soroban_sdk::token::Client as TokenClient;
use test_framework::{StakingTestHarness, TestEnv};

#[test]
fn stake_unstake_and_withdraw_preserve_balances() {
    let mut env = TestEnv::new();
    let mut harness = StakingTestHarness::new(&mut env, 0, 100);
    let staker = harness.create_staker(1_000);

    harness.stake(&staker, 600);
    assert_eq!(harness.user_staked(&staker), 600);
    let before = TokenClient::new(&harness.env.env, &harness.stake_token).balance(&staker);

    let request_id = harness.request_unstake(&staker, 600);
    assert_eq!(harness.user_staked(&staker), 0);
    harness.env.advance_time(100);
    harness.withdraw(&staker, request_id);

    assert_eq!(
        TokenClient::new(&harness.env.env, &harness.stake_token).balance(&staker),
        before
    );
    assert!(harness.client.get_unstake_request(&request_id).withdrawn);
}

#[test]
fn reward_claim_is_idempotent_and_time_bounded() {
    let mut env = TestEnv::new();
    let mut harness = StakingTestHarness::new(&mut env, 10, 0);
    let staker = harness.create_staker(1_000);

    harness.env.set_timestamp(0);
    harness.stake(&staker, 1_000);
    harness.env.advance_time(50);
    assert_eq!(harness.pending_rewards(&staker), 500);
    assert_eq!(harness.claim_rewards(&staker), 500);
    assert_eq!(harness.claim_rewards(&staker), 0);
}

#[test]
fn multiple_stakers_conserve_principal_and_rewards() {
    let mut env = TestEnv::new();
    let mut harness = StakingTestHarness::new(&mut env, 100, 0);
    let alice = harness.create_staker(10_000);
    let bob = harness.create_staker(10_000);

    harness.env.set_timestamp(0);
    harness.stake(&alice, 3_000);
    harness.stake(&bob, 1_000);
    harness.env.set_timestamp(100);

    assert_eq!(harness.total_staked(), 4_000);
    assert_eq!(
        harness.pending_rewards(&alice) + harness.pending_rewards(&bob),
        10_000
    );
}

#[test]
fn initialize_rejects_identical_token_addresses() {
    let mut env = TestEnv::new();
    let token = env.deploy_token_pair().0;
    let contract_id = env.env.register(staking::StakingContract, ());
    let client = staking::StakingContractClient::new(&env.env, &contract_id);
    let admin = env.generate_address();

    let result = client.try_initialize(&admin, &token, &token, &0, &0);
    assert!(result.is_err());
}
