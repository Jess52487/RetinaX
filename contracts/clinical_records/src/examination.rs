use crate::admin::require_authorized_vision_records;
use crate::errors::ContractError;
use crate::types::{
    EyeExamination, IntraocularPressure, OptFundusPhotography, OptPhysicalMeasurement,
    OptRetinalImaging, OptVisualField, PhysicalMeasurement, SlitLampFindings, VisualAcuity,
};
use fhir::types::{Observation, ObservationStatus};
use fhir::validation;
use soroban_sdk::{symbol_short, Address, Env, String, Symbol, Vec};

pub const EXAM_KEY: Symbol = symbol_short!("EXAM");

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
    require_authorized_vision_records(env, &caller)?;
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

pub fn get_eye_examination(
    env: &Env,
    caller: Address,
    record_id: u64,
) -> Result<EyeExamination, ContractError> {
    require_authorized_vision_records(env, &caller)?;
    env.storage()
        .persistent()
        .get(&(EXAM_KEY, record_id))
        .ok_or(ContractError::RecordNotFound)
}

/// Map an [`EyeExamination`] to FHIR R4 [`Observation`] resources.
///
/// Emits one observation per measurement group, each with:
/// * canonical LOINC system URI (`http://loinc.org`)
/// * `status` = `final`
/// * `subject` = `subject_id`
/// * `effective[x]` = the examination's IOP timestamp
///
/// Observations emitted:
/// 1. Uncorrected visual acuity, right eye — LOINC `98499-7`
/// 2. Uncorrected visual acuity, left eye — LOINC `98498-9`
/// 3. Best-corrected visual acuity (right/left) when recorded — LOINC `65893-0` / `65897-1`
/// 4. Intraocular pressure — LOINC `56844-4`
/// 5. Eye physical examination narrative (clinical notes) — LOINC `29271-4`
pub fn eye_examination_to_fhir(
    env: &Env,
    exam: &EyeExamination,
    subject_id: String,
) -> Vec<Observation> {
    let loinc = String::from_str(env, validation::LOINC_SYSTEM);
    let effective = exam.iop.timestamp;
    let mut observations = Vec::new(env);

    let mut push = |id: String, code: &str, value: String| {
        observations.push_back(Observation {
            id,
            status: ObservationStatus::Final,
            code_system: loinc.clone(),
            code_value: String::from_str(env, code),
            subject_id: subject_id.clone(),
            value,
            effective_datetime: effective,
        });
    };

    push(
        String::from_str(
            env,
            &alloc::format!("exam-{}-va-uncorr-right", exam.record_id),
        ),
        validation::LOINC_VISUAL_ACUITY_UNCORR_RIGHT,
        exam.visual_acuity.uncorrected.right_eye.clone(),
    );
    push(
        String::from_str(
            env,
            &alloc::format!("exam-{}-va-uncorr-left", exam.record_id),
        ),
        validation::LOINC_VISUAL_ACUITY_UNCORR_LEFT,
        exam.visual_acuity.uncorrected.left_eye.clone(),
    );

    if let OptPhysicalMeasurement::Some(corrected) = &exam.visual_acuity.corrected {
        push(
            String::from_str(
                env,
                &alloc::format!("exam-{}-va-best-corr-right", exam.record_id),
            ),
            validation::LOINC_VISUAL_ACUITY_BEST_CORR_RIGHT,
            corrected.right_eye.clone(),
        );
        push(
            String::from_str(
                env,
                &alloc::format!("exam-{}-va-best-corr-left", exam.record_id),
            ),
            validation::LOINC_VISUAL_ACUITY_BEST_CORR_LEFT,
            corrected.left_eye.clone(),
        );
    }

    push(
        String::from_str(env, &alloc::format!("exam-{}-iop", exam.record_id)),
        validation::LOINC_IOP,
        String::from_str(
            env,
            &alloc::format!(
                "L:{} R:{} ({})",
                exam.iop.left_eye,
                exam.iop.right_eye,
                exam.iop.method
            ),
        ),
    );
    push(
        String::from_str(env, &alloc::format!("exam-{}-narrative", exam.record_id)),
        validation::LOINC_EYE_PHYSICAL_EXAM,
        exam.clinical_notes.clone(),
    );

    observations
}
