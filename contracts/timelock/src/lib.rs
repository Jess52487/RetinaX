#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TimelockError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    TxAlreadyQueued = 4,
    TxNotFound = 5,
    TooEarly = 6,
    DelayTooShort = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tx {
    pub id: BytesN<32>,
    pub execute_after: u64,
    pub priority: bool,
}

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const QUEUE_KEY: Symbol = symbol_short!("QUEUE");
const MIN_DELAY_KEY: Symbol = symbol_short!("MIN_DLY");

#[contract]
pub struct TimelockContract;

#[contractimpl]
impl TimelockContract {
    pub fn initialize(env: Env, admin: Address, min_delay: u64) -> Result<(), TimelockError> {
        if env.storage().instance().has(&ADMIN_KEY) {
            return Err(TimelockError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN_KEY, &admin);
        env.storage().instance().set(&MIN_DELAY_KEY, &min_delay);
        let queue: Vec<Tx> = Vec::new(&env);
        env.storage().instance().set(&QUEUE_KEY, &queue);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, TimelockError> {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .ok_or(TimelockError::NotInitialized)
    }

    pub fn get_min_delay(env: Env) -> Result<u64, TimelockError> {
        env.storage()
            .instance()
            .get(&MIN_DELAY_KEY)
            .ok_or(TimelockError::NotInitialized)
    }

    pub fn get_queue(env: Env) -> Result<Vec<Tx>, TimelockError> {
        if !env.storage().instance().has(&ADMIN_KEY) {
            return Err(TimelockError::NotInitialized);
        }
        Ok(env
            .storage()
            .instance()
            .get(&QUEUE_KEY)
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_tx(env: Env, id: BytesN<32>) -> Result<Tx, TimelockError> {
        let queue = Self::get_queue(env)?;
        for tx in queue.iter() {
            if tx.id == id {
                return Ok(tx);
            }
        }
        Err(TimelockError::TxNotFound)
    }

    pub fn queue_tx(
        env: Env,
        caller: Address,
        id: BytesN<32>,
        delay: u64,
    ) -> Result<u64, TimelockError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .ok_or(TimelockError::NotInitialized)?;
        if caller != admin {
            return Err(TimelockError::Unauthorized);
        }

        let min_delay: u64 = env.storage().instance().get(&MIN_DELAY_KEY).unwrap_or(0);
        if delay < min_delay {
            return Err(TimelockError::DelayTooShort);
        }

        let mut queue: Vec<Tx> = env
            .storage()
            .instance()
            .get(&QUEUE_KEY)
            .unwrap_or(Vec::new(&env));

        for tx in queue.iter() {
            if tx.id == id {
                return Err(TimelockError::TxAlreadyQueued);
            }
        }

        let now = env.ledger().timestamp();
        let execute_after = now.saturating_add(delay);

        let new_tx = Tx {
            id: id.clone(),
            execute_after,
            priority: false,
        };
        queue.push_back(new_tx);
        env.storage().instance().set(&QUEUE_KEY, &queue);

        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("TX_QUED"), id, caller), execute_after);

        Ok(execute_after)
    }

    pub fn prioritize_tx(env: Env, caller: Address, id: BytesN<32>) -> Result<(), TimelockError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .ok_or(TimelockError::NotInitialized)?;
        if caller != admin {
            return Err(TimelockError::Unauthorized);
        }

        let queue: Vec<Tx> = env
            .storage()
            .instance()
            .get(&QUEUE_KEY)
            .unwrap_or(Vec::new(&env));

        let mut updated_queue: Vec<Tx> = Vec::new(&env);
        let mut found = false;

        for tx in queue.iter() {
            if tx.id == id {
                found = true;
                updated_queue.push_back(Tx {
                    id: tx.id.clone(),
                    execute_after: tx.execute_after,
                    priority: true,
                });
            } else {
                updated_queue.push_back(tx);
            }
        }

        if !found {
            return Err(TimelockError::TxNotFound);
        }

        env.storage().instance().set(&QUEUE_KEY, &updated_queue);

        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("TX_PRIO"), id, caller), ());

        Ok(())
    }

    pub fn execute_tx(env: Env, caller: Address, id: BytesN<32>) -> Result<(), TimelockError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .ok_or(TimelockError::NotInitialized)?;
        if caller != admin {
            return Err(TimelockError::Unauthorized);
        }

        let queue: Vec<Tx> = env
            .storage()
            .instance()
            .get(&QUEUE_KEY)
            .unwrap_or(Vec::new(&env));

        let now = env.ledger().timestamp();
        let mut found = false;
        let mut updated_queue: Vec<Tx> = Vec::new(&env);

        for tx in queue.iter() {
            if tx.id == id {
                found = true;
                if !tx.priority && now < tx.execute_after {
                    return Err(TimelockError::TooEarly);
                }
            } else {
                updated_queue.push_back(tx);
            }
        }

        if !found {
            return Err(TimelockError::TxNotFound);
        }

        env.storage().instance().set(&QUEUE_KEY, &updated_queue);

        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("TX_EXEC"), id, caller), now);

        Ok(())
    }

    pub fn cancel_tx(env: Env, caller: Address, id: BytesN<32>) -> Result<(), TimelockError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .ok_or(TimelockError::NotInitialized)?;
        if caller != admin {
            return Err(TimelockError::Unauthorized);
        }

        let queue: Vec<Tx> = env
            .storage()
            .instance()
            .get(&QUEUE_KEY)
            .unwrap_or(Vec::new(&env));

        let mut found = false;
        let mut updated_queue: Vec<Tx> = Vec::new(&env);

        for tx in queue.iter() {
            if tx.id == id {
                found = true;
            } else {
                updated_queue.push_back(tx);
            }
        }

        if !found {
            return Err(TimelockError::TxNotFound);
        }

        env.storage().instance().set(&QUEUE_KEY, &updated_queue);

        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("TX_CNCL"), id, caller), ());

        Ok(())
    }
}

#[cfg(test)]
mod test;
