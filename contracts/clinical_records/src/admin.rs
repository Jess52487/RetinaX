use crate::errors::ContractError;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub const ADMIN: Symbol = symbol_short!("ADMIN");
pub const INITIALIZED: Symbol = symbol_short!("INIT");
pub const VISION_RECORDS: Symbol = symbol_short!("V_REC");

pub fn initialize(env: &Env, admin: Address, vision_records: Address) -> Result<(), ContractError> {
    if env.storage().instance().has(&INITIALIZED) {
        return Err(ContractError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(&ADMIN, &admin);
    env.storage()
        .instance()
        .set(&VISION_RECORDS, &vision_records);
    env.storage().instance().set(&INITIALIZED, &true);
    Ok(())
}

pub fn get_admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&ADMIN)
        .ok_or(ContractError::NotInitialized)
}

pub fn set_vision_records_contract(
    env: &Env,
    admin: Address,
    vision_records: Address,
) -> Result<(), ContractError> {
    admin.require_auth();
    let stored_admin = get_admin(env)?;
    if admin != stored_admin {
        return Err(ContractError::Unauthorized);
    }
    env.storage()
        .instance()
        .set(&VISION_RECORDS, &vision_records);
    Ok(())
}

pub fn get_vision_records_contract(env: &Env) -> Option<Address> {
    env.storage().instance().get(&VISION_RECORDS)
}

pub fn require_authorized_vision_records(env: &Env, caller: &Address) -> Result<(), ContractError> {
    let allowed = env
        .storage()
        .instance()
        .get(&VISION_RECORDS)
        .ok_or(ContractError::NotInitialized)?;
    if *caller == allowed {
        Ok(())
    } else {
        Err(ContractError::Unauthorized)
    }
}
