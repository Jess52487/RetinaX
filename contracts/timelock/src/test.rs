use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};

fn create_tx_id(env: &Env, val: u8) -> BytesN<32> {
    let mut arr = [0u8; 32];
    arr[0] = val;
    BytesN::from_array(env, &arr)
}

#[test]
fn test_initialize_and_getters() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let min_delay = 3600u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_min_delay(), min_delay);
    assert_eq!(client.get_queue().len(), 0);

    // Initializing twice should fail
    let res = client.try_initialize(&admin, &min_delay);
    assert_eq!(res, Err(Ok(TimelockError::AlreadyInitialized)));
}

#[test]
fn test_queue_and_get_tx() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let min_delay = 100u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    env.ledger().set_timestamp(1000);

    let tx_id = create_tx_id(&env, 1);
    let execute_after = client.queue_tx(&admin, &tx_id, &200);
    assert_eq!(execute_after, 1200);

    let tx = client.get_tx(&tx_id);
    assert_eq!(tx.id, tx_id);
    assert_eq!(tx.execute_after, 1200);
    assert!(!tx.priority);

    // Queueing duplicate tx fails
    let res = client.try_queue_tx(&admin, &tx_id, &200);
    assert_eq!(res, Err(Ok(TimelockError::TxAlreadyQueued)));

    // Queueing with delay < min_delay fails
    let tx_id_2 = create_tx_id(&env, 2);
    let res2 = client.try_queue_tx(&admin, &tx_id_2, &50);
    assert_eq!(res2, Err(Ok(TimelockError::DelayTooShort)));
}

#[test]
fn test_execute_tx_timing() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let min_delay = 100u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    env.ledger().set_timestamp(1000);
    let tx_id = create_tx_id(&env, 1);
    client.queue_tx(&admin, &tx_id, &100);

    // At t=1050, execution should fail with TooEarly
    env.ledger().set_timestamp(1050);
    let res = client.try_execute_tx(&admin, &tx_id);
    assert_eq!(res, Err(Ok(TimelockError::TooEarly)));

    // At t=1100, execution succeeds
    env.ledger().set_timestamp(1100);
    client.execute_tx(&admin, &tx_id);

    // Should no longer be in queue
    assert_eq!(client.get_queue().len(), 0);
    let res = client.try_get_tx(&tx_id);
    assert_eq!(res, Err(Ok(TimelockError::TxNotFound)));
}

#[test]
fn test_prioritize_tx_bypasses_delay() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let min_delay = 1000u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    env.ledger().set_timestamp(1000);
    let tx_id = create_tx_id(&env, 1);
    client.queue_tx(&admin, &tx_id, &1000);

    // Prioritize tx
    client.prioritize_tx(&admin, &tx_id);
    let tx = client.get_tx(&tx_id);
    assert!(tx.priority);

    // Can execute immediately even before execute_after
    env.ledger().set_timestamp(1005);
    client.execute_tx(&admin, &tx_id);
    assert_eq!(client.get_queue().len(), 0);
}

#[test]
fn test_cancel_tx() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let min_delay = 100u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    env.ledger().set_timestamp(1000);
    let tx_id = create_tx_id(&env, 1);
    client.queue_tx(&admin, &tx_id, &100);

    assert_eq!(client.get_queue().len(), 1);

    client.cancel_tx(&admin, &tx_id);
    assert_eq!(client.get_queue().len(), 0);

    let res = client.try_cancel_tx(&admin, &tx_id);
    assert_eq!(res, Err(Ok(TimelockError::TxNotFound)));
}

#[test]
fn test_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register(TimelockContract, ());
    let client = TimelockContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let min_delay = 100u64;

    env.mock_all_auths();
    client.initialize(&admin, &min_delay);

    let tx_id = create_tx_id(&env, 1);

    let res = client.try_queue_tx(&attacker, &tx_id, &100);
    assert_eq!(res, Err(Ok(TimelockError::Unauthorized)));

    client.queue_tx(&admin, &tx_id, &100);

    let res = client.try_prioritize_tx(&attacker, &tx_id);
    assert_eq!(res, Err(Ok(TimelockError::Unauthorized)));

    let res = client.try_cancel_tx(&attacker, &tx_id);
    assert_eq!(res, Err(Ok(TimelockError::Unauthorized)));

    env.ledger().set_timestamp(2000);
    let res = client.try_execute_tx(&attacker, &tx_id);
    assert_eq!(res, Err(Ok(TimelockError::Unauthorized)));
}
