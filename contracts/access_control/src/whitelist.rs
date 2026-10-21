use crate::admin::require_admin;
use crate::types::AccessControlError;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

pub const WL_EN: Symbol = symbol_short!("WL_EN");
pub const WL: Symbol = symbol_short!("WL");
pub const WL_LIST: Symbol = symbol_short!("WL_LIST");

/// Event emitted when whitelist status is enabled/disabled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistEnabledEvent {
    pub caller: Address,
    pub enabled: bool,
    pub timestamp: u64,
}

/// Event emitted when an address is added to the whitelist.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistAddedEvent {
    pub caller: Address,
    pub user: Address,
    pub timestamp: u64,
}

/// Event emitted when an address is removed from the whitelist.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistRemovedEvent {
    pub caller: Address,
    pub user: Address,
    pub timestamp: u64,
}

/// Sets whether whitelist enforcement is active.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the active administrator.
/// * `enabled` - `true` to enforce whitelist checks, `false` to permit all addresses.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn set_whitelist_enabled(
    env: &Env,
    caller: &Address,
    enabled: bool,
) -> Result<(), AccessControlError> {
    require_admin(env, caller)?;
    env.storage().instance().set(&WL_EN, &enabled);
    env.events().publish(
        (symbol_short!("WL_SET"), caller.clone()),
        WhitelistEnabledEvent {
            caller: caller.clone(),
            enabled,
            timestamp: env.ledger().timestamp(),
        },
    );
    Ok(())
}

/// Returns whether whitelist enforcement is currently enabled.
///
/// # Arguments
/// * `env` - The Soroban environment.
pub fn is_whitelist_enabled(env: &Env) -> bool {
    env.storage().instance().get(&WL_EN).unwrap_or(false)
}

/// Adds an address to the authorized whitelist.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the active administrator.
/// * `user` - The target address to whitelist.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn add_to_whitelist(
    env: &Env,
    caller: &Address,
    user: &Address,
) -> Result<(), AccessControlError> {
    require_admin(env, caller)?;
    let already = is_whitelisted(env, user);
    if !already {
        env.storage().persistent().set(&(WL, user.clone()), &true);
        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&WL_LIST)
            .unwrap_or(Vec::new(env));
        list.push_back(user.clone());
        env.storage().persistent().set(&WL_LIST, &list);
    }
    env.events().publish(
        (symbol_short!("WL_ADD"), caller.clone()),
        WhitelistAddedEvent {
            caller: caller.clone(),
            user: user.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
    Ok(())
}

/// Batch adds addresses to the authorized whitelist.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the active administrator.
/// * `users` - Vector of user addresses to add to the whitelist.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn batch_add_to_whitelist(
    env: &Env,
    caller: &Address,
    users: &Vec<Address>,
) -> Result<(), AccessControlError> {
    require_admin(env, caller)?;
    let mut list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&WL_LIST)
        .unwrap_or(Vec::new(env));

    let now = env.ledger().timestamp();
    for i in 0..users.len() {
        let user = users.get(i).unwrap();
        let already = is_whitelisted(env, &user);
        if !already {
            env.storage().persistent().set(&(WL, user.clone()), &true);
            list.push_back(user.clone());
        }
        env.events().publish(
            (symbol_short!("WL_ADD"), caller.clone()),
            WhitelistAddedEvent {
                caller: caller.clone(),
                user,
                timestamp: now,
            },
        );
    }
    env.storage().persistent().set(&WL_LIST, &list);
    Ok(())
}

/// Removes an address from the authorized whitelist.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the active administrator.
/// * `user` - The target address to remove from the whitelist.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn remove_from_whitelist(
    env: &Env,
    caller: &Address,
    user: &Address,
) -> Result<(), AccessControlError> {
    require_admin(env, caller)?;
    let already = is_whitelisted(env, user);
    if already {
        env.storage().persistent().remove(&(WL, user.clone()));
        let list: Option<Vec<Address>> = env.storage().persistent().get(&WL_LIST);
        if let Some(existing) = list {
            let mut new_list = Vec::new(env);
            for u in existing.iter() {
                if u != *user {
                    new_list.push_back(u);
                }
            }
            env.storage().persistent().set(&WL_LIST, &new_list);
        }
    }
    env.events().publish(
        (symbol_short!("WL_REM"), caller.clone()),
        WhitelistRemovedEvent {
            caller: caller.clone(),
            user: user.clone(),
            timestamp: env.ledger().timestamp(),
        },
    );
    Ok(())
}

/// Batch removes addresses from the authorized whitelist.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the active administrator.
/// * `users` - Vector of user addresses to remove from the whitelist.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn batch_remove_from_whitelist(
    env: &Env,
    caller: &Address,
    users: &Vec<Address>,
) -> Result<(), AccessControlError> {
    require_admin(env, caller)?;
    let now = env.ledger().timestamp();
    for i in 0..users.len() {
        let user = users.get(i).unwrap();
        env.storage().persistent().remove(&(WL, user.clone()));
        env.events().publish(
            (symbol_short!("WL_REM"), caller.clone()),
            WhitelistRemovedEvent {
                caller: caller.clone(),
                user,
                timestamp: now,
            },
        );
    }

    let list: Option<Vec<Address>> = env.storage().persistent().get(&WL_LIST);
    if let Some(existing) = list {
        let mut new_list = Vec::new(env);
        for u in existing.iter() {
            let mut remove = false;
            for j in 0..users.len() {
                if u == users.get(j).unwrap() {
                    remove = true;
                    break;
                }
            }
            if !remove {
                new_list.push_back(u);
            }
        }
        env.storage().persistent().set(&WL_LIST, &new_list);
    }
    Ok(())
}

/// Returns a paginated list of whitelisted addresses along with the total count.
/// (paginated_addresses, total_count)
pub fn get_whitelisted_addresses(env: &Env, offset: u32, limit: u32) -> (Vec<Address>, u32) {
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `offset` - The 0-based starting index.
/// * `limit` - The maximum number of entries to return.
///
/// Returns a tuple `(Vec<Address>, u32)` containing the paginated slice and total count.
pub fn get_whitelisted_addresses(
    env: &Env,
    offset: u32,
    limit: u32,
) -> (Vec<Address>, u32) {
    let list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&WL_LIST)
        .unwrap_or(Vec::new(env));
    let total = list.len();
    let mut page = Vec::new(env);

    if offset >= total || limit == 0 {
        return (page, total);
    }

    let end = (offset.saturating_add(limit)).min(total);
    for i in offset..end {
        if let Some(addr) = list.get(i) {
            page.push_back(addr);
        }
    }

    (page, total)
}

/// Checks whether `user` is on the whitelist.
///
/// Returns `true` if whitelist enforcement is disabled or if the user is explicitly whitelisted.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `user` - The address to check.
pub fn check_whitelist(env: &Env, user: &Address) -> bool {
    if !is_whitelist_enabled(env) {
        return true;
    }
    env.storage()
        .persistent()
        .get(&(WL, user.clone()))
        .unwrap_or(false)
}

/// Returns raw status of user on whitelist without checking `is_whitelist_enabled`.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `user` - The address to query.
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&(WL, user.clone()))
        .unwrap_or(false)
}
