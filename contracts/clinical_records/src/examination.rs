use crate::admin::{
    check_permission, get_access_control_contract, require_authorized_vision_records,
    require_initialized,
};
use crate::errors::ContractError;
use crate::types::{
    EyeExamination, IntraocularPressure, OptFundusPhotography, OptPhysicalMeasurement,
    OptRetinalImaging, OptVisualField, PhysicalMeasurement, SlitLampFindings, VisualAcuity,
};
use access_control::Permission;
use soroban_sdk::{symbol_short, Address, Env, String, Symbol};

pub const EXAM_KEY: Symbol = symbol_short!("EXAM");
pub const EXAM_PATIENT_KEY: Symbol = symbol_short!("EX_PAT");

/// Checks that `value` has a byte length between `min_len` and `max_len`, inclusive.
///
/// # Errors
/// Returns `InvalidInput` if the length is out of range.
pub fn validate_string(value: &String, min_len: u32, max_len: u32) -> Result<(), ContractError> {
    let len = value.len();
    if len < min_len || len > max_len {
        return Err(ContractError::InvalidInput);
    }
    Ok(())
}

fn validate_physical_measurement(value: &PhysicalMeasurement) -> Result<(), ContractError> {
    validate_string(&value.left_eye, 1, 64)?;
    validate_string(&value.right_eye, 1, 64)?;
    Ok(())
}

fn validate_visual_acuity(value: &VisualAcuity) -> Result<(), ContractError> {
    validate_physical_measurement(&value.uncorrected)?;
    if let OptPhysicalMeasurement::Some(corrected) = &value.corrected {
        validate_physical_measurement(corrected)?;
    }
    Ok(())
}

fn validate_iop(value: &IntraocularPressure) -> Result<(), ContractError> {
    validate_string(&value.method, 1, 64)?;
    Ok(())
}

fn validate_slit_lamp(value: &SlitLampFindings) -> Result<(), ContractError> {
    validate_string(&value.cornea, 1, 256)?;
    validate_string(&value.anterior_chamber, 1, 256)?;
    validate_string(&value.iris, 1, 256)?;
    validate_string(&value.lens, 1, 256)?;
    Ok(())
}

fn validate_visual_field(value: &OptVisualField) -> Result<(), ContractError> {
    if let OptVisualField::Some(field) = value {
        validate_string(&field.left_eye_reliability, 0, 128)?;
        validate_string(&field.right_eye_reliability, 0, 128)?;
        validate_string(&field.left_eye_defects, 0, 256)?;
        validate_string(&field.right_eye_defects, 0, 256)?;
    }
    Ok(())
}

fn validate_retinal_imaging(value: &OptRetinalImaging) -> Result<(), ContractError> {
    if let OptRetinalImaging::Some(image) = value {
        validate_string(&image.image_url, 1, 256)?;
        validate_string(&image.image_hash, 1, 128)?;
        validate_string(&image.findings, 0, 512)?;
    }
    Ok(())
}

fn validate_fundus_photo(value: &OptFundusPhotography) -> Result<(), ContractError> {
    if let OptFundusPhotography::Some(photo) = value {
        validate_string(&photo.image_url, 1, 256)?;
        validate_string(&photo.image_hash, 1, 128)?;
        validate_string(&photo.cup_to_disc_ratio_left, 1, 64)?;
        validate_string(&photo.cup_to_disc_ratio_right, 1, 64)?;
        validate_string(&photo.macula_status, 0, 256)?;
    }
    Ok(())
}

/// Validates every free-text field of an examination against its length limits.
///
/// The visual field and retinal imaging fields are only checked when they are present.
///
/// # Errors
/// Returns `InvalidInput` if any field is outside its allowed length.
pub fn validate_examination_strings(
    visual_acuity: &VisualAcuity,
    iop: &IntraocularPressure,
    slit_lamp: &SlitLampFindings,
    visual_field: &OptVisualField,
    retina_imaging: &OptRetinalImaging,
    fundus_photo: &OptFundusPhotography,
    clinical_notes: &String,
) -> Result<(), ContractError> {
    validate_visual_acuity(visual_acuity)?;
    validate_iop(iop)?;
    validate_slit_lamp(slit_lamp)?;
    validate_visual_field(visual_field)?;
    validate_retinal_imaging(retina_imaging)?;
    validate_fundus_photo(fundus_photo)?;
    validate_string(clinical_notes, 0, 2048)?;
    Ok(())
}

/// Validates and stores an eye examination under `record_id`, then emits an event.
///
/// See the contract method `add_eye_examination` for the authorization rules.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `InvalidInput`.
pub fn add_eye_examination(
    env: &Env,
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
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, &caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, &caller)?;
    }

    validate_examination_strings(
        &visual_acuity,
        &iop,
        &slit_lamp,
        &visual_field,
        &retina_imaging,
        &fundus_photo,
        &clinical_notes,
    )?;

    let exam = EyeExamination {
        record_id,
        visual_acuity,
        iop,
        slit_lamp,
        visual_field,
        retina_imaging,
        fundus_photo,
        clinical_notes,
    };

    env.storage()
        .persistent()
        .set(&(EXAM_KEY, record_id), &exam);
    crate::events::publish_eye_examination_added(env, record_id, caller);
    Ok(())
}

/// Records which patient owns the examination stored under `record_id`.
///
/// # Errors
/// Returns `NotInitialized` or `Unauthorized`.
pub fn set_examination_patient(
    env: &Env,
    caller: Address,
    record_id: u64,
    patient: Address,
) -> Result<(), ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, &caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, &caller)?;
    }

    env.storage()
        .persistent()
        .set(&(EXAM_PATIENT_KEY, record_id), &patient);
    Ok(())
}

/// Returns the examination stored under `record_id`, subject to read access checks.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `RecordNotFound`.
pub fn get_eye_examination(
    env: &Env,
    caller: Address,
    record_id: u64,
) -> Result<EyeExamination, ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        let is_reader = check_permission(env, &caller, Permission::ReadAnyRecord)?;
        let is_patient = env
            .storage()
            .persistent()
            .get::<_, Address>(&(EXAM_PATIENT_KEY, record_id))
            .map(|p| p == caller)
            .unwrap_or(false);

        if !is_reader && !is_patient {
            return Err(ContractError::Unauthorized);
        }

        env.storage()
            .persistent()
            .get(&(EXAM_KEY, record_id))
            .ok_or(ContractError::RecordNotFound)
    } else {
        require_authorized_vision_records(env, &caller)?;
        env.storage()
            .persistent()
            .get(&(EXAM_KEY, record_id))
            .ok_or(ContractError::RecordNotFound)
    }
}
