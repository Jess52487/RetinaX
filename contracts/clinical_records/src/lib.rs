//! Clinical records contract for RetinaX.
//!
//! Stores eye examinations (visual acuity, intraocular pressure, slit lamp findings,
//! visual field, retinal imaging and fundus photography) and vision correction
//! prescriptions. Writes and reads are authorized either by the `access_control`
//! contract, when one is configured, or by the `vision_records` contract and the
//! admin otherwise.

#![no_std]
#![allow(clippy::too_many_arguments, clippy::large_enum_variant)]

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

use soroban_sdk::{contract, contractimpl, Address, Env, String};

/// Soroban contract that stores eye examinations and prescriptions.
#[contract]
pub struct ClinicalRecordsContract;

#[contractimpl]
impl ClinicalRecordsContract {
    /// Initializes the contract once.
    ///
    /// Stores `admin`, the `vision_records` contract address, and an optional
    /// `access_control` contract address. Requires authorization from `admin`.
    ///
    /// # Errors
    /// Returns `AlreadyInitialized` if the contract has already been initialized.
    pub fn initialize(
        env: Env,
        admin: Address,
        vision_records: Address,
        access_control: Option<Address>,
    ) -> Result<(), ContractError> {
        admin::initialize(&env, admin, vision_records, access_control)
    }

    /// Returns the admin address set during initialization.
    ///
    /// # Errors
    /// Returns `NotInitialized` if the contract has not been initialized.
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        admin::get_admin(&env)
    }

    /// Replaces the `vision_records` contract address.
    ///
    /// Requires authorization from `admin`, which must match the stored admin.
    ///
    /// # Errors
    /// Returns `Unauthorized` if `admin` is not the stored admin.
    pub fn set_vision_records_contract(
        env: Env,
        admin: Address,
        vision_records: Address,
    ) -> Result<(), ContractError> {
        admin::set_vision_records_contract(&env, admin, vision_records)
    }

    /// Returns the configured `vision_records` contract address, if one is set.
    pub fn get_vision_records_contract(env: Env) -> Option<Address> {
        admin::get_vision_records_contract(&env)
    }

    /// Sets the `access_control` contract used for permission checks.
    ///
    /// Requires authorization from `admin`, which must match the stored admin.
    ///
    /// # Errors
    /// Returns `Unauthorized` if `admin` is not the stored admin.
    pub fn set_access_control_contract(
        env: Env,
        admin: Address,
        access_control: Address,
    ) -> Result<(), ContractError> {
        admin::set_access_control_contract(&env, admin, access_control)
    }

    /// Returns the configured `access_control` contract address, if one is set.
    pub fn get_access_control_contract(env: Env) -> Option<Address> {
        admin::get_access_control_contract(&env)
    }

    /// Stores a full eye examination under `record_id`.
    ///
    /// When `access_control` is configured, `caller` needs the `WriteRecord`
    /// permission. Otherwise `caller` must be the `vision_records` contract.
    /// Free-text fields are length-checked, for example `clinical_notes` is limited
    /// to 2048 bytes. Emits an examination-added event on success.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller lacks write access, `InvalidInput` if a
    /// field fails validation, or `NotInitialized` if the contract is not set up.
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

    /// Links an examination record to the patient who owns it.
    ///
    /// The linked patient can read the examination through `get_eye_examination`.
    /// Uses the same write authorization as `add_eye_examination`.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller lacks write access.
    pub fn set_examination_patient(
        env: Env,
        caller: Address,
        record_id: u64,
        patient: Address,
    ) -> Result<(), ContractError> {
        examination::set_examination_patient(&env, caller, record_id, patient)
    }

    /// Returns the eye examination stored under `record_id`.
    ///
    /// When `access_control` is configured, `caller` must hold `ReadAnyRecord` or be
    /// the patient linked to the record. Otherwise `caller` must be the
    /// `vision_records` contract.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller is not allowed to read the record, or
    /// `RecordNotFound` if no examination exists under `record_id`.
    pub fn get_eye_examination(
        env: Env,
        caller: Address,
        record_id: u64,
    ) -> Result<EyeExamination, ContractError> {
        examination::get_eye_examination(&env, caller, record_id)
    }

    /// First step of the two-phase prescription flow. Stores the prescription as pending.
    ///
    /// Assigns the next prescription id, keeps the pending data in temporary storage,
    /// and emits a prepared event. Call `commit_add_prescription` with the returned id
    /// to make it permanent, or `rollback_add_prescription` to discard it. Temporary
    /// entries expire, so the commit must happen before they do.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller lacks write access, or `InvalidInput` if
    /// the prescription values fail validation.
    pub fn prepare_add_prescription(
        env: Env,
        caller: Address,
        patient: Address,
        provider: Address,
        prescription_data: PrescriptionData,
    ) -> Result<u64, ContractError> {
        prescription::prepare_add_prescription(&env, caller, patient, provider, prescription_data)
    }

    /// Second step of the two-phase prescription flow. Makes a pending prescription permanent.
    ///
    /// The committed prescription is a glasses prescription, starts unverified, and
    /// expires one year (31,536,000 seconds) after the prepare step.
    ///
    /// # Errors
    /// Returns `InvalidPhase` if no pending prescription exists for `rx_id`, or
    /// `Unauthorized` if the caller lacks write access.
    pub fn commit_add_prescription(
        env: Env,
        caller: Address,
        rx_id: u64,
    ) -> Result<(), ContractError> {
        prescription::commit_add_prescription(&env, caller, rx_id)
    }

    /// Discards a pending prescription that was prepared but not committed.
    ///
    /// Emits a rolled-back event. The id is not reused.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller lacks write access.
    pub fn rollback_add_prescription(
        env: Env,
        caller: Address,
        rx_id: u64,
    ) -> Result<(), ContractError> {
        prescription::rollback_add_prescription(&env, caller, rx_id)
    }

    /// Returns how many prescription ids have been issued by `prepare_add_prescription`.
    ///
    /// This counts prepared prescriptions, including ones later rolled back, not only
    /// committed ones.
    pub fn get_prescription_count(env: Env) -> u64 {
        prescription::get_prescription_count(&env)
    }

    /// Returns the committed prescription with the given `id`.
    ///
    /// When `access_control` is configured, `caller` must hold `ReadAnyRecord` or be
    /// the patient on the prescription. Otherwise `caller` must be the
    /// `vision_records` contract.
    ///
    /// # Errors
    /// Returns `Unauthorized` if the caller is not allowed to read it, or
    /// `RecordNotFound` if no committed prescription has that id.
    pub fn get_prescription(
        env: Env,
        caller: Address,
        id: u64,
    ) -> Result<Prescription, ContractError> {
        prescription::get_prescription(&env, caller, id)
    }
}
