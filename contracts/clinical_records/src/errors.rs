use soroban_sdk::contracterror;

/// Errors returned by the clinical records contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    /// `initialize` was called more than once.
    AlreadyInitialized = 1,
    /// The contract has not been initialized yet.
    NotInitialized = 2,
    /// The caller is not allowed to perform this action.
    Unauthorized = 3,
    /// An input failed validation, for example a field was too long or empty.
    InvalidInput = 4,
    /// Reserved for unsupported record types. No function currently returns it.
    InvalidRecordType = 5,
    /// No record exists for the given id.
    RecordNotFound = 6,
    /// The operation was called out of order, for example committing a prescription that was
    /// never prepared.
    InvalidPhase = 7,
}
