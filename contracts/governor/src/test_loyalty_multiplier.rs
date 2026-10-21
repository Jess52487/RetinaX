//! Tests for the time-weighted loyalty multiplier (issue #311).
//! Covers: multiplier progression across 1 / 30 / 365 day tiers, exact day
//! boundaries, reset when stake age drops to zero, and monotonicity
//! properties.

extern crate std;

use proptest::prelude::*;

use crate::{
    test_helpers::{setup, DAY},
    voting::{compute_vote_power, loyalty_multiplier_scaled, MAX_LOYALTY_DAYS, SCALE},
};

/// 1_000_000 tokens → sqrt = 1_000, so vote power equals the scaled multiplier.
const STAKE: i128 = 1_000_000;

#[test]
fn test_multiplier_progression_1_30_365_days() {
    let m0 = loyalty_multiplier_scaled(0);
    let m1 = loyalty_multiplier_scaled(DAY);
    let m30 = loyalty_multiplier_scaled(30 * DAY);
    let m365 = loyalty_multiplier_scaled(365 * DAY);

    assert_eq!(m0, 1_000);
    assert_eq!(m1, 1_002); // 1 + 1/365
    assert_eq!(m30, 1_082); // 1 + 30/365
    assert_eq!(m365, 2_000); // capped at 2×
    assert!(m0 < m1 && m1 < m30 && m30 < m365);
}

#[test]
fn test_vote_power_progression_1_30_365_days() {
    let gov = setup(1_000_000_000);
    let voter = gov.staker(STAKE, DAY);
    assert_eq!(gov.client.get_vote_power(&voter), 1_002);

    gov.set_age(&voter, 30 * DAY);
    assert_eq!(gov.client.get_vote_power(&voter), 1_082);

    gov.set_age(&voter, 365 * DAY);
    assert_eq!(gov.client.get_vote_power(&voter), 2_000);
}

#[test]
fn test_multiplier_only_counts_whole_days() {
    // Partial days are floored: one second short of a day earns nothing.
    assert_eq!(loyalty_multiplier_scaled(DAY - 1), SCALE);
    assert_eq!(loyalty_multiplier_scaled(DAY), 1_002);
    assert_eq!(loyalty_multiplier_scaled(2 * DAY - 1), 1_002);
    assert_eq!(loyalty_multiplier_scaled(2 * DAY), 1_005);
}

#[test]
fn test_multiplier_at_cap_boundary() {
    let cap = MAX_LOYALTY_DAYS * DAY;
    assert_eq!(loyalty_multiplier_scaled(cap - 1), 1_997); // 364 whole days
    assert_eq!(loyalty_multiplier_scaled(cap), 2 * SCALE);
    assert_eq!(loyalty_multiplier_scaled(cap + 1), 2 * SCALE);
    assert_eq!(loyalty_multiplier_scaled(cap + DAY), 2 * SCALE);
    assert_eq!(loyalty_multiplier_scaled(u64::MAX), 2 * SCALE);
}

#[test]
fn test_multiplier_resets_when_stake_age_resets() {
    // The staking contract clears the stake-age clock on a full unstake;
    // the governor must fall back to the base 1× multiplier.
    let gov = setup(1_000_000_000);
    let voter = gov.staker(STAKE, 365 * DAY);
    assert_eq!(gov.client.get_vote_power(&voter), 2_000);

    gov.set_age(&voter, 0);
    assert_eq!(gov.client.get_vote_power(&voter), 1_000);
}

#[test]
fn test_partial_unstake_keeps_multiplier() {
    // A partial unstake lowers the stake but keeps the age, so only the
    // quadratic base shrinks while the multiplier is unchanged.
    let gov = setup(1_000_000_000);
    let voter = gov.staker(STAKE, 182 * DAY);
    let before = gov.client.get_vote_power(&voter);

    gov.set_stake(&voter, STAKE / 4); // sqrt halves
    let after = gov.client.get_vote_power(&voter);

    assert_eq!(after, before / 2);
    assert_eq!(after, compute_vote_power(STAKE / 4, 182 * DAY));
}

proptest! {
    #[test]
    fn prop_multiplier_monotonic_non_decreasing(a in any::<u64>(), b in any::<u64>()) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(loyalty_multiplier_scaled(lo) <= loyalty_multiplier_scaled(hi));
    }

    #[test]
    fn prop_multiplier_bounded_between_1x_and_2x(age in any::<u64>()) {
        let m = loyalty_multiplier_scaled(age);
        prop_assert!((SCALE..=2 * SCALE).contains(&m));
    }

    #[test]
    fn prop_vote_power_monotonic_in_time(
        staked in 0i128..1_000_000_000_000,
        a in 0u64..(2 * MAX_LOYALTY_DAYS * DAY),
        b in 0u64..(2 * MAX_LOYALTY_DAYS * DAY),
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(compute_vote_power(staked, lo) <= compute_vote_power(staked, hi));
    }
}
