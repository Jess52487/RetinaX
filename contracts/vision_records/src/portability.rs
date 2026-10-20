//! Data portability export (issue #321).
//!
//! Renders a patient's clinical data as a FHIR R4 `collection` Bundle (JSON)
//! or as CSV. Output is assembled in `alloc` buffers so the module stays
//! `no_std`; the contract returns it as Soroban `Bytes`.
//!
//! ## What is exported
//!
//! | Source                | FHIR resource        | CSV section      |
//! |-----------------------|----------------------|------------------|
//! | Vision records        | `Observation`        | `record`         |
//! | Eye examinations      | `Observation` parts  | `record`         |
//! | Prescriptions         | `VisionPrescription` | `prescription`   |
//! | Access grants         | `Consent`            | `access_grant`   |
//!
//! ## What is withheld (HIPAA scope)
//!
//! Internal identifiers are never written: record IDs, prescription IDs,
//! examination keys, the patient address, provider addresses, and content
//! hashes or image URLs (these are integrity pointers, not clinical data).
//! Grantee addresses are included on access grants because they are the
//! subject of the grant.

#![allow(clippy::arithmetic_side_effects)]

use alloc::{format, string::String as StdString, vec::Vec as StdVec};
use soroban_sdk::{contracttype, String};

use crate::examination::{
    EyeExamination, OptFundusPhotography, OptPhysicalMeasurement, OptRetinalImaging,
    OptVisualField, PhysicalMeasurement,
};
use crate::prescription::{LensType, OptionalContactLensData, Prescription, PrescriptionData};
use crate::{AccessGrant, AccessLevel, RecordType, VisionRecord};

/// Upper bound on records returned by one `export_patient_data` call. Each
/// record costs one or two storage reads, so a page must stay small enough to
/// fit in a single transaction's read budget. Callers page with `offset`.
pub const MAX_EXPORT_PAGE_SIZE: u32 = 50;

/// Output format requested by the caller of `export_patient_data`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortabilityFormat {
    FhirJson,
    Csv,
}

/// One stored record together with its examination payload, when it has one.
pub struct ExportEntry {
    pub record: VisionRecord,
    pub exam: Option<EyeExamination>,
}

/// Everything a single export covers for one patient.
pub struct PatientExport {
    /// Ledger time at export; used to decide whether prescriptions and grants
    /// are still active.
    pub now: u64,
    pub entries: StdVec<ExportEntry>,
    pub prescriptions: StdVec<Prescription>,
    pub grants: StdVec<AccessGrant>,
}

/// A value in an examination, either free text or an intraocular pressure.
enum ExamValue {
    Text(StdString),
    Mmhg(u32),
}

struct ExamField {
    label: StdString,
    value: ExamValue,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Render the export as a FHIR R4 `collection` Bundle.
pub fn render_fhir_bundle(export: &PatientExport) -> StdString {
    let mut entries: StdVec<StdString> = StdVec::new();
    for entry in &export.entries {
        entries.push(json_object(&[("resource", fhir_observation(entry))]));
    }
    for rx in &export.prescriptions {
        entries.push(json_object(&[(
            "resource",
            fhir_vision_prescription(rx, export.now),
        )]));
    }
    for grant in &export.grants {
        entries.push(json_object(&[(
            "resource",
            fhir_consent(grant, export.now),
        )]));
    }

    json_object(&[
        ("resourceType", json_str("Bundle")),
        ("type", json_str("collection")),
        ("total", format!("{}", entries.len())),
        ("entry", json_array(&entries)),
    ])
}

/// Render the export as RFC 4180 CSV with the header
/// `section,record_type,date,field,value`.
pub fn render_csv(export: &PatientExport) -> StdString {
    let mut out = StdString::from("section,record_type,date,field,value\n");

    for entry in &export.entries {
        let record_type = record_type_name(&entry.record.record_type);
        let date = iso_date(entry.record.created_at);
        push_csv_row(
            &mut out,
            "record",
            record_type,
            &date,
            "record_type",
            record_type,
        );
        if let Some(exam) = &entry.exam {
            for field in exam_fields(exam) {
                let value = match &field.value {
                    ExamValue::Text(text) => text.clone(),
                    ExamValue::Mmhg(mmhg) => format!("{} mmHg", mmhg),
                };
                push_csv_row(&mut out, "record", record_type, &date, &field.label, &value);
            }
        }
    }

    for rx in &export.prescriptions {
        let lens = lens_type_name(&rx.lens_type);
        let date = iso_date(rx.issued_at);
        for (eye, data) in [("right", &rx.right_eye), ("left", &rx.left_eye)] {
            push_prescription_rows(&mut out, lens, &date, eye, data);
        }
    }

    for grant in &export.grants {
        let date = iso_date(grant.granted_at);
        let grantee = sstr(&grant.grantee.to_string());
        push_csv_row(
            &mut out,
            "access_grant",
            "Consent",
            &date,
            "grantee",
            &grantee,
        );
        push_csv_row(
            &mut out,
            "access_grant",
            "Consent",
            &date,
            "level",
            access_level_name(&grant.level),
        );
        push_csv_row(
            &mut out,
            "access_grant",
            "Consent",
            &date,
            "expires_at",
            &iso8601(grant.expires_at),
        );
    }

    out
}

// ---------------------------------------------------------------------------
// FHIR resources
// ---------------------------------------------------------------------------

fn fhir_observation(entry: &ExportEntry) -> StdString {
    let record = &entry.record;
    let mut members: StdVec<(&str, StdString)> = alloc::vec![
        ("resourceType", json_str("Observation")),
        ("status", json_str("final")),
        (
            "code",
            json_object(&[("text", json_str(record_type_name(&record.record_type)))]),
        ),
        ("effectiveDateTime", json_str(&iso8601(record.created_at))),
    ];

    if let Some(exam) = &entry.exam {
        let components: StdVec<StdString> = exam_fields(exam)
            .into_iter()
            .map(|field| match field.value {
                ExamValue::Text(text) => fhir_component_text(&field.label, &text),
                ExamValue::Mmhg(mmhg) => fhir_component_mmhg(&field.label, mmhg),
            })
            .collect();
        if !components.is_empty() {
            members.push(("component", json_array(&components)));
        }
        let notes = sstr(&exam.clinical_notes);
        if !notes.is_empty() {
            members.push((
                "note",
                json_array(&[json_object(&[("text", json_str(&notes))])]),
            ));
        }
    }

    json_object(&members)
}

fn fhir_component_text(label: &str, value: &str) -> StdString {
    json_object(&[
        ("code", json_object(&[("text", json_str(label))])),
        ("valueString", json_str(value)),
    ])
}

fn fhir_component_mmhg(label: &str, mmhg: u32) -> StdString {
    json_object(&[
        ("code", json_object(&[("text", json_str(label))])),
        (
            "valueQuantity",
            json_object(&[
                ("value", format!("{}", mmhg)),
                ("unit", json_str("mm[Hg]")),
                ("system", json_str("http://unitsofmeasure.org")),
                ("code", json_str("mm[Hg]")),
            ]),
        ),
    ])
}

fn fhir_vision_prescription(rx: &Prescription, now: u64) -> StdString {
    let status = if rx.expires_at > now {
        "active"
    } else {
        "completed"
    };
    let product = match rx.lens_type {
        LensType::Glasses => "glasses",
        LensType::ContactLens => "contact lens",
    };

    let lenses: StdVec<StdString> = [("right", &rx.right_eye), ("left", &rx.left_eye)]
        .into_iter()
        .map(|(eye, data)| fhir_lens_specification(eye, product, data, &rx.contact_data))
        .collect();

    json_object(&[
        ("resourceType", json_str("VisionPrescription")),
        ("status", json_str(status)),
        ("dateWritten", json_str(&iso8601(rx.issued_at))),
        ("lensSpecification", json_array(&lenses)),
    ])
}

fn fhir_lens_specification(
    eye: &str,
    product: &str,
    data: &PrescriptionData,
    contact: &OptionalContactLensData,
) -> StdString {
    let mut members: StdVec<(&str, StdString)> = alloc::vec![
        ("eye", json_str(eye)),
        ("product", json_object(&[("text", json_str(product))])),
    ];
    let mut notes: StdVec<StdString> = StdVec::new();

    push_decimal(&mut members, &mut notes, "sphere", &sstr(&data.sphere));
    push_decimal(&mut members, &mut notes, "cylinder", &sstr(&data.cylinder));
    push_decimal(&mut members, &mut notes, "axis", &sstr(&data.axis));
    push_decimal(&mut members, &mut notes, "add", &sstr(&data.add));

    // Pupillary distance has no lensSpecification element, so it goes in a note.
    let pd = sstr(&data.pd);
    if !pd.is_empty() {
        notes.push(format!("pd: {}", pd));
    }

    if let OptionalContactLensData::Some(lens) = contact {
        push_decimal(
            &mut members,
            &mut notes,
            "backCurve",
            &sstr(&lens.base_curve),
        );
        push_decimal(&mut members, &mut notes, "diameter", &sstr(&lens.diameter));
        let brand = sstr(&lens.brand);
        if !brand.is_empty() {
            notes.push(format!("Brand: {}", brand));
        }
    }
    if !notes.is_empty() {
        let note_objects: StdVec<StdString> = notes
            .iter()
            .map(|text| json_object(&[("text", json_str(text))]))
            .collect();
        members.push(("note", json_array(&note_objects)));
    }

    json_object(&members)
}

fn fhir_consent(grant: &AccessGrant, now: u64) -> StdString {
    let grantee = sstr(&grant.grantee.to_string());
    let status = if grant.expires_at > now {
        "active"
    } else {
        "inactive"
    };
    let provision_type = match grant.level {
        AccessLevel::None => "deny",
        _ => "permit",
    };

    json_object(&[
        ("resourceType", json_str("Consent")),
        ("status", json_str(status)),
        (
            "scope",
            json_object(&[("text", json_str("patient-privacy"))]),
        ),
        (
            "category",
            json_array(&[json_object(&[("text", json_str("access-grant"))])]),
        ),
        ("dateTime", json_str(&iso8601(grant.granted_at))),
        (
            "provision",
            json_object(&[
                ("type", json_str(provision_type)),
                (
                    "period",
                    json_object(&[
                        ("start", json_str(&iso8601(grant.granted_at))),
                        ("end", json_str(&iso8601(grant.expires_at))),
                    ]),
                ),
                (
                    "actor",
                    json_array(&[json_object(&[
                        ("role", json_object(&[("text", json_str("grantee"))])),
                        ("reference", json_object(&[("display", json_str(&grantee))])),
                    ])]),
                ),
                (
                    "action",
                    json_array(&[json_object(&[(
                        "text",
                        json_str(access_level_name(&grant.level)),
                    )])]),
                ),
            ]),
        ),
    ])
}

// ---------------------------------------------------------------------------
// Shared clinical data mapping
// ---------------------------------------------------------------------------

/// Flatten an examination into labelled values, skipping absent sections.
/// Both the FHIR components and the CSV rows are built from this list so the
/// two formats never disagree.
fn exam_fields(exam: &EyeExamination) -> StdVec<ExamField> {
    let mut fields: StdVec<ExamField> = StdVec::new();

    push_measurement(
        &mut fields,
        "Visual acuity uncorrected",
        &exam.visual_acuity.uncorrected,
    );
    if let OptPhysicalMeasurement::Some(corrected) = &exam.visual_acuity.corrected {
        push_measurement(&mut fields, "Visual acuity corrected", corrected);
    }

    push_text(
        &mut fields,
        "Intraocular pressure left eye",
        ExamValue::Mmhg(exam.iop.left_eye),
    );
    push_text(
        &mut fields,
        "Intraocular pressure right eye",
        ExamValue::Mmhg(exam.iop.right_eye),
    );
    push_text(
        &mut fields,
        "Intraocular pressure method",
        ExamValue::Text(sstr(&exam.iop.method)),
    );

    push_text(
        &mut fields,
        "Slit lamp cornea",
        ExamValue::Text(sstr(&exam.slit_lamp.cornea)),
    );
    push_text(
        &mut fields,
        "Slit lamp anterior chamber",
        ExamValue::Text(sstr(&exam.slit_lamp.anterior_chamber)),
    );
    push_text(
        &mut fields,
        "Slit lamp iris",
        ExamValue::Text(sstr(&exam.slit_lamp.iris)),
    );
    push_text(
        &mut fields,
        "Slit lamp lens",
        ExamValue::Text(sstr(&exam.slit_lamp.lens)),
    );

    if let OptVisualField::Some(vf) = &exam.visual_field {
        push_text(
            &mut fields,
            "Visual field reliability left eye",
            ExamValue::Text(sstr(&vf.left_eye_reliability)),
        );
        push_text(
            &mut fields,
            "Visual field reliability right eye",
            ExamValue::Text(sstr(&vf.right_eye_reliability)),
        );
        push_text(
            &mut fields,
            "Visual field defects left eye",
            ExamValue::Text(sstr(&vf.left_eye_defects)),
        );
        push_text(
            &mut fields,
            "Visual field defects right eye",
            ExamValue::Text(sstr(&vf.right_eye_defects)),
        );
    }

    if let OptRetinalImaging::Some(imaging) = &exam.retina_imaging {
        push_text(
            &mut fields,
            "Retinal imaging findings",
            ExamValue::Text(sstr(&imaging.findings)),
        );
    }

    if let OptFundusPhotography::Some(fundus) = &exam.fundus_photo {
        push_text(
            &mut fields,
            "Cup to disc ratio left eye",
            ExamValue::Text(sstr(&fundus.cup_to_disc_ratio_left)),
        );
        push_text(
            &mut fields,
            "Cup to disc ratio right eye",
            ExamValue::Text(sstr(&fundus.cup_to_disc_ratio_right)),
        );
        push_text(
            &mut fields,
            "Macula status",
            ExamValue::Text(sstr(&fundus.macula_status)),
        );
    }

    fields
}

fn push_measurement(fields: &mut StdVec<ExamField>, label: &str, m: &PhysicalMeasurement) {
    push_text(
        fields,
        &format!("{} left eye", label),
        ExamValue::Text(sstr(&m.left_eye)),
    );
    push_text(
        fields,
        &format!("{} right eye", label),
        ExamValue::Text(sstr(&m.right_eye)),
    );
}

fn push_text(fields: &mut StdVec<ExamField>, label: &str, value: ExamValue) {
    fields.push(ExamField {
        label: StdString::from(label),
        value,
    });
}

fn push_prescription_rows(
    out: &mut StdString,
    lens: &str,
    date: &str,
    eye: &str,
    data: &PrescriptionData,
) {
    let fields = [
        ("sphere", &data.sphere),
        ("cylinder", &data.cylinder),
        ("axis", &data.axis),
        ("add", &data.add),
        ("pd", &data.pd),
    ];
    for (name, value) in fields {
        let field = format!("prescription.{}.{}", eye, name);
        push_csv_row(out, "prescription", lens, date, &field, &sstr(value));
    }
}

fn push_csv_row(
    out: &mut StdString,
    section: &str,
    record_type: &str,
    date: &str,
    field: &str,
    value: &str,
) {
    out.push_str(&format!(
        "{},{},{},{},{}\n",
        csv_field(section),
        csv_field(record_type),
        csv_field(date),
        csv_field(field),
        csv_field(value),
    ));
}

// ---------------------------------------------------------------------------
// Names and conversions
// ---------------------------------------------------------------------------

fn record_type_name(record_type: &RecordType) -> &'static str {
    match record_type {
        RecordType::Examination => "Examination",
        RecordType::Prescription => "Prescription",
        RecordType::Diagnosis => "Diagnosis",
        RecordType::Treatment => "Treatment",
        RecordType::Surgery => "Surgery",
        RecordType::LabResult => "LabResult",
    }
}

fn lens_type_name(lens_type: &LensType) -> &'static str {
    match lens_type {
        LensType::Glasses => "Glasses",
        LensType::ContactLens => "ContactLens",
    }
}

fn access_level_name(level: &AccessLevel) -> &'static str {
    match level {
        AccessLevel::None => "None",
        AccessLevel::Read => "Read",
        AccessLevel::Write => "Write",
        AccessLevel::Full => "Full",
        AccessLevel::Admin => "Admin",
    }
}

/// Copy a Soroban `String` into a Rust string. Invalid UTF-8 is replaced
/// rather than dropped so no clinical text silently disappears.
fn sstr(value: &String) -> StdString {
    let mut buf = alloc::vec![0u8; value.len() as usize];
    value.copy_into_slice(&mut buf);
    StdString::from_utf8_lossy(&buf).into_owned()
}

/// Build a JSON object from pre-encoded member values.
fn json_object(members: &[(&str, StdString)]) -> StdString {
    let body: StdVec<StdString> = members
        .iter()
        .map(|(key, value)| format!("{}:{}", json_str(key), value))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// Build a JSON array from pre-encoded items.
fn json_array(items: &[StdString]) -> StdString {
    format!("[{}]", items.join(","))
}

/// Encode a string as a JSON string literal.
fn json_str(value: &str) -> StdString {
    let mut out = StdString::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Add a decimal-valued member as a JSON number when the text is a valid JSON
/// number (FHIR `decimal`/`integer` elements). Anything else, such as "+1.00"
/// or "N/A", is kept as a note so the original text is not lost.
fn push_decimal<'a>(
    members: &mut StdVec<(&'a str, StdString)>,
    notes: &mut StdVec<StdString>,
    key: &'a str,
    raw: &str,
) {
    if raw.is_empty() {
        return;
    }
    if is_json_number(raw) {
        members.push((key, StdString::from(raw)));
    } else {
        notes.push(format!("{}: {}", key, raw));
    }
}

fn is_json_number(raw: &str) -> bool {
    let body = raw.strip_prefix('-').unwrap_or(raw);
    let (int_part, frac_part) = match body.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (body, None),
    };
    let int_ok = !int_part.is_empty()
        && int_part.chars().all(|c| c.is_ascii_digit())
        && (int_part == "0" || !int_part.starts_with('0'));
    let frac_ok = match frac_part {
        Some(frac) => !frac.is_empty() && frac.chars().all(|c| c.is_ascii_digit()),
        None => true,
    };
    int_ok && frac_ok
}

/// Escape one CSV field per RFC 4180.
fn csv_field(value: &str) -> StdString {
    if value.chars().any(|c| matches!(c, ',' | '"' | '\n' | '\r')) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        StdString::from(value)
    }
}

/// Format a UNIX timestamp (seconds) as an ISO 8601 date-time in UTC.
pub(crate) fn iso8601(ts: u64) -> StdString {
    let (y, m, d) = civil_from_days((ts / 86_400) as i64);
    let secs = ts % 86_400;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs / 3_600,
        (secs % 3_600) / 60,
        secs % 60
    )
}

fn iso_date(ts: u64) -> StdString {
    let (y, m, d) = civil_from_days((ts / 86_400) as i64);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Convert days since 1970-01-01 to a (year, month, day) civil date.
/// Howard Hinnant's `civil_from_days` algorithm, valid for all `u64` inputs
/// that fit in `i64` days.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe as i64 + era * 400;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
