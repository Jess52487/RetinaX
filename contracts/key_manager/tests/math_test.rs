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

fn create_master_key(env: &Env, client: &KeyManagerContractClient, admin: &Address) -> BytesN<32> {
    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(env),
    };
    let key_bytes = BytesN::from_array(env, &[1; 32]);
    client.create_master_key(admin, &KeyType::Encryption, &policy, &0, &key_bytes)
}

#[test]
fn test_record_key_derivation_is_deterministic() {
    let (env, client, admin) = setup();
    let key_id = create_master_key(&env, &client, &admin);

    let first = client.derive_record_key(&key_id, &42);
    let repeated = client.derive_record_key(&key_id, &42);

    assert_eq!(first.key, repeated.key);
    assert_eq!(first.version, repeated.version);
}

#[test]
fn test_record_key_derivation_is_scoped_to_record() {
    let (env, client, admin) = setup();
    let key_id = create_master_key(&env, &client, &admin);

    let first = client.derive_record_key(&key_id, &42);
    let second = client.derive_record_key(&key_id, &43);

    assert_ne!(first.key, second.key);
}

#[test]
fn test_record_key_rejects_unknown_version() {
    let (env, client, admin) = setup();
    let key_id = create_master_key(&env, &client, &admin);

    let result = client.try_derive_record_key_with_version(&key_id, &42, &2);

    assert!(result.is_err());
}
