#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects
)]
extern crate std;
mod common;

use common::setup_test_env;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::xdr::{ContractEventBody, ScVal};
use soroban_sdk::{symbol_short, Address, Env, IntoVal, TryFromVal, Val, Vec};
use vision_records::events::{ConsentGrantedEvent, ConsentRevokedEvent, ConsentUpdatedEvent};
use vision_records::{ConsentType, ContractError};

fn find_matching_event<T>(env: &Env, expected_topics: &Vec<Val>, expected_data: &T) -> bool
where
    T: Clone + IntoVal<Env, Val>,
{
    let events = env.events().all();
    let mut expected_topics_scval = std::vec::Vec::new();
    for topic in expected_topics.iter() {
        let topic_val: Val = topic;
        expected_topics_scval.push(ScVal::try_from_val(env, &topic_val).unwrap());
    }

    let expected_val: Val = expected_data.clone().into_val(env);
    let expected_data_scval: ScVal = ScVal::try_from_val(env, &expected_val).unwrap();

    for event in events.events() {
        let ContractEventBody::V0(body) = &event.body;
        if body.topics.as_slice() == expected_topics_scval.as_slice()
            && body.data == expected_data_scval
        {
            return true;
        }
    }
    false
}

#[test]
fn test_grant_consent_emits_detailed_event() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let doctor = Address::generate(&ctx.env);

    let start_time = 1_700_000_000;
    ctx.env.ledger().set_timestamp(start_time);

    let duration_seconds = 86400; // 1 day
    let expected_expires_at = start_time + duration_seconds;

    // Grant consent
    ctx.client.grant_consent(
        &patient,
        &doctor,
        &ConsentType::Treatment,
        &duration_seconds,
    );

    let expected_topics: Vec<Val> =
        (symbol_short!("CST_GRT"), patient.clone(), doctor.clone()).into_val(&ctx.env);

    let expected_data = ConsentGrantedEvent {
        patient: patient.clone(),
        grantee: doctor.clone(),
        consent_type: ConsentType::Treatment,
        granted_at: start_time,
        expires_at: expected_expires_at,
        duration_seconds,
        timestamp: start_time,
    };

    assert!(
        find_matching_event(&ctx.env, &expected_topics, &expected_data),
        "Expected ConsentGrantedEvent was not emitted with matching topics and payload"
    );
}

#[test]
fn test_revoke_consent_emits_detailed_event() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let doctor = Address::generate(&ctx.env);

    let start_time = 1_700_000_000;
    ctx.env.ledger().set_timestamp(start_time);

    // First grant consent
    ctx.client
        .grant_consent(&patient, &doctor, &ConsentType::Research, &7200);

    let revoke_time = start_time + 1000;
    ctx.env.ledger().set_timestamp(revoke_time);

    // Revoke consent
    ctx.client.revoke_consent(&patient, &doctor);

    let expected_topics: Vec<Val> =
        (symbol_short!("CST_REV"), patient.clone(), doctor.clone()).into_val(&ctx.env);

    let expected_data = ConsentRevokedEvent {
        patient: patient.clone(),
        grantee: doctor.clone(),
        revoked_at: revoke_time,
        timestamp: revoke_time,
    };

    assert!(
        find_matching_event(&ctx.env, &expected_topics, &expected_data),
        "Expected ConsentRevokedEvent was not emitted with matching topics and payload"
    );
}

#[test]
fn test_update_consent_emits_both_update_and_grant_events() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let doctor = Address::generate(&ctx.env);

    let t1 = 1_700_000_000;
    ctx.env.ledger().set_timestamp(t1);

    // Initial grant: 3600 seconds
    ctx.client
        .grant_consent(&patient, &doctor, &ConsentType::Sharing, &3600);

    let t2 = t1 + 1800;
    ctx.env.ledger().set_timestamp(t2);

    // Update grant: extend for 7200 seconds
    ctx.client
        .grant_consent(&patient, &doctor, &ConsentType::Sharing, &7200);

    // Check CST_UPD event
    let expected_update_topics: Vec<Val> =
        (symbol_short!("CST_UPD"), patient.clone(), doctor.clone()).into_val(&ctx.env);

    let expected_update_data = ConsentUpdatedEvent {
        patient: patient.clone(),
        grantee: doctor.clone(),
        consent_type: ConsentType::Sharing,
        old_expires_at: t1 + 3600,
        new_expires_at: t2 + 7200,
        timestamp: t2,
    };

    assert!(
        find_matching_event(&ctx.env, &expected_update_topics, &expected_update_data),
        "Expected ConsentUpdatedEvent was not emitted on consent update"
    );

    // Check new CST_GRT event
    let expected_grant_topics: Vec<Val> =
        (symbol_short!("CST_GRT"), patient.clone(), doctor.clone()).into_val(&ctx.env);

    let expected_grant_data = ConsentGrantedEvent {
        patient: patient.clone(),
        grantee: doctor.clone(),
        consent_type: ConsentType::Sharing,
        granted_at: t2,
        expires_at: t2 + 7200,
        duration_seconds: 7200,
        timestamp: t2,
    };

    assert!(
        find_matching_event(&ctx.env, &expected_grant_topics, &expected_grant_data),
        "Expected new ConsentGrantedEvent was not emitted on consent update"
    );
}

#[test]
fn test_grant_consent_zero_duration_rejected() {
    let ctx = setup_test_env();
    let patient = Address::generate(&ctx.env);
    let doctor = Address::generate(&ctx.env);

    let res = ctx
        .client
        .try_grant_consent(&patient, &doctor, &ConsentType::Treatment, &0);

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, Ok(ContractError::InvalidInput)));
}
