//! FHIR R4 mapping tests for `EyeExamination` (issue #316).
//!
//! Verifies that a stored eye examination maps to well-formed FHIR R4
//! `Observation` resources that satisfy the fhir contract's
//! `validate_eye_exam_observation` rules (required fields + LOINC/SNOMED
//! coding).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]

extern crate std;

use clinical_records::{
    ClinicalRecordsContract, ClinicalRecordsContractClient, ContractError, IntraocularPressure,
    OptFundusPhotography, OptPhysicalMeasurement, OptRetinalImaging, OptVisualField,
    PhysicalMeasurement, SlitLampFindings, VisualAcuity,
};
use fhir::types::ObservationStatus;
use fhir::validation::{
    self, LOINC_EYE_PHYSICAL_EXAM, LOINC_IOP, LOINC_VISUAL_ACUITY_BEST_CORR_LEFT,
    LOINC_VISUAL_ACUITY_BEST_CORR_RIGHT, LOINC_VISUAL_ACUITY_UNCORR_LEFT,
    LOINC_VISUAL_ACUITY_UNCORR_RIGHT,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const EXAM_TS: u64 = 1_700_000_000;

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

fn add_sample_exam(
    client: &ClinicalRecordsContractClient,
    vision_records: &Address,
    record_id: u64,
) {
    client.add_eye_examination(
        vision_records,
        &record_id,
        &VisualAcuity {
            uncorrected: PhysicalMeasurement {
                left_eye: String::from_str(&client.env, "20/40"),
                right_eye: String::from_str(&client.env, "20/30"),
            },
            corrected: OptPhysicalMeasurement::Some(PhysicalMeasurement {
                left_eye: String::from_str(&client.env, "20/20"),
                right_eye: String::from_str(&client.env, "20/20"),
            }),
        },
        &IntraocularPressure {
            left_eye: 16,
            right_eye: 15,
            method: String::from_str(&client.env, "Goldmann Applanation"),
            timestamp: EXAM_TS,
        },
        &SlitLampFindings {
            cornea: String::from_str(&client.env, "Clear"),
            anterior_chamber: String::from_str(&client.env, "Deep and quiet"),
            iris: String::from_str(&client.env, "Normal architecture"),
            lens: String::from_str(&client.env, "Clear, no cataract"),
        },
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&client.env, "Mild dry eye, follow-up in 6 months"),
    );
}

// ── Mapping shape ────────────────────────────────────────────────────────────

#[test]
fn test_eye_examination_maps_to_observations_with_correct_loinc_codes() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let subject = String::from_str(&env, "patient-42");
    let observations = client.get_eye_examination_fhir(&vision_records, &1, &subject);

    // 2 uncorrected VA + 2 best-corrected VA + IOP + narrative = 6
    assert_eq!(observations.len(), 6);

    let codes: std::vec::Vec<&'static str> = observations
        .iter()
        .map(|o| {
            let bytes = o.code_value.to_bytes();
            if bytes.iter().eq(LOINC_VISUAL_ACUITY_UNCORR_RIGHT.bytes()) {
                "va-ur"
            } else if bytes.iter().eq(LOINC_VISUAL_ACUITY_UNCORR_LEFT.bytes()) {
                "va-ul"
            } else if bytes.iter().eq(LOINC_VISUAL_ACUITY_BEST_CORR_RIGHT.bytes()) {
                "va-cr"
            } else if bytes.iter().eq(LOINC_VISUAL_ACUITY_BEST_CORR_LEFT.bytes()) {
                "va-cl"
            } else if bytes.iter().eq(LOINC_IOP.bytes()) {
                "iop"
            } else if bytes.iter().eq(LOINC_EYE_PHYSICAL_EXAM.bytes()) {
                "narrative"
            } else {
                panic!("unexpected LOINC code")
            }
        })
        .collect();

    assert_eq!(
        codes,
        ["va-ur", "va-ul", "va-cr", "va-cl", "iop", "narrative"]
    );
}

#[test]
fn test_mapped_observations_carry_canonical_loinc_system_uri() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let subject = String::from_str(&env, "patient-42");
    let observations = client.get_eye_examination_fhir(&vision_records, &1, &subject);

    for obs in observations.iter() {
        assert_eq!(
            obs.code_system,
            String::from_str(&env, validation::LOINC_SYSTEM),
            "every observation must use the canonical LOINC system URI"
        );
    }
}

#[test]
fn test_mapped_observations_have_all_required_r4_fields() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let subject = String::from_str(&env, "patient-42");
    let observations = client.get_eye_examination_fhir(&vision_records, &1, &subject);

    for obs in observations.iter() {
        // status present and final
        assert_eq!(obs.status, ObservationStatus::Final);
        // subject present and matches the requested patient
        assert_eq!(obs.subject_id, subject);
        // effective[x] present and sourced from the examination timestamp
        assert_eq!(obs.effective_datetime, EXAM_TS);
        // code present
        assert!(!obs.code_system.is_empty());
        assert!(!obs.code_value.is_empty());
        // id present
        assert!(!obs.id.is_empty());
    }
}

#[test]
fn test_every_mapped_observation_passes_fhir_contract_validation() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let subject = String::from_str(&env, "patient-42");
    let observations = client.get_eye_examination_fhir(&vision_records, &1, &subject);

    for obs in observations.iter() {
        assert!(
            validation::validate_eye_examination_observation(&obs).is_ok(),
            "observation with code {} must pass eye-exam validation",
            obs.code_value
        );
    }
}

#[test]
fn test_iop_observation_value_contains_both_eyes_and_method() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let subject = String::from_str(&env, "patient-42");
    let observations = client.get_eye_examination_fhir(&vision_records, &1, &subject);

    let iop = observations
        .iter()
        .find(|o| o.code_value.to_bytes().iter().eq(LOINC_IOP.bytes()))
        .expect("IOP observation must exist");
    let value = iop.value.to_string();
    assert!(value.contains("L:16"));
    assert!(value.contains("R:15"));
    assert!(value.contains("Goldmann Applanation"));
}

#[test]
fn test_corrected_va_observations_omitted_when_no_correction() {
    let (env, client, vision_records) = setup();
    client.add_eye_examination(
        &vision_records,
        &7,
        &VisualAcuity {
            uncorrected: PhysicalMeasurement {
                left_eye: String::from_str(&env, "20/20"),
                right_eye: String::from_str(&env, "20/20"),
            },
            corrected: OptPhysicalMeasurement::None,
        },
        &IntraocularPressure {
            left_eye: 14,
            right_eye: 14,
            method: String::from_str(&env, "Non-contact tonometry"),
            timestamp: EXAM_TS,
        },
        &SlitLampFindings {
            cornea: String::from_str(&env, "Clear"),
            anterior_chamber: String::from_str(&env, "Deep and quiet"),
            iris: String::from_str(&env, "Normal architecture"),
            lens: String::from_str(&env, "Clear"),
        },
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&env, "Routine exam"),
    );

    let subject = String::from_str(&env, "patient-7");
    let observations = client.get_eye_examination_fhir(&vision_records, &7, &subject);

    // 2 uncorrected VA + IOP + narrative (no corrected VA entries)
    assert_eq!(observations.len(), 4);
    for obs in observations.iter() {
        assert!(validation::validate_eye_examination_observation(&obs).is_ok());
    }
}

// ── Access control and error paths ───────────────────────────────────────────

#[test]
fn test_get_eye_examination_fhir_unauthorized_caller_fails() {
    let (env, client, vision_records) = setup();
    add_sample_exam(&client, &vision_records, 1);

    let intruder = Address::generate(&env);
    let subject = String::from_str(&env, "patient-42");
    let result = client.try_get_eye_examination_fhir(&intruder, &1, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_get_eye_examination_fhir_missing_record_fails() {
    let (env, client, vision_records) = setup();
    let subject = String::from_str(&env, "patient-42");
    let result = client.try_get_eye_examination_fhir(&vision_records, &999, &subject);
    assert_eq!(result, Err(Ok(ContractError::RecordNotFound)));
}
