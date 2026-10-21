use soroban_sdk::{contracttype, Bytes, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gender {
    Male,
    Female,
    Other,
    Unknown,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patient {
    pub id: String,
    pub identifier: String, // e.g. MRN
    pub name: String,
    pub active: bool,
    pub gender: Gender,
    pub birth_date: u64, // Unix timestamp
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationStatus {
    Registered,
    Preliminary,
    Final,
    Amended,
    Corrected,
    Cancelled,
    EnteredInError,
    Unknown,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub id: String,
    pub status: ObservationStatus,
    pub code_system: String, // e.g., "LOINC", "SNOMED"
    pub code_value: String,
    pub subject_id: String, // Reference to Patient.id
    pub value: String,      // As string for flexibility, could be parsed as needed
    pub effective_datetime: u64,
}

/// FHIR R4 MedicationRequest.status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MedicationRequestStatus {
    Active,
    OnHold,
    Completed,
    Stopped,
    Draft,
    EnteredInError,
}

/// FHIR R4 MedicationRequest.intent.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MedicationRequestIntent {
    Order,
    Plan,
    Proposal,
}

/// FHIR R4 MedicationRequest (RetinaX lens-prescription subset).
///
/// The required R4 elements are always present: `status`, `intent`,
/// `medication[x]` (as a `CodeableConcept` coding in
/// `medication_system`/`medication_code`/`medication_display`),
/// `subject`, and `authoredOn`. Per-eye lens parameters (sphere,
/// cylinder, axis, addition, PD) are carried in `dosage_text` using the
/// LOINC EYE.GLASSES prescription codes as labels.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicationRequest {
    pub id: String,
    pub status: MedicationRequestStatus,
    pub intent: MedicationRequestIntent,
    pub medication_system: String,  // medication[x].coding.system
    pub medication_code: String,    // medication[x].coding.code
    pub medication_display: String, // medication[x].coding.display
    pub subject_id: String,         // Reference to Patient.id
    pub requester_id: String,       // Reference to Practitioner
    pub authored_on: u64,           // authoredOn (Unix timestamp)
    pub validity_period_start: u64, // dispenseRequest.validityPeriod.start
    pub validity_period_end: u64,   // dispenseRequest.validityPeriod.end
    pub dosage_text: String,        // dosage[].text (lens parameters)
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FhirError {
    AlreadyInitialized = 1,
    RecordNotFound = 2,
    ExternalCallFailed = 3,
    InvalidRecordData = 4,
    RecordAlreadyExists = 5,
    Unauthorized = 6,
    InvalidPayload = 7,
    MigrationFailed = 8,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Resource {
    pub id: String,
    pub payload: Bytes,
    pub version: u32,
    pub last_updated: u64,
}
