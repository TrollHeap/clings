use chrono::Utc;

use crate::constants::{
    DIFFICULTY_2_UNLOCK, DIFFICULTY_3_UNLOCK, DIFFICULTY_4_UNLOCK, DIFFICULTY_5_UNLOCK,
    MASTERY_FAILURE_DELTA, MASTERY_SUCCESS_DELTA,
};
use crate::models::{MasteryScore, Subject};

const DECAY_AMOUNT: f64 = 0.5;
const SECS_PER_DAY: i64 = 86_400;

/// Retourne le delta de score correspondant à un succès ou un échec.
pub fn mastery_delta(success: bool) -> f64 {
    if success {
        MASTERY_SUCCESS_DELTA
    } else {
        -MASTERY_FAILURE_DELTA
    }
}

/// Met à jour le score de maîtrise, les compteurs de tentatives et le niveau déverrouillé.
/// Le score est borné entre `MIN_MASTERY` et `MAX_MASTERY`.
pub fn update_mastery(subject: &mut Subject, success: bool) {
    subject.mastery_score =
        MasteryScore::clamped(subject.mastery_score.get() + mastery_delta(success));
    subject.attempts_total += 1;
    if success {
        subject.attempts_success += 1;
    }
    subject.last_practiced_at = Some(Utc::now().timestamp());

    if subject.mastery_score.get() >= DIFFICULTY_5_UNLOCK {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(5);
    } else if subject.mastery_score.get() >= DIFFICULTY_4_UNLOCK {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(4);
    } else if subject.mastery_score.get() >= DIFFICULTY_3_UNLOCK {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(3);
    } else if subject.mastery_score.get() >= DIFFICULTY_2_UNLOCK {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(2);
    }
}

/// Applique la décroissance temporelle au score : −0.5 par tranche de 14 jours d'inactivité.
/// Sans effet si le sujet n'a jamais été pratiqué.
pub fn apply_decay(subject: &mut Subject) {
    let last_epoch = match subject.last_practiced_at {
        Some(ts) => ts,
        None => return,
    };

    let now = Utc::now().timestamp();
    let days_since = ((now - last_epoch) / SECS_PER_DAY).max(0);
    let decay_days = crate::config::get().srs.decay_days;

    if days_since >= decay_days {
        let intervals = days_since / decay_days;
        let decay = intervals as f64 * DECAY_AMOUNT;
        subject.mastery_score = MasteryScore::clamped(subject.mastery_score.get() - decay);
        // Advance last_practiced_at to "consume" the decayed intervals,
        // preventing the same intervals from being re-applied on the next call.
        subject.last_practiced_at = Some(last_epoch + intervals * decay_days * SECS_PER_DAY);
    }
}

/// Estime le nombre de jours avant le prochain review à partir du score de maîtrise.
/// Utilisé pour l'affichage post-validation en mode review.
/// Formule : round(mastery * interval_multiplier), borné entre base et max.
pub(crate) fn next_interval_days(mastery: f32) -> u32 {
    let cfg = &crate::config::get().srs;
    let raw = (mastery as f64 * cfg.interval_multiplier).round() as i64;
    raw.clamp(cfg.base_interval_days, cfg.max_interval_days) as u32
}

/// Calcule le prochain horodatage de révision et le nouvel intervalle SRS.
/// En cas de succès l'intervalle est multiplié par `interval_multiplier` (max `max_interval_days`) ;
/// en cas d'échec il revient à `base_interval_days`.
/// Retourne `(next_review_at_unix, new_interval_days)`.
pub fn compute_next_review(current_interval_days: i64, success: bool, now: i64) -> (i64, i64) {
    let cfg = &crate::config::get().srs;
    let new_interval = if success {
        let expanded = ((current_interval_days as f64) * cfg.interval_multiplier).round() as i64;
        expanded.clamp(cfg.base_interval_days, cfg.max_interval_days)
    } else {
        cfg.base_interval_days
    };
    let next_review_at = now + new_interval * SECS_PER_DAY;
    (next_review_at, new_interval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{SRS_BASE_INTERVAL_DAYS, SRS_MAX_INTERVAL_DAYS};

    const SUCCESS_DELTA: f64 = crate::constants::MASTERY_SUCCESS_DELTA;
    const FAILURE_DELTA: f64 = -(crate::constants::MASTERY_FAILURE_DELTA);

    fn make_subject(name: &str, score: f64) -> Subject {
        let mut s = Subject::new(name.to_owned());
        s.mastery_score = MasteryScore::clamped(score);
        s
    }

    #[test]
    fn test_score_increment() {
        let mut s = make_subject("test", 0.0);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score.get(), 1.0);
    }

    #[test]
    fn test_score_cap_at_5() {
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score.get(), 5.0);
    }

    #[test]
    fn test_score_decrement() {
        let mut s = make_subject("test", 2.0);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score.get(), 1.5);
    }

    #[test]
    fn test_score_floor() {
        let mut s = make_subject("test", 0.0);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score.get(), 0.0);
    }

    #[test]
    fn test_difficulty_unlock() {
        // 1.5 + 1.0 = 2.5 → unlocks D2
        let mut s = make_subject("test", 1.5);
        update_mastery(&mut s, true);
        assert_eq!(s.difficulty_unlocked, 2);

        // 3.0 + 1.0 = 4.0 → unlocks D3 (needs 4.5 for D4)
        let mut s = make_subject("test", 3.0);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score.get(), 4.0);
        assert_eq!(s.difficulty_unlocked, 3);
    }

    #[test]
    fn test_d4_unlock() {
        // 3.5 + 1.0 = 4.5 → unlocks D4
        let mut s = make_subject("test", 3.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score.get(), 4.5);
        assert_eq!(s.difficulty_unlocked, 4);

        // 4.5 - 0.5 = 4.0 → stays at D3 (needs 4.5 for D4)
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score.get(), 4.0);
        assert_eq!(s.difficulty_unlocked, 3);
    }

    #[test]
    fn test_d5_threshold() {
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score.get(), 5.0);
        assert_eq!(s.difficulty_unlocked, 5);
    }

    #[test]
    fn test_srs_success() {
        // SRS_INTERVAL_MULTIPLIER = 2.5 : round(1 * 2.5) = 3
        let (next, interval) = compute_next_review(1, true, 1_000_000);
        assert_eq!(interval, 3);
        assert_eq!(next, 1_000_000 + 3 * 86400);
    }

    #[test]
    fn test_srs_failure_resets() {
        let (_, interval) = compute_next_review(30, false, 1_000_000);
        assert_eq!(interval, SRS_BASE_INTERVAL_DAYS);
    }

    #[test]
    fn test_srs_interval_capped_at_max() {
        // Succès répétés : l'intervalle doit être plafonné à SRS_MAX_INTERVAL_DAYS (60)
        let (_, interval) = compute_next_review(50, true, 1_000_000);
        assert_eq!(interval, SRS_MAX_INTERVAL_DAYS);
    }

    #[test]
    fn test_apply_decay_after_14_days() {
        let mut s = make_subject("test", 2.0);
        // Inactif depuis 15 jours → 1 intervalle de 14j → décroissance de 0.5
        s.last_practiced_at = Some(Utc::now().timestamp() - 15 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score.get(), 1.5);
    }

    #[test]
    fn test_apply_decay_under_14_days() {
        let mut s = make_subject("test", 2.0);
        // Inactif depuis 10 jours → pas de décroissance
        s.last_practiced_at = Some(Utc::now().timestamp() - 10 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score.get(), 2.0);
    }

    #[test]
    fn test_apply_decay_floor_at_zero() {
        let mut s = make_subject("test", 0.0);
        // Score déjà à 0 → reste à 0 même avec forte inactivité
        s.last_practiced_at = Some(Utc::now().timestamp() - 30 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score.get(), 0.0);
    }

    #[test]
    fn test_apply_decay_never_practiced() {
        let mut s = make_subject("test", 3.0);
        // last_practiced_at = None → retour immédiat, score inchangé
        apply_decay(&mut s);
        assert_eq!(s.mastery_score.get(), 3.0);
    }

    #[test]
    fn test_apply_decay_multiple_intervals() {
        let mut s = make_subject("test", 3.0);
        // Inactif depuis 29 jours → 2 intervalles de 14j → décroissance de 1.0
        s.last_practiced_at = Some(Utc::now().timestamp() - 29 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score.get(), 2.0);
    }

    #[test]
    fn test_apply_decay_idempotent() {
        // Applying decay twice with the same starting timestamp must give the same result
        let ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
        let mut s1 = make_subject("test", 2.0);
        s1.last_practiced_at = Some(ts);
        apply_decay(&mut s1);
        let score_after_first = s1.mastery_score.get();
        let lpa_after_first = s1.last_practiced_at;

        // Apply decay again (simulating a second startup)
        apply_decay(&mut s1);
        // Score must NOT change again
        assert_eq!(
            s1.mastery_score.get(),
            score_after_first,
            "decay must be idempotent"
        );
        assert_eq!(s1.last_practiced_at, lpa_after_first);
    }

    #[test]
    fn test_mastery_delta_success() {
        assert_eq!(mastery_delta(true), SUCCESS_DELTA);
    }

    #[test]
    fn test_mastery_delta_failure() {
        assert_eq!(mastery_delta(false), FAILURE_DELTA);
    }

    // ── next_interval_days ──────────────────────────────────────────────

    #[test]
    fn next_interval_days_min_clamp() {
        // mastery=0.0 → raw=round(0.0 * 2.5)=0 → clamped to base_interval_days (1)
        let result = next_interval_days(0.0);
        assert_eq!(result, SRS_BASE_INTERVAL_DAYS as u32);
    }

    #[test]
    fn next_interval_days_max_clamp() {
        // mastery=30.0 → raw=round(30.0 * 2.5)=75 → clamped to max_interval_days (60)
        let result = next_interval_days(30.0);
        assert_eq!(result, SRS_MAX_INTERVAL_DAYS as u32);
    }

    #[test]
    fn next_interval_days_mid() {
        // mastery=2.0 → raw=round(2.0 * 2.5)=5 → within [1, 60], returns 5
        let result = next_interval_days(2.0);
        assert_eq!(result, 5);
    }
}
