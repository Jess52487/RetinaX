//! Simple, host-side rate limiting helpers that mirror the on-chain pattern.
//!
//! This module is intentionally `std`-only and does not depend on `soroban_sdk`
//! so it can be used from off-chain tooling or simulations.

/// Configuration for a fixed-window rate limiter.
#[derive(Clone, Debug)]
pub struct RateLimiterConfig {
    pub max_requests_per_window: u64,
    pub window_duration_seconds: u64,
}

impl RateLimiterConfig {
    pub fn new(max_requests_per_window: u64, window_duration_seconds: u64) -> Self {
        Self {
            max_requests_per_window,
            window_duration_seconds,
        }
    }

    /// Returns `true` if the configuration represents an enabled limiter.
    pub fn is_enabled(&self) -> bool {
        self.max_requests_per_window > 0 && self.window_duration_seconds > 0
    }
}

/// Per-identity rate limiting state.
///
/// This mirrors the `(count, window_start)` tuple that on-chain contracts
/// persist using Soroban storage.
#[derive(Clone, Debug)]
pub struct RateLimiterState {
    /// Number of requests seen in the current window.
    pub count: u64,
    /// Timestamp (in seconds) when the current window started.
    pub window_start: u64,
}

impl RateLimiterState {
    /// Creates an empty state starting at `now`.
    pub fn new(now: u64) -> Self {
        Self {
            count: 0,
            window_start: now,
        }
    }

    /// Records a single hit at `now` using `cfg`.
    ///
    /// Returns `true` if the hit is allowed, or `false` if it exceeds the
    /// configured limit for the current window.
    pub fn record_hit(&mut self, now: u64, cfg: &RateLimiterConfig) -> bool {
        if !cfg.is_enabled() {
            return true;
        }

        // Reset the window if it has fully elapsed.
        let window_end = self
            .window_start
            .saturating_add(cfg.window_duration_seconds);
        if now >= window_end {
            self.window_start = now;
            self.count = 0;
        }

        let next = self.count.saturating_add(1);
        if next > cfg.max_requests_per_window {
            return false;
        }

        self.count = next;
        true
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── RateLimiterConfig ─────────────────────────────────────────────────────

    #[test]
    fn disabled_limiter_always_allows() {
        let cfg = RateLimiterConfig::new(0, 60);
        assert!(!cfg.is_enabled());
        let mut state = RateLimiterState::new(0);
        // A disabled limiter must allow unlimited hits.
        for _ in 0..1_000 {
            assert!(state.record_hit(0, &cfg));
        }
    }

    #[test]
    fn zero_window_duration_is_disabled() {
        let cfg = RateLimiterConfig::new(10, 0);
        assert!(!cfg.is_enabled());
        let mut state = RateLimiterState::new(0);
        assert!(state.record_hit(0, &cfg));
    }

    #[test]
    fn allows_up_to_max_requests_within_window() {
        let cfg = RateLimiterConfig::new(3, 60);
        let mut state = RateLimiterState::new(0);

        assert!(state.record_hit(0, &cfg)); // request 1
        assert!(state.record_hit(0, &cfg)); // request 2
        assert!(state.record_hit(0, &cfg)); // request 3
                                            // 4th request within the same window must be denied.
        assert!(!state.record_hit(0, &cfg));
    }

    #[test]
    fn window_boundary_exact_resets_counter() {
        let cfg = RateLimiterConfig::new(2, 60);
        let mut state = RateLimiterState::new(0);

        assert!(state.record_hit(0, &cfg)); // 1st in window [0, 60)
        assert!(state.record_hit(0, &cfg)); // 2nd in window [0, 60)
        assert!(!state.record_hit(0, &cfg)); // 3rd rejected

        // At exactly t=60 the window resets.
        assert!(state.record_hit(60, &cfg)); // 1st in window [60, 120)
        assert!(state.record_hit(60, &cfg)); // 2nd in window [60, 120)
        assert!(!state.record_hit(60, &cfg)); // 3rd rejected again
    }

    #[test]
    fn request_one_tick_before_window_boundary_still_rejected() {
        let cfg = RateLimiterConfig::new(1, 60);
        let mut state = RateLimiterState::new(0);

        assert!(state.record_hit(0, &cfg)); // 1st request
                                            // At t=59 the window [0, 60) has not yet expired.
        assert!(!state.record_hit(59, &cfg));
        // At t=60 the window resets and the request is allowed.
        assert!(state.record_hit(60, &cfg));
    }

    #[test]
    fn large_time_jump_resets_window() {
        let cfg = RateLimiterConfig::new(1, 60);
        let mut state = RateLimiterState::new(0);

        assert!(state.record_hit(0, &cfg)); // fill the window
        assert!(!state.record_hit(30, &cfg)); // still in window — rejected

        // Jump far into the future — should open a new window.
        assert!(state.record_hit(10_000, &cfg));
    }

    #[test]
    fn single_request_limit_enforced() {
        let cfg = RateLimiterConfig::new(1, 100);
        let mut state = RateLimiterState::new(0);

        assert!(state.record_hit(0, &cfg));
        assert!(!state.record_hit(0, &cfg));
        assert!(!state.record_hit(50, &cfg));
        // New window.
        assert!(state.record_hit(100, &cfg));
    }

    #[test]
    fn high_max_requests_saturating_count() {
        // Ensure the internal saturating_add does not wrap the count field.
        let cfg = RateLimiterConfig::new(u64::MAX, 60);
        assert!(cfg.is_enabled());
        let mut state = RateLimiterState::new(0);
        // Should allow all hits — count approaches u64::MAX without overflowing.
        for _ in 0..100 {
            assert!(state.record_hit(0, &cfg));
        }
    }

    #[test]
    fn independent_states_do_not_share_counters() {
        let cfg = RateLimiterConfig::new(2, 60);
        let mut state_alice = RateLimiterState::new(0);
        let mut state_bob = RateLimiterState::new(0);

        // Alice exhausts her quota.
        state_alice.record_hit(0, &cfg);
        state_alice.record_hit(0, &cfg);
        assert!(!state_alice.record_hit(0, &cfg));

        // Bob's state is independent — he still has 2 requests left.
        assert!(state_bob.record_hit(0, &cfg));
        assert!(state_bob.record_hit(0, &cfg));
    }

    #[test]
    fn new_state_starts_with_zero_count() {
        let state = RateLimiterState::new(500);
        assert_eq!(state.count, 0);
        assert_eq!(state.window_start, 500);
    }

    // ── Issue #265 Rate Limiting Tests ────────────────────────────────────────

    #[test]
    fn test_burst_requests_at_rate_limit_boundary() {
        let cfg = RateLimiterConfig::new(5, 60);
        let mut state = RateLimiterState::new(100);

        // Burst 5 requests in the exact same second
        for i in 1..=5 {
            assert!(
                state.record_hit(100, &cfg),
                "Request {} of 5 must succeed within quota",
                i
            );
            assert_eq!(state.count, i);
        }

        // Exactly at boundary: 6th request must be rejected
        assert!(
            !state.record_hit(100, &cfg),
            "Request 6 exceeding limit of 5 must be rejected"
        );
        assert_eq!(state.count, 5, "Count must remain saturated at max limit");
    }

    #[test]
    fn test_rate_limit_window_rollover() {
        let cfg = RateLimiterConfig::new(3, 60);
        let mut state = RateLimiterState::new(100);

        // Consume all 3 requests at t=100
        assert!(state.record_hit(100, &cfg));
        assert!(state.record_hit(100, &cfg));
        assert!(state.record_hit(100, &cfg));
        assert!(!state.record_hit(100, &cfg));

        // Just before window rollover at t=159: still rejected
        assert!(!state.record_hit(159, &cfg));

        // Exact rollover at t=160 (100 + 60): resets window and allows request
        assert!(
            state.record_hit(160, &cfg),
            "Request at exact rollover boundary must succeed"
        );
        assert_eq!(state.window_start, 160);
        assert_eq!(state.count, 1);

        // Next 2 requests in new window succeed
        assert!(state.record_hit(160, &cfg));
        assert!(state.record_hit(160, &cfg));
        // 4th request in new window rejected
        assert!(!state.record_hit(160, &cfg));
    }

    #[test]
    fn test_multiple_users_with_independent_rate_limits() {
        let cfg = RateLimiterConfig::new(2, 60);
        let mut user_a = RateLimiterState::new(100);
        let mut user_b = RateLimiterState::new(100);
        let mut user_c = RateLimiterState::new(100);

        // User A exhausts quota
        assert!(user_a.record_hit(100, &cfg));
        assert!(user_a.record_hit(100, &cfg));
        assert!(!user_a.record_hit(100, &cfg));

        // User B and User C are completely independent
        assert!(user_b.record_hit(100, &cfg));
        assert_eq!(user_b.count, 1);
        assert_eq!(user_a.count, 2);

        assert!(user_c.record_hit(100, &cfg));
        assert!(user_c.record_hit(100, &cfg));
        assert!(!user_c.record_hit(100, &cfg));

        // User B still has 1 request left
        assert!(user_b.record_hit(100, &cfg));
        assert!(!user_b.record_hit(100, &cfg));
    }

    #[test]
    fn test_rate_limit_config_changes_during_active_window() {
        let cfg_initial = RateLimiterConfig::new(5, 60);
        let mut state = RateLimiterState::new(10);

        // Send 3 requests under initial config
        assert!(state.record_hit(10, &cfg_initial));
        assert!(state.record_hit(10, &cfg_initial));
        assert!(state.record_hit(10, &cfg_initial));
        assert_eq!(state.count, 3);
        assert_eq!(state.window_start, 10);

        // Reconfigure to lower limit (3) during active window
        let cfg_lowered = RateLimiterConfig::new(3, 60);
        // Next request at t=20 is rejected because count (3) >= new limit (3)
        assert!(!state.record_hit(20, &cfg_lowered));
        // Crucial: window was NOT reset by the config change
        assert_eq!(state.window_start, 10);
        assert_eq!(state.count, 3);

        // Reconfigure to higher limit (10) during active window
        let cfg_raised = RateLimiterConfig::new(10, 60);
        // Next request at t=20 is allowed because 3 < 10
        assert!(state.record_hit(20, &cfg_raised));
        assert_eq!(state.count, 4);
        assert_eq!(state.window_start, 10, "Existing window must not be reset");
    }

    #[test]
    fn test_rate_limit_with_timestamp_manipulation() {
        let cfg = RateLimiterConfig::new(3, 60);
        let mut state = RateLimiterState::new(100);

        assert!(state.record_hit(100, &cfg)); // count = 1, window_start = 100

        // Attempt timestamp rollback (clock skew / manipulation attempt)
        // Timestamp is backwards (now = 80 < window_start = 100)
        // 80 >= 100 + 60 is false, so it does not falsely rollover the window.
        assert!(state.record_hit(80, &cfg));
        assert_eq!(state.count, 2);
        assert_eq!(state.window_start, 100);

        assert!(state.record_hit(90, &cfg)); // count = 3
                                             // Saturated at limit 3; subsequent attempt rejected
        assert!(!state.record_hit(95, &cfg));
        assert_eq!(state.count, 3);

        // Valid forward rollover occurs only when timestamp >= 160
        assert!(!state.record_hit(159, &cfg));
        assert!(state.record_hit(160, &cfg));
        assert_eq!(state.window_start, 160);
        assert_eq!(state.count, 1);
    }
}
