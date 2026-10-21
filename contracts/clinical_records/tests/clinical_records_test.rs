#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use access_control::{AccessControlContract, AccessControlContractClient, Role};
use clinical_records::{
    prescription::MAX_PRESCRIPTION_BATCH, BatchPrescriptionInput, ClinicalRecordsContract,
    ClinicalRecordsContractClient, ContractError, EyeExamination, FundusPhotography,
    IntraocularPressure, LensType, OptFundusPhotography, OptPhysicalMeasurement, OptRetinalImaging,
    OptVisualField, PhysicalMeasurement, Prescription, PrescriptionData, RetinalImaging,
    SlitLampFindings, VisualAcuity, VisualField,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Bytes, Env, String, Vec,
};

fn setup_clinical() -> (
    Env,
    ClinicalRecordsContractClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);

    let contract_id = env.register(ClinicalRecordsContract, ());
    let client = ClinicalRecordsContractClient::new(&env, &contract_id);
    client.initialize(&admin, &vision_records, &None);

    (env, client, admin, vision_records)
}

fn sample_visual_acuity_full(env: &Env) -> VisualAcuity {
    VisualAcuity {
        uncorrected: PhysicalMeasurement {
            left_eye: String::from_str(env, "20/40"),
            right_eye: String::from_str(env, "20/30"),
        },
        corrected: OptPhysicalMeasurement::Some(PhysicalMeasurement {
            left_eye: String::from_str(env, "20/20"),
            right_eye: String::from_str(env, "20/20"),
        }),
    }
}

fn sample_visual_acuity_uncorrected_only(env: &Env) -> VisualAcuity {
    VisualAcuity {
        uncorrected: PhysicalMeasurement {
            left_eye: String::from_str(env, "20/20"),
            right_eye: String::from_str(env, "20/20"),
        },
        corrected: OptPhysicalMeasurement::None,
    }
}

fn sample_iop(env: &Env) -> IntraocularPressure {
    IntraocularPressure {
        left_eye: 16,
        right_eye: 15,
        method: String::from_str(env, "Goldmann Applanation"),
        timestamp: 1_700_000_000,
    }
}

fn sample_slit_lamp(env: &Env) -> SlitLampFindings {
    SlitLampFindings {
        cornea: String::from_str(env, "Clear"),
        anterior_chamber: String::from_str(env, "Deep and quiet"),
        iris: String::from_str(env, "Normal architecture"),
        lens: String::from_str(env, "Clear, no cataract"),
    }
}

fn sample_prescription_data(env: &Env) -> PrescriptionData {
    PrescriptionData {
        sphere: String::from_str(env, "-2.25"),
        cylinder: String::from_str(env, "-0.75"),
        axis: String::from_str(env, "180"),
        add: String::from_str(env, "+1.50"),
        pd: String::from_str(env, "63"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Initialization and Admin Functions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_initialization_lifecycle_and_admin_config() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClinicalRecordsContract, ());
    let client = ClinicalRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);

    // Initial configuration succeeds
    let init_res = client.try_initialize(&admin, &vision_records, &None);
    assert!(init_res.is_ok());

    assert_eq!(client.get_admin(), admin);
    assert_eq!(
        client.get_vision_records_contract(),
        Some(vision_records.clone())
    );

    // Duplicate initialization must return AlreadyInitialized
    let dup_init = client.try_initialize(&admin, &vision_records, &None);
    assert_eq!(dup_init, Err(Ok(ContractError::AlreadyInitialized)));

    // Admin updates vision_records contract address
    let new_vr = Address::generate(&env);
    let update_res = client.try_set_vision_records_contract(&admin, &new_vr);
    assert!(update_res.is_ok());
    assert_eq!(client.get_vision_records_contract(), Some(new_vr.clone()));

    // Non-admin attempting to update vision_records contract address fails
    let unauthorized_caller = Address::generate(&env);
    let unauth_res = client.try_set_vision_records_contract(&unauthorized_caller, &vision_records);
    assert_eq!(unauth_res, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_uninitialized_calls_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClinicalRecordsContract, ());
    let client = ClinicalRecordsContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_get_admin(),
        Err(Ok(ContractError::NotInitialized))
    );
    assert_eq!(client.get_vision_records_contract(), None);

    let caller = Address::generate(&env);
    let exam_res = client.try_get_eye_examination(&caller, &1);
    assert_eq!(exam_res, Err(Ok(ContractError::NotInitialized)));

    let rx_res = client.try_get_prescription(&caller, &1);
    assert_eq!(rx_res, Err(Ok(ContractError::NotInitialized)));
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Eye Examination CRUD & Optional Fields (None / Some Variants)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_eye_examination_crud_with_all_some_fields() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let record_id = 1001u64;

    let visual_field = OptVisualField::Some(VisualField {
        left_eye_reliability: String::from_str(&env, "Good"),
        right_eye_reliability: String::from_str(&env, "Good"),
        left_eye_defects: String::from_str(&env, "None"),
        right_eye_defects: String::from_str(&env, "None"),
    });

    let retina_imaging = OptRetinalImaging::Some(RetinalImaging {
        image_url: String::from_str(&env, "https://storage.retinax.io/oct/1001.png"),
        image_hash: String::from_str(&env, "0xabcd1234"),
        findings: String::from_str(&env, "Normal macula, intact nerve fiber layer"),
    });

    let fundus_photo = OptFundusPhotography::Some(FundusPhotography {
        image_url: String::from_str(&env, "https://storage.retinax.io/fundus/1001.png"),
        image_hash: String::from_str(&env, "0xfeed5678"),
        cup_to_disc_ratio_left: String::from_str(&env, "0.3"),
        cup_to_disc_ratio_right: String::from_str(&env, "0.3"),
        macula_status: String::from_str(&env, "Healthy"),
    });

    let notes = String::from_str(&env, "Routine comprehensive exam complete.");

    // Store eye examination
    let add_res = client.try_add_eye_examination(
        &vision_records,
        &record_id,
        &sample_visual_acuity_full(&env),
        &sample_iop(&env),
        &sample_slit_lamp(&env),
        &visual_field,
        &retina_imaging,
        &fundus_photo,
        &notes,
    );
    assert!(add_res.is_ok());

    // Retrieve and verify
    let stored: EyeExamination = client.get_eye_examination(&vision_records, &record_id);
    assert_eq!(stored.record_id, record_id);
    assert_eq!(stored.clinical_notes, notes);
    assert_eq!(stored.visual_field, visual_field);
    assert_eq!(stored.retina_imaging, retina_imaging);
    assert_eq!(stored.fundus_photo, fundus_photo);
}

#[test]
fn test_eye_examination_crud_with_all_none_fields() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let record_id = 1002u64;

    let notes = String::from_str(&env, "Basic exam without advanced imaging.");

    let add_res = client.try_add_eye_examination(
        &vision_records,
        &record_id,
        &sample_visual_acuity_uncorrected_only(&env),
        &sample_iop(&env),
        &sample_slit_lamp(&env),
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &notes,
    );
    assert!(add_res.is_ok());

    let stored = client.get_eye_examination(&vision_records, &record_id);
    assert_eq!(stored.record_id, record_id);
    assert_eq!(stored.visual_field, OptVisualField::None);
    assert_eq!(stored.retina_imaging, OptRetinalImaging::None);
    assert_eq!(stored.fundus_photo, OptFundusPhotography::None);
    assert_eq!(stored.visual_acuity.corrected, OptPhysicalMeasurement::None);
}

#[test]
fn test_eye_examination_unauthorized_and_not_found() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let unauthorized_caller = Address::generate(&env);

    // Unauthorized add_eye_examination
    let add_res = client.try_add_eye_examination(
        &unauthorized_caller,
        &1,
        &sample_visual_acuity_uncorrected_only(&env),
        &sample_iop(&env),
        &sample_slit_lamp(&env),
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Notes"),
    );
    assert_eq!(add_res, Err(Ok(ContractError::Unauthorized)));

    // Unauthorized get_eye_examination
    let get_res = client.try_get_eye_examination(&unauthorized_caller, &1);
    assert_eq!(get_res, Err(Ok(ContractError::Unauthorized)));

    // RecordNotFound when querying non-existent record
    let not_found_res = client.try_get_eye_examination(&vision_records, &9999);
    assert_eq!(not_found_res, Err(Ok(ContractError::RecordNotFound)));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Prescription 2-Phase Commit Full Lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_prescription_two_phase_commit_lifecycle() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_data = sample_prescription_data(&env);

    assert_eq!(client.get_prescription_count(), 0);

    // Phase 1: Prepare
    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    assert_eq!(rx_id, 1);
    assert_eq!(client.get_prescription_count(), 1);

    // Prescription cannot be queried before commit
    let pre_commit_res = client.try_get_prescription(&vision_records, &rx_id);
    assert_eq!(pre_commit_res, Err(Ok(ContractError::RecordNotFound)));

    // Phase 2: Commit
    let commit_res = client.try_commit_add_prescription(&vision_records, &rx_id);
    assert!(commit_res.is_ok());

    // Verified stored prescription
    let prescription: Prescription = client.get_prescription(&vision_records, &rx_id);
    assert_eq!(prescription.id, rx_id);
    assert_eq!(prescription.patient, patient);
    assert_eq!(prescription.provider, provider);
    assert_eq!(prescription.lens_type, LensType::Glasses);
    assert_eq!(prescription.left_eye, rx_data);
    assert_eq!(prescription.right_eye, rx_data);
    assert!(!prescription.verified);
}

#[test]
fn test_prescription_double_commit_prevention() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_data = sample_prescription_data(&env);

    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);

    // First commit succeeds
    assert!(client
        .try_commit_add_prescription(&vision_records, &rx_id)
        .is_ok());

    // Second commit must fail with InvalidPhase
    let double_commit = client.try_commit_add_prescription(&vision_records, &rx_id);
    assert_eq!(double_commit, Err(Ok(ContractError::InvalidPhase)));
}

#[test]
fn test_prescription_rollback_lifecycle() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_data = sample_prescription_data(&env);

    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);

    // Rollback phase 1 preparation
    let rollback_res = client.try_rollback_add_prescription(&vision_records, &rx_id);
    assert!(rollback_res.is_ok());

    // Attempting to commit after rollback must fail with InvalidPhase
    let commit_after_rollback = client.try_commit_add_prescription(&vision_records, &rx_id);
    assert_eq!(commit_after_rollback, Err(Ok(ContractError::InvalidPhase)));

    // Prescription is not in persistent storage
    assert_eq!(
        client.try_get_prescription(&vision_records, &rx_id),
        Err(Ok(ContractError::RecordNotFound))
    );
}

#[test]
fn test_prescription_rollback_after_commit_does_not_destroy_committed_data() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_data = sample_prescription_data(&env);

    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    client.commit_add_prescription(&vision_records, &rx_id);

    // Calling rollback after commit cleans up or succeeds idempotently
    assert!(client
        .try_rollback_add_prescription(&vision_records, &rx_id)
        .is_ok());

    // Committed prescription remains intact in persistent storage
    let fetched = client.get_prescription(&vision_records, &rx_id);
    assert_eq!(fetched.id, rx_id);
}

#[test]
fn test_prescription_unauthorized_attempts() {
    let (env, client, _admin, _vr) = setup_clinical();
    let unauthorized_caller = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_data = sample_prescription_data(&env);

    assert_eq!(
        client.try_prepare_add_prescription(&unauthorized_caller, &patient, &provider, &rx_data),
        Err(Ok(ContractError::Unauthorized))
    );

    assert_eq!(
        client.try_commit_add_prescription(&unauthorized_caller, &1),
        Err(Ok(ContractError::Unauthorized))
    );

    assert_eq!(
        client.try_rollback_add_prescription(&unauthorized_caller, &1),
        Err(Ok(ContractError::Unauthorized))
    );

    assert_eq!(
        client.try_get_prescription(&unauthorized_caller, &1),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn test_input_validation_errors() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Empty sphere value (< 1 char)
    let invalid_rx_data = PrescriptionData {
        sphere: String::from_str(&env, ""),
        cylinder: String::from_str(&env, "-1.00"),
        axis: String::from_str(&env, "90"),
        add: String::from_str(&env, "0.00"),
        pd: String::from_str(&env, "60"),
    };

    let res =
        client.try_prepare_add_prescription(&vision_records, &patient, &provider, &invalid_rx_data);
    assert_eq!(res, Err(Ok(ContractError::InvalidInput)));
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Events Emission Tests (#268)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_events_emission_on_all_state_changing_operations() {
    use clinical_records::events::{
        EyeExaminationAddedEvent, PrescriptionCommittedEvent, PrescriptionPreparedEvent,
        PrescriptionRolledBackEvent,
    };
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::{symbol_short, IntoVal, TryFromVal};

    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // 1. add_eye_examination emits EXAM_ADD event
    let record_id = 42u64;
    let notes = String::from_str(&env, "Event test exam");
    client.add_eye_examination(
        &vision_records,
        &record_id,
        &sample_visual_acuity_uncorrected_only(&env),
        &sample_iop(&env),
        &sample_slit_lamp(&env),
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &notes,
    );

    let binding1 = env.events().all();
    let all_events = binding1.events();
    assert_eq!(all_events.len(), 1);
    let last_event = all_events.last().unwrap();
    let ContractEventBody::V0(exam_body) = &last_event.body;
    assert_eq!(
        exam_body.topics.first().unwrap(),
        &symbol_short!("EXAM_ADD").into_val(&env)
    );
    let exam_data = EyeExaminationAddedEvent::try_from_val(&env, &exam_body.data).unwrap();
    assert_eq!(exam_data.record_id, record_id);
    assert_eq!(exam_data.caller, vision_records);

    // 2. prepare_add_prescription emits RX_PREP event
    let rx_data = sample_prescription_data(&env);
    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    let binding2 = env.events().all();
    let all_events = binding2.events();
    assert_eq!(all_events.len(), 1);
    let last_event = all_events.last().unwrap();
    let ContractEventBody::V0(prep_body) = &last_event.body;
    assert_eq!(
        prep_body.topics.first().unwrap(),
        &symbol_short!("RX_PREP").into_val(&env)
    );
    let prep_data = PrescriptionPreparedEvent::try_from_val(&env, &prep_body.data).unwrap();
    assert_eq!(prep_data.rx_id, rx_id);
    assert_eq!(prep_data.patient, patient);
    assert_eq!(prep_data.provider, provider);

    // 3. commit_add_prescription emits RX_CMT event
    client.commit_add_prescription(&vision_records, &rx_id);
    let binding3 = env.events().all();
    let all_events = binding3.events();
    assert_eq!(all_events.len(), 1);
    let last_event = all_events.last().unwrap();
    let ContractEventBody::V0(cmt_body) = &last_event.body;
    assert_eq!(
        cmt_body.topics.first().unwrap(),
        &symbol_short!("RX_CMT").into_val(&env)
    );
    let cmt_data = PrescriptionCommittedEvent::try_from_val(&env, &cmt_body.data).unwrap();
    assert_eq!(cmt_data.rx_id, rx_id);
    assert_eq!(cmt_data.caller, vision_records);

    // 4. prepare and rollback_add_prescription emits RX_RBK event
    let rx_id_2 = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    let binding4 = env.events().all();
    let all_events = binding4.events();
    assert_eq!(all_events.len(), 1);

    client.rollback_add_prescription(&vision_records, &rx_id_2);
    let binding5 = env.events().all();
    let all_events = binding5.events();
    assert_eq!(all_events.len(), 1);
    let last_event = all_events.last().unwrap();
    let ContractEventBody::V0(rbk_body) = &last_event.body;
    assert_eq!(
        rbk_body.topics.first().unwrap(),
        &symbol_short!("RX_RBK").into_val(&env)
    );
    let rbk_data = PrescriptionRolledBackEvent::try_from_val(&env, &rbk_body.data).unwrap();
    assert_eq!(rbk_data.rx_id, rx_id_2);
    assert_eq!(rbk_data.caller, vision_records);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Access Control (RBAC) Integration Tests
// ─────────────────────────────────────────────────────────────────────────────

fn setup_clinical_with_access_control() -> (
    Env,
    ClinicalRecordsContractClient<'static>,
    AccessControlContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);
    let doctor = Address::generate(&env);
    let patient = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    let ac_id = env.register(AccessControlContract, ());
    let ac_client = AccessControlContractClient::new(&env, &ac_id);
    ac_client.initialize(&admin);

    // Assign Optometrist role to doctor (gives WriteRecord + ReadAnyRecord)
    ac_client.assign_role(&admin, &doctor, &Role::Optometrist, &0);
    // Assign Patient role to patient (no global WriteRecord or ReadAnyRecord)
    ac_client.assign_role(&admin, &patient, &Role::Patient, &0);

    let cr_id = env.register(ClinicalRecordsContract, ());
    let cr_client = ClinicalRecordsContractClient::new(&env, &cr_id);
    cr_client.initialize(&admin, &vision_records, &Some(ac_id.clone()));

    (
        env,
        cr_client,
        ac_client,
        admin,
        vision_records,
        doctor,
        patient,
        unauthorized,
    )
}

#[test]
fn test_access_control_rbac_eye_examination() {
    let (env, client, _, _admin, _, doctor, patient, unauthorized) =
        setup_clinical_with_access_control();
    let record_id = 5001u64;

    let visual_acuity = sample_visual_acuity_uncorrected_only(&env);
    let iop = sample_iop(&env);
    let slit_lamp = sample_slit_lamp(&env);
    let notes = String::from_str(&env, "Routine diabetic eye check");

    // 1. Caller with WriteRecord (doctor) can add eye examination
    let add_res = client.try_add_eye_examination(
        &doctor,
        &record_id,
        &visual_acuity,
        &iop,
        &slit_lamp,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &notes,
    );
    assert!(add_res.is_ok());

    // Associate patient with this examination
    client.set_examination_patient(&doctor, &record_id, &patient);

    // 2. Caller without WriteRecord cannot add eye examination
    let unauth_add = client.try_add_eye_examination(
        &unauthorized,
        &5002u64,
        &visual_acuity,
        &iop,
        &slit_lamp,
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &notes,
    );
    assert_eq!(unauth_add, Err(Ok(ContractError::Unauthorized)));

    // 3. Caller with ReadAnyRecord (doctor) can read examination
    let doc_read = client.get_eye_examination(&doctor, &record_id);
    assert_eq!(doc_read.record_id, record_id);

    // 4. Patient of this examination can read examination
    let pat_read = client.get_eye_examination(&patient, &record_id);
    assert_eq!(pat_read.record_id, record_id);

    // 5. Unauthorized caller (neither ReadAnyRecord nor the patient) is rejected
    let unauth_read = client.try_get_eye_examination(&unauthorized, &record_id);
    assert_eq!(unauth_read, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_access_control_rbac_prescription() {
    let (env, client, _, _admin, _, doctor, patient, unauthorized) =
        setup_clinical_with_access_control();

    let rx_data = PrescriptionData {
        sphere: String::from_str(&env, "-1.50"),
        cylinder: String::from_str(&env, "-0.25"),
        axis: String::from_str(&env, "180"),
        add: String::from_str(&env, "+1.50"),
        pd: String::from_str(&env, "63"),
    };

    // 1. Caller with WriteRecord (doctor) can prepare prescription
    let rx_id = client.prepare_add_prescription(&doctor, &patient, &doctor, &rx_data);

    // 2. Caller without WriteRecord cannot prepare prescription
    let unauth_prep =
        client.try_prepare_add_prescription(&unauthorized, &patient, &doctor, &rx_data);
    assert_eq!(unauth_prep, Err(Ok(ContractError::Unauthorized)));

    // 3. Caller with WriteRecord can commit prescription
    let commit_res = client.try_commit_add_prescription(&doctor, &rx_id);
    assert!(commit_res.is_ok());

    // 4. Caller with ReadAnyRecord (doctor) can read prescription
    let doc_rx = client.get_prescription(&doctor, &rx_id);
    assert_eq!(doc_rx.id, rx_id);
    assert_eq!(doc_rx.patient, patient);

    // 5. Prescription patient can read their own prescription
    let pat_rx = client.get_prescription(&patient, &rx_id);
    assert_eq!(pat_rx.id, rx_id);

    // 6. Unauthorized caller cannot read prescription
    let unauth_read = client.try_get_prescription(&unauthorized, &rx_id);
    assert_eq!(unauth_read, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_access_control_configuration_and_updates() {
    let (env, client, ac_client, admin, _, doctor, _, unauthorized) =
        setup_clinical_with_access_control();

    assert_eq!(
        client.get_access_control_contract(),
        Some(ac_client.address.clone())
    );

    // Admin updates access control contract address
    let new_ac = Address::generate(&env);
    let update_res = client.try_set_access_control_contract(&admin, &new_ac);
    assert!(update_res.is_ok());
    assert_eq!(client.get_access_control_contract(), Some(new_ac));

    // Non-admin cannot update access control contract address
    let unauth_update = client.try_set_access_control_contract(&doctor, &unauthorized);
    assert_eq!(unauth_update, Err(Ok(ContractError::Unauthorized)));
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Batch Prescription Operations (#272)
// ─────────────────────────────────────────────────────────────────────────────

fn batch_inputs(
    env: &Env,
    patient: &Address,
    provider: &Address,
    count: u32,
) -> Vec<BatchPrescriptionInput> {
    let mut inputs = Vec::new(env);
    for _ in 0..count {
        inputs.push_back(BatchPrescriptionInput {
            patient: patient.clone(),
            provider: provider.clone(),
            prescription_data: sample_prescription_data(env),
        });
    }
    inputs
}

#[test]
fn test_batch_prescription_prepare_and_commit() {
    let (env, client, _admin, vision_records) = setup_clinical();
    env.ledger().set_timestamp(1_700_000_000);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let rx_ids = client
        .prepare_add_prescriptions(&vision_records, &batch_inputs(&env, &patient, &provider, 3));
    // One RX_PREP event per prescription, so existing indexers keep working.
    assert_eq!(env.events().all().events().len(), 3);
    assert_eq!(rx_ids, vec![&env, 1u64, 2, 3]);
    assert_eq!(client.get_prescription_count(), 3);
    // Prepared prescriptions are not readable until committed.
    assert_eq!(
        client.try_get_prescription(&vision_records, &1),
        Err(Ok(ContractError::RecordNotFound))
    );

    client.commit_add_prescriptions(&vision_records, &rx_ids);
    assert_eq!(env.events().all().events().len(), 3);
    for rx_id in rx_ids.iter() {
        let rx = client.get_prescription(&vision_records, &rx_id);
        assert_eq!(rx.id, rx_id);
        assert_eq!(rx.patient, patient);
        assert_eq!(rx.provider, provider);
        assert_eq!(rx.lens_type, LensType::Glasses);
        assert_eq!(rx.right_eye, sample_prescription_data(&env));
        assert_eq!(rx.issued_at, 1_700_000_000);
        assert_eq!(rx.expires_at, 1_700_000_000 + 31_536_000);
    }

    // The pending entries were consumed, so the batch cannot be committed twice.
    assert_eq!(
        client.try_commit_add_prescriptions(&vision_records, &rx_ids),
        Err(Ok(ContractError::InvalidPhase))
    );
}

#[test]
fn test_batch_prescription_prepare_is_atomic() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let mut inputs = batch_inputs(&env, &patient, &provider, 3);
    let mut invalid = inputs.get(1).unwrap();
    invalid.prescription_data.axis = String::from_str(&env, "");
    inputs.set(1, invalid);

    let res = client.try_prepare_add_prescriptions(&vision_records, &inputs);
    assert_eq!(res, Err(Ok(ContractError::InvalidInput)));

    // The valid first entry was rolled back with the rest and no id was consumed.
    assert_eq!(client.get_prescription_count(), 0);
    assert_eq!(
        client.try_commit_add_prescription(&vision_records, &1),
        Err(Ok(ContractError::InvalidPhase))
    );
    let next_id = client.prepare_add_prescription(
        &vision_records,
        &patient,
        &provider,
        &sample_prescription_data(&env),
    );
    assert_eq!(next_id, 1);
}

#[test]
fn test_batch_prescription_commit_is_atomic() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    client.prepare_add_prescriptions(&vision_records, &batch_inputs(&env, &patient, &provider, 2));

    // An unknown id fails the whole batch, including the valid id before it.
    let res = client.try_commit_add_prescriptions(&vision_records, &vec![&env, 1u64, 99]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPhase)));
    assert_eq!(
        client.try_get_prescription(&vision_records, &1),
        Err(Ok(ContractError::RecordNotFound))
    );

    // A repeated id fails because its first occurrence already consumed the pending entry.
    let res = client.try_commit_add_prescriptions(&vision_records, &vec![&env, 2u64, 2]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPhase)));
    assert_eq!(
        client.try_get_prescription(&vision_records, &2),
        Err(Ok(ContractError::RecordNotFound))
    );

    // Both prescriptions are still pending and commit together.
    client.commit_add_prescriptions(&vision_records, &vec![&env, 1u64, 2]);
    assert_eq!(client.get_prescription(&vision_records, &1).id, 1);
    assert_eq!(client.get_prescription(&vision_records, &2).id, 2);
}

#[test]
fn test_batch_prescription_rollback() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    client.prepare_add_prescriptions(&vision_records, &batch_inputs(&env, &patient, &provider, 3));

    client.rollback_add_prescriptions(&vision_records, &vec![&env, 1u64, 2]);
    assert_eq!(env.events().all().events().len(), 2);
    assert_eq!(
        client.try_commit_add_prescriptions(&vision_records, &vec![&env, 1u64]),
        Err(Ok(ContractError::InvalidPhase))
    );

    // Id 1 is no longer pending, so the batch fails and id 3 stays pending.
    let res = client.try_rollback_add_prescriptions(&vision_records, &vec![&env, 3u64, 1]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPhase)));
    client.commit_add_prescriptions(&vision_records, &vec![&env, 3u64]);

    // A committed prescription is not pending and cannot be rolled back by a batch.
    let res = client.try_rollback_add_prescriptions(&vision_records, &vec![&env, 3u64]);
    assert_eq!(res, Err(Ok(ContractError::InvalidPhase)));
    assert_eq!(client.get_prescription(&vision_records, &3).id, 3);
}

#[test]
fn test_batch_prescription_size_limits() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let no_ids: Vec<u64> = Vec::new(&env);

    let empty = batch_inputs(&env, &patient, &provider, 0);
    assert_eq!(
        client.try_prepare_add_prescriptions(&vision_records, &empty),
        Err(Ok(ContractError::InvalidInput))
    );
    assert_eq!(
        client.try_commit_add_prescriptions(&vision_records, &no_ids),
        Err(Ok(ContractError::InvalidInput))
    );
    assert_eq!(
        client.try_rollback_add_prescriptions(&vision_records, &no_ids),
        Err(Ok(ContractError::InvalidInput))
    );

    let oversized = batch_inputs(&env, &patient, &provider, MAX_PRESCRIPTION_BATCH + 1);
    assert_eq!(
        client.try_prepare_add_prescriptions(&vision_records, &oversized),
        Err(Ok(ContractError::InvalidInput))
    );

    let full = batch_inputs(&env, &patient, &provider, MAX_PRESCRIPTION_BATCH);
    let rx_ids = client.prepare_add_prescriptions(&vision_records, &full);
    assert_eq!(rx_ids.len(), MAX_PRESCRIPTION_BATCH);

    let mut too_many_ids = rx_ids.clone();
    too_many_ids.push_back(MAX_PRESCRIPTION_BATCH as u64 + 1);
    assert_eq!(
        client.try_commit_add_prescriptions(&vision_records, &too_many_ids),
        Err(Ok(ContractError::InvalidInput))
    );
    client.commit_add_prescriptions(&vision_records, &rx_ids);
}

#[test]
fn test_batch_prescription_authorization() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let outsider = Address::generate(&env);
    let inputs = batch_inputs(&env, &patient, &provider, 2);

    assert_eq!(
        client.try_prepare_add_prescriptions(&outsider, &inputs),
        Err(Ok(ContractError::Unauthorized))
    );
    let rx_ids = client.prepare_add_prescriptions(&vision_records, &inputs);
    assert_eq!(
        client.try_commit_add_prescriptions(&outsider, &rx_ids),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_rollback_add_prescriptions(&outsider, &rx_ids),
        Err(Ok(ContractError::Unauthorized))
    );

    // With access_control configured, WriteRecord holders can batch and others cannot.
    let (env, client, _, _admin, _, doctor, patient, _) = setup_clinical_with_access_control();
    let inputs = batch_inputs(&env, &patient, &doctor, 2);
    assert_eq!(
        client.try_prepare_add_prescriptions(&patient, &inputs),
        Err(Ok(ContractError::Unauthorized))
    );
    let rx_ids = client.prepare_add_prescriptions(&doctor, &inputs);
    client.commit_add_prescriptions(&doctor, &rx_ids);
    let second = client.get_prescription(&patient, &rx_ids.get(1).unwrap());
    assert_eq!(second.patient, patient);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. FHIR R4 Export (#273)
// ─────────────────────────────────────────────────────────────────────────────

const OBSERVATION_CATEGORY: &str = "http://terminology.hl7.org/CodeSystem/observation-category";

/// Decodes an export and checks the rules every FHIR R4 JSON document must satisfy.
fn fhir_json(bytes: &Bytes) -> serde_json::Value {
    let mut raw = std::vec![0u8; bytes.len() as usize];
    bytes.copy_into_slice(&mut raw);
    let text = std::str::from_utf8(&raw).expect("FHIR export must be UTF-8");
    let json: serde_json::Value = serde_json::from_str(text).expect("FHIR export must be JSON");
    assert_fhir_elements(&json);
    let id = json["id"].as_str().unwrap();
    assert!(
        (1..=64).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'),
        "id {id} does not match [A-Za-z0-9\\-\\.]{{1,64}}"
    );
    json
}

/// FHIR JSON never has nulls, empty strings, empty arrays or empty objects (rule ele-1).
fn assert_fhir_elements(value: &serde_json::Value) {
    use serde_json::Value;
    match value {
        Value::Null => panic!("FHIR JSON must not contain null"),
        Value::String(s) => assert!(!s.trim().is_empty(), "FHIR strings must not be empty"),
        Value::Array(items) => {
            assert!(!items.is_empty(), "FHIR arrays must not be empty");
            items.iter().for_each(assert_fhir_elements);
        }
        Value::Object(fields) => {
            assert!(!fields.is_empty(), "FHIR objects must not be empty");
            fields.values().for_each(assert_fhir_elements);
        }
        _ => {}
    }
}

/// Finds the component whose code has the given LOINC code or, for text-only codes, text.
fn component<'a>(observation: &'a serde_json::Value, code: &str) -> &'a serde_json::Value {
    observation["component"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"]["coding"][0]["code"] == code || c["code"]["text"] == code)
        .unwrap_or_else(|| panic!("missing component {code}"))
}

fn add_exam(
    env: &Env,
    client: &ClinicalRecordsContractClient,
    caller: &Address,
    record_id: u64,
    iop: IntraocularPressure,
    visual_field: OptVisualField,
    notes: &str,
) {
    client.add_eye_examination(
        caller,
        &record_id,
        &sample_visual_acuity_full(env),
        &iop,
        &sample_slit_lamp(env),
        &visual_field,
        &OptRetinalImaging::Some(RetinalImaging {
            image_url: String::from_str(env, "https://storage.retinax.io/oct/1.png"),
            image_hash: String::from_str(env, "0xabcd1234"),
            findings: String::from_str(env, "Intact nerve fiber layer"),
        }),
        &OptFundusPhotography::Some(FundusPhotography {
            image_url: String::from_str(env, "https://storage.retinax.io/fundus/1.png"),
            image_hash: String::from_str(env, "0xfeed5678"),
            cup_to_disc_ratio_left: String::from_str(env, "0.3"),
            cup_to_disc_ratio_right: String::from_str(env, "0.4"),
            macula_status: String::from_str(env, "Healthy"),
        }),
        &String::from_str(env, notes),
    );
}

#[test]
fn test_fhir_observation_export_full_examination() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let patient = Address::generate(&env);
    let visual_field = OptVisualField::Some(VisualField {
        left_eye_reliability: String::from_str(&env, "Good"),
        right_eye_reliability: String::from_str(&env, "Fair"),
        left_eye_defects: String::from_str(&env, "None"),
        right_eye_defects: String::from_str(&env, "Superior arcuate"),
    });
    add_exam(
        &env,
        &client,
        &vision_records,
        1001,
        sample_iop(&env),
        visual_field,
        "Routine exam.",
    );
    client.set_examination_patient(&vision_records, &1001, &patient);

    let obs = fhir_json(&client.export_eye_examination_fhir(&vision_records, &1001));

    assert_eq!(obs["resourceType"], "Observation");
    assert_eq!(obs["id"], "eye-exam-1001");
    assert_eq!(obs["status"], "final");
    assert_eq!(
        obs["category"][0]["coding"][0]["system"],
        OBSERVATION_CATEGORY
    );
    assert_eq!(obs["category"][0]["coding"][0]["code"], "exam");
    assert_eq!(obs["code"]["coding"][0]["system"], "http://snomed.info/sct");
    assert_eq!(obs["code"]["coding"][0]["code"], "36228007");
    assert_eq!(
        obs["subject"]["reference"],
        std::format!("Patient/{}", patient.to_string())
    );
    assert_eq!(obs["effectiveDateTime"], "2023-11-14T22:13:20Z");
    assert_eq!(obs["note"][0]["text"], "Routine exam.");

    assert_eq!(obs["component"].as_array().unwrap().len(), 19);
    assert_eq!(
        component(&obs, "98499-7")["code"]["coding"][0]["system"],
        "http://loinc.org"
    );
    assert_eq!(component(&obs, "98499-7")["valueString"], "20/30");
    assert_eq!(component(&obs, "98498-9")["valueString"], "20/40");
    assert_eq!(
        component(&obs, "Visual acuity with correction Right eye")["valueString"],
        "20/20"
    );
    let right_iop = &component(&obs, "79892-6")["valueQuantity"];
    assert_eq!(right_iop["value"], 15);
    assert_eq!(right_iop["unit"], "mmHg");
    assert_eq!(right_iop["system"], "http://unitsofmeasure.org");
    assert_eq!(right_iop["code"], "mm[Hg]");
    assert_eq!(component(&obs, "79893-4")["valueQuantity"]["value"], 16);
    assert_eq!(
        component(&obs, "79764-7")["valueString"],
        "Goldmann Applanation"
    );
    assert_eq!(
        component(&obs, "Slit lamp examination of lens")["valueString"],
        "Clear, no cataract"
    );
    assert_eq!(
        component(&obs, "Visual field defects Right eye")["valueString"],
        "Superior arcuate"
    );
    assert_eq!(
        component(&obs, "Retinal imaging findings")["valueString"],
        "Intact nerve fiber layer"
    );
    assert_eq!(
        component(&obs, "Cup-to-disc ratio Right eye")["valueString"],
        "0.4"
    );
    assert_eq!(component(&obs, "32451-7")["valueString"], "Healthy");
}

#[test]
fn test_fhir_observation_export_omits_absent_data() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let iop = IntraocularPressure {
        timestamp: 0,
        ..sample_iop(&env)
    };
    client.add_eye_examination(
        &vision_records,
        &7,
        &sample_visual_acuity_uncorrected_only(&env),
        &iop,
        &sample_slit_lamp(&env),
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, ""),
    );

    let obs = fhir_json(&client.export_eye_examination_fhir(&vision_records, &7));
    assert!(obs.get("subject").is_none());
    assert!(obs.get("effectiveDateTime").is_none());
    assert!(obs.get("note").is_none());
    // Uncorrected acuity (2), IOP (2), tonometer type (1) and slit lamp (4).
    assert_eq!(obs["component"].as_array().unwrap().len(), 9);
}

#[test]
fn test_fhir_export_escapes_text_and_skips_empty_values() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let notes = "Patient said \"blurry\" \\ near.\n\tFollow-up\u{1} in 6 months — 视力";
    let visual_field = OptVisualField::Some(VisualField {
        left_eye_reliability: String::from_str(&env, "Good"),
        right_eye_reliability: String::from_str(&env, ""),
        left_eye_defects: String::from_str(&env, ""),
        right_eye_defects: String::from_str(&env, ""),
    });
    add_exam(
        &env,
        &client,
        &vision_records,
        3,
        sample_iop(&env),
        visual_field,
        notes,
    );

    let obs = fhir_json(&client.export_eye_examination_fhir(&vision_records, &3));
    assert_eq!(obs["note"][0]["text"], notes);
    assert_eq!(
        component(&obs, "Visual field test reliability Left eye")["valueString"],
        "Good"
    );
    // Only one of the four visual field values is non-empty.
    assert_eq!(obs["component"].as_array().unwrap().len(), 16);
}

#[test]
fn test_fhir_export_date_time_boundaries() {
    let (env, client, _admin, vision_records) = setup_clinical();
    for (record_id, timestamp, expected) in [
        (1u64, 951_782_400u64, "2000-02-29T00:00:00Z"),
        (2, 4_107_542_399, "2100-02-28T23:59:59Z"),
        (3, 253_402_300_799, "9999-12-31T23:59:59Z"),
    ] {
        let iop = IntraocularPressure {
            timestamp,
            ..sample_iop(&env)
        };
        add_exam(
            &env,
            &client,
            &vision_records,
            record_id,
            iop,
            OptVisualField::None,
            "",
        );
        let obs = fhir_json(&client.export_eye_examination_fhir(&vision_records, &record_id));
        assert_eq!(obs["effectiveDateTime"], expected);
    }

    // Past 9999-12-31 a four-digit FHIR year cannot represent the timestamp.
    let iop = IntraocularPressure {
        timestamp: 253_402_300_800,
        ..sample_iop(&env)
    };
    add_exam(
        &env,
        &client,
        &vision_records,
        4,
        iop,
        OptVisualField::None,
        "",
    );
    assert_eq!(
        client.try_export_eye_examination_fhir(&vision_records, &4),
        Err(Ok(ContractError::InvalidInput))
    );
}

#[test]
fn test_fhir_medication_request_export() {
    let (env, client, _admin, vision_records) = setup_clinical();
    env.ledger().set_timestamp(1_700_000_000);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let rx_id = client.prepare_add_prescription(
        &vision_records,
        &patient,
        &provider,
        &sample_prescription_data(&env),
    );
    client.commit_add_prescription(&vision_records, &rx_id);

    let rx = fhir_json(&client.export_prescription_fhir(&vision_records, &rx_id));
    assert_eq!(rx["resourceType"], "MedicationRequest");
    assert_eq!(rx["id"], "rx-1");
    assert_eq!(rx["status"], "active");
    assert_eq!(rx["intent"], "order");
    let medication = &rx["medicationCodeableConcept"]["coding"][0];
    assert_eq!(medication["system"], "http://snomed.info/sct");
    assert_eq!(medication["code"], "50121007");
    assert_eq!(medication["display"], "Eyeglasses");
    assert_eq!(
        rx["subject"]["reference"],
        std::format!("Patient/{}", patient.to_string())
    );
    assert_eq!(
        rx["requester"]["reference"],
        std::format!("Practitioner/{}", provider.to_string())
    );
    assert_eq!(rx["authoredOn"], "2023-11-14T22:13:20Z");
    assert_eq!(
        rx["dosageInstruction"][0]["text"],
        "OD: SPH -2.25 CYL -0.75 AXIS 180 ADD +1.50 PD 63; \
         OS: SPH -2.25 CYL -0.75 AXIS 180 ADD +1.50 PD 63"
    );
    let validity = &rx["dispenseRequest"]["validityPeriod"];
    assert_eq!(validity["start"], "2023-11-14T22:13:20Z");
    assert_eq!(validity["end"], "2024-11-13T22:13:20Z");

    // Once the validity period has passed the request is no longer actionable.
    env.ledger().set_timestamp(1_700_000_000 + 31_536_000);
    let rx = fhir_json(&client.export_prescription_fhir(&vision_records, &rx_id));
    assert_eq!(rx["status"], "stopped");
}

#[test]
fn test_fhir_export_authorization_and_missing_records() {
    let (env, client, _admin, vision_records) = setup_clinical();
    let outsider = Address::generate(&env);
    assert_eq!(
        client.try_export_eye_examination_fhir(&vision_records, &404),
        Err(Ok(ContractError::RecordNotFound))
    );
    assert_eq!(
        client.try_export_prescription_fhir(&vision_records, &404),
        Err(Ok(ContractError::RecordNotFound))
    );
    add_exam(
        &env,
        &client,
        &vision_records,
        1,
        sample_iop(&env),
        OptVisualField::None,
        "",
    );
    assert_eq!(
        client.try_export_eye_examination_fhir(&outsider, &1),
        Err(Ok(ContractError::Unauthorized))
    );

    // With access_control, the record's patient may export it and an outsider may not.
    let (env, client, _, _admin, _, doctor, patient, unauthorized) =
        setup_clinical_with_access_control();
    add_exam(
        &env,
        &client,
        &doctor,
        9,
        sample_iop(&env),
        OptVisualField::None,
        "",
    );
    client.set_examination_patient(&doctor, &9, &patient);
    let rx_id = client.prepare_add_prescription(
        &doctor,
        &patient,
        &doctor,
        &sample_prescription_data(&env),
    );
    client.commit_add_prescription(&doctor, &rx_id);

    let obs = fhir_json(&client.export_eye_examination_fhir(&patient, &9));
    assert_eq!(obs["id"], "eye-exam-9");
    let rx = fhir_json(&client.export_prescription_fhir(&patient, &rx_id));
    assert_eq!(rx["id"], "rx-1");
    assert_eq!(
        client.try_export_eye_examination_fhir(&unauthorized, &9),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_export_prescription_fhir(&unauthorized, &rx_id),
        Err(Ok(ContractError::Unauthorized))
    );
}
