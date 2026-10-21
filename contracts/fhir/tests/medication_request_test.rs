//! FHIR R4 MedicationRequest validation tests (issue #318).
//!
//! Verifies that:
//! * Required FHIR R4 fields (`id`, `intent`, `medication[x]`, `subject`,
//!   `authoredOn`) are enforced.
//! * `medication[x]` must use the canonical SNOMED CT system URI and code a
//!   RetinaX lens device (Eye glasses `50121007` or Contact lenses
//!   `57368009`).
//! * `dispenseRequest.validityPeriod` must be well-ordered.
//! * LOINC EYE.GLASSES prescription codes are recognised.

#![allow(clippy::unwrap_used)]

use fhir::types::{FhirError, MedicationRequest, MedicationRequestIntent, MedicationRequestStatus};
use fhir::validation::{
    is_lens_device_code, is_lens_prescription_code, validate_medication_request_r4, LOINC_IOP,
    LOINC_RX_ADD_LEFT, LOINC_RX_ADD_RIGHT, LOINC_RX_AXIS_LEFT, LOINC_RX_AXIS_RIGHT,
    LOINC_RX_CYLINDER_LEFT, LOINC_RX_CYLINDER_RIGHT, LOINC_RX_PD_BINOCULAR, LOINC_RX_SPHERE_LEFT,
    LOINC_RX_SPHERE_RIGHT, SNOMED_CONTACT_LENSES, SNOMED_EYE_GLASSES, SNOMED_SYSTEM,
};
use fhir::{FhirContract, FhirContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, FhirContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FhirContract, ());
    let client = FhirContractClient::new(&env, &contract_id);
    client.initialize(&Address::generate(&env));
    (env, client)
}

/// A fully valid glasses MedicationRequest (SNOMED Eye glasses, all
/// required fields).
fn valid_request(env: &Env) -> MedicationRequest {
    MedicationRequest {
        id: String::from_str(env, "rx-1"),
        status: MedicationRequestStatus::Active,
        intent: MedicationRequestIntent::Order,
        medication_system: String::from_str(env, SNOMED_SYSTEM),
        medication_code: String::from_str(env, SNOMED_EYE_GLASSES),
        medication_display: String::from_str(env, "Eye glasses"),
        subject_id: String::from_str(env, "patient-42"),
        requester_id: String::from_str(env, "practitioner-7"),
        authored_on: 1_700_000_000,
        validity_period_start: 1_700_000_000,
        validity_period_end: 1_731_536_000,
        dosage_text: String::from_str(env, "OD: SPH -2.00; OS: SPH -1.75; PD R 63 L 63 mm"),
    }
}

// ── Required R4 fields ────────────────────────────────────────────────────────

#[test]
fn test_valid_glasses_request_passes() {
    let (env, _client) = setup();
    let request = valid_request(&env);
    assert!(validate_medication_request_r4(&request).is_ok());
}

#[test]
fn test_valid_contact_lens_request_passes() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.medication_code = String::from_str(&env, SNOMED_CONTACT_LENSES);
    request.medication_display = String::from_str(&env, "Contact lenses");
    assert!(validate_medication_request_r4(&request).is_ok());
}

#[test]
fn test_empty_id_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.id = String::from_str(&env, "");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

#[test]
fn test_missing_subject_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.subject_id = String::from_str(&env, "");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

#[test]
fn test_zero_authored_on_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.authored_on = 0;
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

#[test]
fn test_empty_medication_code_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.medication_code = String::from_str(&env, "");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

#[test]
fn test_empty_medication_system_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.medication_system = String::from_str(&env, "");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

#[test]
fn test_reversed_validity_period_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.validity_period_start = 2_000_000_000;
    request.validity_period_end = 1_000_000_000;
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidRecordData)
    );
}

// ── Terminology and intent constraints ────────────────────────────────────────

#[test]
fn test_non_order_intent_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.intent = MedicationRequestIntent::Plan;
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidPayload)
    );
}

#[test]
fn test_non_snomed_medication_system_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.medication_system = String::from_str(&env, "http://loinc.org");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidPayload)
    );
}

#[test]
fn test_unknown_lens_device_code_fails() {
    let (env, _client) = setup();
    let mut request = valid_request(&env);
    request.medication_code = String::from_str(&env, "123456789");
    assert_eq!(
        validate_medication_request_r4(&request),
        Err(FhirError::InvalidPayload)
    );
}

// ── OPT lens-prescription code recognition ────────────────────────────────────

#[test]
fn test_lens_prescription_codes_are_recognised() {
    let (env, _client) = setup();
    for code in [
        LOINC_RX_SPHERE_RIGHT,
        LOINC_RX_CYLINDER_RIGHT,
        LOINC_RX_AXIS_RIGHT,
        LOINC_RX_ADD_RIGHT,
        LOINC_RX_SPHERE_LEFT,
        LOINC_RX_CYLINDER_LEFT,
        LOINC_RX_AXIS_LEFT,
        LOINC_RX_ADD_LEFT,
        LOINC_RX_PD_BINOCULAR,
    ] {
        assert!(
            is_lens_prescription_code(&String::from_str(&env, code)),
            "{code} must be a recognised lens-prescription code"
        );
    }
    // Non-prescription LOINC codes must not be accepted as lens parameters.
    assert!(!is_lens_prescription_code(&String::from_str(
        &env, LOINC_IOP
    )));
}

#[test]
fn test_lens_device_codes_are_recognised() {
    let (env, _client) = setup();
    assert!(is_lens_device_code(&String::from_str(
        &env,
        SNOMED_EYE_GLASSES
    )));
    assert!(is_lens_device_code(&String::from_str(
        &env,
        SNOMED_CONTACT_LENSES
    )));
    assert!(!is_lens_device_code(&String::from_str(&env, "999999999")));
}

// ── Contract entrypoint ───────────────────────────────────────────────────────

#[test]
fn test_contract_accepts_valid_request() {
    let (env, client) = setup();
    let request = valid_request(&env);
    client.validate_medication_request(&request);
}

#[test]
fn test_contract_rejects_invalid_request() {
    let (env, client) = setup();
    let mut request = valid_request(&env);
    request.subject_id = String::from_str(&env, "");
    let result = client.try_validate_medication_request(&request);
    assert_eq!(result, Err(Ok(FhirError::InvalidRecordData)));
}

#[test]
fn test_contract_rejects_unknown_device() {
    let (env, client) = setup();
    let mut request = valid_request(&env);
    request.medication_code = String::from_str(&env, "999999999");
    let result = client.try_validate_medication_request(&request);
    assert_eq!(result, Err(Ok(FhirError::InvalidPayload)));
}
