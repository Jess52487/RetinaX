#![no_std]

pub mod admin;
pub mod permissions;
pub mod types;
pub mod whitelist;

#[cfg(test)]
mod test;

pub use admin::*;
pub use permissions::*;
pub use types::*;
pub use whitelist::*;

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

/// AccessControl micro-contract for decentralized role-based and attribute-based permissions.
#[contract]
pub struct AccessControlContract;

#[contractimpl]
impl AccessControlContract {
    /// Initializes the AccessControl micro-contract with an initial administrator.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `admin` - The address designated as the initial administrator. Requires authentication.
    ///
    /// # Errors
    /// * `AccessControlError::AlreadyInitialized` - If the contract has already been initialized.
    pub fn initialize(env: Env, admin: Address) -> Result<(), AccessControlError> {
        admin::initialize(&env, &admin)
    }

    /// Returns the currently active administrator address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    ///
    /// Returns `Some(Address)` if an admin is configured, or `None` if uninitialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        admin::get_admin(&env)
    }

    /// Proposes an administrative transfer in a two-step handshake.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `current_admin` - The currently active administrator address; requires authentication.
    /// * `new_admin` - The nominee address proposed to take over the admin role.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `current_admin` does not match the active administrator.
    pub fn propose_admin(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), AccessControlError> {
        admin::propose_admin(&env, &current_admin, &new_admin)
    }

    /// Accepts a proposed administrative role to finalize the transfer.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `new_admin` - The nominated administrator address; requires authentication.
    ///
    /// # Errors
    /// * `AccessControlError::NotFound` - If no administrative transfer has been proposed.
    /// * `AccessControlError::Unauthorized` - If `new_admin` is not the nominated pending administrator.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), AccessControlError> {
        admin::accept_admin(&env, &new_admin)
    }

    // ── Whitelist Management ────────────────────────────────────────────────

    /// Activates or deactivates whitelist enforcement.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address invoking the function; must be the active administrator.
    /// * `enabled` - `true` to enforce whitelist checks, `false` to permit all addresses.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn set_whitelist_enabled(
        env: Env,
        caller: Address,
        enabled: bool,
    ) -> Result<(), AccessControlError> {
        whitelist::set_whitelist_enabled(&env, &caller, enabled)
    }

    /// Returns whether whitelist enforcement is currently active.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    pub fn is_whitelist_enabled(env: Env) -> bool {
        whitelist::is_whitelist_enabled(&env)
    }

    /// Whitelists a user address for contract access.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address invoking the function; must be the active administrator.
    /// * `user` - The target address to whitelist.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn add_to_whitelist(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), AccessControlError> {
        whitelist::add_to_whitelist(&env, &caller, &user)
    }

    /// Batch adds user addresses to the authorized whitelist.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address invoking the function; must be the active administrator.
    /// * `users` - Vector of user addresses to add to the whitelist.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn batch_add_to_whitelist(
        env: Env,
        caller: Address,
        users: Vec<Address>,
    ) -> Result<(), AccessControlError> {
        whitelist::batch_add_to_whitelist(&env, &caller, &users)
    }

    /// Removes a user address from the authorized whitelist.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address invoking the function; must be the active administrator.
    /// * `user` - The target address to remove from the whitelist.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn remove_from_whitelist(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), AccessControlError> {
        whitelist::remove_from_whitelist(&env, &caller, &user)
    }

    /// Batch removes user addresses from the authorized whitelist.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address invoking the function; must be the active administrator.
    /// * `users` - Vector of user addresses to remove from the whitelist.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn batch_remove_from_whitelist(
        env: Env,
        caller: Address,
        users: Vec<Address>,
    ) -> Result<(), AccessControlError> {
        whitelist::batch_remove_from_whitelist(&env, &caller, &users)
    }

    /// Returns a paginated list of whitelisted addresses along with total count.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `offset` - The 0-based starting index in the whitelist.
    /// * `limit` - The maximum number of entries to return.
    ///
    /// Returns a tuple `(Vec<Address>, u32)` containing the paginated slice and total count.
    pub fn get_whitelisted_addresses(
        env: Env,
        offset: u32,
        limit: u32,
    ) -> (Vec<Address>, u32) {
        whitelist::get_whitelisted_addresses(&env, offset, limit)
    }

    /// Checks if a user is whitelisted (returns `true` if whitelist enforcement is disabled).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `user` - The address of the user to check.
    pub fn check_whitelist(env: Env, user: Address) -> bool {
        whitelist::check_whitelist(&env, &user)
    }

    /// Returns raw whitelist status for an address without checking whitelist enforcement flag.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `user` - The address to query.
    pub fn is_whitelisted(env: Env, user: Address) -> bool {
        whitelist::is_whitelisted(&env, &user)
    }

    // ── RBAC & Role Management ──────────────────────────────────────────────

    /// Assigns a role to a user with an optional expiration timestamp.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The caller granting the role; requires authentication and `Permission::ManageUsers` (or admin).
    /// * `user` - The target address receiving the role.
    /// * `role` - The `Role` variant to assign.
    /// * `expires_at` - Unix timestamp when the role expires, or 0 for permanent assignment.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
    pub fn assign_role(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
        expires_at: u64,
    ) -> Result<(), AccessControlError> {
        permissions::assign_role(&env, &caller, &user, role, expires_at)
    }

    /// Revokes an assigned role from a user.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The caller revoking the role; requires authentication and `Permission::ManageUsers` (or admin).
    /// * `user` - The target address whose role is being revoked.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
    pub fn revoke_role(
        env: Env,
        caller: Address,
        user: Address,
    ) -> Result<(), AccessControlError> {
        permissions::revoke_role(&env, &caller, &user)
    }

    /// Returns a user's active role, returning `Role::None` if expired or unassigned.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `user` - The address of the user to query.
    pub fn get_role(env: Env, user: Address) -> Role {
        permissions::get_role(&env, &user)
    }

    /// Checks if a user holds a permission through roles, custom grants, groups, or admin authority.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `user` - The address of the user to check.
    /// * `permission` - The `Permission` variant to verify.
    pub fn has_permission(env: Env, user: Address, permission: Permission) -> bool {
        permissions::has_permission(&env, &user, &permission)
    }

    /// Checks if a delegatee holds a delegated permission from a delegator.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `delegator` - The address granting the delegation.
    /// * `delegatee` - The address receiving the delegated authority.
    /// * `permission` - The `Permission` variant to verify.
    pub fn has_delegated_permission(
        env: Env,
        delegator: Address,
        delegatee: Address,
        permission: Permission,
    ) -> bool {
        permissions::has_delegated_permission(&env, &delegator, &delegatee, &permission)
    }

    /// Grants an explicit custom permission directly to a user's role assignment.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The caller granting the permission; requires authentication and `Permission::ManageUsers` (or admin).
    /// * `user` - The target address receiving the custom permission.
    /// * `permission` - The `Permission` variant to grant.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
    /// * `AccessControlError::NotFound` - If `user` does not have an active role assignment.
    pub fn grant_custom_permission(
        env: Env,
        caller: Address,
        user: Address,
        permission: Permission,
    ) -> Result<(), AccessControlError> {
        permissions::grant_custom_permission(&env, &caller, &user, permission)
    }

    /// Explicitly revokes a custom permission from a user (explicit deny).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The caller revoking the permission; requires authentication and `Permission::ManageUsers` (or admin).
    /// * `user` - The target address whose permission is being revoked.
    /// * `permission` - The `Permission` variant to revoke.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
    /// * `AccessControlError::NotFound` - If `user` does not have an active role assignment.
    pub fn revoke_custom_permission(
        env: Env,
        caller: Address,
        user: Address,
        permission: Permission,
    ) -> Result<(), AccessControlError> {
        permissions::revoke_custom_permission(&env, &caller, &user, permission)
    }

    /// Sets the maximum delegation chain depth allowed.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The administrator address; requires authentication.
    /// * `max_depth` - The maximum delegation depth ceiling.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` is not the active administrator.
    pub fn set_max_delegation_depth(
        env: Env,
        caller: Address,
        max_depth: u32,
    ) -> Result<(), AccessControlError> {
        permissions::set_max_delegation_depth(&env, &caller, max_depth)
    }

    /// Gets the maximum delegation chain depth allowed.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    pub fn get_max_delegation_depth(env: Env) -> u32 {
        permissions::get_max_delegation_depth(&env)
    }

    /// Delegates a complete role from delegator to delegatee.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `delegator` - The delegating address; requires authentication.
    /// * `delegatee` - The address receiving the delegated role.
    /// * `role` - The `Role` variant to delegate.
    /// * `expires_at` - Unix timestamp when the delegation expires, or 0 for permanent delegation.
    ///
    /// # Errors
    /// * `AccessControlError::DelegationDepthExceeded` - If adding this delegation exceeds `MAX_DEL_DEPTH`.
    pub fn delegate_role(
        env: Env,
        delegator: Address,
        delegatee: Address,
        role: Role,
        expires_at: u64,
    ) -> Result<(), AccessControlError> {
        permissions::delegate_role(&env, &delegator, &delegatee, role, expires_at)
    }

    /// Revokes an active full-role delegation.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `delegator` - The address that granted the delegation; requires authentication.
    /// * `delegatee` - The address whose delegated role is being revoked.
    pub fn revoke_delegation(
        env: Env,
        delegator: Address,
        delegatee: Address,
    ) -> Result<(), AccessControlError> {
        permissions::revoke_delegation(&env, &delegator, &delegatee)
    }

    /// Delegates specific scoped permissions from delegator to delegatee.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `delegator` - The delegating address; requires authentication.
    /// * `delegatee` - The address receiving the scoped permissions.
    /// * `permissions` - Vector of `Permission` variants to delegate.
    /// * `expires_at` - Unix timestamp when the delegation expires, or 0 for permanent delegation.
    ///
    /// # Errors
    /// * `AccessControlError::DelegationDepthExceeded` - If adding this delegation exceeds `MAX_DEL_DEPTH`.
    pub fn delegate_permissions(
        env: Env,
        delegator: Address,
        delegatee: Address,
        permissions: Vec<Permission>,
        expires_at: u64,
    ) -> Result<(), AccessControlError> {
        permissions::delegate_permissions(&env, &delegator, &delegatee, permissions, expires_at)
    }

    /// Revokes an active scoped delegation.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `delegator` - The address that granted the scoped delegation; requires authentication.
    /// * `delegatee` - The address whose scoped permissions are being revoked.
    pub fn revoke_scoped_delegation(
        env: Env,
        delegator: Address,
        delegatee: Address,
    ) -> Result<(), AccessControlError> {
        permissions::revoke_scoped_delegation(&env, &delegator, &delegatee)
    }

    // ── ACL Group Management ────────────────────────────────────────────────

    /// Creates an ACL group with specified permissions.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address creating the group; requires `Permission::ManageAccess` (or admin).
    /// * `group_name` - The unique identifier symbol for the group.
    /// * `permissions` - Vector of `Permission` variants granted to members of this group.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
    pub fn create_group(
        env: Env,
        caller: Address,
        group_name: Symbol,
        permissions: Vec<Permission>,
    ) -> Result<(), AccessControlError> {
        permissions::create_group(&env, &caller, group_name, permissions)
    }

    /// Adds a user to an existing ACL group.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address adding the user; requires `Permission::ManageAccess` (or admin).
    /// * `user` - The target address to add to the group.
    /// * `group_name` - The identifier symbol of the ACL group.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
    pub fn add_to_group(
        env: Env,
        caller: Address,
        user: Address,
        group_name: Symbol,
    ) -> Result<(), AccessControlError> {
        permissions::add_to_group(&env, &caller, &user, group_name)
    }

    /// Removes a user from an ACL group.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address removing the user; requires `Permission::ManageAccess` (or admin).
    /// * `user` - The target address to remove from the group.
    /// * `group_name` - The identifier symbol of the ACL group.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageAccess` and is not admin.
    pub fn remove_from_group(
        env: Env,
        caller: Address,
        user: Address,
        group_name: Symbol,
    ) -> Result<(), AccessControlError> {
        permissions::remove_from_group(&env, &caller, &user, group_name)
    }

    /// Returns all ACL groups a user belongs to.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `user` - The address of the user to query.
    pub fn get_user_groups(env: Env, user: Address) -> Vec<Symbol> {
        permissions::get_user_groups(&env, &user)
    }

    // ── Attribute-Based Access Control (ABAC) ───────────────────────────────

    /// Creates or updates an attribute-based access policy (ABAC).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address creating the policy; requires `Permission::SystemAdmin` (or admin).
    /// * `policy_id` - The unique identifier symbol for the policy.
    /// * `name` - The human-readable name symbol for the policy.
    /// * `conditions` - The `PolicyConditions` struct defining evaluation rules.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::SystemAdmin` and is not admin.
    pub fn create_access_policy(
        env: Env,
        caller: Address,
        policy_id: Symbol,
        name: Symbol,
        conditions: PolicyConditions,
    ) -> Result<(), AccessControlError> {
        permissions::create_access_policy(&env, &caller, policy_id, name, conditions)
    }

    /// Sets a user's professional credential.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address setting the credential; requires `Permission::ManageUsers` (or admin).
    /// * `user` - The target address receiving the credential classification.
    /// * `credential` - The `CredentialType` variant to associate with `user`.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::ManageUsers` and is not admin.
    pub fn set_user_credential(
        env: Env,
        caller: Address,
        user: Address,
        credential: CredentialType,
    ) -> Result<(), AccessControlError> {
        permissions::set_user_credential(&env, &caller, &user, credential)
    }

    /// Sets a medical record's sensitivity classification level.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment.
    /// * `caller` - The address classifying the record; requires `Permission::WriteRecord` (or admin).
    /// * `record_id` - The numeric identifier of the record.
    /// * `sensitivity` - The `SensitivityLevel` variant to assign.
    ///
    /// # Errors
    /// * `AccessControlError::Unauthorized` - If `caller` lacks `Permission::WriteRecord` and is not admin.
    pub fn set_record_sensitivity(
        env: Env,
        caller: Address,
        record_id: u64,
        sensitivity: SensitivityLevel,
    ) -> Result<(), AccessControlError> {
        permissions::set_record_sensitivity(&env, &caller, record_id, sensitivity)
    }

    /// Evaluates an ABAC policy against contextual variables.
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
        env: Env,
        policy_id: Symbol,
        user: Address,
        record_id: u64,
        current_time: u64,
    ) -> bool {
        permissions::evaluate_access_policy(&env, policy_id, &user, record_id, current_time)
    }
}
