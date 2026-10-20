#![allow(clippy::unwrap_used, clippy::expect_used)]

extern crate std;

use key_manager::{KeyManagerContract, KeyManagerContractClient, KeyPolicy, KeyType};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    xdr::{ContractEventBody, ScVal},
    Address, Env, IntoVal, TryFromVal, Val, Vec,
};

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

fn create_master_key(
    env: &Env,
    client: &KeyManagerContractClient,
    admin: &Address,
) -> soroban_sdk::BytesN<32> {
    let policy = KeyPolicy {
        max_uses: 0,
        not_before: 0,
        not_after: 0,
        allowed_ops: Vec::new(env),
    };
    let key_bytes = soroban_sdk::BytesN::from_array(env, &[7; 32]);
    client.create_master_key(admin, &KeyType::Encryption, &policy, &0, &key_bytes)
}

fn assert_audit_event(env: &Env, seq: u64) {
    let emitted = env.events().all();
    let events = emitted.events();
    assert_eq!(events.len(), 1, "expected one audit event");

    let ContractEventBody::V0(body) = &events.last().unwrap().body;
    let topics: Vec<Val> = (symbol_short!("AUDIT"), seq).into_val(env);
    let expected_topics = topics
        .iter()
        .map(|topic| ScVal::try_from_val(env, &topic).unwrap())
        .collect::<std::vec::Vec<_>>();
    assert_eq!(body.topics.as_slice(), expected_topics.as_slice());
}

#[test]
fn test_create_master_key_emits_audit_event() {
    let (env, client, admin) = setup();

    create_master_key(&env, &client, &admin);

    assert_audit_event(&env, 1);
}

#[test]
fn test_use_key_emits_audit_event() {
    let (env, client, admin) = setup();
    let key_id = create_master_key(&env, &client, &admin);

    client.use_key(&admin, &key_id, &symbol_short!("ENC"));

    assert_audit_event(&env, 2);
}

#[test]
fn test_revoke_key_emits_audit_event() {
    let (env, client, admin) = setup();
    let key_id = create_master_key(&env, &client, &admin);

    client.revoke_key(&admin, &key_id);

    assert_audit_event(&env, 2);
}

#[test]
fn test_failed_key_use_does_not_emit_audit_event() {
    let (env, client, admin) = setup();
    let missing_key_id = soroban_sdk::BytesN::from_array(&env, &[0; 32]);

    assert!(client
        .try_use_key(&admin, &missing_key_id, &symbol_short!("ENC"))
        .is_err());
    assert!(env.events().all().events().is_empty());
}
