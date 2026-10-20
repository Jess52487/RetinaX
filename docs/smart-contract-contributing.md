# Smart Contract Developer Contribution Guide

This guide supplements the general [contribution guidelines](../CONTRIBUTING.md) with the workflow and considerations for RetinaX's Soroban contracts.

## Before Making Changes

1. Identify the affected contract crate under `contracts/` and read its `Cargo.toml`, public API, nearby tests, and any contract-specific documentation.
2. Check whether the change affects another contract or a shared crate such as `contracts/common`. Keep changes focused and preserve existing cross-contract interfaces.
3. Review the [testing strategy](testing-strategy.md) and the related issue before changing persistent storage, authorization, or public contract methods.

## Contract Implementation

- Follow the surrounding Rust style and use the workspace's Soroban SDK and dependency versions.
- Preserve the contract's `no_std` compatibility. Avoid standard-library dependencies in contract code unless the crate already gates them appropriately.
- Require authorization at the operation that needs it; do not rely on callers to enforce access control.
- Treat persistent storage keys and public contract interfaces as compatibility-sensitive. Follow existing TTL extension patterns when adding persistent data.
- Return the contract's established error types and emit events using the existing event conventions.
- Preserve the protocol's privacy guarantees: do not expose sensitive patient data in public storage, events, error messages, or logs.
- Keep tests deterministic and avoid depending on live networks or deployed contract IDs.

## Tests and Quality Checks

Add or update tests alongside the affected contract, following that crate's existing layout (`src/test.rs`, other `src/test_*.rs` modules, or `tests/`). Cover successful behavior, authorization, invalid input, and relevant failure paths.

Run the checks that cover your change:

```bash
# Test one contract crate
cargo test -p <contract_crate>

# Check that the contract builds for Soroban's WASM target
cargo build -p <contract_crate> --target wasm32-unknown-unknown --release

# Check formatting and linting
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Replace `<contract_crate>` with the package name in that contract's `Cargo.toml`, for example `vision_records`. For changes spanning contracts or shared crates, run the relevant workspace tests as well. See the [testing strategy](testing-strategy.md) for test conventions and coverage guidance.

## Local Deployment

Use the repository's deployment scripts for integration checks. The core contract stack must be deployed in the order and linked as described in the local testnet stack section of [CONTRIBUTING.md](../CONTRIBUTING.md). Prefer a local network for development; do not deploy experimental changes to a public network.

Never commit secret keys, credentials, real patient data, or private deployment configuration. Keep environment-specific contract IDs out of source changes unless the task explicitly requires a documented public configuration.

## Pull Request Checklist

- Describe the contract behavior changed and any public API or storage compatibility impact.
- Link the related issue with a closing reference such as `Closes #123`.
- List the contract tests, WASM build, and other checks you ran.
- Note any cross-contract deployment or initialization steps required for reviewers.
