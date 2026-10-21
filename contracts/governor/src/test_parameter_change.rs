//! Governor parameter-change tests (issue #314).
//! Covers: changing the voting period through governance, changing the
//! quorum basis (total vote supply), access control on both, and that a
//! change never applies retroactively to a proposal that already exists.

use soroban_sdk::{testutils::Address as _, Address};

use crate::{
    proposal::{ProposalPhase, ProposalType},
    test_helpers::{setup, Gov, DAY, VOTING_SECS},
    voting::VoteChoice,
    ContractError,
};

/// With supply 500 a ParameterChange needs 75 votes (15 %); a 40_000-token
/// voter has power 200.
const SMALL_SUPPLY: i128 = 500;
/// With this supply the same 200-power vote misses quorum (needs 300).
const LARGE_SUPPLY: i128 = 2_000;

/// Drive a ParameterChange proposal all the way to Completed.
fn enact(gov: &Gov, proposer: &Address, voter: &Address) -> u64 {
    let id = gov.create(proposer, ProposalType::ParameterChange);
    gov.open_voting(proposer, id);
    gov.vote(voter, id, VoteChoice::For, 1);
    assert_eq!(gov.close_voting(voter, id), ProposalPhase::Timelock);
    gov.advance_to(gov.client.get_proposal(&id).unwrap().timelock_ends);
    assert_eq!(
        gov.client.advance_phase(voter, &id),
        ProposalPhase::Execution
    );
    gov.client.execute_proposal(voter, &id);
    id
}

#[test]
fn test_change_voting_period_via_governance() {
    let gov = setup(SMALL_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);
    assert_eq!(gov.client.get_voting_period(), VOTING_SECS);

    let id = enact(&gov, &proposer, &voter);
    assert_eq!(
        gov.client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );
    // The executed action is applied by the governor acting as itself.
    gov.client.set_voting_period(&gov.id, &(3 * DAY));
    assert_eq!(gov.client.get_voting_period(), 3 * DAY);

    let next = gov.create(&proposer, ProposalType::ParameterChange);
    let p = gov.client.get_proposal(&next).unwrap();
    assert_eq!(p.voting_ends - p.discussion_ends, 3 * DAY);
}

#[test]
fn test_voting_period_cannot_be_changed_outside_governance() {
    let gov = setup(SMALL_SUPPLY);
    let stranger = Address::generate(&gov.env);
    for caller in [&gov.admin, &stranger] {
        assert_eq!(
            gov.client.try_set_voting_period(caller, &DAY),
            Err(Ok(ContractError::Unauthorized))
        );
    }
    assert_eq!(gov.client.get_voting_period(), VOTING_SECS);
}

#[test]
fn test_voting_period_rejects_zero() {
    let gov = setup(SMALL_SUPPLY);
    assert_eq!(
        gov.client.try_set_voting_period(&gov.id, &0),
        Err(Ok(ContractError::InvalidInput))
    );
}

#[test]
fn test_voting_period_change_not_retroactive() {
    let gov = setup(SMALL_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);

    let active = gov.create(&proposer, ProposalType::ParameterChange);
    gov.open_voting(&proposer, active);
    let original_end = gov.client.get_proposal(&active).unwrap().voting_ends;

    // Lengthen the period mid-vote.
    gov.client.set_voting_period(&gov.id, &(10 * DAY));
    assert_eq!(
        gov.client.get_proposal(&active).unwrap().voting_ends,
        original_end
    );

    // The active proposal still closes on its original schedule.
    gov.vote(&voter, active, VoteChoice::For, 1);
    gov.advance_to(original_end);
    assert_eq!(
        gov.client.advance_phase(&voter, &active),
        ProposalPhase::Timelock
    );

    // Proposals created afterwards use the new period.
    let next = gov.create(&proposer, ProposalType::ParameterChange);
    let p = gov.client.get_proposal(&next).unwrap();
    assert_eq!(p.voting_ends - p.discussion_ends, 10 * DAY);
}

#[test]
fn test_quorum_change_applies_to_next_proposal() {
    let gov = setup(SMALL_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);

    let first = gov.create(&proposer, ProposalType::ParameterChange);
    gov.client.set_total_vote_supply(&gov.admin, &LARGE_SUPPLY);
    let second = gov.create(&proposer, ProposalType::ParameterChange);

    assert_eq!(
        gov.client.get_proposal_vote_supply(&first),
        Some(SMALL_SUPPLY)
    );
    assert_eq!(
        gov.client.get_proposal_vote_supply(&second),
        Some(LARGE_SUPPLY)
    );

    for (salt, id) in [first, second].iter().enumerate() {
        gov.open_voting(&proposer, *id);
        gov.vote(&voter, *id, VoteChoice::For, salt as u8);
    }
    // Same 200-power vote: meets the old 75 quorum, misses the new 300.
    assert_eq!(gov.close_voting(&voter, first), ProposalPhase::Timelock);
    assert_eq!(gov.close_voting(&voter, second), ProposalPhase::Expired);
}

#[test]
fn test_raising_quorum_mid_vote_does_not_fail_active_proposal() {
    let gov = setup(SMALL_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);

    let id = gov.create(&proposer, ProposalType::ParameterChange);
    gov.open_voting(&proposer, id);
    gov.vote(&voter, id, VoteChoice::For, 1);

    gov.client.set_total_vote_supply(&gov.admin, &LARGE_SUPPLY);
    assert_eq!(gov.close_voting(&voter, id), ProposalPhase::Timelock);
}

#[test]
fn test_lowering_quorum_mid_vote_does_not_rescue_active_proposal() {
    let gov = setup(LARGE_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);

    let id = gov.create(&proposer, ProposalType::ParameterChange);
    gov.open_voting(&proposer, id);
    gov.vote(&voter, id, VoteChoice::For, 1);

    gov.client.set_total_vote_supply(&gov.admin, &SMALL_SUPPLY);
    assert_eq!(gov.close_voting(&voter, id), ProposalPhase::Expired);
}

#[test]
fn test_lowering_supply_during_timelock_does_not_enable_veto() {
    // Veto threshold is 33 % of the snapshotted supply (2_000 → 660).
    let gov = setup(LARGE_SUPPLY);
    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(1_000_000, 0); // power 1_000, meets 300 quorum
    let vetoer = gov.staker(40_000, 0); // power 200

    let id = gov.create(&proposer, ProposalType::ParameterChange);
    gov.open_voting(&proposer, id);
    gov.vote(&voter, id, VoteChoice::For, 1);
    gov.commit(&vetoer, id, &VoteChoice::Veto, 2);
    assert_eq!(gov.close_voting(&voter, id), ProposalPhase::Timelock);

    // 200 would exceed 33 % of 500, but the proposal keeps its snapshot.
    gov.client.set_total_vote_supply(&gov.admin, &SMALL_SUPPLY);
    gov.reveal(&vetoer, id, &VoteChoice::Veto, 2);
    gov.advance_to(gov.client.get_proposal(&id).unwrap().timelock_ends);
    assert_eq!(
        gov.client.advance_phase(&voter, &id),
        ProposalPhase::Execution
    );
}

#[test]
fn test_vote_supply_change_requires_admin() {
    let gov = setup(SMALL_SUPPLY);
    let stranger = Address::generate(&gov.env);
    assert_eq!(
        gov.client
            .try_set_total_vote_supply(&stranger, &LARGE_SUPPLY),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        gov.client.try_set_total_vote_supply(&gov.admin, &0),
        Err(Ok(ContractError::InvalidInput))
    );
}
