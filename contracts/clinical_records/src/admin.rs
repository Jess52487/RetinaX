use crate::errors::ContractError;
use access_control::{AccessControlContractClient, Permission};
use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub const ADMIN: Symbol = symbol_short!("ADMIN");
pub const INITIALIZED: Symbol = symbol_short!("INIT");
pub const VISION_RECORDS: Symbol = symbol_short!("V_REC");
pub const ACCESS_CONTROL: Symbol = symbol_short!("ACC_CTRL");

/// Stores the admin, the `vision_records` contract, and the optional `access_control` contract.
///
/// Requires authorization from `admin`. Marks the contract as initialized.
///
/// # Errors
/// Returns `AlreadyInitialized` if the contract was already initialized.
pub fn initialize(
    env: &Env,
    admin: Address,
    vision_records: Address,
    access_control: Option<Address>,
) -> Result<(), ContractError> {
    if env.storage().instance().has(&INITIALIZED) {
        return Err(ContractError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(&ADMIN, &admin);
    env.storage().instance().set(&VISION_RECORDS, &vision_records);
    if let Some(ac) = access_control {
        env.storage().instance().set(&ACCESS_CONTROL, &ac);
    }
    env.storage().instance().set(&INITIALIZED, &true);
    Ok(())
}

/// Returns `Ok` if the contract has been initialized.
///
/// # Errors
/// Returns `NotInitialized` otherwise.
pub fn require_initialized(env: &Env) -> Result<(), ContractError> {
    if !env.storage().instance().has(&INITIALIZED) {
        Err(ContractError::NotInitialized)
    } else {
        Ok(())
    }
}

/// Returns the stored admin address.
///
/// # Errors
/// Returns `NotInitialized` if no admin has been stored.
pub fn get_admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&ADMIN)
        .ok_or(ContractError::NotInitialized)
}

/// Replaces the `vision_records` contract address.
///
/// Requires authorization from `admin`, which must match the stored admin.
///
/// # Errors
/// Returns `Unauthorized` if `admin` is not the stored admin.
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
    env.storage().instance().set(&VISION_RECORDS, &vision_records);
    Ok(())
}

/// Returns the configured `vision_records` contract address, if any.
pub fn get_vision_records_contract(env: &Env) -> Option<Address> {
    env.storage().instance().get(&VISION_RECORDS)
}

/// Sets the `access_control` contract address used for permission checks.
///
/// Requires authorization from `admin`, which must match the stored admin.
///
/// # Errors
/// Returns `Unauthorized` if `admin` is not the stored admin.
pub fn set_access_control_contract(
    env: &Env,
    admin: Address,
    access_control: Address,
) -> Result<(), ContractError> {
    admin.require_auth();
    let stored_admin = get_admin(env)?;
    if admin != stored_admin {
        return Err(ContractError::Unauthorized);
    }
    env.storage().instance().set(&ACCESS_CONTROL, &access_control);
    Ok(())
}

/// Returns the configured `access_control` contract address, if any.
pub fn get_access_control_contract(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ACCESS_CONTROL)
}

/// Succeeds only when `caller` is the configured `vision_records` contract.
///
/// # Errors
/// Returns `NotInitialized` if no `vision_records` contract is stored, or `Unauthorized` if
/// `caller` is a different address.
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

/// Checks if caller has a specific permission via access_control if configured,
/// or falls back to admin check if access_control is not set.
pub fn check_permission(
    env: &Env,
    caller: &Address,
    permission: Permission,
) -> Result<bool, ContractError> {
    if let Some(ac_addr) = get_access_control_contract(env) {
        let client = AccessControlContractClient::new(env, &ac_addr);
        Ok(client.has_permission(caller, &permission))
    } else {
        // Backwards compatibility when access_control is not configured: admin has all permissions
        let admin = get_admin(env)?;
        Ok(*caller == admin)
    }
}

/// Requires caller to have the specified permission, returning Unauthorized error if not.
pub fn require_permission(
    env: &Env,
    caller: &Address,
    permission: Permission,
) -> Result<(), ContractError> {
    if check_permission(env, caller, permission)? {
        Ok(())
    } else {
        Err(ContractError::Unauthorized)
    }
}
