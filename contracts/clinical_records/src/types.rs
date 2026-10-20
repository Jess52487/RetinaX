use soroban_sdk::{contracttype, Address, String};

/// A reading recorded separately for the left and right eye.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMeasurement {
    /// Reading for the left eye (OS, oculus sinister).
    pub left_eye: String,
    /// Reading for the right eye (OD, oculus dexter).
    pub right_eye: String,
}

/// Optional wrapper around `PhysicalMeasurement`, used in contract types in place of `Option`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptPhysicalMeasurement {
    /// No measurement was taken.
    None,
    /// A measurement is present.
    Some(PhysicalMeasurement),
}

/// Visual acuity: how sharply a patient can see, usually written as a Snellen fraction such as
/// 20/20. A value of 20/40 means the patient reads at 20 feet what a person with normal vision
/// reads at 40 feet.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualAcuity {
    /// Acuity measured without glasses or contact lenses.
    pub uncorrected: PhysicalMeasurement,
    /// Acuity measured with the patient's glasses or contact lenses, when it was taken.
    pub corrected: OptPhysicalMeasurement,
}

/// Intraocular pressure (IOP): the fluid pressure inside the eye. Usually reported in mmHg.
/// Elevated IOP is a major risk factor for glaucoma.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntraocularPressure {
    /// Pressure in the left eye, in mmHg.
    pub left_eye: u32,
    /// Pressure in the right eye, in mmHg.
    pub right_eye: u32,
    /// Measurement technique, for example Goldmann applanation tonometry.
    pub method: String,
    /// When the measurement was taken, as a ledger timestamp in seconds.
    pub timestamp: u64,
}

/// Slit lamp examination findings. A slit lamp is a microscope with a narrow, bright beam of
/// light that lets the examiner inspect the front structures of the eye layer by layer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlitLampFindings {
    /// Clear outer surface of the eye. Findings include scars, ulcers, or edema.
    pub cornea: String,
    /// Space between the cornea and the iris, filled with aqueous humor.
    pub anterior_chamber: String,
    /// Colored muscular structure around the pupil.
    pub iris: String,
    /// Transparent structure behind the iris that focuses light. Cloudiness is a cataract.
    pub lens: String,
}

/// Visual field test results. A visual field test maps the area a patient can see while
/// fixating on a central point, and detects blind spots or areas of reduced sensitivity.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualField {
    /// Whether the left eye test is reliable, for example based on fixation losses.
    pub left_eye_reliability: String,
    /// Whether the right eye test is reliable, for example based on fixation losses.
    pub right_eye_reliability: String,
    /// Defects found in the left eye's visual field, such as scotomas.
    pub left_eye_defects: String,
    /// Defects found in the right eye's visual field, such as scotomas.
    pub right_eye_defects: String,
}

/// Optional wrapper around `VisualField`, used in contract types in place of `Option`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptVisualField {
    /// No visual field test was performed.
    None,
    /// A visual field test is present.
    Some(VisualField),
}

/// Retinal imaging study. The image itself is kept off-chain at `image_url`, and `image_hash`
/// lets anyone check that the file has not changed since it was recorded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetinalImaging {
    /// Location of the image file.
    pub image_url: String,
    /// Digest of the image file, used to verify its integrity.
    pub image_hash: String,
    /// Clinical interpretation of the image.
    pub findings: String,
}

/// Optional wrapper around `RetinalImaging`, used in contract types in place of `Option`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptRetinalImaging {
    /// No retinal imaging was performed.
    None,
    /// A retinal imaging study is present.
    Some(RetinalImaging),
}

/// Fundus photography: a color photograph of the back of the eye, showing the retina, optic disc,
/// and macula.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundusPhotography {
    /// Location of the photograph file.
    pub image_url: String,
    /// Digest of the photograph file, used to verify its integrity.
    pub image_hash: String,
    /// Cup-to-disc ratio of the left eye. The optic cup is the pale center of the optic disc.
    /// A high ratio can indicate glaucomatous damage.
    pub cup_to_disc_ratio_left: String,
    /// Cup-to-disc ratio of the right eye. The optic cup is the pale center of the optic disc.
    /// A high ratio can indicate glaucomatous damage.
    pub cup_to_disc_ratio_right: String,
    /// Observations about the macula, the central area of the retina responsible for sharp vision.
    pub macula_status: String,
}

/// Optional wrapper around `FundusPhotography`, used in contract types in place of `Option`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptFundusPhotography {
    /// No fundus photograph was taken.
    None,
    /// A fundus photograph is present.
    Some(FundusPhotography),
}

/// One complete eye examination, stored under a record id.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EyeExamination {
    /// Identifier of the record this examination belongs to.
    pub record_id: u64,
    /// Visual acuity results.
    pub visual_acuity: VisualAcuity,
    /// Intraocular pressure readings.
    pub iop: IntraocularPressure,
    /// Slit lamp findings.
    pub slit_lamp: SlitLampFindings,
    /// Visual field test results, when performed.
    pub visual_field: OptVisualField,
    /// Retinal imaging study, when performed.
    pub retina_imaging: OptRetinalImaging,
    /// Fundus photograph, when taken.
    pub fundus_photo: OptFundusPhotography,
    /// Free-text notes from the clinician.
    pub clinical_notes: String,
}

/// The kind of vision correction a prescription is for.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LensType {
    /// Spectacle lenses worn in a frame.
    Glasses,
    /// Lenses worn directly on the eye.
    ContactLens,
}

/// Refraction values for one prescription. Refraction measures how the eye bends light.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionData {
    /// Sphere power in diopters. Negative values correct nearsightedness (myopia), positive values
    /// correct farsightedness (hyperopia).
    pub sphere: String,
    /// Cylinder power in diopters, which corrects astigmatism.
    pub cylinder: String,
    /// Axis in degrees, from 0 to 180, that gives the orientation of the astigmatism correction.
    pub axis: String,
    /// Additional near power in diopters, used for reading in presbyopia.
    pub add: String,
    /// Pupillary distance in millimeters, the distance between the centers of the pupils.
    pub pd: String,
}

/// Contact lens fitting values.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactLensData {
    /// Base curve radius in millimeters, which sets how the lens fits the cornea.
    pub base_curve: String,
    /// Total lens diameter in millimeters.
    pub diameter: String,
    /// Manufacturer or product name.
    pub brand: String,
}

/// Optional wrapper around `ContactLensData`, used in contract types in place of `Option`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionalContactLensData {
    /// No contact lens data applies.
    None,
    /// Contact lens fitting values are present.
    Some(ContactLensData),
}

/// A committed vision correction prescription issued by a provider to a patient.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prescription {
    /// Prescription identifier, issued sequentially.
    pub id: u64,
    /// Patient the prescription belongs to.
    pub patient: Address,
    /// Provider who issued the prescription.
    pub provider: Address,
    /// Whether this is a glasses or contact lens prescription.
    pub lens_type: LensType,
    /// Refraction values for the left eye.
    pub left_eye: PrescriptionData,
    /// Refraction values for the right eye.
    pub right_eye: PrescriptionData,
    /// Contact lens fitting values, when `lens_type` is a contact lens.
    pub contact_data: OptionalContactLensData,
    /// When the prescription was issued, as a ledger timestamp in seconds.
    pub issued_at: u64,
    /// When the prescription expires, as a ledger timestamp in seconds.
    pub expires_at: u64,
    /// Whether the prescription has been verified.
    pub verified: bool,
    /// Digest of the off-chain metadata for this prescription.
    pub metadata_hash: String,
}
