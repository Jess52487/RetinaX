#![no_std]
#![allow(clippy::too_many_arguments, clippy::large_enum_variant)]

extern crate alloc;

pub mod admin;
pub mod errors;
pub mod events;
pub mod examination;
pub mod prescription;
pub mod types;

#[cfg(test)]
mod test;

pub use errors::ContractError;
pub use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

use fhir::types::{MedicationRequest, Observation};

#[contract]
pub struct ClinicalRecordsContract;

#[contractimpl]
impl ClinicalRecordsContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        vision_records: Address,
    ) -> Result<(), ContractError> {
        admin::initialize(&env, admin, vision_records)
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        admin::get_admin(&env)
    }

    pub fn set_vision_records_contract(
        env: Env,
        admin: Address,
        vision_records: Address,
    ) -> Result<(), ContractError> {
        admin::set_vision_records_contract(&env, admin, vision_records)
    }

    pub fn get_vision_records_contract(env: Env) -> Option<Address> {
        admin::get_vision_records_contract(&env)
    }

    pub fn add_eye_examination(
        env: Env,
        caller: Address,
        record_id: u64,
        visual_acuity: VisualAcuity,
        iop: IntraocularPressure,
        slit_lamp: SlitLampFindings,
        visual_field: OptVisualField,
        retina_imaging: OptRetinalImaging,
        fundus_photo: OptFundusPhotography,
        clinical_notes: String,
    ) -> Result<(), ContractError> {
        examination::add_eye_examination(
            &env,
            caller,
            record_id,
            visual_acuity,
            iop,
            slit_lamp,
            visual_field,
            retina_imaging,
            fundus_photo,
            clinical_notes,
        )
    }

    pub fn get_eye_examination(
        env: Env,
        caller: Address,
        record_id: u64,
    ) -> Result<EyeExamination, ContractError> {
        examination::get_eye_examination(&env, caller, record_id)
    }

    /// Map a stored [`EyeExamination`] to a list of FHIR R4 [`Observation`]
    /// resources (visual acuity, IOP, eye-exam narrative).
    ///
    /// Each observation carries a canonical LOINC system URI, `status`,
    /// `subject`, and `effective[x]` so it passes
    /// [`FhirContract::validate_eye_exam_observation`].
    ///
    /// # Errors
    /// * `Unauthorized`  – caller is not the configured vision-records contract.
    /// * `RecordNotFound`– no examination exists for `record_id`.
    pub fn get_eye_examination_fhir(
        env: Env,
        caller: Address,
        record_id: u64,
        subject_id: String,
    ) -> Result<Vec<Observation>, ContractError> {
        let exam = examination::get_eye_examination(&env, caller, record_id)?;
        Ok(examination::eye_examination_to_fhir(
            &env, &exam, subject_id,
        ))
    }

    /// Map a stored [`Prescription`] to a FHIR R4 [`MedicationRequest`].
    ///
    /// `medication[x]` uses SNOMED CT Eye glasses (`50121007`) or Contact
    /// lenses (`57368009`) coded per `lens_type`; per-eye lens parameters
    /// (sphere / cylinder / axis / addition / PD) are carried in
    /// `dosage_text` with the LOINC EYE.GLASSES prescription codes. The
    /// result always satisfies the fhir contract's
    /// `validate_medication_request` rules.
    ///
    /// # Errors
    /// * `Unauthorized`  – caller is not the configured vision-records
    ///   contract.
    /// * `RecordNotFound`– no prescription exists for `id`.
    pub fn get_prescription_fhir(
        env: Env,
        caller: Address,
        id: u64,
        subject_id: String,
    ) -> Result<MedicationRequest, ContractError> {
        let rx = prescription::get_prescription(&env, caller, id)?;
        Ok(prescription::prescription_to_fhir(&env, &rx, subject_id))
    }

    pub fn prepare_add_prescription(
        env: Env,
        caller: Address,
        patient: Address,
        provider: Address,
        prescription_data: PrescriptionData,
    ) -> Result<u64, ContractError> {
        prescription::prepare_add_prescription(&env, caller, patient, provider, prescription_data)
    }

    pub fn commit_add_prescription(
        env: Env,
        caller: Address,
        rx_id: u64,
    ) -> Result<(), ContractError> {
        prescription::commit_add_prescription(&env, caller, rx_id)
    }

    pub fn rollback_add_prescription(
        env: Env,
        caller: Address,
        rx_id: u64,
    ) -> Result<(), ContractError> {
        prescription::rollback_add_prescription(&env, caller, rx_id)
    }

    pub fn get_prescription_count(env: Env) -> u64 {
        prescription::get_prescription_count(&env)
    }

    pub fn get_prescription(
        env: Env,
        caller: Address,
        id: u64,
    ) -> Result<Prescription, ContractError> {
        prescription::get_prescription(&env, caller, id)
    }
}
