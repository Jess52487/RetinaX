use crate::types::AccessControlError;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub const ADMIN: Symbol = symbol_short!("ADMIN");
pub const PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");

/// Initializes the contract administrator.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `admin` - The address designated as the initial administrator. Requires authentication.
///
/// # Errors
/// * `AccessControlError::AlreadyInitialized` - If the contract already has an administrator configured.
pub fn initialize(env: &Env, admin: &Address) -> Result<(), AccessControlError> {
    if env.storage().instance().has(&ADMIN) {
        return Err(AccessControlError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage().instance().set(&ADMIN, admin);
    Ok(())
}

/// Asserts that `caller` is authenticated and matches the registered administrator.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address asserting admin authority; requires authentication.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), AccessControlError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN)
        .ok_or(AccessControlError::Unauthorized)?;

    if caller != &admin {
        return Err(AccessControlError::Unauthorized);
    }
    Ok(())
}

/// Returns the currently active administrator address.
///
/// # Arguments
/// * `env` - The Soroban environment.
///
/// Returns `Some(Address)` if an administrator is set, or `None` if uninitialized.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ADMIN)
}

/// Proposes a new administrator address in a 2-step transfer.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `current_admin` - The active administrator address; requires authentication.
/// * `new_admin` - The nominated administrator address.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `current_admin` is not the active administrator.
pub fn propose_admin(
    env: &Env,
    current_admin: &Address,
    new_admin: &Address,
) -> Result<(), AccessControlError> {
    require_admin(env, current_admin)?;
    env.storage().instance().set(&PENDING_ADMIN, new_admin);
    Ok(())
}

/// Accepts the pending administrator nomination.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `new_admin` - The nominated administrator address; requires authentication.
///
/// # Errors
/// * `AccessControlError::NotFound` - If no administrative transfer has been proposed.
/// * `AccessControlError::Unauthorized` - If `new_admin` does not match the pending nominee.
pub fn accept_admin(env: &Env, new_admin: &Address) -> Result<(), AccessControlError> {
    new_admin.require_auth();
    let pending: Address = env
        .storage()
        .instance()
        .get(&PENDING_ADMIN)
        .ok_or(AccessControlError::NotFound)?;

    if new_admin != &pending {
        return Err(AccessControlError::Unauthorized);
    }

    env.storage().instance().set(&ADMIN, new_admin);
    env.storage().instance().remove(&PENDING_ADMIN);
    Ok(())
}

/// Returns the pending administrator address if one is nominated.
///
/// # Arguments
/// * `env` - The Soroban environment.
///
/// Returns `Some(Address)` if an administrative nomination is pending, or `None` otherwise.
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&PENDING_ADMIN)
}
