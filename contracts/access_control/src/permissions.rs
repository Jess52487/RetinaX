use crate::types::{
    get_base_permissions, AccessControlError, AccessPolicy, AclGroup, CredentialType, Delegation,
    Permission, PolicyConditions, Role, RoleAssignment, ScopedDelegation, SensitivityLevel,
    TimeRestriction,
};
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

const ROLE_ASSIGN: Symbol = symbol_short!("ROLE_ASN");
const DELEGATION_KEY: Symbol = symbol_short!("DELEG");
const SCOPED_DEL_KEY: Symbol = symbol_short!("SC_DEL");
const ACL_GRP: Symbol = symbol_short!("ACL_GRP");
const USR_GRPS: Symbol = symbol_short!("USR_GRPS");
const ACC_POL: Symbol = symbol_short!("ACC_POL");
const USR_CRED: Symbol = symbol_short!("USR_CRED");
const REC_SENS: Symbol = symbol_short!("REC_SENS");
const MAX_DEL_DEPTH: Symbol = symbol_short!("MAX_D_DP");
const DEL_PARENT: Symbol = symbol_short!("DEL_PAR");

pub const DEFAULT_MAX_DELEGATION_DEPTH: u32 = 3;

/// Sets the maximum delegation chain depth allowed.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address invoking the function; must be the contract administrator.
/// * `max_depth` - The maximum delegation depth ceiling (e.g., 3).
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
pub fn set_max_delegation_depth(
    env: &Env,
    caller: &Address,
    max_depth: u32,
) -> Result<(), AccessControlError> {
    crate::admin::require_admin(env, caller)?;
    env.storage().instance().set(&MAX_DEL_DEPTH, &max_depth);
    Ok(())
}

/// Gets the configured maximum delegation chain depth.
///
/// Defaults to `DEFAULT_MAX_DELEGATION_DEPTH` (3) if not explicitly set.
///
/// # Arguments
/// * `env` - The Soroban environment.
pub fn get_max_delegation_depth(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MAX_DEL_DEPTH)
        .unwrap_or(DEFAULT_MAX_DELEGATION_DEPTH)
}

/// Helper to compute delegation depth for delegator
fn get_delegator_depth(env: &Env, delegator: &Address) -> u32 {
    let mut depth: u32 = 0;
    let mut current: Option<Address> = env
        .storage()
        .persistent()
        .get(&(DEL_PARENT, delegator.clone()));

    // Traverse upwards with safety ceiling to prevent infinite loop on cycles
    while let Some(parent) = current {
        depth = depth.saturating_add(1);
        if depth > 100 {
            break;
        }
        current = env.storage().persistent().get(&(DEL_PARENT, parent));
    }
    depth
}

/// Assigns a role to a user with an optional expiration timestamp.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address granting the role; requires authentication and `Permission::ManageUsers` (or admin).
/// * `user` - The target address receiving the role assignment.
/// * `role` - The `Role` variant to assign.
/// * `expires_at` - Unix timestamp when the role expires, or 0 for a permanent assignment.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
pub fn assign_role(
    env: &Env,
    caller: &Address,
    user: &Address,
    role: Role,
    expires_at: u64,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    // Caller must have ManageUsers permission or be Admin
    if !has_permission(env, caller, &Permission::ManageUsers) {
        return Err(AccessControlError::Unauthorized);
    }

    let assignment = RoleAssignment {
        role,
        custom_grants: Vec::new(env),
        custom_revokes: Vec::new(env),
        expires_at,
    };

    env.storage()
        .persistent()
        .set(&(ROLE_ASSIGN, user.clone()), &assignment);
    Ok(())
}

/// Revokes a user's role assignment.
pub fn revoke_role(env: &Env, caller: &Address, user: &Address) -> Result<(), AccessControlError> {
/// Revokes a user's active role assignment.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address revoking the role; requires authentication and `Permission::ManageUsers` (or admin).
/// * `user` - The target address whose role assignment is being removed.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
pub fn revoke_role(
    env: &Env,
    caller: &Address,
    user: &Address,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageUsers) {
        return Err(AccessControlError::Unauthorized);
    }

    env.storage()
        .persistent()
        .remove(&(ROLE_ASSIGN, user.clone()));
    Ok(())
}

/// Retrieves the active role assigned to a user, accounting for expiration.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `user` - The address of the user to query.
///
/// Returns `Role::Admin` if `user` is the contract admin, the assigned `Role` if active,
/// or `Role::None` if the assignment has expired or was not found.
pub fn get_role(env: &Env, user: &Address) -> Role {
    if let Some(admin) = crate::admin::get_admin(env) {
        if &admin == user {
            return Role::Admin;
        }
    }

    let assignment: Option<RoleAssignment> =
        env.storage().persistent().get(&(ROLE_ASSIGN, user.clone()));

    match assignment {
        Some(a) => {
            let now = env.ledger().timestamp();
            if a.expires_at > 0 && now >= a.expires_at {
                Role::None
            } else {
                a.role
            }
        }
        None => Role::None,
    }
}

/// Checks whether `user` holds a specific `permission`.
///
/// Evaluation pipeline:
/// 0. If user is registered contract admin, return true
/// 1. Check custom revokes (explicit deny)
/// 2. Check custom grants (explicit grant)
/// 3. Check base permissions of active role
/// 4. Check ACL groups the user belongs to
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `user` - The address of the user to check.
/// * `permission` - The permission to verify.
///
/// Returns `true` if authorized, `false` otherwise.
pub fn has_permission(env: &Env, user: &Address, permission: &Permission) -> bool {
    if let Some(admin) = crate::admin::get_admin(env) {
        if &admin == user {
            return true;
        }
    }

    let assignment: Option<RoleAssignment> =
        env.storage().persistent().get(&(ROLE_ASSIGN, user.clone()));

    if let Some(a) = assignment {
        let now = env.ledger().timestamp();
        let is_valid = a.expires_at == 0 || now < a.expires_at;

        if is_valid {
            // 1. Explicit deny
            for revoked in a.custom_revokes.iter() {
                if &revoked == permission {
                    return false;
                }
            }

            // 2. Explicit grant
            for granted in a.custom_grants.iter() {
                if &granted == permission {
                    return true;
                }
            }

            // 3. Base role permissions
            let base_perms = get_base_permissions(&a.role, env);
            for p in base_perms.iter() {
                if &p == permission {
                    return true;
                }
            }
        }
    }

    // 4. ACL Groups
    let groups: Option<Vec<Symbol>> = env.storage().persistent().get(&(USR_GRPS, user.clone()));

    if let Some(user_groups) = groups {
        for group_name in user_groups.iter() {
            let group: Option<AclGroup> = env.storage().persistent().get(&(ACL_GRP, group_name));

            if let Some(grp) = group {
                for p in grp.permissions.iter() {
                    if &p == permission {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Checks whether `delegatee` holds a delegated permission from `delegator`.
///
/// Checks both full-role delegations and scoped permission delegations,
/// verifying that neither delegation has expired.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `delegator` - The address granting the delegation.
/// * `delegatee` - The address receiving the delegated authority.
/// * `permission` - The permission to verify.
///
/// Returns `true` if the delegatee holds the active permission, `false` otherwise.
pub fn has_delegated_permission(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
    permission: &Permission,
) -> bool {
    let now = env.ledger().timestamp();

    // Check full role delegation
    let del_key = (DELEGATION_KEY, delegator.clone(), delegatee.clone());
    let delegation: Option<Delegation> = env.storage().persistent().get(&del_key);

    if let Some(d) = delegation {
        if d.expires_at == 0 || now < d.expires_at {
            let base_perms = get_base_permissions(&d.role, env);
            for p in base_perms.iter() {
                if &p == permission {
                    return true;
                }
            }
        }
    }

    // Check scoped delegation
    let scoped_key = (SCOPED_DEL_KEY, delegator.clone(), delegatee.clone());
    let scoped: Option<ScopedDelegation> = env.storage().persistent().get(&scoped_key);

    if let Some(s) = scoped {
        if s.expires_at == 0 || now < s.expires_at {
            for p in s.permissions.iter() {
                if &p == permission {
                    return true;
                }
            }
        }
    }

    false
}

/// Grants a custom permission directly to a user's role assignment.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address granting the permission; requires authentication and `Permission::ManageUsers` (or admin).
/// * `user` - The target address receiving the custom permission grant.
/// * `permission` - The `Permission` variant to grant.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
/// * `AccessControlError::NotFound` - If `user` does not have an active role assignment in storage.
pub fn grant_custom_permission(
    env: &Env,
    caller: &Address,
    user: &Address,
    permission: Permission,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageUsers) {
        return Err(AccessControlError::Unauthorized);
    }

    let key = (ROLE_ASSIGN, user.clone());
    let mut assignment: RoleAssignment = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(AccessControlError::NotFound)?;

    assignment.custom_grants.push_back(permission);
    env.storage().persistent().set(&key, &assignment);
    Ok(())
}

/// Revokes a custom permission from a user (explicit deny).
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The address executing the revocation; requires authentication and `Permission::ManageUsers` (or admin).
/// * `user` - The target address whose permission is being revoked.
/// * `permission` - The `Permission` variant to explicitly deny.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
/// * `AccessControlError::NotFound` - If `user` does not have an active role assignment in storage.
pub fn revoke_custom_permission(
    env: &Env,
    caller: &Address,
    user: &Address,
    permission: Permission,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageUsers) {
        return Err(AccessControlError::Unauthorized);
    }

    let key = (ROLE_ASSIGN, user.clone());
    let mut assignment: RoleAssignment = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(AccessControlError::NotFound)?;

    assignment.custom_revokes.push_back(permission);
    env.storage().persistent().set(&key, &assignment);
    Ok(())
}

/// Delegates an entire role from delegator to delegatee.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `delegator` - The delegating address; requires authentication.
/// * `delegatee` - The address receiving the delegated role.
/// * `role` - The `Role` variant to delegate.
/// * `expires_at` - Unix timestamp when the delegation expires, or 0 for permanent delegation.
///
/// # Errors
/// * `AccessControlError::DelegationDepthExceeded` - If adding this delegation exceeds the configured `MAX_DEL_DEPTH`.
pub fn delegate_role(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
    role: Role,
    expires_at: u64,
) -> Result<(), AccessControlError> {
    delegator.require_auth();

    let max_depth = get_max_delegation_depth(env);
    let current_depth = get_delegator_depth(env, delegator);
    let new_depth = current_depth.saturating_add(1);
    if new_depth > max_depth {
        return Err(AccessControlError::DelegationDepthExceeded);
    }

    let key = (DELEGATION_KEY, delegator.clone(), delegatee.clone());
    let delegation = Delegation {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        role,
        expires_at,
    };
    env.storage().persistent().set(&key, &delegation);
    env.storage()
        .persistent()
        .set(&(DEL_PARENT, delegatee.clone()), delegator);
    Ok(())
}

/// Revokes an active full-role delegation.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `delegator` - The address that created the delegation; requires authentication.
/// * `delegatee` - The address whose delegated role is being revoked.
pub fn revoke_delegation(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
) -> Result<(), AccessControlError> {
    delegator.require_auth();
    let key = (DELEGATION_KEY, delegator.clone(), delegatee.clone());
    env.storage().persistent().remove(&key);
    env.storage()
        .persistent()
        .remove(&(DEL_PARENT, delegatee.clone()));
    Ok(())
}

/// Delegates specific scoped permissions from delegator to delegatee.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `delegator` - The delegating address; requires authentication.
/// * `delegatee` - The address receiving the scoped permissions.
/// * `permissions` - A list of `Permission` variants to delegate.
/// * `expires_at` - Unix timestamp when the delegation expires, or 0 for permanent delegation.
///
/// # Errors
/// * `AccessControlError::DelegationDepthExceeded` - If adding this delegation exceeds the configured `MAX_DEL_DEPTH`.
pub fn delegate_permissions(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
    permissions: Vec<Permission>,
    expires_at: u64,
) -> Result<(), AccessControlError> {
    delegator.require_auth();

    let max_depth = get_max_delegation_depth(env);
    let current_depth = get_delegator_depth(env, delegator);
    let new_depth = current_depth.saturating_add(1);
    if new_depth > max_depth {
        return Err(AccessControlError::DelegationDepthExceeded);
    }

    let key = (SCOPED_DEL_KEY, delegator.clone(), delegatee.clone());
    let scoped = ScopedDelegation {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        permissions,
        expires_at,
    };
    env.storage().persistent().set(&key, &scoped);
    env.storage()
        .persistent()
        .set(&(DEL_PARENT, delegatee.clone()), delegator);
    Ok(())
}

/// Revokes an active scoped delegation.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `delegator` - The address that granted the scoped delegation; requires authentication.
/// * `delegatee` - The address whose scoped permissions are being revoked.
pub fn revoke_scoped_delegation(
    env: &Env,
    delegator: &Address,
    delegatee: &Address,
) -> Result<(), AccessControlError> {
    delegator.require_auth();
    let key = (SCOPED_DEL_KEY, delegator.clone(), delegatee.clone());
    env.storage().persistent().remove(&key);
    env.storage()
        .persistent()
        .remove(&(DEL_PARENT, delegatee.clone()));
    Ok(())
}

/// Creates a new ACL group with a set of permissions.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller creating the group; requires authentication and `Permission::ManageAccess` (or admin).
/// * `group_name` - The unique identifier symbol for the group.
/// * `permissions` - The list of `Permission` variants granted to members of this group.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
pub fn create_group(
    env: &Env,
    caller: &Address,
    group_name: Symbol,
    permissions: Vec<Permission>,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageAccess) {
        return Err(AccessControlError::Unauthorized);
    }

    let grp = AclGroup {
        name: group_name.clone(),
        permissions,
    };
    env.storage().persistent().set(&(ACL_GRP, group_name), &grp);
    Ok(())
}

/// Adds a user to an existing ACL group.
///
/// If the user is already a member, this operation is idempotent and returns `Ok(())`.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller adding the user; requires authentication and `Permission::ManageAccess` (or admin).
/// * `user` - The target address to add to the group.
/// * `group_name` - The identifier symbol of the target ACL group.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
pub fn add_to_group(
    env: &Env,
    caller: &Address,
    user: &Address,
    group_name: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageAccess) {
        return Err(AccessControlError::Unauthorized);
    }

    let user_key = (USR_GRPS, user.clone());
    let mut groups: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&user_key)
        .unwrap_or(Vec::new(env));

    // Avoid duplicate group assignments
    for g in groups.iter() {
        if g == group_name {
            return Ok(());
        }
    }

    groups.push_back(group_name);
    env.storage().persistent().set(&user_key, &groups);
    Ok(())
}

/// Removes a user from an ACL group.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller removing the user; requires authentication and `Permission::ManageAccess` (or admin).
/// * `user` - The target address to remove from the group.
/// * `group_name` - The identifier symbol of the ACL group.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
pub fn remove_from_group(
    env: &Env,
    caller: &Address,
    user: &Address,
    group_name: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageAccess) {
        return Err(AccessControlError::Unauthorized);
    }

    let user_key = (USR_GRPS, user.clone());
    let groups: Option<Vec<Symbol>> = env.storage().persistent().get(&user_key);

    if let Some(list) = groups {
        let mut new_list = Vec::new(env);
        for g in list.iter() {
            if g != group_name {
                new_list.push_back(g);
            }
        }
        env.storage().persistent().set(&user_key, &new_list);
    }
    Ok(())
}

/// Returns the list of ACL groups a user belongs to.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `user` - The address of the user to query.
pub fn get_user_groups(env: &Env, user: &Address) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&(USR_GRPS, user.clone()))
        .unwrap_or(Vec::new(env))
}

/// Creates or updates an attribute-based access policy.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller creating the policy; requires authentication and `Permission::SystemAdmin` (or admin).
/// * `policy_id` - The unique identifier symbol for the policy.
/// * `name` - The human-readable name symbol for the policy.
/// * `conditions` - The `PolicyConditions` struct defining evaluation rules.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::SystemAdmin` and is not admin.
pub fn create_access_policy(
    env: &Env,
    caller: &Address,
    policy_id: Symbol,
    name: Symbol,
    conditions: PolicyConditions,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::SystemAdmin) {
        return Err(AccessControlError::Unauthorized);
    }

    let policy = AccessPolicy {
        id: policy_id.clone(),
        name,
        conditions,
        enabled: true,
    };
    env.storage()
        .persistent()
        .set(&(ACC_POL, policy_id), &policy);
    Ok(())
}

/// Sets a professional credential for a user.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller setting the credential; requires authentication and `Permission::ManageUsers` (or admin).
/// * `user` - The target address receiving the credential classification.
/// * `credential` - The `CredentialType` variant to associate with `user`.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
pub fn set_user_credential(
    env: &Env,
    caller: &Address,
    user: &Address,
    credential: CredentialType,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::ManageUsers) {
        return Err(AccessControlError::Unauthorized);
    }

    env.storage()
        .persistent()
        .set(&(USR_CRED, user.clone()), &credential);
    Ok(())
}

/// Sets the sensitivity level of a medical record.
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `caller` - The caller classifying the record; requires authentication and `Permission::WriteRecord` (or admin).
/// * `record_id` - The numeric identifier of the record.
/// * `sensitivity` - The `SensitivityLevel` variant to assign.
///
/// # Errors
/// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::WriteRecord` and is not admin.
pub fn set_record_sensitivity(
    env: &Env,
    caller: &Address,
    record_id: u64,
    sensitivity: SensitivityLevel,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !has_permission(env, caller, &Permission::WriteRecord) {
        return Err(AccessControlError::Unauthorized);
    }

    env.storage()
        .persistent()
        .set(&(REC_SENS, record_id), &sensitivity);
    Ok(())
}

/// Evaluates an ABAC policy against context (user role, credential, sensitivity, time).
///
/// # Arguments
/// * `env` - The Soroban environment.
/// * `policy_id` - The identifier symbol of the policy to evaluate.
/// * `user` - The address of the user requesting access.
/// * `record_id` - The numeric identifier of the record being accessed.
/// * `current_time` - Current Unix timestamp for evaluating time window constraints.
///
/// Returns `true` if all policy criteria are satisfied, `false` otherwise.
pub fn evaluate_access_policy(
    env: &Env,
    policy_id: Symbol,
    user: &Address,
    record_id: u64,
    current_time: u64,
) -> bool {
    let policy: Option<AccessPolicy> = env.storage().persistent().get(&(ACC_POL, policy_id));
    let policy = match policy {
        Some(p) if p.enabled => p,
        _ => return false,
    };

    let user_role = get_role(env, user);
    if (user_role as u32) < (policy.conditions.required_role as u32) {
        return false;
    }

    // Check credential
    let cred: CredentialType = env
        .storage()
        .persistent()
        .get(&(USR_CRED, user.clone()))
        .unwrap_or(CredentialType::None);
    if policy.conditions.required_credential != CredentialType::None
        && cred != policy.conditions.required_credential
    {
        return false;
    }

    // Check sensitivity
    let sens: SensitivityLevel = env
        .storage()
        .persistent()
        .get(&(REC_SENS, record_id))
        .unwrap_or(SensitivityLevel::Standard);
    if (sens as u32) < (policy.conditions.min_sensitivity_level as u32) {
        return false;
    }

    // Check time restriction
    match policy.conditions.time_restriction {
        TimeRestriction::None => {}
        TimeRestriction::BusinessHours => {
            let hour = (current_time / 3600) % 24;
            if !(9..17).contains(&hour) {
                return false;
            }
        }
        TimeRestriction::HourRange(start, end) => {
            let hour = ((current_time / 3600) % 24) as u32;
            if hour < start || hour > end {
                return false;
            }
        }
        TimeRestriction::DaysOfWeek(mask) => {
            let day = ((current_time / 86400 + 4) % 7) as u32;
            if (mask & (1 << day)) == 0 {
                return false;
            }
        }
    }

    true
}
