//! Property-based tests for the SRS mastery algorithm.

use proptest::prelude::*;

use clings::mastery::{apply_decay, compute_next_review, mastery_delta, update_mastery};
use clings::models::{MasteryScore, Subject};

/// Build a Subject with a given score and last_practiced timestamp.
fn make_subject(score: f64, last_practiced_at: Option<i64>) -> Subject {
    let mut s = Subject::new("test".to_owned());
    s.mastery_score = MasteryScore::clamped(score);
    s.last_practiced_at = last_practiced_at;
    s
}

proptest! {
    /// Score never exceeds [0.0, 5.0] after any sequence of successes and failures.
    #[test]
    fn score_always_bounded(
        initial in 0.0..=5.0f64,
        outcomes in prop::collection::vec(prop::bool::ANY, 1..50),
    ) {
        let mut s = make_subject(initial, None);
        for success in outcomes {
            update_mastery(&mut s, success);
            let score = s.mastery_score.get();
            prop_assert!(score >= 0.0, "score {} < 0.0", score);
            prop_assert!(score <= 5.0, "score {} > 5.0", score);
        }
    }

    /// Difficulty unlocked is monotonically non-decreasing with rising scores.
    #[test]
    fn difficulty_unlocked_monotonic_on_success(
        initial in 0.0..=4.0f64,
        n_successes in 1..20usize,
    ) {
        let mut s = make_subject(initial, None);
        let mut prev_unlocked = s.difficulty_unlocked;
        for _ in 0..n_successes {
            update_mastery(&mut s, true);
            prop_assert!(
                s.difficulty_unlocked >= prev_unlocked,
                "difficulty_unlocked decreased: {} -> {}",
                prev_unlocked,
                s.difficulty_unlocked,
            );
            prev_unlocked = s.difficulty_unlocked;
        }
    }

    /// Decay never produces a negative score.
    #[test]
    fn decay_never_negative(
        initial_score in 0.0..=5.0f64,
        days_inactive in 0i64..365,
    ) {
        let now = chrono::Utc::now().timestamp();
        let last = now - days_inactive * 86_400;
        let mut s = make_subject(initial_score, Some(last));
        apply_decay(&mut s);
        prop_assert!(
            s.mastery_score.get() >= 0.0,
            "decay produced negative: {}",
            s.mastery_score.get(),
        );
    }

    /// Decay is idempotent: applying twice gives the same result.
    #[test]
    fn decay_idempotent(
        initial_score in 0.0..=5.0f64,
        days_inactive in 0i64..365,
    ) {
        let now = chrono::Utc::now().timestamp();
        let last = now - days_inactive * 86_400;
        let mut s = make_subject(initial_score, Some(last));
        apply_decay(&mut s);
        let after_first = s.mastery_score.get();
        let lpa_first = s.last_practiced_at;
        apply_decay(&mut s);
        prop_assert_eq!(s.mastery_score.get(), after_first, "decay not idempotent");
        prop_assert_eq!(s.last_practiced_at, lpa_first, "last_practiced_at changed");
    }

    /// mastery_delta returns exactly +1.0 on success and -0.5 on failure.
    #[test]
    fn delta_values(success in prop::bool::ANY) {
        let d = mastery_delta(success);
        if success {
            prop_assert_eq!(d, 1.0);
        } else {
            prop_assert_eq!(d, -0.5);
        }
    }

    /// SRS interval on success is always >= base and <= max.
    #[test]
    fn srs_interval_bounded(
        current_interval in 1i64..200,
        success in prop::bool::ANY,
        now in 1_000_000i64..2_000_000_000,
    ) {
        let (next_review, new_interval) = compute_next_review(current_interval, success, now);
        // Base = 1, Max = 60 (from constants)
        prop_assert!(new_interval >= 1, "interval {} < base", new_interval);
        prop_assert!(new_interval <= 60, "interval {} > max", new_interval);
        prop_assert!(next_review > now, "next_review {} <= now {}", next_review, now);
    }

    /// SRS interval grows on success (or stays at max), resets on failure.
    #[test]
    fn srs_interval_direction(
        current_interval in 1i64..50,
        now in 1_000_000i64..2_000_000_000,
    ) {
        let (_, success_interval) = compute_next_review(current_interval, true, now);
        let (_, failure_interval) = compute_next_review(current_interval, false, now);

        // Success: interval should be >= current (or at max)
        prop_assert!(
            success_interval >= current_interval || success_interval == 60,
            "success interval {} < current {} (not at max)",
            success_interval,
            current_interval,
        );

        // Failure: interval resets to base (1)
        prop_assert_eq!(failure_interval, 1, "failure should reset to base");
    }

    /// MasteryScore::clamped always produces a value in [0.0, 5.0].
    #[test]
    fn mastery_score_clamped(v in -100.0..=100.0f64) {
        let s = MasteryScore::clamped(v);
        prop_assert!(s.get() >= 0.0);
        prop_assert!(s.get() <= 5.0);
    }
}
