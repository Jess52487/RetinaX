use crate::{
    circuit_breaker::PauseScope, emergency, emergency::EmergencyCondition, rbac::Role,
    ContractError, RecordType, VisionRecordsContract, VisionRecordsContractClient,
};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String, Vec};

fn setup_test() -> (Env, VisionRecordsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_global_pause() {
    let (env, client, admin) = setup_test();

    let user = Address::generate(&env);

    // Admin pauses globally
    client.pause_contract(&admin, &PauseScope::Global);

    // Attempt to register a user - should fail
    let res = client.try_register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Test"),
    );
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Paused);

    // Admin resumes globally
    client.resume_contract(&admin, &PauseScope::Global);

    // Should now succeed
    client.register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Test"),
    );
}

#[test]
fn test_granular_pause() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );

    // Admin pauses ONLY `ADD_REC` function
    let add_rec_scope = PauseScope::Function(symbol_short!("ADD_REC"));
    client.pause_contract(&admin, &add_rec_scope);

    // Adding a record should fail
    let hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    let res = client.try_add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Paused);

    // But granting access should still work (it uses `GRT_ACC`)
    client.grant_access(
        &patient,
        &patient,
        &doctor,
        &crate::AccessLevel::Read,
        &3600,
    );

    // Resume `ADD_REC`
    client.resume_contract(&admin, &add_rec_scope);

    // Should now succeed
    client.add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
}

#[test]
fn test_unauthorized_pause() {
    let (env, client, admin) = setup_test();

    let staff = Address::generate(&env);
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    // Staff tries to pause - fails
    let res = client.try_pause_contract(&staff, &PauseScope::Global);
    assert_eq!(res.unwrap_err().unwrap(), ContractError::Unauthorized);
}

#[test]
fn test_global_pause_blocks_all_mutating_ops() {
    let (env, client, admin) = setup_test();

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );

    client.pause_contract(&admin, &PauseScope::Global);

    // Every state-mutating entry point honours the global pause.
    let newcomer = Address::generate(&env);
    assert_eq!(
        client
            .try_register_user(
                &admin,
                &newcomer,
                &Role::Patient,
                &String::from_str(&env, "New")
            )
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );

    let hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    assert_eq!(
        client
            .try_add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash)
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );
    assert_eq!(
        client
            .try_grant_access(
                &patient,
                &patient,
                &doctor,
                &crate::AccessLevel::Read,
                &3600
            )
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );
    // Emergency read path is also pause-gated (no bypass).
    assert_eq!(
        client
            .try_access_record_via_emergency(&doctor, &patient, &None)
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );
}

#[test]
fn test_resume_restores_operations_circuit_breaker_recovery() {
    let (env, client, admin) = setup_test();

    client.pause_contract(&admin, &PauseScope::Global);
    let blocked = Address::generate(&env);
    assert_eq!(
        client
            .try_register_user(
                &admin,
                &blocked,
                &Role::Patient,
                &String::from_str(&env, "Blocked")
            )
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );

    // Recovery: resume and verify the breaker is fully cleared.
    client.resume_contract(&admin, &PauseScope::Global);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );
    let hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    client.add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
    client.grant_access(
        &patient,
        &patient,
        &doctor,
        &crate::AccessLevel::Read,
        &3600,
    );
}

#[test]
fn test_emergency_access_blocked_during_pause_no_bypass() {
    let (env, client, admin) = setup_test();
    let cid = client.address.clone();

    let patient = Address::generate(&env);
    let requester = Address::generate(&env);

    client.pause_contract(&admin, &PauseScope::Global);

    // Documented policy: emergency access does NOT bypass the circuit breaker.
    // Grant and revoke both fail with Paused before any provider/auth checks.
    env.as_contract(&cid, || {
        let att = String::from_str(&env, "unconscious, needs immediate care");
        let contacts = Vec::new(&env);
        assert_eq!(
            emergency::grant_emergency_access(
                &env,
                &requester,
                &patient,
                EmergencyCondition::Unconscious,
                att,
                3600,
                contacts
            ),
            Err(ContractError::Paused)
        );
        assert_eq!(
            emergency::revoke_emergency_access(&env, &requester, 1),
            Err(ContractError::Paused)
        );
    });
    assert_eq!(
        client
            .try_access_record_via_emergency(&requester, &patient, &None)
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );

    // After resume the pause is no longer the blocker: grant now reaches the
    // provider check (Unauthorized for an unverified requester) and the read
    // path reaches the access check (AccessDenied with no active grant).
    client.resume_contract(&admin, &PauseScope::Global);
    env.as_contract(&cid, || {
        let att = String::from_str(&env, "unconscious, needs immediate care");
        let contacts = Vec::new(&env);
        assert_eq!(
            emergency::grant_emergency_access(
                &env,
                &requester,
                &patient,
                EmergencyCondition::Unconscious,
                att,
                3600,
                contacts
            ),
            Err(ContractError::Unauthorized)
        );
    });
    assert_eq!(
        client
            .try_access_record_via_emergency(&requester, &patient, &None)
            .unwrap_err()
            .unwrap(),
        ContractError::AccessDenied
    );
}

#[test]
fn test_unauthorized_resume_and_double_pause_resume() {
    let (env, client, admin) = setup_test();

    let staff = Address::generate(&env);
    client.register_user(
        &admin,
        &staff,
        &Role::Staff,
        &String::from_str(&env, "Staff"),
    );

    // Unauthorized resume is rejected just like unauthorized pause.
    assert_eq!(
        client
            .try_resume_contract(&staff, &PauseScope::Global)
            .unwrap_err()
            .unwrap(),
        ContractError::Unauthorized
    );

    // Double-pause is idempotent: still paused, still blocked.
    client.pause_contract(&admin, &PauseScope::Global);
    client.pause_contract(&admin, &PauseScope::Global);
    let user = Address::generate(&env);
    assert_eq!(
        client
            .try_register_user(
                &admin,
                &user,
                &Role::Patient,
                &String::from_str(&env, "Xena")
            )
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );

    // Double-resume is idempotent: still unpaused, operations resume.
    client.resume_contract(&admin, &PauseScope::Global);
    client.resume_contract(&admin, &PauseScope::Global);
    client.register_user(
        &admin,
        &user,
        &Role::Patient,
        &String::from_str(&env, "Xena"),
    );

    // Function-scope double-pause/resume is idempotent too.
    let scope = PauseScope::Function(symbol_short!("ADD_REC"));
    client.pause_contract(&admin, &scope);
    client.pause_contract(&admin, &scope);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    client.register_user(
        &admin,
        &patient,
        &Role::Patient,
        &String::from_str(&env, "Pat"),
    );
    client.register_user(
        &admin,
        &doctor,
        &Role::Optometrist,
        &String::from_str(&env, "Doc"),
    );
    let hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    assert_eq!(
        client
            .try_add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash)
            .unwrap_err()
            .unwrap(),
        ContractError::Paused
    );
    client.resume_contract(&admin, &scope);
    client.resume_contract(&admin, &scope);
    client.add_record(&doctor, &patient, &doctor, &RecordType::Examination, &hash);
}
