#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_test() -> (
    Env,
    ClinicalRecordsContractClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ClinicalRecordsContract, ());
    let client = ClinicalRecordsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let vision_records = Address::generate(&env);
    client.initialize(&admin, &vision_records, &None);
    (env, client, admin, vision_records)
}

#[test]
fn test_initialize_and_admin_workflow() {
    let (env, client, admin, vision_records) = setup_test();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(
        client.get_vision_records_contract(),
        Some(vision_records.clone())
    );

    // Changing vision records contract by admin succeeds
    let new_vision_records = Address::generate(&env);
    client.set_vision_records_contract(&admin, &new_vision_records);
    assert_eq!(
        client.get_vision_records_contract(),
        Some(new_vision_records)
    );

    // Non-admin cannot change vision records contract
    let non_admin = Address::generate(&env);
    let err = client.try_set_vision_records_contract(&non_admin, &vision_records);
    assert!(err.is_err());
}

#[test]
fn test_eye_examination_lifecycle() {
    let (env, client, _, vision_records) = setup_test();

    let visual_acuity = VisualAcuity {
        uncorrected: PhysicalMeasurement {
            left_eye: String::from_str(&env, "20/20"),
            right_eye: String::from_str(&env, "20/25"),
        },
        corrected: OptPhysicalMeasurement::None,
    };
    let iop = IntraocularPressure {
        left_eye: 15,
        right_eye: 16,
        method: String::from_str(&env, "Goldmann"),
        timestamp: 1000,
    };
    let slit_lamp = SlitLampFindings {
        cornea: String::from_str(&env, "Clear"),
        anterior_chamber: String::from_str(&env, "Deep and quiet"),
        iris: String::from_str(&env, "Normal"),
        lens: String::from_str(&env, "Clear"),
    };
    let visual_field = OptVisualField::None;
    let retina_imaging = OptRetinalImaging::None;
    let fundus_photo = OptFundusPhotography::None;
    let notes = String::from_str(&env, "Healthy eyes");

    // Add examination
    client.add_eye_examination(
        &vision_records,
        &101u64,
        &visual_acuity,
        &iop,
        &slit_lamp,
        &visual_field,
        &retina_imaging,
        &fundus_photo,
        &notes,
    );

    // Retrieve examination
    let exam = client.get_eye_examination(&vision_records, &101u64);
    assert_eq!(exam.record_id, 101u64);
    assert_eq!(exam.iop.left_eye, 15);
    assert_eq!(exam.clinical_notes, notes);

    // Non-vision_records caller is rejected
    let unauthorized = Address::generate(&env);
    let err = client.try_get_eye_examination(&unauthorized, &101u64);
    assert!(err.is_err());
}

#[test]
fn test_prescription_two_phase_commit_and_rollback() {
    let (env, client, _, vision_records) = setup_test();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let rx_data = PrescriptionData {
        sphere: String::from_str(&env, "-2.00"),
        cylinder: String::from_str(&env, "-0.50"),
        axis: String::from_str(&env, "180"),
        add: String::from_str(&env, "+1.00"),
        pd: String::from_str(&env, "62"),
    };

    // Prepare
    let rx_id = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    assert_eq!(rx_id, 1);
    assert_eq!(client.get_prescription_count(), 1);

    // Commit
    client.commit_add_prescription(&vision_records, &rx_id);

    let rx = client.get_prescription(&vision_records, &rx_id);
    assert_eq!(rx.id, 1);
    assert_eq!(rx.patient, patient);
    assert_eq!(rx.provider, provider);
    assert_eq!(rx.left_eye.sphere, String::from_str(&env, "-2.00"));

    // Prepare and rollback second prescription
    let rx_id_2 = client.prepare_add_prescription(&vision_records, &patient, &provider, &rx_data);
    assert_eq!(rx_id_2, 2);
    client.rollback_add_prescription(&vision_records, &rx_id_2);

    // Attempting to commit rolled back prescription fails
    let commit_err = client.try_commit_add_prescription(&vision_records, &rx_id_2);
    assert!(commit_err.is_err());
}
