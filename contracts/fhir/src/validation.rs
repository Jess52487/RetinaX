//! FHIR R4 Observation validation for RetinaX clinical data.
//!
//! Validates that an [`Observation`] carries every required R4 field
//! (`status`, `code`, `subject`, `effective[x]`) and that its `code.coding`
//! references a recognised terminology system (LOINC or SNOMED CT) using the
//! canonical FHIR system URIs.
//!
//! Eye-examination-specific helpers additionally verify the coding is one of
//! the optometry codes RetinaX uses to map `EyeExamination` records.

use crate::types::{FhirError, MedicationRequest, MedicationRequestIntent, Observation};

// ── Canonical FHIR code-system URIs ──────────────────────────────────────────

/// LOINC canonical system URI as required by FHIR R4.
pub const LOINC_SYSTEM: &str = "http://loinc.org";
/// SNOMED CT canonical system URI as required by FHIR R4.
pub const SNOMED_SYSTEM: &str = "http://snomed.info/sct";

// ── Eye-examination LOINC codes ──────────────────────────────────────────────

/// Eye physical examination panel.
pub const LOINC_EYE_PHYSICAL_EXAM: &str = "29271-4";
/// Intraocular pressure of Eye (IOP).
pub const LOINC_IOP: &str = "56844-4";
/// Visual acuity uncorrected, right eye.
pub const LOINC_VISUAL_ACUITY_UNCORR_RIGHT: &str = "98499-7";
/// Visual acuity uncorrected, left eye.
pub const LOINC_VISUAL_ACUITY_UNCORR_LEFT: &str = "98498-9";
/// Visual acuity best corrected, right eye.
pub const LOINC_VISUAL_ACUITY_BEST_CORR_RIGHT: &str = "65893-0";
/// Visual acuity best corrected, left eye.
pub const LOINC_VISUAL_ACUITY_BEST_CORR_LEFT: &str = "65897-1";
/// Visual acuity panel.
pub const LOINC_VISUAL_ACUITY_PANEL: &str = "98497-1";

// ── Eye-examination SNOMED CT codes ─────────────────────────────────────────

/// Visual acuity (observable entity) — parent concept.
pub const SNOMED_VISUAL_ACUITY: &str = "363983007";
/// Distance visual acuity (observable entity).
pub const SNOMED_DISTANCE_VISUAL_ACUITY: &str = "251739003";
/// Uncorrected visual acuity (observable entity).
pub const SNOMED_UNCORRECTED_VISUAL_ACUITY: &str = "420050001";

// ── Glasses-prescription LOINC codes (EYE.GLASSES class) ────────────────────
//
// Full codes from the LOINC "Glasses prescription" family, as used by the
// HL7 FHIR DeviceRequest lens examples (devicerequest-right-lens /
// devicerequest-left-lens).

/// Right glasses lens prescription — Sphere far.
pub const LOINC_RX_SPHERE_RIGHT: &str = "28826-6";
/// Right glasses lens prescription — Cylinder far.
pub const LOINC_RX_CYLINDER_RIGHT: &str = "28827-4";
/// Right glasses lens prescription — Axis far.
pub const LOINC_RX_AXIS_RIGHT: &str = "28828-2";
/// Right corrective lens Add 1 by Lensmeter.
pub const LOINC_RX_ADD_RIGHT: &str = "28810-0";
/// Left glasses lens prescription — Sphere far.
pub const LOINC_RX_SPHERE_LEFT: &str = "28842-3";
/// Left glasses lens prescription — Cylinder far.
pub const LOINC_RX_CYLINDER_LEFT: &str = "28843-1";
/// Left glasses lens prescription — Axis far.
pub const LOINC_RX_AXIS_LEFT: &str = "28844-9";
/// Left corrective lens Add 1 by Lensmeter.
pub const LOINC_RX_ADD_LEFT: &str = "28812-6";
/// Glasses prescription — Pupillary distance, binocular.
pub const LOINC_RX_PD_BINOCULAR: &str = "28858-9";

// ── SNOMED CT lens-device codes used as medication[x] ───────────────────────

/// Eye glasses, device (physical object).
pub const SNOMED_EYE_GLASSES: &str = "50121007";
/// Contact lenses, device (physical object).
pub const SNOMED_CONTACT_LENSES: &str = "57368009";

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Compare a Soroban `String` with a `&str` byte-for-byte without allocating
/// or panicking on malformed UTF-8.
fn soroban_str_eq(value: &soroban_sdk::String, expected: &str) -> bool {
    let bytes = value.to_bytes();
    if bytes.len() as usize != expected.len() {
        return false;
    }
    bytes.iter().eq(expected.bytes())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminology {
    Loinc,
    Snomed,
}

/// Recognise the coding system, accepting both canonical FHIR URIs and the
/// legacy shorthand labels historically used across RetinaX contracts.
fn system_kind(system: &soroban_sdk::String) -> Option<Terminology> {
    if soroban_str_eq(system, LOINC_SYSTEM) || soroban_str_eq(system, "LOINC") {
        Some(Terminology::Loinc)
    } else if soroban_str_eq(system, SNOMED_SYSTEM)
        || soroban_str_eq(system, "SNOMED")
        || soroban_str_eq(system, "SNOMED CT")
        || soroban_str_eq(system, "SNOMED-CT")
    {
        Some(Terminology::Snomed)
    } else {
        None
    }
}

// ── Public validation API ────────────────────────────────────────────────────

/// Returns `true` when `system` is a recognised LOINC or SNOMED CT reference.
pub fn is_supported_coding_system(system: &soroban_sdk::String) -> bool {
    system_kind(system).is_some()
}

/// Validate an [`Observation`] against the FHIR R4 required-field rules:
///
/// * `status` — always present (required enum field in the RetinaX schema).
/// * `code`   — `code_system` and `code_value` must both be non-empty.
/// * `subject`— `subject_id` must be non-empty.
/// * `effective[x]` — `effective_datetime` must be non-zero.
/// * `code.coding.system` — must reference LOINC or SNOMED CT.
pub fn validate_observation_r4(obs: &Observation) -> Result<(), FhirError> {
    // status: required by FHIR R4; the ObservationStatus enum guarantees a
    // value is always present, so there is nothing to reject here.

    if obs.code_system.is_empty() || obs.code_value.is_empty() {
        return Err(FhirError::InvalidRecordData); // code required
    }
    if obs.subject_id.is_empty() {
        return Err(FhirError::InvalidRecordData); // subject required
    }
    if obs.effective_datetime == 0 {
        return Err(FhirError::InvalidRecordData); // effective[x] required
    }
    if !is_supported_coding_system(&obs.code_system) {
        return Err(FhirError::InvalidPayload); // unknown coding system
    }
    Ok(())
}

/// Validate an eye-examination [`Observation`]: full R4 field validation plus
/// a check that the coding references one of the optometry codes used to
/// represent `EyeExamination` data (LOINC eye/VA/IOP codes or SNOMED
/// visual-acuity codes).
pub fn validate_eye_examination_observation(obs: &Observation) -> Result<(), FhirError> {
    validate_observation_r4(obs)?;

    let Some(kind) = system_kind(&obs.code_system) else {
        // Unreachable after validate_observation_r4, kept defensive.
        return Err(FhirError::InvalidPayload);
    };

    let recognised = match kind {
        Terminology::Loinc => {
            let c = &obs.code_value;
            soroban_str_eq(c, LOINC_EYE_PHYSICAL_EXAM)
                || soroban_str_eq(c, LOINC_IOP)
                || soroban_str_eq(c, LOINC_VISUAL_ACUITY_UNCORR_RIGHT)
                || soroban_str_eq(c, LOINC_VISUAL_ACUITY_UNCORR_LEFT)
                || soroban_str_eq(c, LOINC_VISUAL_ACUITY_BEST_CORR_RIGHT)
                || soroban_str_eq(c, LOINC_VISUAL_ACUITY_BEST_CORR_LEFT)
                || soroban_str_eq(c, LOINC_VISUAL_ACUITY_PANEL)
        }
        Terminology::Snomed => {
            let c = &obs.code_value;
            soroban_str_eq(c, SNOMED_VISUAL_ACUITY)
                || soroban_str_eq(c, SNOMED_DISTANCE_VISUAL_ACUITY)
                || soroban_str_eq(c, SNOMED_UNCORRECTED_VISUAL_ACUITY)
        }
    };

    if recognised {
        Ok(())
    } else {
        Err(FhirError::InvalidPayload)
    }
}

// ── MedicationRequest validation (issue #318) ───────────────────────────────

/// Returns `true` when `code` is one of the LOINC glasses-prescription
/// (EYE.GLASSES) codes RetinaX uses to label lens parameters.
pub fn is_lens_prescription_code(code: &soroban_sdk::String) -> bool {
    soroban_str_eq(code, LOINC_RX_SPHERE_RIGHT)
        || soroban_str_eq(code, LOINC_RX_CYLINDER_RIGHT)
        || soroban_str_eq(code, LOINC_RX_AXIS_RIGHT)
        || soroban_str_eq(code, LOINC_RX_ADD_RIGHT)
        || soroban_str_eq(code, LOINC_RX_SPHERE_LEFT)
        || soroban_str_eq(code, LOINC_RX_CYLINDER_LEFT)
        || soroban_str_eq(code, LOINC_RX_AXIS_LEFT)
        || soroban_str_eq(code, LOINC_RX_ADD_LEFT)
        || soroban_str_eq(code, LOINC_RX_PD_BINOCULAR)
}

/// Returns `true` when `code` is a recognised lens-device medication code
/// (SNOMED CT Eye glasses or Contact lenses).
pub fn is_lens_device_code(code: &soroban_sdk::String) -> bool {
    soroban_str_eq(code, SNOMED_EYE_GLASSES) || soroban_str_eq(code, SNOMED_CONTACT_LENSES)
}

/// Validate a [`MedicationRequest`] against the FHIR R4 required-field rules
/// and RetinaX lens-prescription terminology:
///
/// * `id` — must be non-empty.
/// * `intent` — must be `Order` (the only intent RetinaX emits for written
///   prescriptions).
/// * `medication[x]` — `medication_system`/`medication_code` must both be
///   non-empty; the coding system must be the canonical SNOMED CT URI and the
///   code must be a lens device (`50121007` Eye glasses or `57368009` Contact
///   lenses).
/// * `subject` — `subject_id` must be non-empty.
/// * `authoredOn` — `authored_on` must be non-zero.
/// * `dispenseRequest.validityPeriod` — `validity_period_end` must not
///   precede `validity_period_start`.
pub fn validate_medication_request_r4(request: &MedicationRequest) -> Result<(), FhirError> {
    if request.id.is_empty() {
        return Err(FhirError::InvalidRecordData); // id required
    }
    if request.intent != MedicationRequestIntent::Order {
        return Err(FhirError::InvalidPayload); // lens prescriptions are orders
    }
    if request.medication_system.is_empty() || request.medication_code.is_empty() {
        return Err(FhirError::InvalidRecordData); // medication[x] required
    }
    if request.subject_id.is_empty() {
        return Err(FhirError::InvalidRecordData); // subject required
    }
    if request.authored_on == 0 {
        return Err(FhirError::InvalidRecordData); // authoredOn required
    }
    if request.validity_period_end < request.validity_period_start {
        return Err(FhirError::InvalidRecordData); // validityPeriod invalid
    }
    if !soroban_str_eq(&request.medication_system, SNOMED_SYSTEM) {
        return Err(FhirError::InvalidPayload); // unknown coding system
    }
    if !is_lens_device_code(&request.medication_code) {
        return Err(FhirError::InvalidPayload); // unknown lens device
    }
    Ok(())
}
