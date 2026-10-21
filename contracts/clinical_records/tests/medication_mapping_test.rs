//! FHIR R4 MedicationRequest mapping tests for prescriptions (issue #318).
//!
//! Verifies that a stored `Prescription` maps to a well-formed FHIR R4
//! `MedicationRequest` that satisfies the fhir contract's
//! `validate_medication_request` rules: SNOMED CT lens-device
//! `medication[x]`, per-eye lens parameters (sphere / cylinder / axis /
//! addition / PD) in `dosage_text`, and correct `status`, `intent`,
//! `authoredOn`, and validity period.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

extern crate std;

use clinical_records::{
    prescription::prescription_to_fhir, ClinicalRecordsContract, ClinicalRecordsContractClient,
    ContactLensData, ContractError, LensType, OptionalContactLensData, Prescription,
    PrescriptionData,
};
use fhir::types::{MedicationRequestIntent, MedicationRequestStatus};
use fhir::validation::{
    validate_medication_request_r4, LOINC_RX_AXIS_RIGHT, LOINC_RX_CYLINDER_RIGHT,
    LOINC_RX_PD_BINOCULAR, LOINC_RX_SPHERE_RIGHT, SNOMED_CONTACT_LENSES, SNOMED_EYE_GLASSES,
    SNOMED_SYSTEM,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

const ISSUED_AT: u64 = 1_700_000_000;

fn setup() -> (
    Env,
    ClinicalRecordsContractClient<'static>,
    Address, // vision_records (authorized caller)
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);
    let contract_id = env.register(ClinicalRecordsContract, ());
    let client = ClinicalRecordsContractClient::new(&env, &contract_id);
    client.initialize(&admin, &vision_records);

    (env, client, vision_records)
}

fn sample_rx_data(env: &Env) -> PrescriptionData {
    PrescriptionData {
        sphere: String::from_str(env, "-2.00"),
        cylinder: String::from_str(env, "-0.75"),
        axis: String::from_str(env, "090"),
        add: String::from_str(env, "+1.50"),
        pd: String::from_str(env, "63"),
    }
}

/// Add a prescription through the contract's two-phase commit flow.
fn add_sample_prescription(
    env: &Env,
    client: &ClinicalRecordsContractClient,
    vision_records: &Address,
) -> u64 {
    env.ledger().with_mut(|l| l.timestamp = ISSUED_AT);
    let patient = Address::generate(env);
    let provider = Address::generate(env);
    let rx_id =
        client.prepare_add_prescription(vision_records, &patient, &provider, &sample_rx_data(env));
    client.commit_add_prescription(vision_records, &rx_id);
    rx_id
}

// ── Mapping shape ────────────────────────────────────────────────────────────

#[test]
fn test_prescription_maps_to_valid_medication_request() {
    let (env, client, vision_records) = setup();
    let rx_id = add_sample_prescription(&env, &client, &vision_records);

    let subject = String::from_str(&env, "patient-42");
    let request = client.get_prescription_fhir(&vision_records, &rx_id, &subject);

    assert!(
        validate_medication_request_r4(&request).is_ok(),
        "mapped MedicationRequest must pass FHIR R4 validation"
    );
}

#[test]
fn test_mapped_request_carries_required_r4_fields() {
    let (env, client, vision_records) = setup();
    let rx_id = add_sample_prescription(&env, &client, &vision_records);

    let subject = String::from_str(&env, "patient-42");
    let request = client.get_prescription_fhir(&vision_records, &rx_id, &subject);

    assert_eq!(request.id, String::from_str(&env, "rx-1"));
    assert_eq!(request.intent, MedicationRequestIntent::Order);
    assert_eq!(request.subject_id, subject);
    assert_eq!(request.authored_on, ISSUED_AT);
    assert_eq!(request.validity_period_start, ISSUED_AT);
    assert_eq!(request.validity_period_end, ISSUED_AT + 31_536_000);
    // Unexpired at the ledger timestamp used for issuance.
    assert_eq!(request.status, MedicationRequestStatus::Active);
    // requester (who wrote the order) is the provider address, non-empty.
    assert!(!request.requester_id.is_empty());
}

#[test]
fn test_glasses_mapping_uses_snomed_eye_glasses_code() {
    let (env, client, vision_records) = setup();
    let rx_id = add_sample_prescription(&env, &client, &vision_records);

    let subject = String::from_str(&env, "patient-42");
    let request = client.get_prescription_fhir(&vision_records, &rx_id, &subject);

    assert_eq!(
        request.medication_system,
        String::from_str(&env, SNOMED_SYSTEM),
        "medication[x] must use the canonical SNOMED CT system URI"
    );
    assert_eq!(
        request.medication_code,
        String::from_str(&env, SNOMED_EYE_GLASSES),
        "glasses prescriptions must code SNOMED Eye glasses (50121007)"
    );
    assert_eq!(
        request.medication_display,
        String::from_str(&env, "Eye glasses")
    );
}

#[test]
fn test_dosage_text_contains_lens_parameters_and_loinc_codes() {
    let (env, client, vision_records) = setup();
    let rx_id = add_sample_prescription(&env, &client, &vision_records);

    let subject = String::from_str(&env, "patient-42");
    let request = client.get_prescription_fhir(&vision_records, &rx_id, &subject);

    let dosage = request.dosage_text.to_string();
    // Per-eye lens prescription values.
    assert!(dosage.contains("-2.00"), "sphere value present: {dosage}");
    assert!(dosage.contains("-0.75"), "cylinder value present: {dosage}");
    assert!(dosage.contains("090"), "axis value present: {dosage}");
    assert!(dosage.contains("+1.50"), "addition value present: {dosage}");
    assert!(dosage.contains("63"), "PD value present: {dosage}");
    // OPT-specific LOINC EYE.GLASSES codes label the parameters.
    assert!(dosage.contains(LOINC_RX_SPHERE_RIGHT));
    assert!(dosage.contains(LOINC_RX_CYLINDER_RIGHT));
    assert!(dosage.contains(LOINC_RX_AXIS_RIGHT));
    assert!(dosage.contains(LOINC_RX_PD_BINOCULAR));
}

#[test]
fn test_contact_lens_mapping_uses_snomed_contact_lenses_code() {
    let (env, _client, _vision_records) = setup();
    env.ledger().with_mut(|l| l.timestamp = ISSUED_AT);

    let rx = Prescription {
        id: 9,
        patient: Address::generate(&env),
        provider: Address::generate(&env),
        lens_type: LensType::ContactLens,
        left_eye: sample_rx_data(&env),
        right_eye: sample_rx_data(&env),
        contact_data: OptionalContactLensData::Some(ContactLensData {
            base_curve: String::from_str(&env, "8.6"),
            diameter: String::from_str(&env, "14.2"),
            brand: String::from_str(&env, "Acuvue"),
        }),
        issued_at: ISSUED_AT,
        expires_at: ISSUED_AT + 31_536_000,
        verified: false,
        metadata_hash: String::from_str(&env, ""),
    };

    let subject = String::from_str(&env, "patient-9");
    let request = prescription_to_fhir(&env, &rx, subject);

    assert!(validate_medication_request_r4(&request).is_ok());
    assert_eq!(
        request.medication_code,
        String::from_str(&env, SNOMED_CONTACT_LENSES),
        "contact-lens prescriptions must code SNOMED Contact lenses (57368009)"
    );
    assert_eq!(
        request.medication_display,
        String::from_str(&env, "Contact lenses")
    );

    let dosage = request.dosage_text.to_string();
    assert!(dosage.contains("Acuvue"), "brand recorded: {dosage}");
    assert!(dosage.contains("8.6"), "base curve recorded: {dosage}");
    assert!(dosage.contains("14.2"), "diameter recorded: {dosage}");
}

#[test]
fn test_expired_prescription_maps_to_completed_status() {
    let (env, _client, _vision_records) = setup();
    env.ledger()
        .with_mut(|l| l.timestamp = ISSUED_AT + 40_000_000);

    let rx = Prescription {
        id: 3,
        patient: Address::generate(&env),
        provider: Address::generate(&env),
        lens_type: LensType::Glasses,
        left_eye: sample_rx_data(&env),
        right_eye: sample_rx_data(&env),
        contact_data: OptionalContactLensData::None,
        issued_at: ISSUED_AT,
        expires_at: ISSUED_AT + 31_536_000,
        verified: false,
        metadata_hash: String::from_str(&env, ""),
    };

    let subject = String::from_str(&env, "patient-3");
    let request = prescription_to_fhir(&env, &rx, subject);

    assert_eq!(
        request.status,
        MedicationRequestStatus::Completed,
        "prescriptions past validityPeriod end map to completed"
    );
    assert!(validate_medication_request_r4(&request).is_ok());
}

// ── Access control and error paths ───────────────────────────────────────────

#[test]
fn test_get_prescription_fhir_unauthorized_caller_fails() {
    let (env, client, vision_records) = setup();
    let rx_id = add_sample_prescription(&env, &client, &vision_records);

    let intruder = Address::generate(&env);
    let subject = String::from_str(&env, "patient-42");
    let result = client.try_get_prescription_fhir(&intruder, &rx_id, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_get_prescription_fhir_missing_record_fails() {
    let (env, client, vision_records) = setup();
    let subject = String::from_str(&env, "patient-42");
    let result = client.try_get_prescription_fhir(&vision_records, &999, &subject);
    assert_eq!(result, Err(Ok(ContractError::RecordNotFound)));
}
