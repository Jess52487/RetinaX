#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use access_control::{AccessControlContract, AccessControlContractClient, Role};
use clinical_records::{
    ClinicalRecordsContract, ClinicalRecordsContractClient, ContractError, EyeExamination,
    FundusPhotography, IntraocularPressure, LensType, OptFundusPhotography, OptPhysicalMeasurement,
    OptRetinalImaging, OptVisualField, PhysicalMeasurement, Prescription, PrescriptionData,
    RetinalImaging, SlitLampFindings, VisualAcuity, VisualField,
};
use soroban_sdk::{
    testutils::Address as _,
    testutils::{Address as _, Events as _},
    Address, Env, String,
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

    (env, cr_client, ac_client, admin, vision_records, doctor, patient, unauthorized)
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
    let unauth_prep = client.try_prepare_add_prescription(&unauthorized, &patient, &doctor, &rx_data);
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

    assert_eq!(client.get_access_control_contract(), Some(ac_client.address.clone()));

    // Admin updates access control contract address
    let new_ac = Address::generate(&env);
    let update_res = client.try_set_access_control_contract(&admin, &new_ac);
    assert!(update_res.is_ok());
    assert_eq!(client.get_access_control_contract(), Some(new_ac));

    // Non-admin cannot update access control contract address
    let unauth_update = client.try_set_access_control_contract(&doctor, &unauthorized);
    assert_eq!(unauth_update, Err(Ok(ContractError::Unauthorized)));
}

