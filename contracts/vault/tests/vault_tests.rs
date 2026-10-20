#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
#![cfg(test)]

extern crate std;

use identity::{IdentityContract, IdentityContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, String, Vec,
};
use vault::{
    sss, VaultContract, VaultContractClient, VaultError, VaultPolicy, VaultRecord, VaultShare,
    VaultSnapshot,
};

fn setup_vault(
    env: &Env,
) -> (
    VaultContractClient<'_>,
    IdentityContractClient<'_>,
    Address,
    Address,
    Address,
) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let owner = Address::generate(env);

    let identity_id = env.register(IdentityContract, ());
    let identity = IdentityContractClient::new(env, &identity_id);
    identity.initialize(&owner);

    let vault_id = env.register(VaultContract, ());
    let vault = VaultContractClient::new(env, &vault_id);
    vault.initialize(&admin, &identity_id);

    (vault, identity, admin, owner, identity_id)
}

fn add_guardians(
    env: &Env,
    identity: &IdentityContractClient,
    owner: &Address,
    count: u32,
) -> std::vec::Vec<Address> {
    let mut list = std::vec::Vec::new();
    for _ in 0..count {
        let g = Address::generate(env);
        identity.add_guardian(owner, &g);
        list.push(g);
    }
    list
}

#[test]
fn test_initialization_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = env.register(VaultContract, ());
    let vault = VaultContractClient::new(&env, &vault_id);

    let admin = Address::generate(&env);
    let identity_id = Address::generate(&env);

    // Initial setup succeeds
    let init_res = vault.try_initialize(&admin, &identity_id);
    assert!(init_res.is_ok());

    // Second initialization must fail with AlreadyInitialized
    let double_init = vault.try_initialize(&admin, &identity_id);
    assert_eq!(double_init, Err(Ok(VaultError::AlreadyInitialized)));
}

#[test]
fn test_uninitialized_calls_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = env.register(VaultContract, ());
    let vault = VaultContractClient::new(&env, &vault_id);

    let owner = Address::generate(&env);
    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let data_ref = String::from_str(&env, "ipfs://meta");

    // configure_vault rejected when uninitialized
    let res_conf = vault.try_configure_vault(&owner, &2, &3, &2, &3600, &data_ref, &seed);
    assert_eq!(res_conf, Err(Ok(VaultError::NotInitialized)));

    // reconstruct_key rejected when uninitialized
    let shares = Vec::new(&env);
    let res_recon = vault.try_reconstruct_key(&owner, &owner, &shares);
    assert_eq!(res_recon, Err(Ok(VaultError::NotInitialized)));

    // submit_emergency_approval rejected
    let res_apr = vault.try_submit_emergency_approval(&owner, &owner);
    assert_eq!(res_apr, Err(Ok(VaultError::NotInitialized)));

    // emergency_reconstruct rejected
    let res_emg = vault.try_emergency_reconstruct(&owner);
    assert_eq!(res_emg, Err(Ok(VaultError::NotInitialized)));

    // rotate_key rejected
    let res_rot = vault.try_rotate_key(&owner, &seed);
    assert_eq!(res_rot, Err(Ok(VaultError::NotInitialized)));
}

#[test]
fn test_configure_vault_validation_matrix() {
    let env = Env::default();
    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 5);

    let seed = BytesN::from_array(&env, &[42u8; 32]);
    let data_ref = String::from_str(&env, "ipfs://doc");

    // Zero threshold
    assert_eq!(
        vault.try_configure_vault(&owner, &0, &5, &3, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );

    // Zero shard count
    assert_eq!(
        vault.try_configure_vault(&owner, &3, &0, &3, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );

    // Threshold greater than shard count
    assert_eq!(
        vault.try_configure_vault(&owner, &6, &5, &6, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );

    // Emergency threshold below normal threshold
    assert_eq!(
        vault.try_configure_vault(&owner, &3, &5, &2, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );

    // Emergency threshold greater than shard count
    assert_eq!(
        vault.try_configure_vault(&owner, &3, &5, &6, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );

    // Shard count exceeding registered guardians count
    let other_owner = Address::generate(&env);
    identity.initialize(&other_owner);
    add_guardians(&env, &identity, &other_owner, 2);
    assert_eq!(
        vault.try_configure_vault(&other_owner, &2, &4, &3, &3600, &data_ref, &seed),
        Err(Ok(VaultError::InvalidConfig))
    );
}

#[test]
fn test_configure_and_get_vault_invariants() {
    let env = Env::default();
    env.ledger().set_timestamp(1_000_000);
    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    let guardians = add_guardians(&env, &identity, &owner, 4);

    let seed = BytesN::from_array(&env, &[7u8; 32]);
    let data_ref = String::from_str(&env, "ipfs://vault-config");

    let snapshot: VaultSnapshot =
        vault.configure_vault(&owner, &2, &4, &3, &7200, &data_ref, &seed);

    assert_eq!(snapshot.record.owner, owner);
    assert_eq!(snapshot.record.epoch, 1);
    assert_eq!(snapshot.record.created_at, 1_000_000);
    assert_eq!(snapshot.record.last_activity_at, 1_000_000);
    assert_eq!(snapshot.record.deadman_release_at, 1_007_200);
    assert_eq!(snapshot.shard_holders.len(), 4);

    for (i, expected_g) in guardians.iter().enumerate() {
        assert_eq!(snapshot.shard_holders.get(i as u32).unwrap(), *expected_g);
    }

    let fetched: VaultRecord = vault.get_vault(&owner);
    assert_eq!(fetched, snapshot.record);

    let non_owner = Address::generate(&env);
    assert_eq!(
        vault.try_get_vault(&non_owner),
        Err(Ok(VaultError::VaultNotFound))
    );
}

#[test]
fn test_reconstruct_key_and_insufficient_shares() {
    let env = Env::default();
    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    let guardians = add_guardians(&env, &identity, &owner, 4);

    let seed_bytes = [15u8; 32];
    let seed = BytesN::from_array(&env, &seed_bytes);
    let data_ref = String::from_str(&env, "vault-ref");

    // 2-of-4 threshold
    vault.configure_vault(&owner, &2, &4, &3, &3600, &data_ref, &seed);

    let split_shares = sss::split(
        seed_bytes,
        2,
        4,
        env.crypto().sha256(&seed.clone().into()).to_array(),
    );

    // Insufficient shares (only 1 provided when 2 are required)
    let mut insufficient = Vec::new(&env);
    insufficient.push_back(VaultShare {
        guardian: guardians[0].clone(),
        x: split_shares[0].x as u32,
        y: BytesN::from_array(&env, &split_shares[0].y),
    });
    let err_recon = vault.try_reconstruct_key(&owner, &owner, &insufficient);
    assert_eq!(err_recon, Err(Ok(VaultError::InsufficientShares)));

    // Invalid/corrupted share provided
    let mut corrupted = Vec::new(&env);
    corrupted.push_back(VaultShare {
        guardian: guardians[0].clone(),
        x: split_shares[0].x as u32,
        y: BytesN::from_array(&env, &split_shares[0].y),
    });
    corrupted.push_back(VaultShare {
        guardian: guardians[1].clone(),
        x: 99, // tampered x coordinate
        y: BytesN::from_array(&env, &split_shares[1].y),
    });
    let err_corrupt = vault.try_reconstruct_key(&owner, &owner, &corrupted);
    assert_eq!(err_corrupt, Err(Ok(VaultError::InvalidShare)));

    // Valid threshold shares reconstruct key
    let mut valid = Vec::new(&env);
    valid.push_back(VaultShare {
        guardian: guardians[0].clone(),
        x: split_shares[0].x as u32,
        y: BytesN::from_array(&env, &split_shares[0].y),
    });
    valid.push_back(VaultShare {
        guardian: guardians[1].clone(),
        x: split_shares[1].x as u32,
        y: BytesN::from_array(&env, &split_shares[1].y),
    });

    let reconstructed = vault.reconstruct_key(&owner, &owner, &valid);
    assert_eq!(reconstructed, seed);
}

#[test]
fn test_emergency_approval_and_reconstruction_flow() {
    let env = Env::default();
    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    let guardians = add_guardians(&env, &identity, &owner, 5);

    let seed_bytes = [88u8; 32];
    let seed = BytesN::from_array(&env, &seed_bytes);
    let data_ref = String::from_str(&env, "vault-emergency");

    // Threshold: 3, Emergency Threshold: 4, Shard Count: 5
    vault.configure_vault(&owner, &3, &5, &4, &3600, &data_ref, &seed);

    // Non-guardian cannot submit emergency approval
    let random_user = Address::generate(&env);
    assert_eq!(
        vault.try_submit_emergency_approval(&random_user, &owner),
        Err(Ok(VaultError::Unauthorized))
    );

    // Emergency reconstruction fails before threshold met
    assert_eq!(
        vault.try_emergency_reconstruct(&owner),
        Err(Ok(VaultError::EmergencyThresholdNotMet))
    );

    // Submit 3 approvals (below emergency threshold of 4)
    for g in guardians.iter().take(3) {
        vault.submit_emergency_approval(g, &owner);
    }
    assert_eq!(
        vault.try_emergency_reconstruct(&owner),
        Err(Ok(VaultError::EmergencyThresholdNotMet))
    );

    // Submit 4th approval - meets emergency threshold
    vault.submit_emergency_approval(&guardians[3], &owner);
    let recovered_seed = vault.emergency_reconstruct(&owner);
    assert_eq!(recovered_seed, seed);
}

#[test]
fn test_key_rotation_and_epoch_increment() {
    let env = Env::default();
    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    let _guardians = add_guardians(&env, &identity, &owner, 3);

    let old_seed = BytesN::from_array(&env, &[11u8; 32]);
    let new_seed = BytesN::from_array(&env, &[22u8; 32]);
    let data_ref = String::from_str(&env, "doc");

    vault.configure_vault(&owner, &2, &3, &2, &3600, &data_ref, &old_seed);

    let rotation = vault.rotate_key(&owner, &new_seed);
    assert_eq!(rotation.owner, owner);
    assert_eq!(rotation.previous_epoch, 1);
    assert_eq!(rotation.new_epoch, 2);

    let record = vault.get_vault(&owner);
    assert_eq!(record.epoch, 2);
}

#[test]
fn test_touch_activity_and_deadman_switch() {
    let env = Env::default();
    env.ledger().set_timestamp(5_000);

    let (vault, identity, _admin, owner, _id) = setup_vault(&env);
    add_guardians(&env, &identity, &owner, 3);

    let seed = BytesN::from_array(&env, &[33u8; 32]);
    let data_ref = String::from_str(&env, "doc");
    vault.configure_vault(&owner, &2, &3, &2, &1000, &data_ref, &seed);

    // Deadman release fails before timeout
    env.ledger().set_timestamp(5_500);
    assert_eq!(
        vault.try_trigger_deadman_release(&owner),
        Err(Ok(VaultError::DeadmanNotReady))
    );

    // Activity reset extends deadline
    vault.touch_activity(&owner);
    let updated = vault.get_vault(&owner);
    assert_eq!(updated.last_activity_at, 5_500);
    assert_eq!(updated.deadman_release_at, 6_500);

    // Still not ready at 6_000
    env.ledger().set_timestamp(6_000);
    assert_eq!(
        vault.try_trigger_deadman_release(&owner),
        Err(Ok(VaultError::DeadmanNotReady))
    );

    // Ready after extended deadline elapses
    env.ledger().set_timestamp(6_501);
    assert!(vault.trigger_deadman_release(&owner));
}
