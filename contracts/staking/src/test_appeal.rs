extern crate std;

use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

use crate::{AppealStatus, ContractError, StakingContract, StakingContractClient};

// ── Test helpers ──────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    StakingContractClient<'static>,
    Address, // admin
    Address, // stake_token
) {
    let env = Env::default();
    env.mock_all_auths();

    let stake_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let reward_token = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let stake_token_id = stake_token.address();
    let reward_token_id = reward_token.address();

    let contract_id = env.register(StakingContract, ());
    let client = StakingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &stake_token_id, &reward_token_id, &0i128, &0u64);

    (env, client, admin, stake_token_id)
}

fn mint_and_stake(
    env: &Env,
    client: &StakingContractClient,
    stake_token: &Address,
    staker: &Address,
    amount: i128,
) {
    StellarAssetClient::new(env, stake_token).mint(staker, &amount);
    client.stake(staker, &amount);
}

/// Slash `amount` from `validator` and return the slash_id (always 1 for a fresh contract).
fn slash_and_get_id(
    _env: &Env,
    client: &StakingContractClient,
    admin: &Address,
    validator: &Address,
    amount: i128,
) -> u64 {
    client.slash(admin, validator, &amount);
    // First slash on a fresh instance always gets id 1.
    1
}

// ── Slash record creation ────────────────────────────────────────────────────

#[test]
fn test_slash_creates_slash_record() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    client.slash(&admin, &validator, &400);

    let record = client.get_slash_record(&1);
    assert_eq!(record.validator, validator);
    assert_eq!(record.slashed_by, admin);
    assert_eq!(record.amount, 400);
}

#[test]
fn test_get_slash_record_not_found() {
    let (_env, client, _, _) = setup();
    let result = client.try_get_slash_record(&99);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::SlashRecordNotFound),
        _ => panic!("expected SlashRecordNotFound"),
    }
}

// ── Filing appeals ───────────────────────────────────────────────────────────

#[test]
fn test_file_appeal_succeeds() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    let slash_id = slash_and_get_id(&env, &client, &admin, &validator, 500);

    let appeal_id = client.file_appeal(&validator, &slash_id, &300);
    assert_eq!(appeal_id, 1);

    let appeal = client.get_appeal(&1);
    assert_eq!(appeal.validator, validator);
    assert_eq!(appeal.slash_id, slash_id);
    assert_eq!(appeal.requested_amount, 300);
    assert_eq!(appeal.status, AppealStatus::Pending);
}

#[test]
fn test_file_appeal_zero_amount_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    let slash_id = slash_and_get_id(&env, &client, &admin, &validator, 500);

    let result = client.try_file_appeal(&validator, &slash_id, &0);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AppealAmountInvalid),
        _ => panic!("expected AppealAmountInvalid"),
    }
}

#[test]
fn test_file_appeal_exceeding_slash_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    let slash_id = slash_and_get_id(&env, &client, &admin, &validator, 500);

    let result = client.try_file_appeal(&validator, &slash_id, &600);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::RecoveryExceedsSlash),
        _ => panic!("expected RecoveryExceedsSlash"),
    }
}

#[test]
fn test_file_appeal_wrong_validator_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    let impostor = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    let slash_id = slash_and_get_id(&env, &client, &admin, &validator, 500);

    let result = client.try_file_appeal(&impostor, &slash_id, &100);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::Unauthorized),
        _ => panic!("expected Unauthorized"),
    }
}

#[test]
fn test_duplicate_pending_appeal_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    let slash_id = slash_and_get_id(&env, &client, &admin, &validator, 500);

    client.file_appeal(&validator, &slash_id, &300);
    let result = client.try_file_appeal(&validator, &slash_id, &200);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AppealAlreadyExists),
        _ => panic!("expected AppealAlreadyExists"),
    }
}

// ── Resolving appeals (governance path) ──────────────────────────────────────

#[test]
fn test_resolve_appeal_partial_recovery() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    slash_and_get_id(&env, &client, &admin, &validator, 500);
    assert_eq!(client.get_staked(&validator), 500);

    let appeal_id = client.file_appeal(&validator, &1, &400);
    client.resolve_appeal(&admin, &appeal_id, &300);

    let appeal = client.get_appeal(&appeal_id);
    assert_eq!(appeal.status, AppealStatus::Approved);
    assert_eq!(appeal.approved_amount, 300);
    assert_eq!(
        client.get_staked(&validator),
        800,
        "partial recovery of 300 must restore stake from 500 to 800"
    );
}

#[test]
fn test_resolve_appeal_full_recovery() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    slash_and_get_id(&env, &client, &admin, &validator, 500);

    let appeal_id = client.file_appeal(&validator, &1, &500);
    client.resolve_appeal(&admin, &appeal_id, &500);

    assert_eq!(client.get_staked(&validator), 1_000);
    assert_eq!(client.get_total_staked(), 1_000);
}

#[test]
fn test_resolve_appeal_reject() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    slash_and_get_id(&env, &client, &admin, &validator, 500);

    let appeal_id = client.file_appeal(&validator, &1, &400);
    client.resolve_appeal(&admin, &appeal_id, &0);

    let appeal = client.get_appeal(&appeal_id);
    assert_eq!(appeal.status, AppealStatus::Rejected);
    assert_eq!(appeal.approved_amount, 0);
    assert_eq!(
        client.get_staked(&validator),
        500,
        "rejected appeal restores nothing"
    );
}

#[test]
fn test_resolve_appeal_twice_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    slash_and_get_id(&env, &client, &admin, &validator, 500);

    let appeal_id = client.file_appeal(&validator, &1, &300);
    client.resolve_appeal(&admin, &appeal_id, &200);

    let result = client.try_resolve_appeal(&admin, &appeal_id, &100);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AppealNotPending),
        _ => panic!("expected AppealNotPending"),
    }
}

#[test]
fn test_resolve_appeal_exceeding_slash_fails() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);
    slash_and_get_id(&env, &client, &admin, &validator, 500);

    let appeal_id = client.file_appeal(&validator, &1, &400);
    let result = client.try_resolve_appeal(&admin, &appeal_id, &600);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::RecoveryExceedsSlash),
        _ => panic!("expected RecoveryExceedsSlash"),
    }
}

#[test]
fn test_resolve_appeal_not_found() {
    let (_env, client, admin, _) = setup();
    let result = client.try_resolve_appeal(&admin, &99, &100);
    match result {
        Err(Ok(e)) => assert_eq!(e, ContractError::AppealNotFound),
        _ => panic!("expected AppealNotFound"),
    }
}

// ── Full appeal lifecycle ────────────────────────────────────────────────────

#[test]
fn test_full_appeal_lifecycle() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 2_000);

    // 1. Slash
    client.slash(&admin, &validator, &1_000);
    assert_eq!(client.get_staked(&validator), 1_000);
    assert_eq!(client.get_total_staked(), 1_000);

    // 2. File appeal for partial recovery
    let appeal_id = client.file_appeal(&validator, &1, &600);
    assert_eq!(client.get_appeal(&appeal_id).status, AppealStatus::Pending);

    // 3. Governance approves partial recovery
    client.resolve_appeal(&admin, &appeal_id, &600);
    assert_eq!(client.get_appeal(&appeal_id).status, AppealStatus::Approved);
    assert_eq!(client.get_staked(&validator), 1_600);
    assert_eq!(client.get_total_staked(), 1_600);

    // 4. Second slash on the remaining stake
    client.slash(&admin, &validator, &300);
    assert_eq!(client.get_staked(&validator), 1_300);

    // 5. Appeal the second slash (new slash record id 2)
    let appeal2 = client.file_appeal(&validator, &2, &100);
    client.resolve_appeal(&admin, &appeal2, &0); // rejected
    assert_eq!(client.get_appeal(&appeal2).status, AppealStatus::Rejected);
    assert_eq!(client.get_staked(&validator), 1_300);
}

#[test]
fn test_appeal_after_full_slash_and_recovery_allows_restake() {
    let (env, client, admin, stake_token) = setup();
    let validator = Address::generate(&env);
    mint_and_stake(&env, &client, &stake_token, &validator, 1_000);

    client.slash(&admin, &validator, &1_000);
    assert_eq!(client.get_staked(&validator), 0);

    let appeal_id = client.file_appeal(&validator, &1, &1_000);
    client.resolve_appeal(&admin, &appeal_id, &1_000);
    assert_eq!(client.get_staked(&validator), 1_000);

    // Validator can unstake normally after full recovery.
    let request_id = client.request_unstake(&validator, &1_000);
    assert!(request_id > 0);
}
