#![allow(clippy::unwrap_used, clippy::expect_used)]

use cross_chain::{
    bridge::{self, BridgeError, ExportPackage},
    CrossChainContract, CrossChainContractClient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, Bytes, BytesN, Env, Vec,
};

fn setup() -> (Env, CrossChainContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainContract, ());
    let client = CrossChainContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, contract_id, admin)
}

fn export_package(env: &Env, record_id: BytesN<32>, data: &[u8]) -> ExportPackage {
    bridge::export_record(
        env,
        record_id,
        Bytes::from_slice(env, data),
        Vec::new(env),
        None,
        symbol_short!("ETH"),
    )
}

fn anchor_package(env: &Env, contract_id: &Address, package: &ExportPackage) {
    let root = package.state_root.clone();
    env.as_contract(contract_id, || {
        bridge::anchor_root(env, root, symbol_short!("ETH"));
    });
}

fn import_timestamp(env: &Env, contract_id: &Address, record_id: BytesN<32>) -> Option<u64> {
    env.as_contract(contract_id, || bridge::get_import_timestamp(env, record_id))
}

#[test]
fn test_valid_import_records_timestamp() {
    let (env, client, contract_id, _) = setup();
    let package = export_package(&env, BytesN::from_array(&env, &[1; 32]), b"clinical record");
    anchor_package(&env, &contract_id, &package);

    let result = client.try_import_record(&package, &package.state_root);

    assert!(result.is_ok());
    assert_eq!(
        import_timestamp(&env, &contract_id, package.record_id.clone()),
        Some(package.timestamp)
    );
}

#[test]
fn test_tampered_import_does_not_record_timestamp() {
    let (env, client, contract_id, _) = setup();
    let mut package = export_package(&env, BytesN::from_array(&env, &[2; 32]), b"clinical record");
    anchor_package(&env, &contract_id, &package);
    package.record_data = Bytes::from_slice(&env, b"tampered record");

    let result = client.try_import_record(&package, &package.state_root);

    assert!(result.is_err());
    assert_eq!(
        import_timestamp(&env, &contract_id, package.record_id),
        None
    );
}

#[test]
fn test_unanchored_root_is_rejected() {
    let (env, client, contract_id, _) = setup();
    let package = export_package(&env, BytesN::from_array(&env, &[3; 32]), b"clinical record");

    let result = client.try_import_record(&package, &package.state_root);

    assert!(result.is_err());
    assert_eq!(
        import_timestamp(&env, &contract_id, package.record_id),
        None
    );
}

#[test]
fn test_recent_anchor_is_rejected_until_finality() {
    let (env, _, contract_id, _) = setup();
    let package = export_package(&env, BytesN::from_array(&env, &[4; 32]), b"clinical record");
    anchor_package(&env, &contract_id, &package);
    let root = package.state_root.clone();
    let anchor_sequence = env.ledger().sequence();
    env.ledger().set_sequence_number(anchor_sequence + 1);

    let result = env.as_contract(&contract_id, || {
        bridge::import_record(&env, package, root, 2)
    });

    assert_eq!(result, Err(BridgeError::ChainReorgDetected));
}
