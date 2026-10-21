//! Tests for how delegated votes are weighted in the governor (issue #312).
//! Covers: delegating, revoking, chained delegation (A→B→C), self-delegation,
//! delegating mid-vote, and double-counting protection.

use soroban_sdk::BytesN;

use crate::{
    test_helpers::{commitment, setup, Gov},
    voting::{compute_vote_power, VoteChoice},
    ContractError,
};

/// Open a ParameterChange proposal in the Voting phase.
fn voting_proposal(gov: &Gov) -> u64 {
    let proposer = gov.staker(1_000, 0);
    let id = gov.create(&proposer, crate::proposal::ProposalType::ParameterChange);
    gov.open_voting(&proposer, id);
    id
}

/// Delegate commits on behalf of `voter`, then the vote is revealed.
fn vote_as_delegate(
    gov: &Gov,
    delegate: &soroban_sdk::Address,
    voter: &soroban_sdk::Address,
    id: u64,
    salt: u8,
) -> i128 {
    let c = commitment(&gov.env, id, &VoteChoice::For, salt);
    gov.client.commit_vote_as_delegate(delegate, voter, &id, &c);
    gov.reveal(voter, id, &VoteChoice::For, salt)
}

#[test]
fn test_delegation_adds_delegator_weight_to_delegate_tally() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0); // power 100
    let b = gov.staker(40_000, 0); // power 200
    let d = gov.staker(90_000, 0); // power 300

    gov.client.delegate(&a, &d);
    gov.client.delegate(&b, &d);
    assert_eq!(gov.client.get_delegation_count(&d), 2);

    let id = voting_proposal(&gov);
    let own = gov.vote(&d, id, VoteChoice::For, 1);
    let via_a = vote_as_delegate(&gov, &d, &a, id, 2);
    let via_b = vote_as_delegate(&gov, &d, &b, id, 3);

    assert_eq!((own, via_a, via_b), (300, 100, 200));
    let p = gov.client.get_proposal(&id).unwrap();
    assert_eq!(p.votes_for, 600);
    assert_eq!(p.reveal_count, 3);
}

#[test]
fn test_delegated_weight_uses_delegators_own_stake_age() {
    let gov = setup(1_000_000_000);
    let veteran = gov.staker(10_000, 365 * 86_400); // 100 × 2
    let d = gov.staker(10_000, 0);
    gov.client.delegate(&veteran, &d);

    let id = voting_proposal(&gov);
    let power = vote_as_delegate(&gov, &d, &veteran, id, 1);

    assert_eq!(power, compute_vote_power(10_000, 365 * 86_400));
    assert_eq!(power, 200);
}

#[test]
fn test_revoke_removes_weight_from_delegate() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let b = gov.staker(40_000, 0);
    let d = gov.staker(90_000, 0);
    gov.client.delegate(&a, &d);
    gov.client.delegate(&b, &d);

    gov.client.revoke_delegation(&a);
    assert_eq!(gov.client.get_delegation_count(&d), 1);
    assert!(gov.client.get_delegation(&a).is_none());

    let id = voting_proposal(&gov);
    let c = commitment(&gov.env, id, &VoteChoice::For, 9);
    assert_eq!(
        gov.client.try_commit_vote_as_delegate(&d, &a, &id, &c),
        Err(Ok(ContractError::NotADelegate))
    );

    // `a` votes directly again; `d` still carries `b`.
    gov.vote(&a, id, VoteChoice::Against, 1);
    gov.vote(&d, id, VoteChoice::For, 2);
    vote_as_delegate(&gov, &d, &b, id, 3);

    let p = gov.client.get_proposal(&id).unwrap();
    assert_eq!(p.votes_against, 100);
    assert_eq!(p.votes_for, 300 + 200);
}

#[test]
fn test_redelegation_moves_weight_between_delegates() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let d1 = gov.staker(1, 0);
    let d2 = gov.staker(1, 0);

    gov.client.delegate(&a, &d1);
    gov.client.delegate(&a, &d2);

    assert_eq!(gov.client.get_delegation_count(&d1), 0);
    assert_eq!(gov.client.get_delegation_count(&d2), 1);
    assert_eq!(gov.client.get_delegation(&a).unwrap().delegate, d2);
}

#[test]
fn test_chain_delegation_is_not_transitive() {
    // A → B → C: C only represents B; A's weight stays with B.
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0); // 100
    let b = gov.staker(40_000, 0); // 200
    let c = gov.staker(90_000, 0); // 300
    gov.client.delegate(&a, &b);
    gov.client.delegate(&b, &c);

    assert_eq!(gov.client.get_delegation_count(&b), 1);
    assert_eq!(gov.client.get_delegation_count(&c), 1);

    let id = voting_proposal(&gov);
    let cm = commitment(&gov.env, id, &VoteChoice::For, 1);
    assert_eq!(
        gov.client.try_commit_vote_as_delegate(&c, &a, &id, &cm),
        Err(Ok(ContractError::NotADelegate))
    );
    // B has delegated, so cannot vote for itself directly.
    assert_eq!(
        gov.client.try_commit_vote(&b, &id, &cm),
        Err(Ok(ContractError::HasDelegated))
    );

    vote_as_delegate(&gov, &b, &a, id, 2);
    vote_as_delegate(&gov, &c, &b, id, 3);
    gov.vote(&c, id, VoteChoice::For, 4);

    // Every holder counted exactly once with their own stake.
    let p = gov.client.get_proposal(&id).unwrap();
    assert_eq!(p.votes_for, 100 + 200 + 300);
    assert_eq!(p.reveal_count, 3);
}

#[test]
fn test_self_delegation_rejected_and_weight_unchanged() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);

    assert_eq!(
        gov.client.try_delegate(&a, &a),
        Err(Ok(ContractError::SelfDelegation))
    );
    assert!(gov.client.get_delegation(&a).is_none());
    assert_eq!(gov.client.get_delegation_count(&a), 0);

    let id = voting_proposal(&gov);
    assert_eq!(gov.vote(&a, id, VoteChoice::For, 1), 100);
}

#[test]
fn test_delegating_during_active_vote_hands_over_uncast_vote() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let d = gov.staker(90_000, 0);
    let id = voting_proposal(&gov);

    gov.client.delegate(&a, &d);
    let c = commitment(&gov.env, id, &VoteChoice::For, 1);
    assert_eq!(
        gov.client.try_commit_vote(&a, &id, &c),
        Err(Ok(ContractError::HasDelegated))
    );
    assert_eq!(vote_as_delegate(&gov, &d, &a, id, 1), 100);
}

#[test]
fn test_delegating_after_committing_does_not_double_count() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let d = gov.staker(90_000, 0);
    let id = voting_proposal(&gov);

    gov.commit(&a, id, &VoteChoice::Against, 1);
    gov.client.delegate(&a, &d);

    let c = commitment(&gov.env, id, &VoteChoice::For, 2);
    assert_eq!(
        gov.client.try_commit_vote_as_delegate(&d, &a, &id, &c),
        Err(Ok(ContractError::AlreadyCommitted))
    );

    gov.reveal(&a, id, &VoteChoice::Against, 1);
    let p = gov.client.get_proposal(&id).unwrap();
    assert_eq!((p.votes_for, p.votes_against), (0, 100));
}

#[test]
fn test_revoking_after_delegate_committed_does_not_double_count() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let d = gov.staker(90_000, 0);
    gov.client.delegate(&a, &d);
    let id = voting_proposal(&gov);

    let c = commitment(&gov.env, id, &VoteChoice::For, 1);
    gov.client.commit_vote_as_delegate(&d, &a, &id, &c);
    gov.client.revoke_delegation(&a);

    let c2 = commitment(&gov.env, id, &VoteChoice::Against, 2);
    assert_eq!(
        gov.client.try_commit_vote(&a, &id, &c2),
        Err(Ok(ContractError::AlreadyCommitted))
    );

    gov.reveal(&a, id, &VoteChoice::For, 1);
    let salt = BytesN::from_array(&gov.env, &[1u8; 32]);
    assert_eq!(
        gov.client.try_reveal_vote(&a, &id, &VoteChoice::For, &salt),
        Err(Ok(ContractError::AlreadyRevealed))
    );
    assert_eq!(gov.client.get_proposal(&id).unwrap().votes_for, 100);
}

#[test]
fn test_delegate_cannot_commit_twice_for_same_delegator() {
    let gov = setup(1_000_000_000);
    let a = gov.staker(10_000, 0);
    let d = gov.staker(90_000, 0);
    gov.client.delegate(&a, &d);
    let id = voting_proposal(&gov);

    let c = commitment(&gov.env, id, &VoteChoice::For, 1);
    gov.client.commit_vote_as_delegate(&d, &a, &id, &c);
    assert_eq!(
        gov.client.try_commit_vote_as_delegate(&d, &a, &id, &c),
        Err(Ok(ContractError::AlreadyCommitted))
    );
    assert_eq!(gov.client.get_proposal(&id).unwrap().commit_count, 1);
}
