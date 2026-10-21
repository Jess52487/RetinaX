//! Shared fixtures for the governor's lifecycle-driven test modules.

#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Bytes, BytesN, Env, String, Vec,
};

use crate::{
    proposal::{ProposalAction, ProposalPhase, ProposalType},
    voting::VoteChoice,
    GovernorContract, GovernorContractClient,
};

pub const DAY: u64 = 86_400;
/// Default Voting phase length used by `create_proposal`.
pub const VOTING_SECS: u64 = 5 * DAY;

pub struct Gov {
    pub env: Env,
    pub id: Address,
    pub client: GovernorContractClient<'static>,
    pub admin: Address,
}

/// Deploy and initialise a governor with the given total vote supply.
pub fn setup(total_vote_supply: i128) -> Gov {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let id = env.register(GovernorContract, ());
    let client = GovernorContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(
        &admin,
        &Address::generate(&env),
        &Address::generate(&env),
        &total_vote_supply,
    );
    Gov {
        env,
        id,
        client,
        admin,
    }
}

impl Gov {
    /// Generate an address with a mocked stake and stake age.
    pub fn staker(&self, amount: i128, age_secs: u64) -> Address {
        let who = Address::generate(&self.env);
        self.set_stake(&who, amount);
        self.set_age(&who, age_secs);
        who
    }

    pub fn set_stake(&self, who: &Address, amount: i128) {
        self.env.as_contract(&self.id, || {
            let key = (symbol_short!("M_STK"), who.clone());
            self.env.storage().persistent().set(&key, &amount);
        });
    }

    pub fn set_age(&self, who: &Address, age_secs: u64) {
        self.env.as_contract(&self.id, || {
            let key = (symbol_short!("M_AGE"), who.clone());
            self.env.storage().persistent().set(&key, &age_secs);
        });
    }

    pub fn advance(&self, secs: u64) {
        self.env.ledger().with_mut(|l| l.timestamp += secs);
    }

    pub fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    /// Advance the ledger to exactly `ts` (no-op if already past it).
    pub fn advance_to(&self, ts: u64) {
        if ts > self.now() {
            self.advance(ts - self.now());
        }
    }

    pub fn create(&self, proposer: &Address, proposal_type: ProposalType) -> u64 {
        let mut actions = Vec::new(&self.env);
        actions.push_back(ProposalAction {
            target: self.id.clone(),
            function: symbol_short!("GOV_PRM"),
            params_hash: BytesN::from_array(&self.env, &[7u8; 32]),
        });
        self.client.create_proposal(
            proposer,
            &proposal_type,
            &String::from_str(&self.env, "test proposal"),
            &actions,
        )
    }

    /// Move a Draft proposal into the Voting phase.
    pub fn open_voting(&self, proposer: &Address, id: u64) {
        self.client.advance_phase(proposer, &id);
        let p = self.client.get_proposal(&id).unwrap();
        self.advance_to(p.discussion_ends);
        assert_eq!(
            self.client.advance_phase(proposer, &id),
            ProposalPhase::Voting
        );
    }

    /// Close the Voting phase and return the resulting phase.
    pub fn close_voting(&self, caller: &Address, id: u64) -> ProposalPhase {
        let p = self.client.get_proposal(&id).unwrap();
        self.advance_to(p.voting_ends);
        self.client.advance_phase(caller, &id)
    }

    pub fn commit(&self, voter: &Address, id: u64, choice: &VoteChoice, salt: u8) {
        let c = commitment(&self.env, id, choice, salt);
        self.client.commit_vote(voter, &id, &c);
    }

    pub fn reveal(&self, voter: &Address, id: u64, choice: &VoteChoice, salt: u8) -> i128 {
        let s = BytesN::from_array(&self.env, &[salt; 32]);
        self.client.reveal_vote(voter, &id, choice, &s)
    }

    /// Commit and immediately reveal a direct vote; returns the vote power.
    pub fn vote(&self, voter: &Address, id: u64, choice: VoteChoice, salt: u8) -> i128 {
        self.commit(voter, id, &choice, salt);
        self.reveal(voter, id, &choice, salt)
    }
}

/// Mirror of `GovernorContract::hash_commitment`.
pub fn commitment(env: &Env, proposal_id: u64, choice: &VoteChoice, salt: u8) -> BytesN<32> {
    let mut data = Bytes::new(env);
    for b in proposal_id.to_le_bytes().iter() {
        data.push_back(*b);
    }
    data.push_back(match choice {
        VoteChoice::For => 0,
        VoteChoice::Against => 1,
        VoteChoice::Veto => 2,
    });
    for _ in 0..32 {
        data.push_back(salt);
    }
    env.crypto().sha256(&data).into()
}
