//! Governor ↔ Timelock integration tests (issue #313).
//!
//! The governor is the timelock's admin: once a proposal enters its
//! Timelock phase the governor queues a matching transaction whose delay
//! equals the remaining governor timelock, and both contracts must agree on
//! when execution becomes possible.
//! Covers: end-to-end queue → delay → execute, veto during the timelock,
//! priority bypass, and multiple pending transactions.

use soroban_sdk::{testutils::Address as _, Address, BytesN};
use timelock::{TimelockContract, TimelockContractClient, TimelockError};

use crate::{
    execution::{TIMELOCK_EMERGENCY, TIMELOCK_STANDARD, TIMELOCK_UPGRADE},
    proposal::{ProposalPhase, ProposalType},
    test_helpers::{setup, Gov},
    voting::VoteChoice,
    ContractError,
};

/// Small supply so a single 40_000-token voter (power 200) meets every quorum
/// and the 33 % veto threshold (165).
const SUPPLY: i128 = 500;

struct Fixture {
    gov: Gov,
    tl: TimelockContractClient<'static>,
    proposer: Address,
    voter: Address,
}

fn fixture() -> Fixture {
    let gov = setup(SUPPLY);
    let tl_id = gov.env.register(TimelockContract, ());
    let tl = TimelockContractClient::new(&gov.env, &tl_id);
    // Governor owns the timelock; the shortest governor delay is the floor.
    tl.initialize(&gov.id, &TIMELOCK_EMERGENCY);

    let proposer = gov.staker(1_000, 0);
    let voter = gov.staker(40_000, 0);
    Fixture {
        gov,
        tl,
        proposer,
        voter,
    }
}

fn tx_id(f: &Fixture, proposal_id: u64) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&proposal_id.to_be_bytes());
    BytesN::from_array(&f.gov.env, &raw)
}

impl Fixture {
    /// Create a proposal and vote it into the governor's Timelock phase.
    fn passed(&self, kind: ProposalType, salt: u8) -> u64 {
        let id = self.gov.create(&self.proposer, kind);
        self.gov.open_voting(&self.proposer, id);
        self.gov.vote(&self.voter, id, VoteChoice::For, salt);
        assert_eq!(
            self.gov.close_voting(&self.voter, id),
            ProposalPhase::Timelock
        );
        id
    }

    /// Governor queues the proposal's transaction for its remaining timelock.
    fn queue(&self, proposal_id: u64) -> u64 {
        let p = self.gov.client.get_proposal(&proposal_id).unwrap();
        let delay = p.timelock_ends - self.gov.now();
        self.tl
            .queue_tx(&self.gov.id, &tx_id(self, proposal_id), &delay)
    }
}

#[test]
fn test_end_to_end_proposal_queue_delay_execute() {
    let f = fixture();
    let id = f.passed(ProposalType::ParameterChange, 1);
    let execute_after = f.queue(id);

    let p = f.gov.client.get_proposal(&id).unwrap();
    assert_eq!(execute_after, p.timelock_ends);
    assert_eq!(execute_after - p.voting_ends, TIMELOCK_STANDARD);

    // Still locked in both contracts one second before expiry.
    f.gov.advance_to(execute_after - 1);
    assert_eq!(
        f.tl.try_execute_tx(&f.gov.id, &tx_id(&f, id)),
        Err(Ok(TimelockError::TooEarly))
    );
    assert_eq!(
        f.gov.client.try_advance_phase(&f.voter, &id),
        Err(Ok(ContractError::TimelockNotExpired))
    );

    // Both unlock together.
    f.gov.advance(1);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, id));
    assert_eq!(
        f.gov.client.advance_phase(&f.voter, &id),
        ProposalPhase::Execution
    );
    f.gov.client.execute_proposal(&f.voter, &id);

    assert_eq!(
        f.gov.client.get_proposal(&id).unwrap().phase,
        ProposalPhase::Completed
    );
    assert_eq!(f.tl.get_queue().len(), 0);
}

#[test]
fn test_only_governor_can_queue_on_timelock() {
    let f = fixture();
    let id = f.passed(ProposalType::ParameterChange, 1);
    let stranger = Address::generate(&f.gov.env);
    assert_eq!(
        f.tl.try_queue_tx(&stranger, &tx_id(&f, id), &TIMELOCK_STANDARD),
        Err(Ok(TimelockError::Unauthorized))
    );
}

#[test]
fn test_queue_rejects_delay_below_minimum() {
    let f = fixture();
    let id = f.passed(ProposalType::ParameterChange, 1);
    assert_eq!(
        f.tl.try_queue_tx(&f.gov.id, &tx_id(&f, id), &(TIMELOCK_EMERGENCY - 1)),
        Err(Ok(TimelockError::DelayTooShort))
    );
}

#[test]
fn test_veto_during_timelock_rejects_and_cancels() {
    let f = fixture();
    let vetoer = f.gov.staker(40_000, 0); // power 200 ≥ 165 veto threshold
    let id = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    f.gov.open_voting(&f.proposer, id);
    f.gov.vote(&f.voter, id, VoteChoice::For, 1);
    f.gov.commit(&vetoer, id, &VoteChoice::Veto, 2);
    assert_eq!(f.gov.close_voting(&f.voter, id), ProposalPhase::Timelock);
    f.queue(id);

    // Veto revealed while the timelock is running.
    f.gov.advance(TIMELOCK_STANDARD / 2);
    f.gov.reveal(&vetoer, id, &VoteChoice::Veto, 2);
    assert_eq!(
        f.gov.client.advance_phase(&f.voter, &id),
        ProposalPhase::Rejected
    );
    f.tl.cancel_tx(&f.gov.id, &tx_id(&f, id));

    // Nothing can be executed afterwards, even once the delay has passed.
    f.gov.advance(TIMELOCK_STANDARD);
    assert_eq!(
        f.tl.try_execute_tx(&f.gov.id, &tx_id(&f, id)),
        Err(Ok(TimelockError::TxNotFound))
    );
    assert_eq!(
        f.gov.client.try_execute_proposal(&f.voter, &id),
        Err(Ok(ContractError::WrongPhase))
    );
}

#[test]
fn test_veto_below_threshold_does_not_block_execution() {
    let f = fixture();
    let small = f.gov.staker(10_000, 0); // power 100 < 165
    let id = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    f.gov.open_voting(&f.proposer, id);
    f.gov.vote(&f.voter, id, VoteChoice::For, 1);
    f.gov.commit(&small, id, &VoteChoice::Veto, 2);
    f.gov.close_voting(&f.voter, id);
    let execute_after = f.queue(id);

    f.gov.reveal(&small, id, &VoteChoice::Veto, 2);
    f.gov.advance_to(execute_after);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, id));
    assert_eq!(
        f.gov.client.advance_phase(&f.voter, &id),
        ProposalPhase::Execution
    );
}

#[test]
fn test_only_veto_reveals_accepted_during_timelock() {
    let f = fixture();
    let late = f.gov.staker(10_000, 0);
    let id = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    f.gov.open_voting(&f.proposer, id);
    f.gov.vote(&f.voter, id, VoteChoice::For, 1);
    f.gov.commit(&late, id, &VoteChoice::For, 2);
    f.gov.close_voting(&f.voter, id);

    let salt = BytesN::from_array(&f.gov.env, &[2u8; 32]);
    assert_eq!(
        f.gov
            .client
            .try_reveal_vote(&late, &id, &VoteChoice::For, &salt),
        Err(Ok(ContractError::WrongPhase))
    );
}

#[test]
fn test_priority_transaction_bypasses_delay() {
    let f = fixture();
    let id = f.passed(ProposalType::EmergencyAction, 1);
    let execute_after = f.queue(id);
    let p = f.gov.client.get_proposal(&id).unwrap();
    assert_eq!(execute_after - p.voting_ends, TIMELOCK_EMERGENCY);

    f.tl.prioritize_tx(&f.gov.id, &tx_id(&f, id));
    assert!(f.tl.get_tx(&tx_id(&f, id)).priority);

    // Executes immediately, well before execute_after.
    assert!(f.gov.now() < execute_after);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, id));
    assert_eq!(f.tl.get_queue().len(), 0);
}

#[test]
fn test_only_governor_can_prioritize() {
    let f = fixture();
    let id = f.passed(ProposalType::EmergencyAction, 1);
    f.queue(id);
    let stranger = Address::generate(&f.gov.env);
    assert_eq!(
        f.tl.try_prioritize_tx(&stranger, &tx_id(&f, id)),
        Err(Ok(TimelockError::Unauthorized))
    );
    assert!(!f.tl.get_tx(&tx_id(&f, id)).priority);
}

#[test]
fn test_multiple_pending_transactions_mature_independently() {
    let f = fixture();
    let emergency = f.gov.create(&f.proposer, ProposalType::EmergencyAction);
    let standard = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    let upgrade = f.gov.create(&f.proposer, ProposalType::ContractUpgrade);
    for (salt, id) in [emergency, standard, upgrade].iter().enumerate() {
        f.gov.open_voting(&f.proposer, *id);
        f.gov.vote(&f.voter, *id, VoteChoice::For, salt as u8);
    }
    for id in [emergency, standard, upgrade] {
        assert_eq!(f.gov.close_voting(&f.voter, id), ProposalPhase::Timelock);
    }
    let start = f.gov.now();
    for id in [emergency, standard, upgrade] {
        f.queue(id);
    }
    assert_eq!(f.tl.get_queue().len(), 3);
    assert_eq!(
        f.tl.try_queue_tx(&f.gov.id, &tx_id(&f, standard), &TIMELOCK_STANDARD),
        Err(Ok(TimelockError::TxAlreadyQueued))
    );

    // After 6 h only the emergency transaction is executable.
    f.gov.advance_to(start + TIMELOCK_EMERGENCY);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, emergency));
    assert_eq!(
        f.tl.try_execute_tx(&f.gov.id, &tx_id(&f, standard)),
        Err(Ok(TimelockError::TooEarly))
    );
    assert_eq!(f.tl.get_queue().len(), 2);

    // After 2 days the standard one matures; the upgrade is still locked.
    f.gov.advance_to(start + TIMELOCK_STANDARD);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, standard));
    assert_eq!(
        f.tl.try_execute_tx(&f.gov.id, &tx_id(&f, upgrade)),
        Err(Ok(TimelockError::TooEarly))
    );
    assert_eq!(
        f.gov.client.try_advance_phase(&f.voter, &upgrade),
        Err(Ok(ContractError::TimelockNotExpired))
    );

    // After 7 days everything has executed.
    f.gov.advance_to(start + TIMELOCK_UPGRADE);
    f.tl.execute_tx(&f.gov.id, &tx_id(&f, upgrade));
    assert_eq!(f.tl.get_queue().len(), 0);
    for id in [emergency, standard, upgrade] {
        assert_eq!(
            f.gov.client.advance_phase(&f.voter, &id),
            ProposalPhase::Execution
        );
    }
}

#[test]
fn test_cancelling_one_pending_tx_leaves_others_queued() {
    let f = fixture();
    let a = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    let b = f.gov.create(&f.proposer, ProposalType::ParameterChange);
    for (salt, id) in [a, b].iter().enumerate() {
        f.gov.open_voting(&f.proposer, *id);
        f.gov.vote(&f.voter, *id, VoteChoice::For, salt as u8);
    }
    for id in [a, b] {
        assert_eq!(f.gov.close_voting(&f.voter, id), ProposalPhase::Timelock);
    }
    f.queue(a);
    f.queue(b);

    f.tl.cancel_tx(&f.gov.id, &tx_id(&f, a));
    assert_eq!(f.tl.get_queue().len(), 1);
    assert_eq!(f.tl.get_tx(&tx_id(&f, b)).id, tx_id(&f, b));
}
