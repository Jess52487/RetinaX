#![allow(clippy::unwrap_used, clippy::expect_used)]

use key_manager::{KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};

fn setup() -> (Env, KeyManagerContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KeyManagerContract, ());
    let client = KeyManagerContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let identity = Address::generate(&env);
    client.initialize(&admin, &identity);

    (env, client, admin)
}

fn policy(env: &Env, not_before: u64, not_after: u64) -> KeyPolicy {
    KeyPolicy {
        max_uses: 0,
        not_before,
        not_after,
        allowed_ops: Vec::new(env),
    }
}

#[test]
fn test_create_master_key_rejects_inverted_policy_window() {
    let (env, client, admin) = setup();
    let key_bytes = BytesN::from_array(&env, &[1; 32]);

    let result = client.try_create_master_key(
        &admin,
        &KeyType::Encryption,
        &policy(&env, 100, 50),
        &0,
        &key_bytes,
    );

    assert!(result.is_err());
}

#[test]
fn test_non_admin_cannot_create_master_key() {
    let (env, client, _) = setup();
    let caller = Address::generate(&env);
    let key_bytes = BytesN::from_array(&env, &[2; 32]);

    let result = client.try_create_master_key(
        &caller,
        &KeyType::Encryption,
        &policy(&env, 0, 0),
        &0,
        &key_bytes,
    );

    assert!(result.is_err());
}

#[test]
fn test_use_key_rejects_unknown_key() {
    let (env, client, admin) = setup();
    let unknown_key_id = BytesN::from_array(&env, &[0; 32]);

    let result = client.try_use_key(&admin, &unknown_key_id, &soroban_sdk::symbol_short!("ENC"));

    assert!(result.is_err());
}
