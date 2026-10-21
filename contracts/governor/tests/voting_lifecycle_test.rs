//! voting_lifecycle_test.rs
//!
//! Comprehensive governance voting lifecycle integration tests for the Governor DAO contract.
//!
//! Covers:
//! 1. Full proposal lifecycle: Draft -> Discussion -> Voting -> Timelock -> Execution -> Completed
//! 2. Proposal state transitions, validation, and unauthorized advance rejections
//! 3. Quorum not met -> proposal expires and execution is blocked
//! 4. Majority not met -> proposal rejected
//! 5. Commit-reveal voting mechanics & timing edge cases (double commit, non-voting phase, wrong salt, wrong choice, reveal without commit)
//! 6. Delegated commit and reveal lifecycle & delegation revocation
//! 7. Quadratic voting weight verification and loyalty multiplier math
//! 8. Veto threshold reached during timelock -> execution blocked and proposal rejected
//! 9. All proposal types: quorum requirements and timelock durations

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use governor::{
    proposal::{ProposalAction, ProposalPhase, ProposalType},
    voting::{compute_vote_power, isqrt, loyalty_multiplier_scaled, VoteChoice, SCALE},
    ContractError, GovernorContract, GovernorContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    vec, Address, Bytes, BytesN, Env, String, Vec,
};

// ── Test Setup Helpers ────────────────────────────────────────────────────────

#[allow(dead_code)]
struct TestContext<'a> {
    env: Env,
    contract_id: Address,
    client: GovernorContractClient<'a>,
    admin: Address,
    staking: Address,
    treasury: Address,
}

fn setup_test_context(total_supply: i128) -> TestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let staking = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.initialize(&admin, &staking, &treasury, &total_supply);

    TestContext {
        env,
        contract_id,
        client,
        admin,
        staking,
        treasury,
    }
}

fn set_mock_stake(env: &Env, contract_id: &Address, voter: &Address, amount: i128) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_STK"), voter.clone());
        env.storage().persistent().set(&key, &amount);
    });
}

fn set_mock_age(env: &Env, contract_id: &Address, voter: &Address, age_secs: u64) {
    env.as_contract(contract_id, || {
        let key = (symbol_short!("M_AGE"), voter.clone());
        env.storage().persistent().set(&key, &age_secs);
    });
}

fn advance_time(env: &Env, secs: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp.saturating_add(secs);
    });
}

fn sample_actions(env: &Env) -> Vec<ProposalAction> {
    let mut actions = vec![env];
    actions.push_back(ProposalAction {
        target: Address::generate(env),
        function: symbol_short!("GOV_PRM"),
        params_hash: BytesN::from_array(env, &[0xAAu8; 32]),
    });
    actions
}

fn compute_commitment(
    env: &Env,
    proposal_id: u64,
    choice: &VoteChoice,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    for b in proposal_id.to_le_bytes().iter() {
        data.push_back(*b);
    }
    let choice_byte: u8 = match choice {
        VoteChoice::For => 0,
        VoteChoice::Against => 1,
        VoteChoice::Veto => 2,
    };
    data.push_back(choice_byte);
    for i in 0..32u32 {
        data.push_back(salt.get(i).unwrap_or(0));
    }
    env.crypto().sha256(&data).into()
}

// ── Test 1: Full Proposal Lifecycle ──────────────────────────────────────────

#[test]
fn test_full_proposal_lifecycle_create_vote_queue_execute() {
    // Total vote supply = 500. ParameterChange quorum = 15% (75 votes).
    let ctx = setup_test_context(500);

    let proposer = Address::generate(&ctx.env);
    let voter1 = Address::generate(&ctx.env);
    let voter2 = Address::generate(&ctx.env);

    // Staked amounts:
    // proposer: 10,000 tokens (raw power 100)
    // voter1: 10,000 tokens (raw power 100)
    // voter2: 2,500 tokens (raw power 50)
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 10_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &voter1, 10_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &voter2, 2_500);

    // 1. Create Proposal (Draft phase)
    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Update governance fee parameter"),
        &actions,
    );

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Draft);
    assert_eq!(prop.votes_for, 0);
    assert_eq!(prop.votes_against, 0);
    assert_eq!(prop.votes_veto, 0);
    assert_eq!(prop.commit_count, 0);
    assert_eq!(prop.reveal_count, 0);

    // 2. Draft -> Discussion
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Discussion);

    // 3. Discussion -> Voting
    // Advancing before discussion period (3 days) fails
    advance_time(&ctx.env, 100);
    assert_eq!(
        ctx.client.try_advance_phase(&proposer, &prop_id),
        Err(Ok(ContractError::PhaseNotAdvanceable))
    );

    // Advance after 3 days discussion
    advance_time(&ctx.env, 3 * 86_400);
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Voting);

    // 4. Voting Phase: Commit-Reveal
    let salt1 = BytesN::from_array(&ctx.env, &[11u8; 32]);
    let salt2 = BytesN::from_array(&ctx.env, &[22u8; 32]);
    let commit1 = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt1);
    let commit2 = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt2);

    ctx.client.commit_vote(&voter1, &prop_id, &commit1);
    ctx.client.commit_vote(&voter2, &prop_id, &commit2);

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.commit_count, 2);

    let power1 = ctx
        .client
        .reveal_vote(&voter1, &prop_id, &VoteChoice::For, &salt1);
    let power2 = ctx
        .client
        .reveal_vote(&voter2, &prop_id, &VoteChoice::For, &salt2);
    assert_eq!(power1, 100);
    assert_eq!(power2, 50);

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.reveal_count, 2);
    assert_eq!(prop.votes_for, 150);
    assert_eq!(prop.votes_against, 0);

    // 5. Voting -> Timelock (Queue)
    // Advancing before voting period (5 days) fails
    advance_time(&ctx.env, 100);
    assert_eq!(
        ctx.client.try_advance_phase(&proposer, &prop_id),
        Err(Ok(ContractError::PhaseNotAdvanceable))
    );

    // Advance after 5 days voting period
    advance_time(&ctx.env, 5 * 86_400);
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Timelock);

    // 6. Timelock -> Execution
    // Advancing before timelock duration (ParameterChange: 2 days) fails
    advance_time(&ctx.env, 100);
    assert_eq!(
        ctx.client.try_advance_phase(&proposer, &prop_id),
        Err(Ok(ContractError::TimelockNotExpired))
    );

    // Advance after 2 days timelock duration
    advance_time(&ctx.env, 2 * 86_400);
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Execution);

    // 7. Execute Proposal -> Completed
    let caller = Address::generate(&ctx.env);
    ctx.client.execute_proposal(&caller, &prop_id);

    let final_prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(final_prop.phase, ProposalPhase::Completed);

    let summary = ctx.client.get_proposal_summary(&prop_id).unwrap();
    assert_eq!(summary.phase, ProposalPhase::Completed);
    assert_eq!(summary.votes_for, 150);
    assert_eq!(summary.reveal_count, 2);

    // Replay execution should fail
    assert_eq!(
        ctx.client.try_execute_proposal(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );

    // Advancing from Completed phase should fail
    assert_eq!(
        ctx.client.try_advance_phase(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );
}

// ── Test 2: State Transitions & Unauthorized Advancement ─────────────────────

#[test]
fn test_proposal_state_transitions_and_unauthorized_rejections() {
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let stranger = Address::generate(&ctx.env);
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Test Transitions"),
        &actions,
    );

    // Stranger cannot advance Draft to Discussion
    let res = ctx.client.try_advance_phase(&stranger, &prop_id);
    assert_eq!(res, Err(Ok(ContractError::Unauthorized)));

    // Proposer can advance
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Discussion);

    // Cannot advance Discussion -> Voting before discussion_ends
    let res = ctx.client.try_advance_phase(&proposer, &prop_id);
    assert_eq!(res, Err(Ok(ContractError::PhaseNotAdvanceable)));

    // After discussion_ends, anyone can advance Discussion -> Voting
    advance_time(&ctx.env, 3 * 86_400 + 1);
    let phase = ctx.client.advance_phase(&stranger, &prop_id);
    assert_eq!(phase, ProposalPhase::Voting);

    // Cannot advance Voting -> Timelock before voting_ends
    let res = ctx.client.try_advance_phase(&stranger, &prop_id);
    assert_eq!(res, Err(Ok(ContractError::PhaseNotAdvanceable)));
}

// ── Test 3: Quorum Not Met -> Proposal Expired ────────────────────────────────

#[test]
fn test_quorum_not_met_proposal_expires() {
    // Total supply = 1,000. ParameterChange quorum = 15% (150 power needed).
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let voter = Address::generate(&ctx.env);

    // Proposer has 5,000 stake; voter has only 2,500 stake (raw power 50 < 150 needed)
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &voter, 2_500);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Low Participation Proposal"),
        &actions,
    );

    ctx.client.advance_phase(&proposer, &prop_id);
    advance_time(&ctx.env, 3 * 86_400 + 1);
    ctx.client.advance_phase(&proposer, &prop_id);

    // Only voter commits and reveals (50 power total)
    let salt = BytesN::from_array(&ctx.env, &[42u8; 32]);
    let commit = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt);
    ctx.client.commit_vote(&voter, &prop_id, &commit);
    ctx.client.reveal_vote(&voter, &prop_id, &VoteChoice::For, &salt);

    // Voting period elapses
    advance_time(&ctx.env, 5 * 86_400 + 1);

    // Advance phase -> quorum (150) not met (only 50 cast) -> Expired
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Expired);

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.phase, ProposalPhase::Expired);

    // Cannot execute expired proposal
    let caller = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client.try_execute_proposal(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );

    // Cannot advance phase from Expired
    assert_eq!(
        ctx.client.try_advance_phase(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );
}

// ── Test 4: Quorum Met but Majority Failed -> Rejected ───────────────────────

#[test]
fn test_majority_not_met_proposal_rejected() {
    // Total supply = 1,000. Quorum needed = 150.
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);
    let opponent = Address::generate(&ctx.env);

    // Supporter: 2,500 stake (power 50)
    // Opponent: 22,500 stake (power 150)
    // Total votes = 200 > 150 quorum. For ratio = 50 / 200 = 25% < 51% pass threshold.
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &supporter, 2_500);
    set_mock_stake(&ctx.env, &ctx.contract_id, &opponent, 22_500);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Controversial Proposal"),
        &actions,
    );

    ctx.client.advance_phase(&proposer, &prop_id);
    advance_time(&ctx.env, 3 * 86_400 + 1);
    ctx.client.advance_phase(&proposer, &prop_id);

    let salt_sup = BytesN::from_array(&ctx.env, &[1u8; 32]);
    let salt_opp = BytesN::from_array(&ctx.env, &[2u8; 32]);

    let commit_sup = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt_sup);
    let commit_opp = compute_commitment(&ctx.env, prop_id, &VoteChoice::Against, &salt_opp);

    ctx.client.commit_vote(&supporter, &prop_id, &commit_sup);
    ctx.client.commit_vote(&opponent, &prop_id, &commit_opp);

    ctx.client
        .reveal_vote(&supporter, &prop_id, &VoteChoice::For, &salt_sup);
    ctx.client
        .reveal_vote(&opponent, &prop_id, &VoteChoice::Against, &salt_opp);

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.votes_for, 50);
    assert_eq!(prop.votes_against, 150);

    advance_time(&ctx.env, 5 * 86_400 + 1);

    // Advance phase -> Rejected
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Rejected);

    // Execution rejected
    let caller = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client.try_execute_proposal(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );
}

// ── Test 5: Commit-Reveal Voting Edge Cases ──────────────────────────────────

#[test]
fn test_commit_reveal_voting_edge_cases_and_validation() {
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let voter = Address::generate(&ctx.env);
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &voter, 10_000);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Commit Reveal Validation"),
        &actions,
    );

    let salt = BytesN::from_array(&ctx.env, &[77u8; 32]);
    let wrong_salt = BytesN::from_array(&ctx.env, &[88u8; 32]);
    let commit = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt);

    // 1. Commit in Draft phase -> WrongPhase
    assert_eq!(
        ctx.client.try_commit_vote(&voter, &prop_id, &commit),
        Err(Ok(ContractError::WrongPhase))
    );

    ctx.client.advance_phase(&proposer, &prop_id); // -> Discussion

    // 2. Commit in Discussion phase -> WrongPhase
    assert_eq!(
        ctx.client.try_commit_vote(&voter, &prop_id, &commit),
        Err(Ok(ContractError::WrongPhase))
    );

    advance_time(&ctx.env, 3 * 86_400 + 1);
    ctx.client.advance_phase(&proposer, &prop_id); // -> Voting

    // 3. Reveal without commit -> NoCommitFound
    assert_eq!(
        ctx.client
            .try_reveal_vote(&voter, &prop_id, &VoteChoice::For, &salt),
        Err(Ok(ContractError::NoCommitFound))
    );

    // 4. Valid Commit
    assert!(ctx.client.try_commit_vote(&voter, &prop_id, &commit).is_ok());
    assert!(ctx.client.has_committed(&prop_id, &voter));

    // 5. Double commit -> AlreadyCommitted
    assert_eq!(
        ctx.client.try_commit_vote(&voter, &prop_id, &commit),
        Err(Ok(ContractError::AlreadyCommitted))
    );

    // 6. Reveal with wrong salt -> CommitmentMismatch
    assert_eq!(
        ctx.client
            .try_reveal_vote(&voter, &prop_id, &VoteChoice::For, &wrong_salt),
        Err(Ok(ContractError::CommitmentMismatch))
    );

    // 7. Reveal with wrong choice -> CommitmentMismatch
    assert_eq!(
        ctx.client
            .try_reveal_vote(&voter, &prop_id, &VoteChoice::Against, &salt),
        Err(Ok(ContractError::CommitmentMismatch))
    );

    // 8. Valid Reveal
    let power = ctx
        .client
        .reveal_vote(&voter, &prop_id, &VoteChoice::For, &salt);
    assert_eq!(power, 100);
    assert!(ctx.client.has_voted(&prop_id, &voter));

    // 9. Double reveal -> AlreadyRevealed
    assert_eq!(
        ctx.client
            .try_reveal_vote(&voter, &prop_id, &VoteChoice::For, &salt),
        Err(Ok(ContractError::AlreadyRevealed))
    );

    // 10. Commit after already committed/revealed -> AlreadyCommitted
    assert_eq!(
        ctx.client.try_commit_vote(&voter, &prop_id, &commit),
        Err(Ok(ContractError::AlreadyCommitted))
    );
}

// ── Test 6: Delegated Commit & Reveal Lifecycle ──────────────────────────────

#[test]
fn test_delegated_commit_and_reveal_lifecycle() {
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let delegator = Address::generate(&ctx.env);
    let delegate = Address::generate(&ctx.env);
    let rogue = Address::generate(&ctx.env);

    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &delegator, 10_000);
    set_mock_age(&ctx.env, &ctx.contract_id, &delegator, 365 * 86_400); // 2x loyalty

    // Delegator delegates voting power to delegate
    ctx.client.delegate(&delegator, &delegate);
    assert_eq!(ctx.client.get_delegation_count(&delegate), 1);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Delegation Test"),
        &actions,
    );

    ctx.client.advance_phase(&proposer, &prop_id);
    advance_time(&ctx.env, 3 * 86_400 + 1);
    ctx.client.advance_phase(&proposer, &prop_id); // Voting

    let salt = BytesN::from_array(&ctx.env, &[55u8; 32]);
    let commit = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt);

    // Delegator direct commit fails with HasDelegated
    assert_eq!(
        ctx.client.try_commit_vote(&delegator, &prop_id, &commit),
        Err(Ok(ContractError::HasDelegated))
    );

    // Rogue caller trying to commit as delegate fails with NotADelegate
    assert_eq!(
        ctx.client
            .try_commit_vote_as_delegate(&rogue, &delegator, &prop_id, &commit),
        Err(Ok(ContractError::NotADelegate))
    );

    // Legitimate delegate commits on delegator's behalf
    assert!(ctx
        .client
        .try_commit_vote_as_delegate(&delegate, &delegator, &prop_id, &commit)
        .is_ok());

    // Delegator reveals vote directly (delegator's key is authorized)
    let power = ctx
        .client
        .reveal_vote(&delegator, &prop_id, &VoteChoice::For, &salt);
    // sqrt(10,000) = 100, 365 days loyalty = 2x -> 200 power!
    assert_eq!(power, 200);

    // Delegator revokes delegation
    ctx.client.revoke_delegation(&delegator);
    assert_eq!(ctx.client.get_delegation_count(&delegate), 0);
}

// ── Test 7: Quadratic Voting Weight Math & Loyalty Multipliers ────────────────

#[test]
fn test_quadratic_voting_weight_verification_and_loyalty_math() {
    let ctx = setup_test_context(10_000);

    // 1. Verify pure isqrt integer arithmetic floor property
    assert_eq!(isqrt(0), 0);
    assert_eq!(isqrt(100), 10);
    assert_eq!(isqrt(10_000), 100);
    assert_eq!(isqrt(40_000), 200); // 4x tokens = 2x raw voting weight
    assert_eq!(isqrt(90_000), 300); // 9x tokens = 3x raw voting weight
    assert_eq!(isqrt(1_000_000), 1_000); // 100x tokens = 10x raw voting weight
    assert_eq!(isqrt(10_005), 100); // floor property
    assert_eq!(isqrt(39_999), 199);

    // 2. Verify loyalty multiplier values
    let day = 86_400u64;
    assert_eq!(loyalty_multiplier_scaled(0), SCALE); // 1.0x (1,000)
    assert_eq!(loyalty_multiplier_scaled(182 * day), 1_498); // ~1.5x at 6 months
    assert_eq!(loyalty_multiplier_scaled(365 * day), 2 * SCALE); // 2.0x (2,000)
    assert_eq!(loyalty_multiplier_scaled(730 * day), 2 * SCALE); // Capped at 2.0x

    // 3. Combined vote power computation
    // 10,000 tokens, 0 age -> 100 * 1,000 / 1,000 = 100
    assert_eq!(compute_vote_power(10_000, 0), 100);
    // 10,000 tokens, 1 year age -> 100 * 2,000 / 1,000 = 200
    assert_eq!(compute_vote_power(10_000, 365 * day), 200);
    // 40,000 tokens, 1 year age -> 200 * 2,000 / 1,000 = 400
    assert_eq!(compute_vote_power(40_000, 365 * day), 400);

    // 4. On-chain get_vote_power view matches exact formula
    let voter_fresh = Address::generate(&ctx.env);
    let voter_mature = Address::generate(&ctx.env);

    set_mock_stake(&ctx.env, &ctx.contract_id, &voter_fresh, 40_000);
    set_mock_age(&ctx.env, &ctx.contract_id, &voter_fresh, 0);

    set_mock_stake(&ctx.env, &ctx.contract_id, &voter_mature, 10_000);
    set_mock_age(&ctx.env, &ctx.contract_id, &voter_mature, 365 * day);

    // Fresh voter with 4x stake has 200 power
    assert_eq!(ctx.client.get_vote_power(&voter_fresh), 200);
    // Mature voter with 1x stake but 1 year loyalty also has 200 power!
    assert_eq!(ctx.client.get_vote_power(&voter_mature), 200);
}

// ── Test 8: Veto Threshold Met Blocks Execution ──────────────────────────────

#[test]
fn test_veto_threshold_reached_blocks_execution() {
    // Total supply = 1,000. ParameterChange veto threshold = 33% (330 power needed).
    let ctx = setup_test_context(1_000);

    let proposer = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);
    let vetoer = Address::generate(&ctx.env);

    // Supporter: 40,000 stake (power 200) -> meets 15% quorum (150) and passes majority
    // Vetoer: 122_500 stake (power 350) -> meets 33% veto threshold (330)
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 5_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &supporter, 40_000);
    set_mock_stake(&ctx.env, &ctx.contract_id, &vetoer, 122_500);

    let actions = sample_actions(&ctx.env);
    let prop_id = ctx.client.create_proposal(
        &proposer,
        &ProposalType::ParameterChange,
        &String::from_str(&ctx.env, "Veto Target Proposal"),
        &actions,
    );

    ctx.client.advance_phase(&proposer, &prop_id);
    advance_time(&ctx.env, 3 * 86_400 + 1);
    ctx.client.advance_phase(&proposer, &prop_id); // Voting

    // Supporter commits and reveals For
    let salt_sup = BytesN::from_array(&ctx.env, &[10u8; 32]);
    let commit_sup = compute_commitment(&ctx.env, prop_id, &VoteChoice::For, &salt_sup);
    ctx.client.commit_vote(&supporter, &prop_id, &commit_sup);
    ctx.client
        .reveal_vote(&supporter, &prop_id, &VoteChoice::For, &salt_sup);

    // Vetoer commits Veto during voting
    let salt_veto = BytesN::from_array(&ctx.env, &[20u8; 32]);
    let commit_veto = compute_commitment(&ctx.env, prop_id, &VoteChoice::Veto, &salt_veto);
    ctx.client.commit_vote(&vetoer, &prop_id, &commit_veto);

    // Advance to Timelock
    advance_time(&ctx.env, 5 * 86_400 + 1);
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Timelock);

    // During Timelock, Vetoer reveals Veto choice (contract explicitly permits Veto reveals in Timelock)
    let veto_power = ctx
        .client
        .reveal_vote(&vetoer, &prop_id, &VoteChoice::Veto, &salt_veto);
    assert_eq!(veto_power, 350);

    let prop = ctx.client.get_proposal(&prop_id).unwrap();
    assert_eq!(prop.votes_veto, 350);

    // Advance phase from Timelock: veto threshold (330) is met -> transitions to Rejected!
    let phase = ctx.client.advance_phase(&proposer, &prop_id);
    assert_eq!(phase, ProposalPhase::Rejected);

    // Execution is blocked
    let caller = Address::generate(&ctx.env);
    assert_eq!(
        ctx.client.try_execute_proposal(&caller, &prop_id),
        Err(Ok(ContractError::WrongPhase))
    );
}

// ── Test 9: All Proposal Types Quorum and Timelock Verification ──────────────

#[test]
fn test_all_proposal_types_quorum_and_timelock_requirements() {
    let ctx = setup_test_context(10_000);

    let proposer = Address::generate(&ctx.env);
    set_mock_stake(&ctx.env, &ctx.contract_id, &proposer, 50_000);

    let types_and_timelocks = [
        (ProposalType::TreasurySpend, 2 * 86_400u64, 1_000u32), // 2 days, 10% quorum
        (ProposalType::ParameterChange, 2 * 86_400u64, 1_500u32), // 2 days, 15% quorum
        (ProposalType::PolicyModification, 2 * 86_400u64, 2_000u32), // 2 days, 20% quorum
        (ProposalType::ContractUpgrade, 7 * 86_400u64, 3_000u32), // 7 days, 30% quorum
        (ProposalType::EmergencyAction, 21_600u64, 500u32),   // 6 hours, 5% quorum
    ];

    for (prop_type, expected_timelock, expected_quorum) in types_and_timelocks {
        assert_eq!(governor::proposal::quorum_bps(&prop_type), expected_quorum);

        let actions = sample_actions(&ctx.env);
        let prop_id = ctx.client.create_proposal(
            &proposer,
            &prop_type,
            &String::from_str(&ctx.env, "Type Check"),
            &actions,
        );

        let prop = ctx.client.get_proposal(&prop_id).unwrap();
        assert_eq!(prop.proposal_type, prop_type);
        assert_eq!(prop.timelock_ends - prop.voting_ends, expected_timelock);
    }
}
