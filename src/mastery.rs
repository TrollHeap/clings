use chrono::Utc;

use crate::models::Subject;

const DECAY_INTERVAL_DAYS: i64 = 14;
const DECAY_AMOUNT: f64 = 0.5;

/// Score de maîtrise maximal atteignable
pub const MAX_MASTERY: f64 = 5.0;
/// Score de maîtrise minimal (plancher)
pub const MIN_MASTERY: f64 = 0.0;
/// Incrément appliqué au score lors d'une réussite
pub const SUCCESS_DELTA: f64 = 1.0;
/// Décrément appliqué au score lors d'un échec
pub const FAILURE_DELTA: f64 = -0.5;
/// Score requis pour déverrouiller la difficulté 2
pub const UNLOCK_D2_THRESHOLD: f64 = 2.0;
/// Score requis pour déverrouiller la difficulté 3
pub const UNLOCK_D3_THRESHOLD: f64 = 4.0;
/// Score requis pour déverrouiller la difficulté 4
pub const UNLOCK_D4_THRESHOLD: f64 = 4.5;
/// Score requis pour déverrouiller la difficulté 5
pub const UNLOCK_D5_THRESHOLD: f64 = 5.0;

const SRS_MULTIPLIER: f64 = 1.8;
const SRS_MAX_INTERVAL_DAYS: i64 = 14;
const SECS_PER_DAY: i64 = 86_400;

/// Retourne le delta de score correspondant à un succès ou un échec.
pub fn mastery_delta(success: bool) -> f64 {
    if success {
        SUCCESS_DELTA
    } else {
        FAILURE_DELTA
    }
}

/// Met à jour le score de maîtrise, les compteurs de tentatives et le niveau déverrouillé.
/// Le score est borné entre `MIN_MASTERY` et `MAX_MASTERY`.
pub fn update_mastery(subject: &mut Subject, success: bool) {
    subject.mastery_score =
        (subject.mastery_score + mastery_delta(success)).clamp(MIN_MASTERY, MAX_MASTERY);
    subject.attempts_total += 1;
    if success {
        subject.attempts_success += 1;
    }
    subject.last_practiced_at = Some(Utc::now().timestamp());

    if subject.mastery_score >= UNLOCK_D5_THRESHOLD {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(5);
    } else if subject.mastery_score >= UNLOCK_D4_THRESHOLD {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(4);
    } else if subject.mastery_score >= UNLOCK_D3_THRESHOLD {
        subject.difficulty_unlocked = subject.difficulty_unlocked.max(3);
    } else if subject.mastery_score >= UNLOCK_D2_THRESHOLD {
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
    let days_since = (now - last_epoch) / SECS_PER_DAY;

    if days_since >= DECAY_INTERVAL_DAYS {
        let intervals = days_since / DECAY_INTERVAL_DAYS;
        let decay = intervals as f64 * DECAY_AMOUNT;
        subject.mastery_score = (subject.mastery_score - decay).max(MIN_MASTERY);
    }
}

/// Calcule le prochain horodatage de révision et le nouvel intervalle SRS.
/// En cas de succès l'intervalle est multiplié par 2.5 (max 365 jours) ; en cas d'échec il revient à 1 jour.
/// Retourne `(next_review_at_unix, new_interval_days)`.
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
        // 1.5 + 1.0 = 2.5 → unlocks D2
        let mut s = make_subject("test", 1.5);
        update_mastery(&mut s, true);
        assert_eq!(s.difficulty_unlocked, 2);

        // 3.0 + 1.0 = 4.0 → unlocks D3 (needs 4.5 for D4)
        let mut s = make_subject("test", 3.0);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score, 4.0);
        assert_eq!(s.difficulty_unlocked, 3);
    }

    #[test]
    fn test_d4_unlock() {
        // 3.5 + 1.0 = 4.5 → unlocks D4
        let mut s = make_subject("test", 3.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score, 4.5);
        assert_eq!(s.difficulty_unlocked, 4);

        // 4.5 - 0.5 = 4.0 → stays at D3 (needs 4.5 for D4)
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, false);
        assert_eq!(s.mastery_score, 4.0);
        assert_eq!(s.difficulty_unlocked, 3);
    }

    #[test]
    fn test_d5_threshold() {
        let mut s = make_subject("test", 4.5);
        update_mastery(&mut s, true);
        assert_eq!(s.mastery_score, 5.0);
        assert_eq!(s.difficulty_unlocked, 5);
    }

    #[test]
    fn test_srs_success() {
        // SRS_MULTIPLIER = 1.8 : round(1 * 1.8) = 2
        let (next, interval) = compute_next_review(1, true, 1_000_000);
        assert_eq!(interval, 2);
        assert_eq!(next, 1_000_000 + 2 * 86400);
    }

    #[test]
    fn test_srs_failure_resets() {
        let (_, interval) = compute_next_review(30, false, 1_000_000);
        assert_eq!(interval, 1);
    }

    #[test]
    fn test_srs_interval_capped_at_max() {
        // Succès répétés : l'intervalle doit être plafonné à SRS_MAX_INTERVAL_DAYS (30)
        let (_, interval) = compute_next_review(20, true, 1_000_000);
        assert_eq!(interval, SRS_MAX_INTERVAL_DAYS);
    }

    #[test]
    fn test_apply_decay_after_14_days() {
        let mut s = make_subject("test", 2.0);
        // Inactif depuis 15 jours → 1 intervalle de 14j → décroissance de 0.5
        s.last_practiced_at = Some(Utc::now().timestamp() - 15 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score, 1.5);
    }

    #[test]
    fn test_apply_decay_under_14_days() {
        let mut s = make_subject("test", 2.0);
        // Inactif depuis 10 jours → pas de décroissance
        s.last_practiced_at = Some(Utc::now().timestamp() - 10 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score, 2.0);
    }

    #[test]
    fn test_apply_decay_floor_at_zero() {
        let mut s = make_subject("test", 0.0);
        // Score déjà à 0 → reste à 0 même avec forte inactivité
        s.last_practiced_at = Some(Utc::now().timestamp() - 30 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score, 0.0);
    }

    #[test]
    fn test_apply_decay_never_practiced() {
        let mut s = make_subject("test", 3.0);
        // last_practiced_at = None → retour immédiat, score inchangé
        apply_decay(&mut s);
        assert_eq!(s.mastery_score, 3.0);
    }

    #[test]
    fn test_apply_decay_multiple_intervals() {
        let mut s = make_subject("test", 3.0);
        // Inactif depuis 29 jours → 2 intervalles de 14j → décroissance de 1.0
        s.last_practiced_at = Some(Utc::now().timestamp() - 29 * SECS_PER_DAY);
        apply_decay(&mut s);
        assert_eq!(s.mastery_score, 2.0);
    }

    #[test]
    fn test_mastery_delta_success() {
        assert_eq!(mastery_delta(true), SUCCESS_DELTA);
    }

    #[test]
    fn test_mastery_delta_failure() {
        assert_eq!(mastery_delta(false), FAILURE_DELTA);
    }
}
