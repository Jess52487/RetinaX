#![cfg(test)]

use crate::{
    rate_limit::{
        check_rate_limit, get_rate_limit_count, get_rate_limit_window, has_rate_limit_bypass,
        set_rate_limit_bypass, set_rate_limit_config, RateLimitConfig,
    },
    VisionRecordsContract,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

fn setup_test() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VisionRecordsContract, ());
    (env, contract_id)
}

#[test]
fn test_burst_requests_at_rate_limit_exact_boundary() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let user = Address::generate(&env);
        let op = String::from_str(&env, "add_record");
        let max_requests = 3u32;
        let window_seconds = 60u64;

        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests,
                window_seconds,
                operation: op.clone(),
            },
        );

        // Requests 1, 2, 3 must all be allowed
        let res1 = check_rate_limit(&env, &user, &op);
        assert!(res1.0, "1st request must be allowed");
        assert_eq!(res1.1, 1);

        let res2 = check_rate_limit(&env, &user, &op);
        assert!(res2.0, "2nd request must be allowed");
        assert_eq!(res2.1, 2);

        let res3 = check_rate_limit(&env, &user, &op);
        assert!(res3.0, "3rd request (exact boundary) must be allowed");
        assert_eq!(res3.1, 3);

        // 4th request must be rejected at boundary
        let res4 = check_rate_limit(&env, &user, &op);
        assert!(
            !res4.0,
            "4th request exceeding max_requests must be rejected"
        );
        assert_eq!(res4.1, 3, "Count must remain saturated at limit");
    });
}

#[test]
fn test_rate_limit_bypass_and_revocation() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let provider = Address::generate(&env);
        let op = String::from_str(&env, "emergency_record");

        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 2,
                window_seconds: 60,
                operation: op.clone(),
            },
        );

        // Grant bypass to verified emergency provider
        set_rate_limit_bypass(&env, &provider, true);
        assert!(has_rate_limit_bypass(&env, &provider));

        // Provider can burst far beyond rate limit without throttling
        for _ in 0..20 {
            let res = check_rate_limit(&env, &provider, &op);
            assert!(res.0, "Bypassed provider must not be throttled");
        }

        // Revoke bypass
        set_rate_limit_bypass(&env, &provider, false);
        assert!(!has_rate_limit_bypass(&env, &provider));

        // Provider is now subject to rate limiting
        let res1 = check_rate_limit(&env, &provider, &op);
        assert!(res1.0);
        let res2 = check_rate_limit(&env, &provider, &op);
        assert!(res2.0);
        let res3 = check_rate_limit(&env, &provider, &op);
        assert!(
            !res3.0,
            "Provider must be rate limited after bypass revocation"
        );
    });
}

#[test]
fn test_rate_limit_window_rollover() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let user = Address::generate(&env);
        let op = String::from_str(&env, "get_record");

        env.ledger().set_timestamp(1000);

        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 2,
                window_seconds: 60,
                operation: op.clone(),
            },
        );

        assert!(check_rate_limit(&env, &user, &op).0);
        assert!(check_rate_limit(&env, &user, &op).0);
        assert!(!check_rate_limit(&env, &user, &op).0);

        // 1 second before rollover (1059)
        env.ledger().set_timestamp(1059);
        assert!(!check_rate_limit(&env, &user, &op).0);

        // Exact rollover at 1060 (1000 + 60)
        env.ledger().set_timestamp(1060);
        let rollover = check_rate_limit(&env, &user, &op);
        assert!(rollover.0, "Request at window rollover must succeed");
        assert_eq!(rollover.1, 1);
        assert_eq!(rollover.3, 1060 + 60);
    });
}

#[test]
fn test_multiple_users_independent_rate_limits() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let user_a = Address::generate(&env);
        let user_b = Address::generate(&env);
        let user_c = Address::generate(&env);
        let op = String::from_str(&env, "query_records");

        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 2,
                window_seconds: 120,
                operation: op.clone(),
            },
        );

        // User A exhausts quota
        assert!(check_rate_limit(&env, &user_a, &op).0);
        assert!(check_rate_limit(&env, &user_a, &op).0);
        assert!(!check_rate_limit(&env, &user_a, &op).0);

        // User B and User C are completely unaffected
        assert!(check_rate_limit(&env, &user_b, &op).0);
        assert_eq!(get_rate_limit_count(&env, &user_b, &op), 1);
        assert_eq!(get_rate_limit_count(&env, &user_a, &op), 2);

        assert!(check_rate_limit(&env, &user_c, &op).0);
        assert!(check_rate_limit(&env, &user_c, &op).0);
        assert!(!check_rate_limit(&env, &user_c, &op).0);

        // User B has one more request available
        assert!(check_rate_limit(&env, &user_b, &op).0);
        assert!(!check_rate_limit(&env, &user_b, &op).0);
    });
}

#[test]
fn test_config_changes_do_not_reset_existing_windows() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let user = Address::generate(&env);
        let op = String::from_str(&env, "prescribe");
        env.ledger().set_timestamp(2000);

        // Initial config: 5 requests per 300 seconds
        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 5,
                window_seconds: 300,
                operation: op.clone(),
            },
        );

        // User makes 3 requests
        assert!(check_rate_limit(&env, &user, &op).0);
        assert!(check_rate_limit(&env, &user, &op).0);
        assert!(check_rate_limit(&env, &user, &op).0);
        assert_eq!(get_rate_limit_count(&env, &user, &op), 3);
        let window_start = get_rate_limit_window(&env, &user, &op);
        assert_eq!(window_start, 2000);

        // Dynamically lower limit to 3 during active window
        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 3,
                window_seconds: 300,
                operation: op.clone(),
            },
        );

        // Advance time slightly within same window
        env.ledger().set_timestamp(2050);

        // 4th request must be rejected because count (3) >= new max (3)
        let res_lowered = check_rate_limit(&env, &user, &op);
        assert!(!res_lowered.0);

        // Verify window was not reset
        assert_eq!(get_rate_limit_window(&env, &user, &op), 2000);
        assert_eq!(get_rate_limit_count(&env, &user, &op), 3);

        // Dynamically raise limit to 10 during active window
        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 10,
                window_seconds: 300,
                operation: op.clone(),
            },
        );

        // User can now proceed and window_start is STILL 2000
        let res_raised = check_rate_limit(&env, &user, &op);
        assert!(res_raised.0);
        assert_eq!(res_raised.1, 4);
        assert_eq!(get_rate_limit_window(&env, &user, &op), 2000);
    });
}

#[test]
fn test_rate_limit_timestamp_manipulation() {
    let (env, contract_id) = setup_test();
    env.as_contract(&contract_id, || {
        let user = Address::generate(&env);
        let op = String::from_str(&env, "exam_upload");

        env.ledger().set_timestamp(5000);

        set_rate_limit_config(
            &env,
            &RateLimitConfig {
                max_requests: 2,
                window_seconds: 100,
                operation: op.clone(),
            },
        );

        assert!(check_rate_limit(&env, &user, &op).0); // count = 1, window_start = 5000

        // Attacker attempts non-monotonic / backwards timestamp (4900)
        env.ledger().set_timestamp(4900);
        // Window end is 5000 + 100 = 5100; 4900 < 5100 so it does not trigger window reset
        assert!(check_rate_limit(&env, &user, &op).0); // count = 2
        assert_eq!(get_rate_limit_count(&env, &user, &op), 2);
        assert_eq!(get_rate_limit_window(&env, &user, &op), 5000);

        // Limit reached: next backwards request rejected
        assert!(!check_rate_limit(&env, &user, &op).0);

        // Advance to 5100: valid rollover happens
        env.ledger().set_timestamp(5100);
        let rollover = check_rate_limit(&env, &user, &op);
        assert!(rollover.0);
        assert_eq!(rollover.1, 1);
        assert_eq!(get_rate_limit_window(&env, &user, &op), 5100);
    });
}
