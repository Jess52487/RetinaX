//! Gas budget harness for `vision_records` (issue #323).
//!
//! Measures CPU instructions and memory bytes for the most expensive public
//! operations, using the host budget tracker exposed by `soroban_sdk`'s
//! testutils. Results come from the host cost model. Treat them as relative
//! guidance for comparing changes, not as network fees.
//!
//! The harness prints a report instead of asserting, so it is `#[ignore]`d and
//! stays out of the default `cargo test` run. Run it with:
//!
//! ```text
//! cargo test -p vision_records --test gas_budget -- --ignored --nocapture
//! ```
//!
//! Operations are chosen by the static ranking in `docs/gas-budget.md`
//! (storage reads and writes, cross-contract access checks, and loops).

mod common;

use common::{create_test_record, create_test_user, setup_test_env, TestContext};
use soroban_sdk::{testutils::Address as _, Address, String};
use vision_records::portability::PortabilityFormat;
use vision_records::{
    AccessLevel, IntraocularPressure, OptFundusPhotography, OptPhysicalMeasurement,
    OptRetinalImaging, OptVisualField, PhysicalMeasurement, RecordType, Role, SlitLampFindings,
    VisualAcuity,
};

const RECORDS_FOR_EXPORT: u32 = 10;

struct Sample {
    operation: &'static str,
    cpu_instructions: u64,
    memory_bytes: u64,
}

/// Run `op` with an unlimited budget and record what it consumed.
fn measure<T>(
    ctx: &TestContext,
    samples: &mut std::vec::Vec<Sample>,
    operation: &'static str,
    op: impl FnOnce() -> T,
) -> T {
    // The budget handle shares state with the host, so resetting it here
    // resets the tracker used by the call below.
    ctx.env.cost_estimate().budget().reset_unlimited();
    let out = op();
    let budget = ctx.env.cost_estimate().budget();
    samples.push(Sample {
        operation,
        cpu_instructions: budget.cpu_instruction_cost(),
        memory_bytes: budget.memory_bytes_cost(),
    });
    out
}

fn add_examination(ctx: &TestContext, provider: &Address, patient: &Address) -> u64 {
    let record_id = create_test_record(
        ctx,
        provider,
        patient,
        provider,
        RecordType::Examination,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    ctx.client.add_eye_examination(
        provider,
        &record_id,
        &VisualAcuity {
            uncorrected: PhysicalMeasurement {
                left_eye: String::from_str(&ctx.env, "20/40"),
                right_eye: String::from_str(&ctx.env, "20/25"),
            },
            corrected: OptPhysicalMeasurement::Some(PhysicalMeasurement {
                left_eye: String::from_str(&ctx.env, "20/20"),
                right_eye: String::from_str(&ctx.env, "20/20"),
            }),
        },
        &IntraocularPressure {
            left_eye: 16,
            right_eye: 22,
            method: String::from_str(&ctx.env, "Goldmann"),
            timestamp: ctx.env.ledger().timestamp(),
        },
        &SlitLampFindings {
            cornea: String::from_str(&ctx.env, "Clear"),
            anterior_chamber: String::from_str(&ctx.env, "Deep and quiet"),
            iris: String::from_str(&ctx.env, "Normal"),
            lens: String::from_str(&ctx.env, "Clear"),
        },
        &OptVisualField::None,
        &OptRetinalImaging::None,
        &OptFundusPhotography::None,
        &String::from_str(&ctx.env, "Routine exam"),
    );
    record_id
}

fn print_report(samples: &[Sample]) {
    println!();
    println!(
        "{:<40} {:>18} {:>16}",
        "operation", "cpu_instructions", "memory_bytes"
    );
    println!("{}", "-".repeat(76));
    for sample in samples {
        println!(
            "{:<40} {:>18} {:>16}",
            sample.operation, sample.cpu_instructions, sample.memory_bytes
        );
    }
    println!();
}

#[test]
#[ignore = "measurement harness: run with -- --ignored --nocapture"]
fn gas_budget_report() {
    let ctx = setup_test_env();
    let mut samples: std::vec::Vec<Sample> = std::vec::Vec::new();

    let patient = create_test_user(&ctx, Role::Patient, "Patient");
    let provider = create_test_user(&ctx, Role::Optometrist, "Provider");
    let grantee = Address::generate(&ctx.env);

    measure(&ctx, &mut samples, "register_user", || {
        let user = Address::generate(&ctx.env);
        ctx.client.register_user(
            &ctx.admin,
            &user,
            &Role::Patient,
            &String::from_str(&ctx.env, "Measured"),
        );
    });

    let record_id = measure(&ctx, &mut samples, "add_record", || {
        create_test_record(
            &ctx,
            &provider,
            &patient,
            &provider,
            RecordType::Diagnosis,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
    });

    measure(&ctx, &mut samples, "get_record", || {
        ctx.client.get_record(&patient, &record_id)
    });

    measure(&ctx, &mut samples, "grant_access", || {
        ctx.client
            .grant_access(&patient, &patient, &grantee, &AccessLevel::Read, &3_600)
    });

    measure(&ctx, &mut samples, "check_access", || {
        ctx.client.check_access(&patient, &grantee)
    });

    measure(&ctx, &mut samples, "get_patient_records_paginated", || {
        ctx.client.get_patient_records_paginated(&patient, &0, &10)
    });

    for _ in 0..RECORDS_FOR_EXPORT {
        add_examination(&ctx, &provider, &patient);
    }

    measure(
        &ctx,
        &mut samples,
        "export_patient_data (FHIR, 10 records)",
        || {
            ctx.client.export_patient_data(
                &patient,
                &patient,
                &PortabilityFormat::FhirJson,
                &0,
                &RECORDS_FOR_EXPORT,
            )
        },
    );

    measure(
        &ctx,
        &mut samples,
        "export_patient_data (CSV, 10 records)",
        || {
            ctx.client.export_patient_data(
                &patient,
                &patient,
                &PortabilityFormat::Csv,
                &0,
                &RECORDS_FOR_EXPORT,
            )
        },
    );

    print_report(&samples);
}
