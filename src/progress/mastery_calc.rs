//! SRS mastery application — applies mastery updates to a Subject in place.

use crate::mastery;
use crate::models::{SrsIntervalDays, Subject};

/// Applies a mastery attempt update to a subject: updates score, attempts counters,
/// difficulty unlocks, and SRS interval based on success/failure.
///
/// This function encapsulates the mastery logic that was previously inline in
/// `record_attempt()`. It mutates the subject in place.
pub fn apply_mastery_attempt(subject: &mut Subject, success: bool, now: i64) {
    mastery::update_mastery(subject, success);

    let (next_review, new_interval) =
        mastery::compute_next_review(subject.srs_interval_days.get(), success, now);
    subject.next_review_at = Some(next_review);
    subject.srs_interval_days = SrsIntervalDays::clamped(new_interval);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_mastery_attempt_success() {
        let mut sub = Subject::new("test".to_string());
        let now = 1000000i64;
        apply_mastery_attempt(&mut sub, true, now);

        // After success, mastery should increase by 1.0
        assert_eq!(sub.mastery_score.get(), 1.0);
        assert_eq!(sub.attempts_total, 1);
        assert_eq!(sub.attempts_success, 1);
        assert!(sub.next_review_at.is_some());
    }

    #[test]
    fn test_apply_mastery_attempt_failure() {
        let mut sub = Subject::new("test".to_string());
        // Set initial score > 0 so failure makes a difference
        sub.mastery_score = crate::models::MasteryScore::clamped(2.0);
        sub.attempts_total = 1;
        sub.attempts_success = 1;

        let now = 1000000i64;
        apply_mastery_attempt(&mut sub, false, now);

        // After failure on a 2.0 score, should decrease by 0.5
        assert_eq!(sub.mastery_score.get(), 1.5);
        assert_eq!(sub.attempts_total, 2);
        assert_eq!(sub.attempts_success, 1);
    }

    #[test]
    fn test_apply_mastery_attempt_srs_interval_on_success() {
        let mut sub = Subject::new("test".to_string());
        sub.srs_interval_days = crate::models::SrsIntervalDays::clamped(3);

        let now = 1000000i64;
        apply_mastery_attempt(&mut sub, true, now);

        // SRS interval should have expanded on success
        assert!(sub.srs_interval_days.get() >= 3);
        assert!(sub.next_review_at.is_some());
    }

    #[test]
    fn test_apply_mastery_attempt_srs_interval_on_failure() {
        let mut sub = Subject::new("test".to_string());
        sub.mastery_score = crate::models::MasteryScore::clamped(2.0);
        sub.srs_interval_days = crate::models::SrsIntervalDays::clamped(10);

        let now = 1000000i64;
        apply_mastery_attempt(&mut sub, false, now);

        // SRS interval should reset to base on failure
        assert_eq!(
            sub.srs_interval_days.get(),
            crate::constants::SRS_BASE_INTERVAL_DAYS
        );
    }
}
