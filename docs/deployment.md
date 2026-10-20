# Deployment Guide

This guide covers repeatable deployment of RetinaX contracts to local and Stellar networks. It reflects the repository's current contract graph and is aligned with the scripts under `scripts/`.

## Scope

- Build contract WASM artifacts
- Deploy a selected contract or dependency-first stack
- Initialize state after deployment
- Link dependent contracts (`access_control`, `vision_records`, `clinical_records`)
- Verify deployment health on testnet
- Roll back consumer references to a previous contract ID

## Prerequisites

1. Rust toolchain with the `wasm32-unknown-unknown` target
2. Soroban CLI installed and authenticated (`soroban keys` and configured networks)
3. Network alias configured in Soroban CLI (`local`, `testnet`, `futurenet`, or `mainnet`)
4. Deployer key available as the `default` source account
5. Write access to `deployments/`

## Deployment Order and Dependencies

Deploy contracts in dependency order so that downstream contracts receive the addresses they depend on.

### Recommended order

```text
1. audit
2. key_manager (after identity)
3. identity
4. access_control
5. zk_verifier
6. vision_records
7. clinical_records (requires vision_records address)
8. remaining domain contracts
```

This ordering matters because:

- `key_manager` depends on `identity`
- `vision_records` can optionally delegate permission evaluation to `access_control`
- `clinical_records` must be initialized with the `vision_records` contract address
- downstream contracts frequently store addresses of upstream dependencies on initialization

## Standard Deployment Procedure

### 1. Pre-deployment checks

Run checks before deploying:

```bash
cargo test
cargo build --target wasm32-unknown-unknown --release
```

Optional:

```bash
./scripts/build_release_artifacts.sh
```

### 2. Deploy a single contract

```bash
./scripts/deploy.sh <network> <contract> [--admin <address>] [--vision-records <contract-id>]
```

Examples:

```bash
./scripts/deploy.sh local access_control
./scripts/deploy.sh testnet vision_records --admin GB3...ADMIN
./scripts/deploy.sh testnet clinical_records --admin GB3...ADMIN --vision-records <VISION_RECORDS_ID>
```

On success, the script prints:

- `DEPLOYMENT_CONTRACT_ID=<id>`
- a deployment descriptor in `deployments/`

### 3. Deploy access_control

`access_control` is the standalone RBAC/micro-contract used by `vision_records` for cross-contract role and permission checks.

```bash
./scripts/deploy.sh testnet access_control --admin <ADMIN_ADDRESS>
```

Then configure the contract pointer from `vision_records`:

```bash
soroban contract invoke \
  --id <VISION_RECORDS_ID> \
  --source default \
  --network testnet \
  -- \
  set_access_control \
  --admin <ADMIN_ADDRESS> \
  --access_control <ACCESS_CONTROL_ID>
```

You can verify the link with:

```bash
soroban contract invoke \
  --id <VISION_RECORDS_ID> \
  --source default \
  --network testnet \
  -- \
  get_access_control
```

### 4. Deploy clinical_records

`clinical_records` stores prescriptions and eye examinations and is initialized with the `vision_records` contract address so it can validate the canonical owner of the medical record graph.

```bash
./scripts/deploy.sh testnet clinical_records --admin <ADMIN_ADDRESS> --vision-records <VISION_RECORDS_ID>
```

Equivalent direct Soroban invocation:

```bash
soroban contract invoke \
  --id <CLINICAL_RECORDS_ID> \
  --source default \
  --network testnet \
  -- \
  initialize \
  --admin <ADMIN_ADDRESS> \
  --vision_records <VISION_RECORDS_ID>
```

The script also validates this dependency requirement before proceeding.

### 5. Initialize contract state

Most contracts require explicit initialization after deployment. Example for `vision_records`:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source default \
  --network <network> \
  -- \
  initialize \
  --admin <ADMIN_ADDRESS>
```

### 6. Admin handoff (if needed)

Contracts in this repository generally use a two-step admin transfer (`propose_admin`, `accept_admin`).

Step 1 (current admin):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <CURRENT_ADMIN_SOURCE> \
  --network <network> \
  -- \
  propose_admin \
  --current_admin <CURRENT_ADMIN_ADDRESS> \
  --new_admin <NEW_ADMIN_ADDRESS>
```

Step 2 (new admin):

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <NEW_ADMIN_SOURCE> \
  --network <network> \
  -- \
  accept_admin \
  --new_admin <NEW_ADMIN_ADDRESS>
```

### 7. Post-deployment verification

Minimum required checks:

1. Contract responds to a lightweight read method (`version`, `get_admin`, `get_access_control`, `get_vision_records_contract`)
2. The admin address is correct
3. Deployment descriptor contains the expected `contract_id`, timestamp, and `wasm_hash`
4. Downstream contract references are configured correctly

Examples:

```bash
soroban contract invoke \
  --id <VISION_RECORDS_ID> \
  --source default \
  --network testnet \
  -- \
  get_access_control

soroban contract invoke \
  --id <CLINICAL_RECORDS_ID> \
  --source default \
  --network testnet \
  -- \
  get_vision_records_contract
```

## Testnet Deployment Verification Steps

Use this sequence before declaring a contract live on testnet:

```bash
./scripts/deploy.sh testnet access_control --admin <ADMIN_ADDRESS>
./scripts/deploy.sh testnet vision_records --admin <ADMIN_ADDRESS>
./scripts/deploy.sh testnet clinical_records --admin <ADMIN_ADDRESS> --vision-records <VISION_RECORDS_ID>
```

Then verify wiring:

```bash
soroban contract invoke --id <ACCESS_CONTROL_ID> --source default --network testnet -- --fn get_admin
soroban contract invoke --id <VISION_RECORDS_ID> --source default --network testnet -- --fn get_access_control
soroban contract invoke --id <CLINICAL_RECORDS_ID> --source default --network testnet -- --fn get_vision_records_contract
```

If any verification fails, restore the previous deployment descriptor and re-check configuration before using the contract in production workflows.

## Rollback Procedure

On Soroban, deployed contracts are immutable. Rollback means repointing off-chain consumers.

### Steps

1. Identify the last known-good contract ID from:
   - `deployments/<network>_<contract>_previous.json`
   - release notes or deployment logs
2. Update all consumers to use that contract ID:
   - backend services
   - indexers
   - frontend/app config
3. Re-run smoke checks against the previous contract ID
4. Record the incident and root cause in ops or release notes

## CI/CD Recommendations

1. Build/test gates before deployment
2. One environment promotion path (`local -> testnet -> production`)
3. Persist deployment descriptors in git history
4. Capture contract ID + WASM checksum in release notes
5. Require manual approval for production deployment jobs

## Operational Handoff Checklist

Before marking a deployment complete:

1. Contract IDs are published to all consuming systems
2. Monitoring tracks the new contract IDs
3. Alerts are green for at least one observation window
4. Backup snapshots are updated (artifacts, descriptors, checksums)
5. Upgrade and rollback metadata are documented
