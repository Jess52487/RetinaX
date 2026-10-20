#![allow(clippy::unwrap_used, clippy::expect_used)]

use key_manager::{
    ContractError, KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Vec,
};

fn setup(timestamp: u64) -> (Env, KeyManagerContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(timestamp);

    let contract_id = env.register(KeyManagerContract, ());
    let client = KeyManagerContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let identity = Address::generate(&env);
    client.initialize(&admin, &identity);

    (env, client, admin)
}

fn create_key(
    env: &Env,
    client: &KeyManagerContractClient,
    admin: &Address,
    not_before: u64,
    not_after: u64,
) -> BytesN<32> {
    let policy = KeyPolicy {
        max_uses: 0,
        not_before,
        not_after,
        allowed_ops: Vec::new(env),
    };
    let key_bytes = BytesN::from_array(env, &[3; 32]);
    client.create_master_key(admin, &KeyType::Encryption, &policy, &0, &key_bytes)
}

#[test]
fn test_key_expired_after_deadline_is_rejected() {
    let (env, client, admin) = setup(1_000);
    let key_id = create_key(&env, &client, &admin, 0, 1_100);
    env.ledger().set_timestamp(1_101);

    let result = client.try_use_key(&admin, &key_id, &symbol_short!("ENC"));

    assert_eq!(result, Err(Ok(ContractError::PolicyViolation)));
}

#[test]
fn test_key_is_valid_before_expiry() {
    let (env, client, admin) = setup(2_000);
    let key_id = create_key(&env, &client, &admin, 0, 2_100);
    env.ledger().set_timestamp(2_099);

    let result = client.try_use_key(&admin, &key_id, &symbol_short!("ENC"));

    assert!(result.is_ok());
}

#[test]
fn test_key_use_is_blocked_until_not_before() {
    let (env, client, admin) = setup(3_000);
    let key_id = create_key(&env, &client, &admin, 3_200, 0);

    let early_result = client.try_use_key(&admin, &key_id, &symbol_short!("ENC"));
    assert_eq!(early_result, Err(Ok(ContractError::PolicyViolation)));

    env.ledger().set_timestamp(3_201);
    let valid_result = client.try_use_key(&admin, &key_id, &symbol_short!("ENC"));
    assert!(valid_result.is_ok());
}
