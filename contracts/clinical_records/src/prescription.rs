use crate::admin::{
    check_permission, get_access_control_contract, require_authorized_vision_records,
    require_initialized,
};
use crate::errors::ContractError;
use crate::examination::validate_string;
use crate::types::{LensType, OptionalContactLensData, Prescription, PrescriptionData};
use fhir::types::{MedicationRequest, MedicationRequestIntent, MedicationRequestStatus};
use fhir::validation;
use access_control::Permission;
use soroban_sdk::{symbol_short, Address, Env, String, Symbol};

pub const PRESCRIPTION_KEY: Symbol = symbol_short!("RX");
pub const RX_COUNTER: Symbol = symbol_short!("RX_CTR");

/// Checks the length of each refraction field (sphere, cylinder, axis, add, and pupillary
/// distance) in a prescription.
///
/// # Errors
/// Returns `InvalidInput` if any field is empty or too long.
pub fn validate_prescription_data(value: &PrescriptionData) -> Result<(), ContractError> {
    validate_string(&value.sphere, 1, 64)?;
    validate_string(&value.cylinder, 1, 64)?;
    validate_string(&value.axis, 1, 32)?;
    validate_string(&value.add, 1, 32)?;
    validate_string(&value.pd, 1, 32)?;
    Ok(())
}

/// Validates refraction data and stores it as a pending prescription.
///
/// Returns the new prescription id. The pending entry lives in temporary storage until it is
/// committed or rolled back.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `InvalidInput`.
pub fn prepare_add_prescription(
    env: &Env,
    caller: Address,
    patient: Address,
    provider: Address,
    prescription_data: PrescriptionData,
) -> Result<u64, ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, &caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, &caller)?;
    }

    validate_prescription_data(&prescription_data)?;
    let next_id = env
        .storage()
        .instance()
        .get(&RX_COUNTER)
        .unwrap_or(0u64)
        .saturating_add(1u64);
    env.storage().instance().set(&RX_COUNTER, &next_id);
    let prep_key = (symbol_short!("P_ADD_RX"), next_id);
    let prep = (
        patient.clone(),
        provider.clone(),
        prescription_data,
        env.ledger().timestamp(),
    );
    env.storage().temporary().set(&prep_key, &prep);
    crate::events::publish_prescription_prepared(env, next_id, patient, provider);
    Ok(next_id)
}

/// Moves a pending prescription into persistent storage.
///
/// The committed prescription is a glasses prescription, starts unverified, and expires one year
/// after the prepare step.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `InvalidPhase` if no pending prescription exists.
pub fn commit_add_prescription(
    env: &Env,
    caller: Address,
    rx_id: u64,
) -> Result<(), ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, &caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, &caller)?;
    }

    let prep_key = (symbol_short!("P_ADD_RX"), rx_id);
    let prepared: (Address, Address, PrescriptionData, u64) = env
        .storage()
        .temporary()
        .get(&prep_key)
        .ok_or(ContractError::InvalidPhase)?;

    let prescription = Prescription {
        id: rx_id,
        patient: prepared.0,
        provider: prepared.1,
        lens_type: LensType::Glasses,
        left_eye: prepared.2.clone(),
        right_eye: prepared.2,
        contact_data: OptionalContactLensData::None,
        issued_at: prepared.3,
        expires_at: prepared.3.saturating_add(31_536_000),
        verified: false,
        metadata_hash: String::from_str(env, ""),
    };

    env.storage()
        .persistent()
        .set(&(PRESCRIPTION_KEY, rx_id), &prescription);
    env.storage().temporary().remove(&prep_key);
    crate::events::publish_prescription_committed(env, rx_id, caller);
    Ok(())
}

/// Removes a pending prescription from temporary storage and emits a rolled-back event.
///
/// # Errors
/// Returns `NotInitialized` or `Unauthorized`.
pub fn rollback_add_prescription(
    env: &Env,
    caller: Address,
    rx_id: u64,
) -> Result<(), ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, &caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, &caller)?;
    }

    env.storage().temporary().remove(&(symbol_short!("P_ADD_RX"), rx_id));
    crate::events::publish_prescription_rolled_back(env, rx_id, caller);
    Ok(())
}

/// Returns the last prescription id issued, which is also the number of ids issued so far.
pub fn get_prescription_count(env: &Env) -> u64 {
    env.storage().instance().get(&RX_COUNTER).unwrap_or(0)
}

/// Returns the committed prescription with the given `id`, subject to read access checks.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `RecordNotFound`.
pub fn get_prescription(env: &Env, caller: Address, id: u64) -> Result<Prescription, ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        let is_reader = check_permission(env, &caller, Permission::ReadAnyRecord)?;
        let rx: Prescription = env
            .storage()
            .persistent()
            .get(&(PRESCRIPTION_KEY, id))
            .ok_or(ContractError::RecordNotFound)?;

        let is_patient = rx.patient == caller;
        if !is_reader && !is_patient {
            return Err(ContractError::Unauthorized);
        }

        Ok(rx)
    } else {
        require_authorized_vision_records(env, &caller)?;
        env.storage()
            .persistent()
            .get(&(PRESCRIPTION_KEY, id))
            .ok_or(ContractError::RecordNotFound)
    }
}

/// Map a stored [`Prescription`] to a FHIR R4 [`MedicationRequest`].
///
/// * `medication[x]` is coded with SNOMED CT `50121007` (Eye glasses) or
///   `57368009` (Contact lenses) depending on `lens_type`, using the
///   canonical SNOMED system URI.
/// * Per-eye lens parameters (sphere / cylinder / axis / addition / PD) are
///   carried in `dosage_text`, labelled with the LOINC EYE.GLASSES
///   prescription codes (`28826-6`, `28827-4`, `28828-2`, `28810-0`,
///   `28842-3`, `28843-1`, `28844-9`, `28812-6`, `28858-9`).
/// * `status` is derived from the ledger clock: `active` while
///   `env.ledger().timestamp() < expires_at`, otherwise `completed`.
/// * `intent` is `order`; `authoredOn` is `issued_at`; the dispense
///   validity period spans `issued_at ..= expires_at`.
pub fn prescription_to_fhir(env: &Env, rx: &Prescription, subject_id: String) -> MedicationRequest {
    let status = if env.ledger().timestamp() < rx.expires_at {
        MedicationRequestStatus::Active
    } else {
        MedicationRequestStatus::Completed
    };

    let (med_code, med_display) = match rx.lens_type {
        LensType::Glasses => (validation::SNOMED_EYE_GLASSES, "Eye glasses"),
        LensType::ContactLens => (validation::SNOMED_CONTACT_LENSES, "Contact lenses"),
    };

    let mut dosage = alloc::format!(
        "OD: SPH {sr} [{csr}] CYL {cr} [{ccr}] AXIS {ar} [{car}] ADD {adr} [{adrc}]; \
         OS: SPH {sl} [{csl}] CYL {cl} [{ccl}] AXIS {al} [{cal}] ADD {adl} [{adlc}]; \
         PD R {pdr} L {pdl} mm [{cpd}]",
        sr = rx.right_eye.sphere,
        csr = validation::LOINC_RX_SPHERE_RIGHT,
        cr = rx.right_eye.cylinder,
        ccr = validation::LOINC_RX_CYLINDER_RIGHT,
        ar = rx.right_eye.axis,
        car = validation::LOINC_RX_AXIS_RIGHT,
        adr = rx.right_eye.add,
        adrc = validation::LOINC_RX_ADD_RIGHT,
        sl = rx.left_eye.sphere,
        csl = validation::LOINC_RX_SPHERE_LEFT,
        cl = rx.left_eye.cylinder,
        ccl = validation::LOINC_RX_CYLINDER_LEFT,
        al = rx.left_eye.axis,
        cal = validation::LOINC_RX_AXIS_LEFT,
        adl = rx.left_eye.add,
        adlc = validation::LOINC_RX_ADD_LEFT,
        pdr = rx.right_eye.pd,
        pdl = rx.left_eye.pd,
        cpd = validation::LOINC_RX_PD_BINOCULAR,
    );

    if let OptionalContactLensData::Some(cl) = &rx.contact_data {
        dosage = alloc::format!(
            "{}; CL [{}]: BC {} DIA {} brand {}",
            dosage,
            validation::SNOMED_CONTACT_LENSES,
            cl.base_curve,
            cl.diameter,
            cl.brand
        );
    }

    let id = String::from_str(env, &alloc::format!("rx-{}", rx.id));

    MedicationRequest {
        id,
        status,
        intent: MedicationRequestIntent::Order,
        medication_system: String::from_str(env, validation::SNOMED_SYSTEM),
        medication_code: String::from_str(env, med_code),
        medication_display: String::from_str(env, med_display),
        subject_id,
        requester_id: rx.provider.to_string(),
        authored_on: rx.issued_at,
        validity_period_start: rx.issued_at,
        validity_period_end: rx.expires_at,
        dosage_text: String::from_str(env, &dosage),
    }
}
