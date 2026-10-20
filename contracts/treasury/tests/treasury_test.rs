#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Symbol, Vec,
};
use treasury::{
    AllocationSummary, ContractError, Proposal, ProposalKind, ProposalStatus, SwapProposal,
    TransferProposal, TreasuryConfig, TreasuryContract, TreasuryContractClient,
};

#[contract]
pub struct MockDexRouter;

#[contractimpl]
impl MockDexRouter {
    pub fn swap(
        env: Env,
        from: Address,
        path: Vec<Address>,
        amount_in: i128,
        _min_out: i128,
    ) -> i128 {
        from.require_auth();
        let asset_out = path.get(path.len() - 1).unwrap();
        let token_out = TokenClient::new(&env, &asset_out);
        token_out.transfer(&env.current_contract_address(), &from, &amount_in);
        amount_in
    }
}

struct TestContext<'a> {
    env: Env,
    client: TreasuryContractClient<'a>,
    admin: Address,
    signers: std::vec::Vec<Address>,
    token1: Address,
    token2: Address,
    router: Address,
    contract_id: Address,
}

fn setup_treasury() -> TestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let asset1_admin = Address::generate(&env);
    let asset1 = env.register_stellar_asset_contract_v2(asset1_admin);
    let token1 = asset1.address();

    let asset2_admin = Address::generate(&env);
    let asset2 = env.register_stellar_asset_contract_v2(asset2_admin);
    let token2 = asset2.address();

    let router = env.register(MockDexRouter, ());

    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);

    let mut signers_vec = Vec::new(&env);
    signers_vec.push_back(s1.clone());
    signers_vec.push_back(s2.clone());
    signers_vec.push_back(s3.clone());

    // 2-of-3 threshold
    client.initialize(&admin, &signers_vec, &2);

    // Initial deposit: 1,000,000 units of token1 and token2
    StellarAssetClient::new(&env, &token1).mint(&contract_id, &1_000_000i128);
    StellarAssetClient::new(&env, &token2).mint(&contract_id, &1_000_000i128);
    StellarAssetClient::new(&env, &token2).mint(&router, &1_000_000i128);

    TestContext {
        env,
        client,
        admin,
        signers: std::vec![s1, s2, s3],
        token1,
        token2,
        router,
        contract_id,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Initialization and Parameter Validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_initialization_lifecycle_and_validation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let s1 = Address::generate(&env);

    let mut empty_signers = Vec::new(&env);
    assert_eq!(
        client.try_initialize(&admin, &empty_signers, &1),
        Err(Ok(ContractError::NoSigners))
    );

    let mut signers = Vec::new(&env);
    signers.push_back(s1);

    // Threshold 0
    assert_eq!(
        client.try_initialize(&admin, &signers, &0),
        Err(Ok(ContractError::InvalidThreshold))
    );

    // Threshold greater than signers count
    assert_eq!(
        client.try_initialize(&admin, &signers, &2),
        Err(Ok(ContractError::InvalidThreshold))
    );

    // Valid initialization
    assert!(client.try_initialize(&admin, &signers, &1).is_ok());

    // Already initialized
    assert_eq!(
        client.try_initialize(&admin, &signers, &1),
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Admin Configurations and Authorization
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_admin_functions_authorization() {
    let ctx = setup_treasury();
    let unauthorized = Address::generate(&ctx.env);

    // DEX Router
    assert_eq!(
        ctx.client.try_set_dex_router(&unauthorized, &ctx.router),
        Err(Ok(ContractError::UnauthorisedAdmin))
    );
    assert!(ctx
        .client
        .try_set_dex_router(&ctx.admin, &ctx.router)
        .is_ok());

    let cfg: TreasuryConfig = ctx.client.get_config();
    assert_eq!(cfg.dex_router, Some(ctx.router));

    // Asset Limits
    assert_eq!(
        ctx.client
            .try_set_asset_limit(&unauthorized, &ctx.token1, &50_000i128),
        Err(Ok(ContractError::UnauthorisedAdmin))
    );
    assert!(ctx
        .client
        .try_set_asset_limit(&ctx.admin, &ctx.token1, &50_000i128)
        .is_ok());
    assert_eq!(ctx.client.get_limit(&ctx.token1), 50_000i128);
    // Unset asset returns i128::MAX
    assert_eq!(ctx.client.get_limit(&ctx.token2), i128::MAX);

    // Governor
    let governor = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client.try_set_governor(&unauthorized, &governor),
        Err(Ok(ContractError::UnauthorisedAdmin))
    );
    assert!(ctx.client.try_set_governor(&ctx.admin, &governor).is_ok());
    assert_eq!(ctx.client.get_governor(), Some(governor));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Governor Spend Lifecycle & Balance Invariants
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_governor_spend_lifecycle_and_balance_invariants() {
    let ctx = setup_treasury();
    let governor = Address::generate(&ctx.env);
    ctx.client.set_governor(&ctx.admin, &governor);

    let recipient = Address::generate(&ctx.env);
    let token_client = TokenClient::new(&ctx.env, &ctx.token1);

    let initial_balance = token_client.balance(&ctx.contract_id);
    let spend_amount = 25_000i128;

    // Unauthorized caller cannot spend
    let impostor = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client
            .try_governor_spend(&impostor, &ctx.token1, &recipient, &spend_amount),
        Err(Ok(ContractError::NotAuthorizedCaller))
    );

    // Zero / negative amount rejected
    assert_eq!(
        ctx.client
            .try_governor_spend(&governor, &ctx.token1, &recipient, &0i128),
        Err(Ok(ContractError::PositiveAmountRequired))
    );

    // Governor spend exceeds limit
    ctx.client
        .set_asset_limit(&ctx.admin, &ctx.token1, &10_000i128);
    assert_eq!(
        ctx.client
            .try_governor_spend(&governor, &ctx.token1, &recipient, &spend_amount),
        Err(Ok(ContractError::LimitExceeded))
    );

    // Reset limit and execute valid spend
    ctx.client
        .set_asset_limit(&ctx.admin, &ctx.token1, &100_000i128);
    let spend_res =
        ctx.client
            .try_governor_spend(&governor, &ctx.token1, &recipient, &spend_amount);
    assert!(spend_res.is_ok());

    // Balance invariant: initial = spent + remaining
    let remaining_balance = token_client.balance(&ctx.contract_id);
    let recipient_balance = token_client.balance(&recipient);
    assert_eq!(recipient_balance, spend_amount);
    assert_eq!(initial_balance, remaining_balance + recipient_balance);

    // Allocation tracking for governor spending
    let alloc: AllocationSummary = ctx
        .client
        .get_allocation_for_category(&ctx.token1, &symbol_short!("GOVERN"));
    assert_eq!(alloc.total_spent, spend_amount);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Transfer Proposal Full Multi-Sig Lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_transfer_proposal_full_multisig_lifecycle() {
    let ctx = setup_treasury();
    let proposer = ctx.signers[0].clone();
    let second_signer = ctx.signers[1].clone();
    let recipient = Address::generate(&ctx.env);
    let token_client = TokenClient::new(&ctx.env, &ctx.token1);

    let initial_balance = token_client.balance(&ctx.contract_id);
    let amount = 100_000i128;
    let category = symbol_short!("GRANT");
    let desc = String::from_str(&ctx.env, "Research Grant");
    let expires_at = ctx.env.ledger().timestamp() + 3600;

    // 1. Create proposal
    let proposal: Proposal = ctx.client.create_transfer_proposal(
        &proposer,
        &ctx.token1,
        &recipient,
        &amount,
        &category,
        &desc,
        &expires_at,
    );
    assert_eq!(proposal.id, 1);
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.approvals.len(), 1);

    // Attempting to execute with 1 approval fails (threshold is 2)
    assert_eq!(
        ctx.client.try_execute_proposal(&proposer, &proposal.id),
        Err(Ok(ContractError::InsufficientApprovals))
    );

    // 2. Second signer approves
    assert!(ctx
        .client
        .try_approve_proposal(&second_signer, &proposal.id)
        .is_ok());

    let approved_prop = ctx.client.get_proposal(&proposal.id).unwrap();
    assert_eq!(approved_prop.approvals.len(), 2);

    // 3. Execute proposal
    assert!(ctx
        .client
        .try_execute_proposal(&second_signer, &proposal.id)
        .is_ok());

    let executed_prop = ctx.client.get_proposal(&proposal.id).unwrap();
    assert_eq!(executed_prop.status, ProposalStatus::Executed);

    // Balance invariant verification
    let remaining = token_client.balance(&ctx.contract_id);
    let recipient_balance = token_client.balance(&recipient);
    assert_eq!(recipient_balance, amount);
    assert_eq!(initial_balance, remaining + recipient_balance);

    // Allocation tracking for category
    let alloc = ctx
        .client
        .get_allocation_for_category(&ctx.token1, &category);
    assert_eq!(alloc.total_spent, amount);

    // Executing already executed proposal fails
    assert_eq!(
        ctx.client
            .try_execute_proposal(&second_signer, &proposal.id),
        Err(Ok(ContractError::ProposalNotPending))
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Transfer Proposal Authorization and Edge Cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_transfer_proposal_authorization_and_validation() {
    let ctx = setup_treasury();
    let unauthorized = Address::generate(&ctx.env);
    let recipient = Address::generate(&ctx.env);
    let expires_at = ctx.env.ledger().timestamp() + 3600;

    // Non-signer cannot propose
    assert_eq!(
        ctx.client.try_create_transfer_proposal(
            &unauthorized,
            &ctx.token1,
            &recipient,
            &1000i128,
            &symbol_short!("OPS"),
            &String::from_str(&ctx.env, "desc"),
            &expires_at,
        ),
        Err(Ok(ContractError::UnauthorisedProposer))
    );

    // Zero or negative amount
    assert_eq!(
        ctx.client.try_create_transfer_proposal(
            &ctx.signers[0],
            &ctx.token1,
            &recipient,
            &0i128,
            &symbol_short!("OPS"),
            &String::from_str(&ctx.env, "desc"),
            &expires_at,
        ),
        Err(Ok(ContractError::PositiveAmountRequired))
    );

    // Past expiry
    assert_eq!(
        ctx.client.try_create_transfer_proposal(
            &ctx.signers[0],
            &ctx.token1,
            &recipient,
            &1000i128,
            &symbol_short!("OPS"),
            &String::from_str(&ctx.env, "desc"),
            &ctx.env.ledger().timestamp(),
        ),
        Err(Ok(ContractError::FutureExpiryRequired))
    );

    // Amount exceeding asset limit
    ctx.client
        .set_asset_limit(&ctx.admin, &ctx.token1, &500i128);
    assert_eq!(
        ctx.client.try_create_transfer_proposal(
            &ctx.signers[0],
            &ctx.token1,
            &recipient,
            &1000i128,
            &symbol_short!("OPS"),
            &String::from_str(&ctx.env, "desc"),
            &expires_at,
        ),
        Err(Ok(ContractError::LimitExceeded))
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Proposal Expiration
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_proposal_expiration() {
    let ctx = setup_treasury();
    ctx.env.ledger().set_timestamp(1_000);

    let proposal = ctx.client.create_transfer_proposal(
        &ctx.signers[0],
        &ctx.token1,
        &Address::generate(&ctx.env),
        &100i128,
        &symbol_short!("OPS"),
        &String::from_str(&ctx.env, "expiring"),
        &1_500, // expires at 1500
    );

    // Advance time past expiry
    ctx.env.ledger().set_timestamp(1_501);

    // Approval after expiry fails and marks proposal expired
    assert_eq!(
        ctx.client
            .try_approve_proposal(&ctx.signers[1], &proposal.id),
        Err(Ok(ContractError::ProposalExpired))
    );

    // Execution after expiry fails
    assert_eq!(
        ctx.client
            .try_execute_proposal(&ctx.signers[0], &proposal.id),
        Err(Ok(ContractError::ProposalExpired))
    );

    let stored = ctx.client.get_proposal(&proposal.id).unwrap();
    assert_eq!(stored.status, ProposalStatus::Expired);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Swap Proposal Lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_swap_proposal_lifecycle() {
    let ctx = setup_treasury();
    ctx.client.set_dex_router(&ctx.admin, &ctx.router);

    let mut path = Vec::new(&ctx.env);
    path.push_back(ctx.token1.clone());
    path.push_back(ctx.token2.clone());

    let expires_at = ctx.env.ledger().timestamp() + 3600;

    // Single-asset path rejected
    let mut invalid_path = Vec::new(&ctx.env);
    invalid_path.push_back(ctx.token1.clone());
    assert_eq!(
        ctx.client.try_create_swap_proposal(
            &ctx.signers[0],
            &invalid_path,
            &1000i128,
            &900i128,
            &symbol_short!("SWAP"),
            &String::from_str(&ctx.env, "bad path"),
            &expires_at,
        ),
        Err(Ok(ContractError::InvalidPath))
    );

    // Valid swap proposal
    let proposal: Proposal = ctx.client.create_swap_proposal(
        &ctx.signers[0],
        &path,
        &1000i128,
        &900i128,
        &symbol_short!("SWAP"),
        &String::from_str(&ctx.env, "rebalance"),
        &expires_at,
    );

    // Approve & execute
    ctx.client.approve_proposal(&ctx.signers[1], &proposal.id);
    let exec_res = ctx
        .client
        .try_execute_proposal(&ctx.signers[0], &proposal.id);
    assert!(exec_res.is_ok());

    let executed = ctx.client.get_proposal(&proposal.id).unwrap();
    assert_eq!(executed.status, ProposalStatus::Executed);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Non-Existent Proposals
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_non_existent_proposal_handling() {
    let ctx = setup_treasury();
    assert_eq!(ctx.client.get_proposal(&999), None);
    assert_eq!(
        ctx.client.try_approve_proposal(&ctx.signers[0], &999),
        Err(Ok(ContractError::ProposalNotFound))
    );
    assert_eq!(
        ctx.client.try_execute_proposal(&ctx.signers[0], &999),
        Err(Ok(ContractError::ProposalNotFound))
    );
}
