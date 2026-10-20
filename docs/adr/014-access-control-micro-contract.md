# ADR-014: Extract RBAC into a Dedicated Access Control Micro-Contract

- **Status**: Accepted
- **Date**: 2026-10-19

## Context

The original `vision_records` contract embedded a large inline RBAC implementation in `contracts/vision_records/src/rbac.rs`. That design was effective for early prototyping but created several problems as the protocol grew:

- the core medical record contract owned too much permission logic
- role and policy state became harder to audit separately from clinical record logic
- other contracts could not reuse the same RBAC rules without re-implementing them
- cross-contract permission checks were harder to reason about when the trust boundary mixed data storage and authorization

The project also needed a reusable, explicit entitlement layer for healthcare workflows, especially when external components needed to verify permitted operations without storing sensitive record data in the same contract.

## Decision

We will extract RBAC and delegated permission logic into a dedicated `access_control` micro-contract and let `vision_records` and related contracts delegate permission checks to it when configured.

The extraction includes:

- role assignment and revocation
- custom grants and revokes
- delegation and scoped permissions
- ACL group logic
- permission checks for `ReadAnyRecord`, `WriteRecord`, `ManageAccess`, `ManageUsers`, and `SystemAdmin`

`vision_records` will retain the ability to call the external micro-contract through the configured address, while continuing to support its own local checks for legacy code paths and backwards compatibility.

## Rationale

### Why extraction is beneficial

- Separation of concerns: record management and authorization logic are independently auditable.
- Reuse: multiple contracts can read the same role model without duplicating RBAC logic.
- Clearer trust boundaries: healthcare data storage is decoupled from policy evaluation.
- Simpler migration: the access-control module can be upgraded or replaced without rewriting the full records contract.
- Better governance: roles and policy checks can be reviewed and validated as a first-class system concern.

### Trade-offs

- Additional deployment and configuration steps are required.
- Cross-contract calls increase operational complexity compared with inline checks.
- Contract references must be initialized and verified correctly.
- Some legacy code paths may require compatibility shims during migration.

## Consequences

### Positive outcomes

- More modular contract composition
- Clearer ownership of permissions and roles
- Easier integration with external authorization layers and future policy engines
- Better support for multi-contract deployments and auditability

### Risks and downsides

- If the micro-contract address is misconfigured, permission checks may silently fall back to an unexpected path
- Migration must preserve current role assignments to avoid access regressions
- Operational deployment sequencing becomes more important

## Implementation Notes

The current inline RBAC implementation remains in `contracts/vision_records/src/rbac.rs` and is the reference for the extracted logic. The dedicated implementation lives in `contracts/access_control/` and is wired into `vision_records` via:

- `set_access_control(admin, access_control)`
- `get_access_control()`
- permission delegation in the internal `has_permission_unified` logic

## Migration path from inline RBAC to access_control

1. Deploy the new `access_control` contract and initialize it with an admin.
2. Assign the initial admin roles and permissions in the standalone contract.
3. Deploy or configure `vision_records` and register the `access_control` contract address.
4. Reconcile any existing role assignments from the old inline RBAC model.
5. Prefer `access_control` for authorization checks during rollout.
6. Keep a compatibility window for legacy callers and verify that permissions match expected production roles.
7. Remove or deprecate the inline-only path after operational validation and customer migration.

## Decision Summary

The project chooses explicit micro-contract RBAC over tightly-coupled inline authorization. This makes the security boundary clearer, improves reuse, and provides a cleaner path to future policy enforcement and auditing while keeping patient-record logic separate from permission logic.
