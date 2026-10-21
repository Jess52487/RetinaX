//! HL7 FHIR R4 (v4.0.1) JSON export.
//!
//! Eye examinations export as an `Observation` and prescriptions as a `MedicationRequest`. The
//! documents are returned as UTF-8 JSON bytes, the same payload format the `fhir` contract's
//! `register_resource` accepts.
//!
//! Codes used (checked against the HL7 terminology server, tx.fhir.org):
//! - Observation category `exam`, `http://terminology.hl7.org/CodeSystem/observation-category`.
//! - SNOMED CT 36228007 "Ophthalmic examination and evaluation" for the examination.
//! - LOINC 98499-7 / 98498-9 uncorrected visual acuity, 79892-6 / 79893-4 intraocular pressure,
//!   79764-7 type of tonometer, 32451-7 physical findings of macula.
//! - SNOMED CT 50121007 "Eyeglasses" and 57368009 "Contact lenses" for the prescribed product.
//!
//! Findings with no exact LOINC term (acuity with the patient's own correction, slit lamp
//! structures recorded for both eyes at once, visual field, retinal imaging and cup-to-disc
//! ratio) use a text-only `CodeableConcept`, which R4 allows, rather than a near-miss code.

use crate::errors::ContractError;
use crate::examination::{get_examination_patient, get_eye_examination};
use crate::prescription::get_prescription;
use crate::types::{
    EyeExamination, LensType, OptFundusPhotography, OptPhysicalMeasurement, OptRetinalImaging,
    OptVisualField, OptionalContactLensData, Prescription, PrescriptionData,
};
use soroban_sdk::{Address, Bytes, Env, String};

/// Size of the stack buffer the writer fills before copying into host `Bytes`.
const CHUNK_LEN: usize = 512;
/// Longest stored text field (`clinical_notes`, 2048 bytes); bounds the text scratch buffer.
const MAX_TEXT_LEN: usize = 2048;
/// 9999-12-31T23:59:59Z, the last instant a four-digit FHIR `dateTime` year can express.
const MAX_FHIR_TIMESTAMP: u64 = 253_402_300_799;
const SECS_PER_DAY: u64 = 86_400;

macro_rules! loinc {
    ($code:literal, $display:literal) => {
        concat!(
            r#"{"coding":[{"system":"http://loinc.org","code":""#,
            $code,
            r#"","display":""#,
            $display,
            r#""}]}"#
        )
    };
}

macro_rules! snomed {
    ($code:literal, $display:literal) => {
        concat!(
            r#"{"coding":[{"system":"http://snomed.info/sct","code":""#,
            $code,
            r#"","display":""#,
            $display,
            r#""}],"text":""#,
            $display,
            r#""}"#
        )
    };
}

macro_rules! text_code {
    ($text:literal) => {
        concat!(r#"{"text":""#, $text, r#""}"#)
    };
}

/// Exports the examination stored under `record_id` as a FHIR R4 `Observation`, applying the
/// same read access rules as `get_eye_examination`.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, `RecordNotFound`, or `InvalidInput` (see
/// `eye_examination_to_observation`).
pub fn export_eye_examination(
    env: &Env,
    caller: Address,
    record_id: u64,
) -> Result<Bytes, ContractError> {
    let exam = get_eye_examination(env, caller, record_id)?;
    eye_examination_to_observation(env, &exam, get_examination_patient(env, record_id))
}

/// Exports the committed prescription `id` as a FHIR R4 `MedicationRequest`, applying the same
/// read access rules as `get_prescription`.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, `RecordNotFound`, or `InvalidInput` (see
/// `prescription_to_medication_request`).
pub fn export_prescription(env: &Env, caller: Address, id: u64) -> Result<Bytes, ContractError> {
    let rx = get_prescription(env, caller, id)?;
    prescription_to_medication_request(env, &rx)
}

/// Builds a FHIR R4 `Observation` for `exam`. `patient`, when the examination has been linked to
/// one, becomes the `subject` reference.
///
/// # Complexity
/// O(n) time in the size of the examination and O(1) extra space (two fixed stack buffers).
///
/// # Errors
/// Returns `InvalidInput` if a stored text field is not valid UTF-8 or the measurement timestamp
/// is beyond the FHIR `dateTime` range.
fn eye_examination_to_observation(
    env: &Env,
    exam: &EyeExamination,
    patient: Option<Address>,
) -> Result<Bytes, ContractError> {
    let mut w = JsonWriter::new(env);
    w.raw(r#"{"resourceType":"Observation","id":"eye-exam-"#);
    w.uint(exam.record_id);
    w.raw(concat!(
        r#"","status":"final","category":[{"coding":[{"system":"#,
        r#""http://terminology.hl7.org/CodeSystem/observation-category","code":"exam","#,
        r#""display":"Exam"}]}],"code":"#,
        snomed!("36228007", "Ophthalmic examination and evaluation")
    ));
    if let Some(patient) = patient {
        w.raw(r#","subject":{"reference":"Patient/"#);
        w.text(&patient.to_string())?;
        w.raw(r#""}"#);
    }
    // The IOP reading carries the only clinical timestamp recorded with an examination.
    if exam.iop.timestamp != 0 {
        w.raw(r#","effectiveDateTime":""#);
        w.date_time(exam.iop.timestamp)?;
        w.raw(r#"""#);
    }
    if !exam.clinical_notes.is_empty() {
        w.raw(r#","note":[{"text":""#);
        w.text(&exam.clinical_notes)?;
        w.raw(r#""}]"#);
    }

    w.raw(r#","component":["#);
    let mut c = Components::default();
    let acuity = &exam.visual_acuity;
    c.string(
        &mut w,
        loinc!("98499-7", "Visual acuity uncorrected Right eye"),
        &acuity.uncorrected.right_eye,
    )?;
    c.string(
        &mut w,
        loinc!("98498-9", "Visual acuity uncorrected Left eye"),
        &acuity.uncorrected.left_eye,
    )?;
    if let OptPhysicalMeasurement::Some(corrected) = &acuity.corrected {
        c.string(
            &mut w,
            text_code!("Visual acuity with correction Right eye"),
            &corrected.right_eye,
        )?;
        c.string(
            &mut w,
            text_code!("Visual acuity with correction Left eye"),
            &corrected.left_eye,
        )?;
    }
    c.pressure(
        &mut w,
        loinc!("79892-6", "Right eye Intraocular pressure"),
        exam.iop.right_eye,
    );
    c.pressure(
        &mut w,
        loinc!("79893-4", "Left eye Intraocular pressure"),
        exam.iop.left_eye,
    );
    c.string(
        &mut w,
        loinc!("79764-7", "Type of Tonometer"),
        &exam.iop.method,
    )?;
    let slit_lamp = &exam.slit_lamp;
    c.string(
        &mut w,
        text_code!("Slit lamp examination of cornea"),
        &slit_lamp.cornea,
    )?;
    c.string(
        &mut w,
        text_code!("Slit lamp examination of anterior chamber"),
        &slit_lamp.anterior_chamber,
    )?;
    c.string(
        &mut w,
        text_code!("Slit lamp examination of iris"),
        &slit_lamp.iris,
    )?;
    c.string(
        &mut w,
        text_code!("Slit lamp examination of lens"),
        &slit_lamp.lens,
    )?;
    if let OptVisualField::Some(field) = &exam.visual_field {
        c.string(
            &mut w,
            text_code!("Visual field test reliability Right eye"),
            &field.right_eye_reliability,
        )?;
        c.string(
            &mut w,
            text_code!("Visual field test reliability Left eye"),
            &field.left_eye_reliability,
        )?;
        c.string(
            &mut w,
            text_code!("Visual field defects Right eye"),
            &field.right_eye_defects,
        )?;
        c.string(
            &mut w,
            text_code!("Visual field defects Left eye"),
            &field.left_eye_defects,
        )?;
    }
    if let OptRetinalImaging::Some(image) = &exam.retina_imaging {
        c.string(
            &mut w,
            text_code!("Retinal imaging findings"),
            &image.findings,
        )?;
    }
    if let OptFundusPhotography::Some(photo) = &exam.fundus_photo {
        c.string(
            &mut w,
            text_code!("Cup-to-disc ratio Right eye"),
            &photo.cup_to_disc_ratio_right,
        )?;
        c.string(
            &mut w,
            text_code!("Cup-to-disc ratio Left eye"),
            &photo.cup_to_disc_ratio_left,
        )?;
        c.string(
            &mut w,
            loinc!("32451-7", "Physical findings of Macula"),
            &photo.macula_status,
        )?;
    }
    w.raw("]}");
    Ok(w.finish())
}

/// Builds a FHIR R4 `MedicationRequest` for `rx`.
///
/// `status` is `active` until `expires_at` and `stopped` afterwards; the validity window is also
/// carried in `dispenseRequest.validityPeriod`. The refraction values go in the dosage text.
///
/// # Complexity
/// O(n) time in the size of the prescription and O(1) extra space.
///
/// # Errors
/// Returns `InvalidInput` if a stored text field is not valid UTF-8 or a timestamp is beyond the
/// FHIR `dateTime` range.
fn prescription_to_medication_request(
    env: &Env,
    rx: &Prescription,
) -> Result<Bytes, ContractError> {
    let mut w = JsonWriter::new(env);
    w.raw(r#"{"resourceType":"MedicationRequest","id":"rx-"#);
    w.uint(rx.id);
    w.raw(if env.ledger().timestamp() < rx.expires_at {
        r#"","status":"active""#
    } else {
        r#"","status":"stopped""#
    });
    w.raw(r#","intent":"order","medicationCodeableConcept":"#);
    w.raw(match rx.lens_type {
        LensType::Glasses => snomed!("50121007", "Eyeglasses"),
        LensType::ContactLens => snomed!("57368009", "Contact lenses"),
    });
    w.raw(r#","subject":{"reference":"Patient/"#);
    w.text(&rx.patient.to_string())?;
    w.raw(r#""},"authoredOn":""#);
    w.date_time(rx.issued_at)?;
    w.raw(r#"","requester":{"reference":"Practitioner/"#);
    w.text(&rx.provider.to_string())?;
    w.raw(r#""},"dosageInstruction":[{"text":"OD: "#);
    w.refraction(&rx.right_eye)?;
    w.raw("; OS: ");
    w.refraction(&rx.left_eye)?;
    if let OptionalContactLensData::Some(contact) = &rx.contact_data {
        w.raw("; Contact lens: BC ");
        w.text(&contact.base_curve)?;
        w.raw(" DIA ");
        w.text(&contact.diameter)?;
        w.raw(" Brand ");
        w.text(&contact.brand)?;
    }
    w.raw(r#""}],"dispenseRequest":{"validityPeriod":{"start":""#);
    w.date_time(rx.issued_at)?;
    w.raw(r#"","end":""#);
    w.date_time(rx.expires_at)?;
    w.raw(r#""}}}"#);
    Ok(w.finish())
}

/// Tracks whether an `Observation.component` entry needs a leading comma.
#[derive(Default)]
struct Components {
    written: bool,
}

impl Components {
    /// Writes a component with a `valueString`. Empty values are skipped, since FHIR strings must
    /// not be empty.
    fn string(
        &mut self,
        w: &mut JsonWriter,
        code: &str,
        value: &String,
    ) -> Result<(), ContractError> {
        if value.is_empty() {
            return Ok(());
        }
        self.open(w, code);
        w.raw(r#","valueString":""#);
        w.text(value)?;
        w.raw(r#""}"#);
        Ok(())
    }

    /// Writes a component with an intraocular pressure `valueQuantity` in mmHg (UCUM `mm[Hg]`).
    fn pressure(&mut self, w: &mut JsonWriter, code: &str, mm_hg: u32) {
        self.open(w, code);
        w.raw(r#","valueQuantity":{"value":"#);
        w.uint(u64::from(mm_hg));
        w.raw(r#","unit":"mmHg","system":"http://unitsofmeasure.org","code":"mm[Hg]"}}"#);
    }

    fn open(&mut self, w: &mut JsonWriter, code: &str) {
        if self.written {
            w.raw(",");
        }
        self.written = true;
        w.raw(r#"{"code":"#);
        w.raw(code);
    }
}

/// Appends JSON to host `Bytes` through a fixed stack buffer, so the number of host calls grows
/// with the output size rather than with the number of fragments written.
struct JsonWriter {
    out: Bytes,
    buf: [u8; CHUNK_LEN],
    len: usize,
}

impl JsonWriter {
    fn new(env: &Env) -> Self {
        Self {
            out: Bytes::new(env),
            buf: [0; CHUNK_LEN],
            len: 0,
        }
    }

    fn raw(&mut self, s: &str) {
        self.push(s.as_bytes());
    }

    fn push(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if self.len == CHUNK_LEN {
                self.flush();
            }
            let n = (CHUNK_LEN - self.len).min(bytes.len());
            self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
            self.len += n;
            bytes = &bytes[n..];
        }
    }

    fn flush(&mut self) {
        self.out.extend_from_slice(&self.buf[..self.len]);
        self.len = 0;
    }

    fn finish(mut self) -> Bytes {
        self.flush();
        self.out
    }

    /// Writes the contents of a JSON string (without quotes), escaped per RFC 8259.
    fn text(&mut self, value: &String) -> Result<(), ContractError> {
        let len = value.len() as usize;
        if len > MAX_TEXT_LEN {
            return Err(ContractError::InvalidInput);
        }
        let mut scratch = [0u8; MAX_TEXT_LEN];
        let bytes = &mut scratch[..len];
        value.copy_into_slice(bytes);
        if core::str::from_utf8(bytes).is_err() {
            return Err(ContractError::InvalidInput);
        }

        // Copy unescaped runs in one push and only break them at bytes that need escaping.
        let mut start = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let short: Option<&[u8]> = match b {
                b'"' => Some(b"\\\""),
                b'\\' => Some(b"\\\\"),
                b'\n' => Some(b"\\n"),
                b'\r' => Some(b"\\r"),
                b'\t' => Some(b"\\t"),
                0x08 => Some(b"\\b"),
                0x0C => Some(b"\\f"),
                0x00..=0x1F => None,
                _ => continue,
            };
            self.push(&bytes[start..i]);
            match short {
                Some(escape) => self.push(escape),
                None => self.push(&unicode_escape(b)),
            }
            start = i + 1;
        }
        self.push(&bytes[start..]);
        Ok(())
    }

    fn refraction(&mut self, data: &PrescriptionData) -> Result<(), ContractError> {
        self.raw("SPH ");
        self.text(&data.sphere)?;
        self.raw(" CYL ");
        self.text(&data.cylinder)?;
        self.raw(" AXIS ");
        self.text(&data.axis)?;
        self.raw(" ADD ");
        self.text(&data.add)?;
        self.raw(" PD ");
        self.text(&data.pd)
    }

    fn uint(&mut self, mut n: u64) {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(&digits[i..]);
    }

    /// Writes a Unix timestamp as a FHIR `dateTime` in UTC, `YYYY-MM-DDThh:mm:ssZ`.
    fn date_time(&mut self, timestamp: u64) -> Result<(), ContractError> {
        if timestamp > MAX_FHIR_TIMESTAMP {
            return Err(ContractError::InvalidInput);
        }
        let (year, month, day) = civil_from_days(timestamp / SECS_PER_DAY);
        let secs = timestamp % SECS_PER_DAY;
        let mut out = *b"0000-00-00T00:00:00Z";
        write_digits(&mut out[0..4], year);
        write_digits(&mut out[5..7], month);
        write_digits(&mut out[8..10], day);
        write_digits(&mut out[11..13], secs / 3600);
        write_digits(&mut out[14..16], secs % 3600 / 60);
        write_digits(&mut out[17..19], secs % 60);
        self.push(&out);
        Ok(())
    }
}

/// Returns the JSON `\u00XX` escape for a control character.
fn unicode_escape(b: u8) -> [u8; 6] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    [
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX[usize::from(b >> 4)],
        HEX[usize::from(b & 0x0F)],
    ]
}

/// Fills `out` with the zero-padded decimal digits of `value`.
fn write_digits(out: &mut [u8], mut value: u64) {
    for slot in out.iter_mut().rev() {
        *slot = b'0' + (value % 10) as u8;
        value /= 10;
    }
}

/// Converts days since 1970-01-01 to a proleptic Gregorian (year, month, day), using Howard
/// Hinnant's `civil_from_days` algorithm restricted to non-negative day counts.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}
