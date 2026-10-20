#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

//! # Access Control & Vision Records Cross-Contract Integration Tests
//!
//! Verifies:
//! 1. Assign role in `AccessControl`, verify access in `VisionRecords`
//! 2. Revoke role in `AccessControl`, verify access denied in `VisionRecords`
//! 3. Cross-contract permission delegation (role delegation & scoped permissions)
//! 4. Emergency access override scenarios
//! 5. Permission cascading between contracts

extern crate std;

use access_control::{
    AccessControlContract, AccessControlContractClient, Permission as AcPermission, Role as AcRole,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};
use vision_records::{
    Certification, EmergencyCondition, EmergencyStatus, License, Location,
    Permission as VrPermission, VerificationStatus, VisionRecordsContract,
    VisionRecordsContractClient,
};

struct TestFixture<'a> {
    env: Env,
    admin: Address,
    ac_client: AccessControlContractClient<'a>,
    vr_client: VisionRecordsContractClient<'a>,
    ac_id: Address,
    vr_id: Address,
}

fn setup_integration_fixture() -> TestFixture<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Deploy AccessControl micro-contract
    let ac_id = env.register(AccessControlContract, ());
    let ac_client = AccessControlContractClient::new(&env, &ac_id);
    ac_client.initialize(&admin);

    // Deploy VisionRecords contract
    let vr_id = env.register(VisionRecordsContract, ());
    let vr_client = VisionRecordsContractClient::new(&env, &vr_id);
    vr_client.initialize(&admin);

    // Link AccessControl to VisionRecords
    vr_client.set_access_control(&admin, &ac_id);

    TestFixture {
        env,
        admin,
        ac_client,
        vr_client,
        ac_id,
        vr_id,
    }
}

fn register_and_verify_provider(
    env: &Env,
    vr_client: &VisionRecordsContractClient,
    admin: &Address,
    provider: &Address,
) {
    let name = String::from_str(env, "Dr. Integration Provider");
    let mut licenses = Vec::new(env);
    licenses.push_back(License {
        number: String::from_str(env, "LIC999"),
        issuing_authority: String::from_str(env, "Board"),
        issued_date: 100,
        expiry_date: 2000,
        license_type: String::from_str(env, "Ophthalmology"),
    });

    let mut specialties = Vec::new(env);
    specialties.push_back(String::from_str(env, "Retina Specialist"));

    let mut certifications = Vec::new(env);
    certifications.push_back(Certification {
        name: String::from_str(env, "Fellowship"),
        issuer: String::from_str(env, "Board"),
        issued_date: 100,
        expiry_date: 2000,
        credential_id: String::from_str(env, "FEL-1"),
    });

    let mut locations = Vec::new(env);
    locations.push_back(Location {
        name: String::from_str(env, "Main Clinic"),
        address: String::from_str(env, "100 Medical Way"),
        city: String::from_str(env, "Metropolis"),
        state: String::from_str(env, "NY"),
        zip: String::from_str(env, "10001"),
        country: String::from_str(env, "USA"),
    });

    vr_client.register_provider(
        admin,
        provider,
        &name,
        &licenses,
        &specialties,
        &certifications,
        &locations,
    );
    vr_client.verify_provider(admin, provider, &VerificationStatus::Verified);
}

#[test]
fn test_cross_contract_role_assignment_and_verification() {
    let f = setup_integration_fixture();
    let doctor = Address::generate(&f.env);

    // Pre-condition: Configuration is verified across contracts
    assert_eq!(f.vr_client.get_access_control(), Some(f.ac_id));

    // Initially doctor has no permissions in VisionRecords
    assert!(
        !f.vr_client
            .check_permission(&doctor, &VrPermission::ReadAnyRecord),
        "Unassigned user should not have ReadAnyRecord"
    );
    assert!(
        !f.vr_client
            .check_permission(&doctor, &VrPermission::WriteRecord),
        "Unassigned user should not have WriteRecord"
    );

    // Assign Ophthalmologist role in AccessControl
    let expires_at = f.env.ledger().timestamp() + 86400;
    f.ac_client
        .assign_role(&f.admin, &doctor, &AcRole::Ophthalmologist, &expires_at);

    assert_eq!(f.ac_client.get_role(&doctor), AcRole::Ophthalmologist);

    // VisionRecords queries AccessControl cross-contract and confirms permissions
    assert!(
        f.vr_client
            .check_permission(&doctor, &VrPermission::ReadAnyRecord),
        "Ophthalmologist should have ReadAnyRecord in VisionRecords"
    );
    assert!(
        f.vr_client
            .check_permission(&doctor, &VrPermission::WriteRecord),
        "Ophthalmologist should have WriteRecord in VisionRecords"
    );
    assert!(
        f.vr_client
            .check_permission(&doctor, &VrPermission::ManageAccess),
        "Ophthalmologist should have ManageAccess in VisionRecords"
    );
    assert!(
        f.vr_client
            .check_permission(&doctor, &VrPermission::ManageUsers),
        "Ophthalmologist should have ManageUsers in VisionRecords"
    );
    assert!(
        !f.vr_client
            .check_permission(&doctor, &VrPermission::SystemAdmin),
        "Ophthalmologist should not have SystemAdmin in VisionRecords"
    );
}

#[test]
fn test_cross_contract_role_revocation() {
    let f = setup_integration_fixture();
    let doctor = Address::generate(&f.env);

    let expires_at = f.env.ledger().timestamp() + 86400;
    f.ac_client
        .assign_role(&f.admin, &doctor, &AcRole::Optometrist, &expires_at);

    // Verify granted
    assert!(f
        .vr_client
        .check_permission(&doctor, &VrPermission::ReadAnyRecord));
    assert!(f
        .vr_client
        .check_permission(&doctor, &VrPermission::WriteRecord));

    // Revoke role in AccessControl
    f.ac_client.revoke_role(&f.admin, &doctor);

    // Permission check immediately cascades and denies access in VisionRecords
    assert!(
        !f.vr_client
            .check_permission(&doctor, &VrPermission::ReadAnyRecord),
        "Revoked role must deny ReadAnyRecord in VisionRecords"
    );
    assert!(
        !f.vr_client
            .check_permission(&doctor, &VrPermission::WriteRecord),
        "Revoked role must deny WriteRecord in VisionRecords"
    );
}

#[test]
fn test_cross_contract_permission_delegation() {
    let f = setup_integration_fixture();
    let doctor = Address::generate(&f.env);
    let assistant = Address::generate(&f.env);

    let expires_at = f.env.ledger().timestamp() + 3600;

    // Assign doctor role
    f.ac_client
        .assign_role(&f.admin, &doctor, &AcRole::Ophthalmologist, &expires_at);

    // Doctor delegates complete Optometrist role to assistant
    f.ac_client
        .delegate_role(&doctor, &assistant, &AcRole::Optometrist, &expires_at);

    // Assistant now holds delegated permissions
    assert!(
        f.ac_client
            .has_delegated_permission(&doctor, &assistant, &AcPermission::ReadAnyRecord),
        "Assistant should hold delegated ReadAnyRecord"
    );
    assert!(
        f.ac_client
            .has_delegated_permission(&doctor, &assistant, &AcPermission::WriteRecord),
        "Assistant should hold delegated WriteRecord"
    );

    // Revoke role delegation
    f.ac_client.revoke_delegation(&doctor, &assistant);
    assert!(
        !f.ac_client
            .has_delegated_permission(&doctor, &assistant, &AcPermission::ReadAnyRecord),
        "Delegated permission must be revoked"
    );

    // Scoped permission delegation
    let mut scoped = Vec::new(&f.env);
    scoped.push_back(AcPermission::ReadAnyRecord);

    f.ac_client
        .delegate_permissions(&doctor, &assistant, &scoped, &expires_at);

    assert!(f.ac_client.has_delegated_permission(
        &doctor,
        &assistant,
        &AcPermission::ReadAnyRecord
    ));
    assert!(!f
        .ac_client
        .has_delegated_permission(&doctor, &assistant, &AcPermission::WriteRecord));

    // Revoke scoped delegation
    f.ac_client.revoke_scoped_delegation(&doctor, &assistant);
    assert!(!f.ac_client.has_delegated_permission(
        &doctor,
        &assistant,
        &AcPermission::ReadAnyRecord
    ));
}

#[test]
fn test_permission_cascading_and_custom_permissions() {
    let f = setup_integration_fixture();
    let staff = Address::generate(&f.env);
    let expires_at = f.env.ledger().timestamp() + 86400;

    // Staff role has ManageUsers but not WriteRecord or SystemAdmin
    f.ac_client
        .assign_role(&f.admin, &staff, &AcRole::Staff, &expires_at);

    assert!(f
        .vr_client
        .check_permission(&staff, &VrPermission::ManageUsers));
    assert!(!f
        .vr_client
        .check_permission(&staff, &VrPermission::WriteRecord));

    // Admin grants custom permission WriteRecord to staff
    f.ac_client
        .grant_custom_permission(&f.admin, &staff, &AcPermission::WriteRecord);

    // Cascades into VisionRecords permission query
    assert!(
        f.vr_client
            .check_permission(&staff, &VrPermission::WriteRecord),
        "Custom permission grant must cascade to VisionRecords"
    );

    // Role upgrade from Staff to Admin cascades all admin permissions
    f.ac_client
        .assign_role(&f.admin, &staff, &AcRole::Admin, &expires_at);
    assert!(f
        .vr_client
        .check_permission(&staff, &VrPermission::SystemAdmin));
    assert!(f
        .vr_client
        .check_permission(&staff, &VrPermission::ReadAnyRecord));
}

#[test]
fn test_emergency_access_override_scenarios() {
    let f = setup_integration_fixture();
    let provider = Address::generate(&f.env);
    let patient = Address::generate(&f.env);

    // Register & verify medical provider in VisionRecords
    register_and_verify_provider(&f.env, &f.vr_client, &f.admin, &provider);

    // Provider does not have broad RBAC access to patient records
    assert!(!f
        .vr_client
        .check_permission(&provider, &VrPermission::ReadAnyRecord));

    // Emergency condition occurs: Patient is unconscious
    let attestation = String::from_str(&f.env, "Severe trauma requiring immediate retinal scan");
    let mut contacts = Vec::new(&f.env);
    contacts.push_back(Address::generate(&f.env));

    let duration_secs = 3600u64;
    let access_id = f.vr_client.grant_emergency_access(
        &provider,
        &patient,
        &EmergencyCondition::Unconscious,
        &attestation,
        &duration_secs,
        &contacts,
    );
    assert!(access_id > 0);

    // Verify active emergency grant overrides standard RBAC restriction
    let active_grant = f.vr_client.check_emergency_access(&patient, &provider);
    assert!(active_grant.is_some());
    let grant = active_grant.unwrap();
    assert_eq!(grant.id, access_id);
    assert_eq!(grant.condition, EmergencyCondition::Unconscious);
    assert_eq!(grant.status, EmergencyStatus::Active);

    // Access record via emergency override
    let access_result =
        f.vr_client
            .try_access_record_via_emergency(&provider, &patient, &Some(101));
    assert!(access_result.is_ok());

    // Verify audit trail entry was recorded
    let trail = f.vr_client.get_emergency_audit_trail(&access_id);
    assert_eq!(trail.len(), 1);
    assert_eq!(trail.get(0).unwrap().actor, provider);
}
