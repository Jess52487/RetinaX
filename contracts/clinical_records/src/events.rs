#![allow(deprecated)] // events().publish migration tracked separately

use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// Emitted when an eye examination is stored.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EyeExaminationAddedEvent {
    /// Record the examination was stored under.
    pub record_id: u64,
    /// Address that performed the write.
    pub caller: Address,
    /// Ledger timestamp in seconds when the event was emitted.
    pub timestamp: u64,
}

/// Emitted when a prescription is prepared (pending, not yet committed).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionPreparedEvent {
    /// Prescription id that was reserved.
    pub rx_id: u64,
    /// Patient the prescription is for.
    pub patient: Address,
    /// Provider who issued the prescription.
    pub provider: Address,
    /// Ledger timestamp in seconds when the event was emitted.
    pub timestamp: u64,
}

/// Emitted when a pending prescription is committed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionCommittedEvent {
    /// Prescription id that was committed.
    pub rx_id: u64,
    /// Address that committed the prescription.
    pub caller: Address,
    /// Ledger timestamp in seconds when the event was emitted.
    pub timestamp: u64,
}

/// Emitted when a pending prescription is rolled back.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionRolledBackEvent {
    /// Prescription id that was discarded.
    pub rx_id: u64,
    /// Address that rolled the prescription back.
    pub caller: Address,
    /// Ledger timestamp in seconds when the event was emitted.
    pub timestamp: u64,
}

/// Publishes an `EXAM_ADD` event for a stored eye examination.
pub fn publish_eye_examination_added(env: &Env, record_id: u64, caller: Address) {
    let topics = (symbol_short!("EXAM_ADD"), record_id, caller.clone());
    let data = EyeExaminationAddedEvent {
        record_id,
        caller,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an `RX_PREP` event for a prescription that was prepared.
pub fn publish_prescription_prepared(env: &Env, rx_id: u64, patient: Address, provider: Address) {
    let topics = (
        symbol_short!("RX_PREP"),
        rx_id,
        patient.clone(),
        provider.clone(),
    );
    let data = PrescriptionPreparedEvent {
        rx_id,
        patient,
        provider,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an `RX_CMT` event for a prescription that was committed.
pub fn publish_prescription_committed(env: &Env, rx_id: u64, caller: Address) {
    let topics = (symbol_short!("RX_CMT"), rx_id, caller.clone());
    let data = PrescriptionCommittedEvent {
        rx_id,
        caller,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}

/// Publishes an `RX_RBK` event for a prescription that was rolled back.
pub fn publish_prescription_rolled_back(env: &Env, rx_id: u64, caller: Address) {
    let topics = (symbol_short!("RX_RBK"), rx_id, caller.clone());
    let data = PrescriptionRolledBackEvent {
        rx_id,
        caller,
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish(topics, data);
}
