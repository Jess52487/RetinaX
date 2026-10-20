# Error Codes

Standardised error codes used across all RetinaX contracts.
Every contract re-exports `CommonError` from the `common` crate as `ContractError` so that error codes are identical everywhere.

## Code Ranges

| Range   | Category                       | Description                                   |
|---------|--------------------------------|-----------------------------------------------|
| 1 – 9   | Lifecycle / Initialisation     | Contract setup and one-time operations         |
| 10 – 19 | Authentication & Authorisation | Caller identity and permission checks          |
| 20 – 29 | Resource Not Found             | Requested entity does not exist in storage     |
| 30 – 39 | Validation / Input             | Invalid parameters or malformed data           |
| 40 – 49 | Contract State                 | Runtime state constraints (paused, locked, …)  |
| 100+    | Contract-Specific              | Reserved for individual contract extensions    |

## Common Errors

| Code | Variant              | When it is returned                                                                                   |
|------|----------------------|-------------------------------------------------------------------------------------------------------|
| 1    | `NotInitialized`     | A function that requires prior initialisation is called before `initialize()`.                        |
| 2    | `AlreadyInitialized` | `initialize()` is called more than once on the same contract instance.                                |
| 10   | `AccessDenied`       | The caller lacks the required role or permission (e.g. not admin, not record owner, expired grant).   |
| 20   | `UserNotFound`       | The requested user address does not exist in contract storage.                                        |
| 21   | `RecordNotFound`     | The requested record ID does not exist in contract storage.                                           |
| 30   | `InvalidInput`       | One or more parameters are invalid (empty list, zero duration, malformed hash, etc.).                 |
| 40   | `Paused`             | The contract is temporarily paused and cannot process the request.                                    |

## Migration Notes

### `Unauthorized` → `AccessDenied` (code 10)

Previous versions of `vision_records` defined two overlapping error variants:
- `Unauthorized` (code 3) — caller is not allowed
- `AccessDenied` (code 7) — caller is not allowed

These have been consolidated into a single `AccessDenied` (code **10**).
Any off-chain code that matched on error code `3` or `7` should now match on `10`.

## Extending Errors

Contracts that need domain-specific error codes define them in their own `#[contracterror]` enum. In Soroban, contract error codes are scoped to the executing contract; contracts may use dedicated domain error types or reserve error code offsets starting at **100+** when sharing namespace boundaries with `CommonError`.

```rust
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum VisionError {
    /// The emergency access window has expired.
    EmergencyExpired = 100,
    /// Maximum record limit per patient exceeded.
    RecordLimitExceeded = 101,
}
```

## Contract-Specific Error References

### Access Control (`AccessControlError`)

The `access_control` contract defines domain-specific error codes for fine-grained role-based (RBAC) and attribute-based access control (ABAC).

| Code | Variant                   | When it is returned                                                                                    |
|------|---------------------------|--------------------------------------------------------------------------------------------------------|
| 1    | `Unauthorized`            | Caller lacks administrative privileges or the required permissions to perform the operation.           |
| 2    | `InvalidInput`            | Provided string parameter or identifier is empty or exceeds length limits (`MAX_STRING_LEN`).           |
| 3    | `NotFound`                | The specified role assignment, delegation, user credential, policy, or entity was not found in storage.|
| 4    | `DelegationExpired`       | Requested delegation or scoped permission grant has reached its expiration timestamp or is invalid.     |
| 5    | `Paused`                  | The access control micro-contract is temporarily paused by the emergency admin.                        |
| 6    | `AlreadyExists`           | The target user already belongs to the group, holds the credential, or the entity already exists.      |
| 7    | `AlreadyInitialized`      | `initialize()` is invoked on an access control contract instance that has already been initialized.     |
| 8    | `DelegationDepthExceeded` | The delegation chain depth exceeds the configured maximum allowed limit (`MAX_DEL_DEPTH`).            |

### Clinical Records (`ContractError`)

The `clinical_records` contract defines error codes for managing eye examination and ophthalmic prescription lifecycles.

| Code | Variant              | When it is returned                                                                                    |
|------|----------------------|--------------------------------------------------------------------------------------------------------|
| 1    | `AlreadyInitialized` | `initialize()` was called more than once on the clinical records contract instance.                    |
| 2    | `NotInitialized`     | A function requiring prior configuration was invoked before `initialize()` completed.                 |
| 3    | `Unauthorized`       | The caller is not authorized (e.g. not admin, not assigned practitioner, or not patient record owner). |
| 4    | `InvalidInput`       | An input failed validation (e.g. empty string, string exceeding character bounds, or malformed data).  |
| 5    | `InvalidRecordType`  | Reserved for unsupported clinical record types.                                                        |
| 6    | `RecordNotFound`     | No examination or prescription record exists for the given record ID.                                  |
| 7    | `InvalidPhase`       | The prescription lifecycle operation was called out of order (e.g. committing an un-prepared record).  |

