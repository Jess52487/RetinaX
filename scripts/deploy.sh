#!/bin/bash

# Stellar Teye - Contract Deployment Script
# Usage: ./scripts/deploy.sh <network> [contract|stack] [--admin <address>] [--vision-records <id>]
#
# Networks: local, testnet, futurenet, mainnet
#
# Options:
#   --admin <address>            Permanent admin address. When provided for a single
#                               contract, the script initializes the contract with the
#                               deployer as a temporary admin, then transfers admin
#                               rights to this address (least-privilege deployment).
#   --vision-records <contract>  Required for deployments of clinical_records so the
#                               script can initialize the contract with the deployed
#                               vision_records contract address.
#
# `stack` deploys the local core in dependency order:
#   audit -> key_manager -> identity -> access_control -> zk_verifier -> vision_records -> clinical_records
# and initializes each contract with the contract IDs deployed earlier in
# the run.
#
# Without --admin a single contract is deployed but NOT initialized. The
# caller must initialize and transfer admin separately. `stack` always
# initializes, using the deployer key as the on-chain admin.

set -euo pipefail

NETWORK=${1:-local}
CONTRACT=${2:-vision_records}
ADMIN_ADDRESS=""
VISION_RECORDS_ID=""

# Parse optional flags
shift 2 2>/dev/null || true
while [ "$#" -gt 0 ]; do
    case "$1" in
        --admin)
            ADMIN_ADDRESS="${2:-}"
            if [ -z "$ADMIN_ADDRESS" ]; then
                echo "ERROR: --admin requires an address."
                exit 1
            fi
            shift 2
            ;;
        --vision-records)
            VISION_RECORDS_ID="${2:-}"
            if [ -z "$VISION_RECORDS_ID" ]; then
                echo "ERROR: --vision-records requires a contract ID."
                exit 1
            fi
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DEPLOY_DIR="$ROOT_DIR/deployments"
mkdir -p "$DEPLOY_DIR"

# Core local/testnet stack. Order is fixed: each later contract is initialized
# with contract IDs captured from earlier deploys in this same run.
STACK_CONTRACTS=(audit key_manager identity access_control zk_verifier vision_records clinical_records)

resolve_deployer() {
    local deployer
    deployer=$(soroban keys address default 2>/dev/null || true)
    if [ -z "$deployer" ]; then
        echo "ERROR: Could not resolve deployer address for identity 'default'." >&2
        exit 1
    fi
    printf '%s' "$deployer"
}

invoke_contract() {
    local contract_id="$1"
    shift
    soroban contract invoke \
        --id "$contract_id" \
        --source default \
        --network "$NETWORK" \
        -- \
        "$@" >&2
}

write_descriptor() {
    local contract_name="$1"
    local contract_id="$2"
    local wasm_path="$3"
    local linked_ids="${4:-}"
    local deploy_file="$DEPLOY_DIR/${NETWORK}_${contract_name}.json"
    local admin_transferred="false"
    if [ -n "$ADMIN_ADDRESS" ]; then
        admin_transferred="true"
    fi

    cat > "$deploy_file" << EOF
{
    "network": "$NETWORK",
    "contract": "$contract_name",
    "contract_id": "$contract_id",
    "admin": "$ADMIN_ADDRESS",
    "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "wasm_hash": "$(sha256sum "$wasm_path" | cut -d' ' -f1)",
    "admin_transferred": $admin_transferred,
    "linked_contracts": {${linked_ids}}
}
EOF
    echo "Deployment info saved to: $deploy_file"
}

deploy_wasm() {
    local contract_name="$1"
    local wasm_path="target/wasm32-unknown-unknown/release/${contract_name}.wasm"

    if [ ! -f "$wasm_path" ]; then
        echo "WASM file not found: $wasm_path"
        exit 1
    fi

    echo "Deploying $contract_name to $NETWORK..." >&2
    local contract_id
    contract_id=$(soroban contract deploy \
        --wasm "$wasm_path" \
        --source default \
        --network "$NETWORK")
    printf '%s' "$contract_id"
}

# Initialize one stack contract, passing IDs from contracts already deployed.
initialize_stack_contract() {
    local contract_name="$1"
    local contract_id="$2"
    local deployer="$3"
    local key_manager_id="$4"
    local identity_id="$5"
    local zk_verifier_id="$6"
    local vision_records_id="$7"
    local clinical_records_id="$8"
    local access_control_id="$9"
    local linked=""

    case "$contract_name" in
        audit)
            echo "Initializing audit..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            ;;
        key_manager)
            if [ -z "$identity_id" ]; then
                echo "ERROR: key_manager initialization requires the identity contract ID." >&2
                exit 1
            fi
            echo "Initializing key_manager with identity $identity_id..." >&2
            invoke_contract "$contract_id" initialize \
                --admin "$deployer" \
                --identity_contract "$identity_id"
            linked="\"identity\": \"$identity_id\""
            ;;
        identity)
            echo "Initializing identity..." >&2
            invoke_contract "$contract_id" initialize --owner "$deployer"
            if [ -n "$zk_verifier_id" ]; then
                echo "Linking identity to zk_verifier $zk_verifier_id..." >&2
                invoke_contract "$contract_id" set_zk_verifier \
                    --caller "$deployer" \
                    --verifier_id "$zk_verifier_id"
                linked="\"zk_verifier\": \"$zk_verifier_id\""
            fi
            ;;
        access_control)
            echo "Initializing access_control..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            ;;
        zk_verifier)
            echo "Initializing zk_verifier..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            ;;
        vision_records)
            echo "Initializing vision_records..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            if [ -n "$key_manager_id" ]; then
                local placeholder_root
                placeholder_root="$(printf '00%.0s' {1..32})"
                echo "Linking vision_records to key_manager $key_manager_id..." >&2
                invoke_contract "$contract_id" set_key_manager \
                    --caller "$deployer" \
                    --manager "$key_manager_id" \
                    --root_key_id "$placeholder_root"
                linked="\"key_manager\": \"$key_manager_id\""
            fi
            if [ -n "$access_control_id" ]; then
                echo "Linking vision_records to access_control $access_control_id..." >&2
                invoke_contract "$contract_id" set_access_control \
                    --admin "$deployer" \
                    --access_control "$access_control_id"
                if [ -n "$linked" ]; then
                    linked="${linked}, \"access_control\": \"$access_control_id\""
                else
                    linked="\"access_control\": \"$access_control_id\""
                fi
            fi
            if [ -n "$clinical_records_id" ]; then
                echo "Linking vision_records to clinical_records $clinical_records_id..." >&2
                invoke_contract "$contract_id" set_clinical_records_contract \
                    --admin "$deployer" \
                    --clinical_records "$clinical_records_id"
                if [ -n "$linked" ]; then
                    linked="${linked}, \"clinical_records\": \"$clinical_records_id\""
                else
                    linked="\"clinical_records\": \"$clinical_records_id\""
                fi
            fi
            ;;
        clinical_records)
            if [ -z "$vision_records_id" ]; then
                echo "ERROR: clinical_records initialization requires the vision_records contract ID." >&2
                exit 1
            fi
            echo "Initializing clinical_records with vision_records $vision_records_id..." >&2
            invoke_contract "$contract_id" initialize \
                --admin "$deployer" \
                --vision_records "$vision_records_id"
            linked="\"vision_records\": \"$vision_records_id\""
            ;;
        *)
            echo "ERROR: $contract_name is not part of the deployment stack."
            exit 1
            ;;
    esac

    printf '%s' "$linked"
}

initialize_single_contract() {
    local contract_name="$1"
    local contract_id="$2"
    local deployer="$3"

    case "$contract_name" in
        access_control)
            echo "Initializing access_control..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            ;;
        clinical_records)
            if [ -z "$VISION_RECORDS_ID" ]; then
                echo "ERROR: clinical_records requires --vision-records <contract-id> when using --admin." >&2
                exit 1
            fi
            echo "Initializing clinical_records with vision_records $VISION_RECORDS_ID..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer" --vision_records "$VISION_RECORDS_ID"
            ;;
        *)
            echo "Initializing $contract_name with deployer as temporary admin..." >&2
            invoke_contract "$contract_id" initialize --admin "$deployer"
            ;;
    esac
}

deploy_single() {
    local contract_name="$1"

    echo "Deploying $contract_name to $NETWORK..."
    echo "Building contract..."
    cargo build --target wasm32-unknown-unknown --release

    local wasm_path="target/wasm32-unknown-unknown/release/${contract_name}.wasm"
    local contract_id
    contract_id=$(deploy_wasm "$contract_name")
    echo "Contract deployed: $contract_id"
    echo "DEPLOYMENT_CONTRACT_ID=$contract_id"

    if [ -n "$ADMIN_ADDRESS" ]; then
        local deployer
        deployer=$(resolve_deployer)

        initialize_single_contract "$contract_name" "$contract_id" "$deployer"

        echo "Transferring admin to permanent address: $ADMIN_ADDRESS"
        soroban contract invoke \
            --id "$contract_id" \
            --source default \
            --network "$NETWORK" \
            -- \
            transfer_admin \
            --current_admin "$deployer" \
            --new_admin "$ADMIN_ADDRESS"

        local verified_admin
        verified_admin=$(soroban contract invoke \
            --id "$contract_id" \
            --source default \
            --network "$NETWORK" \
            -- \
            get_admin 2>/dev/null || echo "")

        if echo "$verified_admin" | grep -q "$ADMIN_ADDRESS"; then
            echo "Admin transfer verified."
        else
            echo "WARNING: Could not verify admin transfer."
            echo "Verify manually: soroban contract invoke --id $contract_id -- get_admin"
        fi
    else
        echo "NOTE: No --admin flag provided. Contract deployed but NOT initialized."
        echo "Run initialization and admin transfer separately."
        echo "See docs/deployment-security.md for the secure deployment procedure."
    fi

    write_descriptor "$contract_name" "$contract_id" "$wasm_path" ""
}

deploy_stack() {
    echo "Deploying core stack to $NETWORK..."
    echo "Order: ${STACK_CONTRACTS[*]}"
    echo "Building contracts..."
    cargo build --target wasm32-unknown-unknown --release

    local deployer
    deployer=$(resolve_deployer)
    echo "Deployer (initializing admin): $deployer"
    if [ -n "$ADMIN_ADDRESS" ] && [ "$ADMIN_ADDRESS" != "$deployer" ]; then
        echo "NOTE: --admin is recorded as the intended permanent admin."
        echo "Stack contracts are initialized as $deployer because initialization requires that key's signature."
        echo "Complete admin handoff with propose_admin / accept_admin after this script finishes."
    fi

    declare -A IDS=()
    local name contract_id
    for name in "${STACK_CONTRACTS[@]}"; do
        contract_id=$(deploy_wasm "$name")
        IDS["$name"]="$contract_id"
        echo "Contract deployed: $contract_id"
        echo "DEPLOYMENT_CONTRACT_ID=$contract_id"
    done

    # Initialize only after every ID exists so later contracts receive the
    # addresses of contracts deployed earlier in STACK_CONTRACTS.
    local linked wasm_path
    for name in "${STACK_CONTRACTS[@]}"; do
        wasm_path="target/wasm32-unknown-unknown/release/${name}.wasm"
        linked=$(initialize_stack_contract \
            "$name" \
            "${IDS[$name]}" \
            "$deployer" \
            "${IDS[key_manager]:-}" \
            "${IDS[identity]:-}" \
            "${IDS[zk_verifier]:-}" \
            "${IDS[vision_records]:-}" \
            "${IDS[clinical_records]:-}" \
            "${IDS[access_control]:-}")
        write_descriptor "$name" "${IDS[$name]}" "$wasm_path" "$linked"
    done

    local summary="$DEPLOY_DIR/${NETWORK}_stack.json"
    cat > "$summary" << EOF
{
    "network": "$NETWORK",
    "admin": "$ADMIN_ADDRESS",
    "deployer": "$deployer",
    "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "contracts": {
        "audit": "${IDS[audit]}",
        "key_manager": "${IDS[key_manager]}",
        "identity": "${IDS[identity]}",
        "zk_verifier": "${IDS[zk_verifier]}",
        "vision_records": "${IDS[vision_records]}"
    }
}
EOF
    echo "Stack summary saved to: $summary"
    echo "Core stack deployment complete."
}

if [ "$CONTRACT" = "stack" ]; then
    deploy_stack
else
    deploy_single "$CONTRACT"
fi
