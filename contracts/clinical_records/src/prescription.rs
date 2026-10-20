use crate::admin::{
    check_permission, get_access_control_contract, require_authorized_vision_records,
    require_initialized,
};
use crate::errors::ContractError;
use crate::examination::validate_string;
use crate::types::{LensType, OptionalContactLensData, Prescription, PrescriptionData};
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
