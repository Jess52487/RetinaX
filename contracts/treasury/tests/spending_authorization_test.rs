#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use governor::{GovernorContract, GovernorContractClient};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Vec,
};
use treasury::{
    AllocationSummary, ContractError, ProposalStatus, TreasuryContract,
    TreasuryContractClient,
};

// ── Mock DEX Router ──────────────────────────────────────────────────────────

#[contract]
pub struct MockDexRouter;

#[contractimpl]
impl MockDexRouter {
    pub fn swap(
        env: Env,
        from: Address,
        path: Vec<Address>,
        amount_in: i128,
        min_out: i128,
    ) -> i128 {
        from.require_auth();
        let asset_in = path.get(0).unwrap();
        let token_in = TokenClient::new(&env, &asset_in);
        token_in.transfer_from(&env.current_contract_address(), &from, &env.current_contract_address(), &amount_in);

        let asset_out = path.get(path.len() - 1).unwrap();
        let token_out = TokenClient::new(&env, &asset_out);
        let out_amount = if amount_in >= min_out { amount_in } else { min_out };
        token_out.transfer(&env.current_contract_address(), &from, &out_amount);
        out_amount
    }
}

// ── Test Context ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
struct IntegratedContext<'a> {
    env: Env,
    treasury_client: TreasuryContractClient<'a>,
    treasury_id: Address,
    governor_client: GovernorContractClient<'a>,
    governor_id: Address,
    admin: Address,
    signers: std::vec::Vec<Address>,
    token_a: Address,
    token_b: Address,
    token_c: Address,
    router_id: Address,
}

fn setup_integrated_system() -> IntegratedContext<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let admin = Address::generate(&env);
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);

    // Deploy tokens
    let asset_a_admin = Address::generate(&env);
    let token_a = env.register_stellar_asset_contract_v2(asset_a_admin).address();

    let asset_b_admin = Address::generate(&env);
    let token_b = env.register_stellar_asset_contract_v2(asset_b_admin).address();

    let asset_c_admin = Address::generate(&env);
    let token_c = env.register_stellar_asset_contract_v2(asset_c_admin).address();

    // Deploy DEX router
    let router_id = env.register(MockDexRouter, ());

    // Deploy Treasury
    let treasury_id = env.register(TreasuryContract, ());
    let treasury_client = TreasuryContractClient::new(&env, &treasury_id);

    let mut signers_vec = Vec::new(&env);
    signers_vec.push_back(s1.clone());
    signers_vec.push_back(s2.clone());
    signers_vec.push_back(s3.clone());

    // Initialize Treasury (2-of-3 multisig)
    treasury_client.initialize(&admin, &signers_vec, &2);
    treasury_client.set_dex_router(&admin, &router_id);

    // Deploy Governor
    let governor_id = env.register(GovernorContract, ());
    let governor_client = GovernorContractClient::new(&env, &governor_id);

    let staking_contract = Address::generate(&env);
    governor_client.initialize(&admin, &staking_contract, &treasury_id, &1_000_000i128);

    // Link Governor to Treasury
    treasury_client.set_governor(&admin, &governor_id);

    // Mint balances to Treasury: 500,000 token A, 300,000 token B, 200,000 token C
    StellarAssetClient::new(&env, &token_a).mint(&treasury_id, &500_000i128);
    StellarAssetClient::new(&env, &token_b).mint(&treasury_id, &300_000i128);
    StellarAssetClient::new(&env, &token_c).mint(&treasury_id, &200_000i128);

    // Mint to router for swaps
    StellarAssetClient::new(&env, &token_b).mint(&router_id, &500_000i128);
    StellarAssetClient::new(&env, &token_c).mint(&router_id, &500_000i128);

    IntegratedContext {
        env,
        treasury_client,
        treasury_id,
        governor_client,
        governor_id,
        admin,
        signers: std::vec![s1, s2, s3],
        token_a,
        token_b,
        token_c,
        router_id,
    }
}

// ── Test 1: Governor spending proposal passes and executes ───────────────────

#[test]
fn test_governor_spending_proposal_passes_and_executes() {
    let ctx = setup_integrated_system();
    let recipient = Address::generate(&ctx.env);
    let spend_amount = 75_000i128;

    let initial_treasury_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);

    // Verify governor is properly linked
    assert_eq!(ctx.treasury_client.get_governor(), Some(ctx.governor_id.clone()));

    // Governor executes spend on Treasury
    let spend_res = ctx.treasury_client.try_governor_spend(
        &ctx.governor_id,
        &ctx.token_a,
        &recipient,
        &spend_amount,
    );
    assert!(spend_res.is_ok());

    // Verify recipient received exact amount
    let recipient_balance = TokenClient::new(&ctx.env, &ctx.token_a).balance(&recipient);
    assert_eq!(recipient_balance, spend_amount);

    // Balance invariant: initial = remaining + recipient
    let remaining_treasury_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);
    assert_eq!(initial_treasury_a, remaining_treasury_a + recipient_balance);

    // Category allocation invariant: GOVERN allocation tracked
    let alloc: AllocationSummary = ctx
        .treasury_client
        .get_allocation_for_category(&ctx.token_a, &symbol_short!("GOVERN"));
    assert_eq!(alloc.total_spent, spend_amount);
}

// ── Test 2: Direct treasury withdrawal without governor is rejected ──────────

#[test]
fn test_direct_treasury_withdrawal_without_governor_rejected() {
    let ctx = setup_integrated_system();
    let attacker = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);

    // 1. Random attacker calling governor_spend is rejected
    let res = ctx.treasury_client.try_governor_spend(
        &attacker,
        &ctx.token_a,
        &recipient,
        &10_000i128,
    );
    assert_eq!(res, Err(Ok(ContractError::NotAuthorizedCaller)));

    // 2. Even a multisig signer calling governor_spend directly is rejected
    let signer_attempt = ctx.treasury_client.try_governor_spend(
        &ctx.signers[0],
        &ctx.token_a,
        &recipient,
        &10_000i128,
    );
    assert_eq!(signer_attempt, Err(Ok(ContractError::NotAuthorizedCaller)));

    // 3. Admin calling governor_spend directly is rejected (must go through governor)
    let admin_attempt = ctx.treasury_client.try_governor_spend(
        &ctx.admin,
        &ctx.token_a,
        &recipient,
        &10_000i128,
    );
    assert_eq!(admin_attempt, Err(Ok(ContractError::NotAuthorizedCaller)));
}

// ── Test 3: Spending proposal exceeding balance or limits is rejected ────────

#[test]
fn test_spending_proposal_exceeding_balance_or_limit_rejected() {
    let ctx = setup_integrated_system();
    let recipient = Address::generate(&ctx.env);

    // 1. Non-positive spend amount rejected
    let res_zero = ctx.treasury_client.try_governor_spend(
        &ctx.governor_id,
        &ctx.token_a,
        &recipient,
        &0i128,
    );
    assert_eq!(res_zero, Err(Ok(ContractError::PositiveAmountRequired)));

    let res_neg = ctx.treasury_client.try_governor_spend(
        &ctx.governor_id,
        &ctx.token_a,
        &recipient,
        &-100i128,
    );
    assert_eq!(res_neg, Err(Ok(ContractError::PositiveAmountRequired)));

    // 2. Set asset limit to 50,000
    ctx.treasury_client.set_asset_limit(&ctx.admin, &ctx.token_a, &50_000i128);

    // Spend of 50,001 exceeds limit
    let res_limit = ctx.treasury_client.try_governor_spend(
        &ctx.governor_id,
        &ctx.token_a,
        &recipient,
        &50_001i128,
    );
    assert_eq!(res_limit, Err(Ok(ContractError::LimitExceeded)));

    // Spend of 50,000 within limit succeeds
    assert!(ctx
        .treasury_client
        .try_governor_spend(&ctx.governor_id, &ctx.token_a, &recipient, &50_000i128)
        .is_ok());

    // 3. Transfer proposal exceeding limit is rejected
    let prop_res = ctx.treasury_client.try_create_transfer_proposal(
        &ctx.signers[0],
        &ctx.token_a,
        &recipient,
        &60_000i128,
        &symbol_short!("OPS"),
        &String::from_str(&ctx.env, "Exceeds limit"),
        &2_000u64,
    );
    assert_eq!(prop_res, Err(Ok(ContractError::LimitExceeded)));
}

// ── Test 4: Multi-token treasury management ──────────────────────────────────

#[test]
fn test_multi_token_treasury_management() {
    let ctx = setup_integrated_system();
    let recipient_a = Address::generate(&ctx.env);
    let recipient_b = Address::generate(&ctx.env);
    let recipient_c = Address::generate(&ctx.env);

    // Set distinct limits per token
    ctx.treasury_client.set_asset_limit(&ctx.admin, &ctx.token_a, &80_000i128);
    ctx.treasury_client.set_asset_limit(&ctx.admin, &ctx.token_b, &40_000i128);
    // Token C has no limit (defaults to i128::MAX)

    assert_eq!(ctx.treasury_client.get_limit(&ctx.token_a), 80_000i128);
    assert_eq!(ctx.treasury_client.get_limit(&ctx.token_b), 40_000i128);
    assert_eq!(ctx.treasury_client.get_limit(&ctx.token_c), i128::MAX);

    // 1. Governor spend for Token A
    ctx.treasury_client.governor_spend(
        &ctx.governor_id,
        &ctx.token_a,
        &recipient_a,
        &30_000i128,
    );

    // 2. Multisig transfer proposal for Token B
    let prop_b = ctx.treasury_client.create_transfer_proposal(
        &ctx.signers[0],
        &ctx.token_b,
        &recipient_b,
        &25_000i128,
        &symbol_short!("GRANTS"),
        &String::from_str(&ctx.env, "Token B grant"),
        &5_000u64,
    );
    ctx.treasury_client.approve_proposal(&ctx.signers[1], &prop_b.id);
    ctx.treasury_client.execute_proposal(&ctx.signers[0], &prop_b.id);

    // 3. Multisig transfer proposal for Token C
    let prop_c = ctx.treasury_client.create_transfer_proposal(
        &ctx.signers[1],
        &ctx.token_c,
        &recipient_c,
        &100_000i128,
        &symbol_short!("DEV"),
        &String::from_str(&ctx.env, "Token C dev fund"),
        &5_000u64,
    );
    ctx.treasury_client.approve_proposal(&ctx.signers[2], &prop_c.id);
    ctx.treasury_client.execute_proposal(&ctx.signers[1], &prop_c.id);

    // Verify all recipients received their designated token amounts
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_a).balance(&recipient_a),
        30_000i128
    );
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_b).balance(&recipient_b),
        25_000i128
    );
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_c).balance(&recipient_c),
        100_000i128
    );

    // Verify remaining balances in treasury
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id),
        470_000i128
    );
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_b).balance(&ctx.treasury_id),
        275_000i128
    );
    assert_eq!(
        TokenClient::new(&ctx.env, &ctx.token_c).balance(&ctx.treasury_id),
        100_000i128
    );
}

// ── Test 5: Balance invariants maintained across operations ──────────────────

#[test]
fn test_balance_invariants_and_category_allocation_tracking() {
    let ctx = setup_integrated_system();
    let initial_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);

    let rec1 = Address::generate(&ctx.env);
    let rec2 = Address::generate(&ctx.env);

    // Operation 1: Governor Spend (20,000)
    ctx.treasury_client.governor_spend(&ctx.governor_id, &ctx.token_a, &rec1, &20_000i128);

    // Operation 2: Multisig Transfer (50,000)
    let prop = ctx.treasury_client.create_transfer_proposal(
        &ctx.signers[0],
        &ctx.token_a,
        &rec2,
        &50_000i128,
        &symbol_short!("MED_RES"),
        &String::from_str(&ctx.env, "Medical Research"),
        &5_000u64,
    );
    ctx.treasury_client.approve_proposal(&ctx.signers[1], &prop.id);
    ctx.treasury_client.execute_proposal(&ctx.signers[0], &prop.id);

    let remaining_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);
    let bal1 = TokenClient::new(&ctx.env, &ctx.token_a).balance(&rec1);
    let bal2 = TokenClient::new(&ctx.env, &ctx.token_a).balance(&rec2);

    // Strict balance invariant
    assert_eq!(initial_a, remaining_a + bal1 + bal2);

    // Allocation tracking invariants
    let gov_alloc = ctx
        .treasury_client
        .get_allocation_for_category(&ctx.token_a, &symbol_short!("GOVERN"));
    assert_eq!(gov_alloc.total_spent, 20_000i128);

    let med_alloc = ctx
        .treasury_client
        .get_allocation_for_category(&ctx.token_a, &symbol_short!("MED_RES"));
    assert_eq!(med_alloc.total_spent, 50_000i128);
}

// ── Test 6: Multi-token DEX swap proposal lifecycle ──────────────────────────

#[test]
fn test_swap_proposal_multi_token_rebalance() {
    let ctx = setup_integrated_system();

    let initial_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);
    let initial_b = TokenClient::new(&ctx.env, &ctx.token_b).balance(&ctx.treasury_id);

    let mut path = Vec::new(&ctx.env);
    path.push_back(ctx.token_a.clone());
    path.push_back(ctx.token_b.clone());

    let swap_amount = 15_000i128;
    let min_out = 15_000i128;

    let proposal = ctx.treasury_client.create_swap_proposal(
        &ctx.signers[0],
        &path,
        &swap_amount,
        &min_out,
        &symbol_short!("REBAL"),
        &String::from_str(&ctx.env, "Rebalance treasury A to B"),
        &5_000u64,
    );

    ctx.treasury_client.approve_proposal(&ctx.signers[1], &proposal.id);
    ctx.treasury_client.execute_proposal(&ctx.signers[0], &proposal.id);

    let final_b = TokenClient::new(&ctx.env, &ctx.token_b).balance(&ctx.treasury_id);
    let final_a = TokenClient::new(&ctx.env, &ctx.token_a).balance(&ctx.treasury_id);
    assert_eq!(final_b - initial_b, swap_amount);
    assert_eq!(initial_a - final_a, swap_amount);

    let updated_prop = ctx.treasury_client.get_proposal(&proposal.id).unwrap();
    assert_eq!(updated_prop.status, ProposalStatus::Executed);
}

// ── Test 7: Unauthorized Governor registration rejected ──────────────────────

#[test]
fn test_set_governor_unauthorized_rejected() {
    let ctx = setup_integrated_system();
    let stranger = Address::generate(&ctx.env);
    let fake_governor = Address::generate(&ctx.env);

    let res = ctx.treasury_client.try_set_governor(&stranger, &fake_governor);
    assert_eq!(res, Err(Ok(ContractError::UnauthorisedAdmin)));

    // Admin can update governor
    let new_governor = Address::generate(&ctx.env);
    assert!(ctx.treasury_client.try_set_governor(&ctx.admin, &new_governor).is_ok());
    assert_eq!(ctx.treasury_client.get_governor(), Some(new_governor));
}
