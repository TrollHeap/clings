use chrono::Utc;

use crate::models::Subject;

const DECAY_INTERVAL_DAYS: i64 = 14;
const DECAY_AMOUNT: f64 = 0.5;

pub const MAX_MASTERY: f64 = 5.0;
pub const MIN_MASTERY: f64 = 0.0;
pub const SUCCESS_DELTA: f64 = 1.0;
pub const FAILURE_DELTA: f64 = -0.5;
pub const UNLOCK_D2_THRESHOLD: f64 = 2.0;
pub const UNLOCK_D3_THRESHOLD: f64 = 4.0;

const SRS_MULTIPLIER: f64 = 2.5;
const SRS_MAX_INTERVAL_DAYS: i64 = 365;
const SECS_PER_DAY: i64 = 86_400;

pub fn mastery_delta(success: bool) -> f64 {
    if success {
        SUCCESS_DELTA
    } else {
        FAILURE_DELTA
    }
}

pub fn update_mastery(subject: &mut Subject, success: bool) {
    subject.mastery_score =
        (subject.mastery_score + mastery_delta(success)).clamp(MIN_MASTERY, MAX_MASTERY);
    subject.attempts_total += 1;
    if success {
        subject.attempts_success += 1;
    }
    subject.last_practiced_at = Some(Utc::now().timestamp());

    if subject.mastery_score >= UNLOCK_D3_THRESHOLD {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(3);
    } else if subject.mastery_score >= UNLOCK_D2_THRESHOLD {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(2);
    }
}

pub fn apply_decay(subject: &mut Subject) {
    let last_epoch = match subject.last_practiced_at {
        Some(ts) => ts,
        None => return,
    };

    let now = Utc::now().timestamp();
    let days_since = (now - last_epoch) / SECS_PER_DAY;

    if days_since >= DECAY_INTERVAL_DAYS {
        let intervals = days_since / DECAY_INTERVAL_DAYS;
        let decay = intervals as f64 * DECAY_AMOUNT;
        subject.mastery_score = (subject.mastery_score - decay).max(MIN_MASTERY);
    }
}

pub fn compute_next_review(current_interval_days: i64, success: bool, now: i64) -> (i64, i64) {
    let new_interval = if success {
        let expanded = ((current_interval_days as f64) * SRS_MULTIPLIER).round() as i64;
        expanded.clamp(1, SRS_MAX_INTERVAL_DAYS)
    } else {
        1
    };
    let next_review_at = now + new_interval * SECS_PER_DAY;
    (next_review_at, new_interval)
}

#[allow(dead_code)]
pub fn priority_sorted(mut subjects: Vec<Subject>) -> Vec<Subject> {
    subjects.sort_by(|a, b| {
        a.mastery_score
            .partial_cmp(&b.mastery_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let a_time = a.last_practiced_at.unwrap_or(0);
                let b_time = b.last_practiced_at.unwrap_or(0);
                a_time.cmp(&b_time)
            })
    });
    subjects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_subject(name: &str, score: f64) -> Subject {
        let mut s = Subject::new(name.to_string());
        s.mastery_score = score;
        s
    }

    #[test]
    fn test_score_increment() {
        let mut s = make_subject("test", 0.0);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score, 1.0);
    }

    #[test]
    fn test_score_cap_at_5() {
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score, 5.0);
    }

    #[test]
    fn test_score_decrement() {
        let mut s = make_subject("test", 2.0);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score, 1.5);
    }

    #[test]
    fn test_score_floor() {
        let mut s = make_subject("test", 0.0);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score, 0.0);
    }

    #[test]
    fn test_difficulty_unlock() {
        let mut s = make_subject("test", 1.5);
        update_mastery(&mut s, true);
        assert_eq!(s.difficulty_unlocked, 2);

        let mut s = make_subject("test", 3.5);
        update_mastery(&mut s, true);
        assert_eq!(s.difficulty_unlocked, 3);
    }

    #[test]
    fn test_srs_success() {
        let (next, interval) = compute_next_review(1, true, 1_000_000);
        assert_eq!(interval, 3);
        assert_eq!(next, 1_000_000 + 3 * 86400);
    }

    #[test]
    fn test_srs_failure_resets() {
        let (_, interval) = compute_next_review(30, false, 1_000_000);
        assert_eq!(interval, 1);
    }

    #[test]
    fn test_priority_sorted() {
        let subjects = vec![
            make_subject("high", 4.0),
            make_subject("low", 1.0),
            make_subject("mid", 2.5),
        ];
        let sorted = priority_sorted(subjects);
        assert_eq!(sorted[0].name, "low");
        assert_eq!(sorted[1].name, "mid");
        assert_eq!(sorted[2].name, "high");
    }
}
