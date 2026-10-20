#![cfg(test)]

use crate::{
    AccessControlContract, AccessControlContractClient, AccessControlError, CredentialType,
    Permission, PolicyConditions, Role, SensitivityLevel, TimeRestriction,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, Vec,
};

#[test]
fn test_initialize_and_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Propose admin
    let new_admin = Address::generate(&env);
    client.propose_admin(&admin, &new_admin);

    // Accept admin
    client.accept_admin(&new_admin);
    assert_eq!(client.get_admin(), Some(new_admin));
}

#[test]
fn test_roles_and_permissions() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let doctor = Address::generate(&env);
    let patient = Address::generate(&env);

    // Admin has ManageUsers, so assign Optometrist to doctor
    client.assign_role(&admin, &doctor, &Role::Optometrist, &0);
    assert_eq!(client.get_role(&doctor), Role::Optometrist);

    // Optometrist can WriteRecord and ReadAnyRecord
    assert!(client.has_permission(&doctor, &Permission::WriteRecord));
    assert!(client.has_permission(&doctor, &Permission::ReadAnyRecord));
    assert!(!client.has_permission(&doctor, &Permission::SystemAdmin));

    // Patient has no global permissions
    client.assign_role(&admin, &patient, &Role::Patient, &0);
    assert!(!client.has_permission(&patient, &Permission::WriteRecord));
}

#[test]
fn test_custom_permissions() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let staff = Address::generate(&env);
    client.assign_role(&admin, &staff, &Role::Staff, &0);

    // Staff cannot write record by default
    assert!(!client.has_permission(&staff, &Permission::WriteRecord));

    // Grant custom permission
    client.grant_custom_permission(&admin, &staff, &Permission::WriteRecord);
    assert!(client.has_permission(&staff, &Permission::WriteRecord));

    // Revoke custom permission
    client.revoke_custom_permission(&admin, &staff, &Permission::WriteRecord);
    assert!(!client.has_permission(&staff, &Permission::WriteRecord));
}

#[test]
fn test_delegation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let ophth = Address::generate(&env);
    let assistant = Address::generate(&env);

    client.assign_role(&admin, &ophth, &Role::Ophthalmologist, &0);

    // Scoped delegation of WriteRecord to assistant
    let mut perms = Vec::new(&env);
    perms.push_back(Permission::WriteRecord);
    client.delegate_permissions(&ophth, &assistant, &perms, &0);

    assert!(client.has_delegated_permission(&ophth, &assistant, &Permission::WriteRecord));
    assert!(!client.has_delegated_permission(&ophth, &assistant, &Permission::SystemAdmin));

    // Revoke scoped delegation
    client.revoke_scoped_delegation(&ophth, &assistant);
    assert!(!client.has_delegated_permission(&ophth, &assistant, &Permission::WriteRecord));

    // Delegation chain depth tests (default limit: 3)
    assert_eq!(client.get_max_delegation_depth(), 3);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);
    let user_d = Address::generate(&env);
    let user_e = Address::generate(&env);

    client.assign_role(&admin, &user_a, &Role::Ophthalmologist, &0);

    // Depth 1: user_a -> user_b (Allowed)
    let res1 = client.try_delegate_role(&user_a, &user_b, &Role::Ophthalmologist, &0);
    assert!(res1.is_ok());

    // Depth 2: user_b -> user_c (Allowed)
    let res2 = client.try_delegate_role(&user_b, &user_c, &Role::Ophthalmologist, &0);
    assert!(res2.is_ok());

    // Depth 3: user_c -> user_d (Allowed, reaching default limit of 3)
    let res3 = client.try_delegate_role(&user_c, &user_d, &Role::Ophthalmologist, &0);
    assert!(res3.is_ok());

    // Depth 4: user_d -> user_e (Exceeds limit of 3, must return DelegationDepthExceeded)
    let res4 = client.try_delegate_role(&user_d, &user_e, &Role::Ophthalmologist, &0);
    assert_eq!(res4, Err(Ok(AccessControlError::DelegationDepthExceeded)));

    // Test configurable depth limit
    client.set_max_delegation_depth(&admin, &4);
    assert_eq!(client.get_max_delegation_depth(), 4);

    // Now Depth 4 succeeds
    let res4_retry = client.try_delegate_role(&user_d, &user_e, &Role::Ophthalmologist, &0);
    assert!(res4_retry.is_ok());
}


#[test]
fn test_whitelist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);

    // When disabled, all pass
    assert!(!client.is_whitelist_enabled());
    assert!(client.check_whitelist(&user));

    // Enable whitelist
    client.set_whitelist_enabled(&admin, &true);
    assert!(client.is_whitelist_enabled());
    assert!(!client.check_whitelist(&user));

    // Add to whitelist
    client.add_to_whitelist(&admin, &user);
    assert!(client.check_whitelist(&user));
    assert!(client.is_whitelisted(&user));

    // Remove from whitelist
    client.remove_from_whitelist(&admin, &user);
    assert!(!client.check_whitelist(&user));

    // Batch add to whitelist
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    let mut batch = Vec::new(&env);
    batch.push_back(user1.clone());
    batch.push_back(user2.clone());
    batch.push_back(user3.clone());

    client.batch_add_to_whitelist(&admin, &batch);
    assert!(client.is_whitelisted(&user1));
    assert!(client.is_whitelisted(&user2));
    assert!(client.is_whitelisted(&user3));

    // Test pagination
    let (page1, total1) = client.get_whitelisted_addresses(&0, &2);
    assert_eq!(total1, 3);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap(), user1);
    assert_eq!(page1.get(1).unwrap(), user2);

    let (page2, total2) = client.get_whitelisted_addresses(&2, &2);
    assert_eq!(total2, 3);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2.get(0).unwrap(), user3);

    // Batch remove
    let mut remove_batch = Vec::new(&env);
    remove_batch.push_back(user1.clone());
    remove_batch.push_back(user3.clone());

    client.batch_remove_from_whitelist(&admin, &remove_batch);
    assert!(!client.is_whitelisted(&user1));
    assert!(client.is_whitelisted(&user2));
    assert!(!client.is_whitelisted(&user3));

    let (page_rem, total_rem) = client.get_whitelisted_addresses(&0, &10);
    assert_eq!(total_rem, 1);
    assert_eq!(page_rem.len(), 1);
    assert_eq!(page_rem.get(0).unwrap(), user2);
}


#[test]
fn test_acl_groups() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    let group_name = symbol_short!("CLINIC_A");

    let mut perms = Vec::new(&env);
    perms.push_back(Permission::ReadAnyRecord);

    client.create_group(&admin, &group_name, &perms);
    client.add_to_group(&admin, &user, &group_name);

    assert!(client.has_permission(&user, &Permission::ReadAnyRecord));

    let groups = client.get_user_groups(&user);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups.get(0), Some(group_name.clone()));

    client.remove_from_group(&admin, &user, &group_name);
    assert!(!client.has_permission(&user, &Permission::ReadAnyRecord));
}

#[test]
fn test_abac_policy() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1700000000);
    let contract_id = env.register(AccessControlContract, ());
    let client = AccessControlContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let doctor = Address::generate(&env);
    client.assign_role(&admin, &doctor, &Role::Optometrist, &0);
    client.set_user_credential(&admin, &doctor, &CredentialType::MedicalLicense);

    let policy_id = symbol_short!("EXAM_POL");
    let conditions = PolicyConditions {
        required_role: Role::Optometrist,
        time_restriction: TimeRestriction::None,
        required_credential: CredentialType::MedicalLicense,
        min_sensitivity_level: SensitivityLevel::Standard,
        consent_required: false,
    };

    client.create_access_policy(&admin, &policy_id, &symbol_short!("EXAM"), &conditions);
    client.set_record_sensitivity(&doctor, &101, &SensitivityLevel::Standard);

    assert!(client.evaluate_access_policy(&policy_id, &doctor, &101, &1700000000));
}
