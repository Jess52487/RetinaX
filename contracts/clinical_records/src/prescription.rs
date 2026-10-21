use crate::admin::{
    check_permission, get_access_control_contract, require_authorized_vision_records,
    require_initialized,
};
use crate::errors::ContractError;
use crate::examination::validate_string;
use crate::types::{
    BatchPrescriptionInput, LensType, OptionalContactLensData, Prescription, PrescriptionData,
};
use access_control::Permission;
use soroban_sdk::{symbol_short, Address, Env, String, Symbol, Vec};

pub const PRESCRIPTION_KEY: Symbol = symbol_short!("RX");
pub const RX_COUNTER: Symbol = symbol_short!("RX_CTR");
const PENDING_RX_KEY: Symbol = symbol_short!("P_ADD_RX");

/// Largest number of prescriptions one batch call accepts.
///
/// A committed batch of `n` writes `2n` ledger entries (n persistent writes plus n temporary
/// removals) and emits `n` events, so 10 keeps every batch far below the per-transaction limits
/// of 200 written entries and 16 KiB of events.
pub const MAX_PRESCRIPTION_BATCH: u32 = 10;

/// How long a committed prescription stays valid: one year, in seconds.
const PRESCRIPTION_VALIDITY_SECS: u64 = 31_536_000;

/// A prepared prescription waiting in temporary storage: patient, provider, refraction data,
/// and the ledger timestamp of the prepare step.
type PendingPrescription = (Address, Address, PrescriptionData, u64);

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
    require_write_access(env, &caller)?;
    validate_prescription_data(&prescription_data)?;

    let rx_id = get_prescription_count(env).saturating_add(1);
    env.storage().instance().set(&RX_COUNTER, &rx_id);
    store_pending(
        env,
        rx_id,
        patient,
        provider,
        prescription_data,
        env.ledger().timestamp(),
    );
    Ok(rx_id)
}

/// Prepares every prescription in `prescriptions` and returns their ids in input order.
///
/// Authorization, the id counter and the ledger timestamp are each handled once for the whole
/// batch. If any entry fails validation the call returns an error and the host rolls back every
/// write made so far, so either all prescriptions are prepared or none are.
///
/// # Complexity
/// O(n) time and O(n) space for the returned ids, where n is the batch size.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, or `InvalidInput` (also for an empty batch or one
/// larger than `MAX_PRESCRIPTION_BATCH`).
pub fn prepare_add_prescriptions(
    env: &Env,
    caller: Address,
    prescriptions: Vec<BatchPrescriptionInput>,
) -> Result<Vec<u64>, ContractError> {
    require_write_access(env, &caller)?;
    require_batch_size(prescriptions.len())?;

    let now = env.ledger().timestamp();
    let mut rx_id = get_prescription_count(env);
    let mut rx_ids = Vec::new(env);
    for input in prescriptions.iter() {
        validate_prescription_data(&input.prescription_data)?;
        rx_id = rx_id.saturating_add(1);
        store_pending(
            env,
            rx_id,
            input.patient,
            input.provider,
            input.prescription_data,
            now,
        );
        rx_ids.push_back(rx_id);
    }
    env.storage().instance().set(&RX_COUNTER, &rx_id);
    Ok(rx_ids)
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
    require_write_access(env, &caller)?;
    let pending = load_pending(env, rx_id)?;
    commit_pending(env, &caller, rx_id, pending);
    Ok(())
}

/// Commits every pending prescription in `rx_ids`.
///
/// Fails, and the host rolls back the whole batch, if any id is not pending. A repeated id
/// fails the same way, because its first occurrence already consumed the pending entry.
///
/// # Complexity
/// O(n) time and O(1) extra space, where n is the batch size.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, `InvalidInput` for an empty or oversized batch, or
/// `InvalidPhase` if an id has no pending prescription.
pub fn commit_add_prescriptions(
    env: &Env,
    caller: Address,
    rx_ids: Vec<u64>,
) -> Result<(), ContractError> {
    require_write_access(env, &caller)?;
    require_batch_size(rx_ids.len())?;

    for rx_id in rx_ids.iter() {
        let pending = load_pending(env, rx_id)?;
        commit_pending(env, &caller, rx_id, pending);
    }
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
    require_write_access(env, &caller)?;
    discard_pending(env, &caller, rx_id);
    Ok(())
}

/// Discards every pending prescription in `rx_ids`.
///
/// Unlike the single rollback, every id must still be pending, so a batch never reports success
/// for prescriptions it did not discard. Fails, and the host rolls back the whole batch, on the
/// first id that is not pending (including a repeated id).
///
/// # Complexity
/// O(n) time and O(1) extra space, where n is the batch size.
///
/// # Errors
/// Returns `NotInitialized`, `Unauthorized`, `InvalidInput` for an empty or oversized batch, or
/// `InvalidPhase` if an id has no pending prescription.
pub fn rollback_add_prescriptions(
    env: &Env,
    caller: Address,
    rx_ids: Vec<u64>,
) -> Result<(), ContractError> {
    require_write_access(env, &caller)?;
    require_batch_size(rx_ids.len())?;

    for rx_id in rx_ids.iter() {
        if !env.storage().temporary().has(&(PENDING_RX_KEY, rx_id)) {
            return Err(ContractError::InvalidPhase);
        }
        discard_pending(env, &caller, rx_id);
    }
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
pub fn get_prescription(
    env: &Env,
    caller: Address,
    id: u64,
) -> Result<Prescription, ContractError> {
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

/// Write authorization shared by every prescription mutation: `WriteRecord` through
/// `access_control` when it is configured, otherwise the `vision_records` contract.
fn require_write_access(env: &Env, caller: &Address) -> Result<(), ContractError> {
    require_initialized(env)?;

    if get_access_control_contract(env).is_some() {
        if !check_permission(env, caller, Permission::WriteRecord)? {
            return Err(ContractError::Unauthorized);
        }
    } else {
        require_authorized_vision_records(env, caller)?;
    }
    Ok(())
}

fn require_batch_size(len: u32) -> Result<(), ContractError> {
    if len == 0 || len > MAX_PRESCRIPTION_BATCH {
        return Err(ContractError::InvalidInput);
    }
    Ok(())
}

fn store_pending(
    env: &Env,
    rx_id: u64,
    patient: Address,
    provider: Address,
    prescription_data: PrescriptionData,
    prepared_at: u64,
) {
    let pending: PendingPrescription = (
        patient.clone(),
        provider.clone(),
        prescription_data,
        prepared_at,
    );
    env.storage()
        .temporary()
        .set(&(PENDING_RX_KEY, rx_id), &pending);
    crate::events::publish_prescription_prepared(env, rx_id, patient, provider);
}

fn load_pending(env: &Env, rx_id: u64) -> Result<PendingPrescription, ContractError> {
    env.storage()
        .temporary()
        .get(&(PENDING_RX_KEY, rx_id))
        .ok_or(ContractError::InvalidPhase)
}

fn commit_pending(env: &Env, caller: &Address, rx_id: u64, pending: PendingPrescription) {
    let (patient, provider, prescription_data, prepared_at) = pending;
    let prescription = Prescription {
        id: rx_id,
        patient,
        provider,
        lens_type: LensType::Glasses,
        left_eye: prescription_data.clone(),
        right_eye: prescription_data,
        contact_data: OptionalContactLensData::None,
        issued_at: prepared_at,
        expires_at: prepared_at.saturating_add(PRESCRIPTION_VALIDITY_SECS),
        verified: false,
        metadata_hash: String::from_str(env, ""),
    };

    env.storage()
        .persistent()
        .set(&(PRESCRIPTION_KEY, rx_id), &prescription);
    env.storage().temporary().remove(&(PENDING_RX_KEY, rx_id));
    crate::events::publish_prescription_committed(env, rx_id, caller.clone());
}

fn discard_pending(env: &Env, caller: &Address, rx_id: u64) {
    env.storage().temporary().remove(&(PENDING_RX_KEY, rx_id));
    crate::events::publish_prescription_rolled_back(env, rx_id, caller.clone());
}
