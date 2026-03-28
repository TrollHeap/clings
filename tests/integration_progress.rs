//! Integration tests for progress module — database, persistence, and SRS logic.

use clings::progress::*;

// ── Test database setup ────────────────────────────────────────────────────────

/// Open an in-memory database for testing (schema initialized).
fn open_test_db() -> clings::error::Result<rusqlite::Connection> {
    clings::progress::open_db_for_test()
}

// ── Subjects CRUD ──────────────────────────────────────────────────────────────

#[test]
fn test_ensure_subject_for_test_creates_row() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    let sub = get_subject(&conn, "pointers")?;
    assert!(sub.is_some());
    let sub = sub.expect("subject should exist after insert");
    assert_eq!(sub.name, "pointers");
    assert_eq!(sub.mastery_score.get(), 0.0);
    assert_eq!(sub.difficulty_unlocked, 1);
    Ok(())
}

#[test]
fn test_ensure_subject_for_test_idempotent() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    ensure_subject_for_test(&conn, "pointers")?;
    let subjects = get_all_subjects(&conn)?;
    assert_eq!(subjects.len(), 1);
    Ok(())
}

#[test]
fn test_record_attempt_success() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "structs")?;
    let sub = record_attempt(&conn, "structs", "struct-point-01", true)?;
    assert_eq!(sub.mastery_score.get(), 1.0);
    assert_eq!(sub.attempts_total, 1);
    assert_eq!(sub.attempts_success, 1);
    Ok(())
}

#[test]
fn test_record_attempt_failure() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "structs")?;

    // First succeed to have score > 0
    record_attempt(&conn, "structs", "struct-point-01", true)?;
    let sub = record_attempt(&conn, "structs", "struct-point-01", false)?;
    assert_eq!(sub.mastery_score.get(), 0.5);
    assert_eq!(sub.attempts_total, 2);
    assert_eq!(sub.attempts_success, 1);
    Ok(())
}

#[test]
fn test_reset_progress() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
    reset_progress(&conn)?;
    let subjects = get_all_subjects(&conn)?;
    assert!(subjects.is_empty());
    Ok(())
}

#[test]
fn test_get_subject_missing() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    let sub = get_subject(&conn, "nonexistent")?;
    assert!(sub.is_none());
    Ok(())
}

#[test]
fn test_reset_subject_isolated() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    ensure_subject_for_test(&conn, "structs")?;
    record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
    record_attempt(&conn, "structs", "struct-point-01", true)?;

    reset_subject(&conn, "pointers")?;

    // pointers supprimé
    assert!(get_subject(&conn, "pointers")?.is_none());
    // structs intact
    let s = get_subject(&conn, "structs")?.expect("structs should exist after insert");
    assert_eq!(s.mastery_score.get(), 1.0);
    // log pointers supprimé, log structs intact
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM practice_log WHERE subject = 'structs'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1);
    let count_ptr: i64 = conn.query_row(
        "SELECT COUNT(*) FROM practice_log WHERE subject = 'pointers'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(count_ptr, 0);
    Ok(())
}

// ── Checkpoints (KV store) ────────────────────────────────────────────────────

#[test]
fn test_kv_checkpoint_roundtrip() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_piscine_checkpoint(&conn, 42)?;
    let loaded = load_piscine_checkpoint(&conn)?;
    assert_eq!(loaded, Some(42));
    Ok(())
}

#[test]
fn test_kv_checkpoint_missing_returns_none() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    let loaded = load_piscine_checkpoint(&conn)?;
    assert_eq!(loaded, None);
    Ok(())
}

#[test]
fn test_kv_checkpoint_clear() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_piscine_checkpoint(&conn, 7)?;
    clear_piscine_checkpoint(&conn)?;
    let loaded = load_piscine_checkpoint(&conn)?;
    assert_eq!(loaded, None);
    Ok(())
}

#[test]
fn test_kv_checkpoint_overwrite() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_piscine_checkpoint(&conn, 3)?;
    save_piscine_checkpoint(&conn, 17)?;
    let loaded = load_piscine_checkpoint(&conn)?;
    assert_eq!(loaded, Some(17));
    Ok(())
}

#[test]
fn test_exam_checkpoint_roundtrip() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_exam_checkpoint(&conn, "nsy103-2024", 5)?;
    let loaded = load_exam_checkpoint(&conn, "nsy103-2024")?;
    assert_eq!(loaded, Some(5));
    Ok(())
}

#[test]
fn test_exam_checkpoint_session_isolation() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_exam_checkpoint(&conn, "nsy103-2024", 3)?;
    let other = load_exam_checkpoint(&conn, "utc502-2023")?;
    assert_eq!(other, None);
    Ok(())
}

#[test]
fn test_exam_checkpoint_session_id_with_colon() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_exam_checkpoint(&conn, "utc502:2024", 7)?;
    let loaded = load_exam_checkpoint(&conn, "utc502:2024")?;
    assert_eq!(loaded, Some(7));
    // A session_id that only matches the prefix must not match
    let wrong = load_exam_checkpoint(&conn, "utc502")?;
    assert_eq!(wrong, None);
    Ok(())
}

#[test]
fn test_exam_checkpoint_clear() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_exam_checkpoint(&conn, "nsy103-2024", 2)?;
    clear_exam_checkpoint(&conn)?;
    let loaded = load_exam_checkpoint(&conn, "nsy103-2024")?;
    assert_eq!(loaded, None);
    Ok(())
}

// ── SRS: due subjects ──────────────────────────────────────────────────────────

#[test]
fn test_due_subjects_past_review() -> clings::error::Result<()> {
    use chrono::Utc;
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    // next_review_at dans le passé → sujet doit apparaître
    let past = Utc::now().timestamp() - 3_600;
    conn.execute(
        "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
        rusqlite::params![past],
    )?;
    let due = get_due_subjects(&conn)?;
    assert!(due.contains(&"pointers".to_string()));
    Ok(())
}

#[test]
fn test_due_subjects_future_review() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    // next_review_at dans le futur → sujet absent
    let future = Utc::now().timestamp() + SECS_PER_DAY;
    conn.execute(
        "UPDATE subjects SET next_review_at = ?1 WHERE name = 'pointers'",
        rusqlite::params![future],
    )?;
    let due = get_due_subjects(&conn)?;
    assert!(!due.contains(&"pointers".to_string()));
    Ok(())
}

#[test]
fn test_due_subjects_null_review() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    // next_review_at NULL par défaut → sujet absent
    let due = get_due_subjects(&conn)?;
    assert!(!due.contains(&"pointers".to_string()));
    Ok(())
}

// ── Streak tracking ────────────────────────────────────────────────────────────

#[test]
fn test_get_streak_empty() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    assert_eq!(get_streak(&conn)?, 0);
    Ok(())
}

#[test]
fn test_get_streak_today() -> clings::error::Result<()> {
    use chrono::Utc;
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at, error_type, duration_ms, hint_count_used) VALUES ('t1', 'pointers', 'ex1', 1, ?1, NULL, NULL, 0)",
        rusqlite::params![now],
    )?;
    assert_eq!(get_streak(&conn)?, 1);
    Ok(())
}

#[test]
fn test_get_streak_consecutive_days() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    let now = Utc::now().timestamp();
    for (i, id) in ["c1", "c2", "c3"].iter().enumerate() {
        let ts = now - (i as i64) * SECS_PER_DAY;
        conn.execute(
            "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at, error_type, duration_ms, hint_count_used) VALUES (?1, 'pointers', 'ex1', 1, ?2, NULL, NULL, 0)",
            rusqlite::params![id, ts],
        )?;
    }
    assert_eq!(get_streak(&conn)?, 3);
    Ok(())
}

#[test]
fn test_get_streak_broken() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    let now = Utc::now().timestamp();
    // Aujourd'hui et il y a 3 jours (pas hier) → streak = 1
    for (id, offset) in [("b1", 0i64), ("b2", 3)] {
        conn.execute(
            "INSERT INTO practice_log (id, subject, exercise_id, success, practiced_at, error_type, duration_ms, hint_count_used) VALUES (?1, 'pointers', 'ex1', 1, ?2, NULL, NULL, 0)",
            rusqlite::params![id, now - offset * SECS_PER_DAY],
        )?;
    }
    assert_eq!(get_streak(&conn)?, 1);
    Ok(())
}

// ── Decay (SRS time-decay) ─────────────────────────────────────────────────────

#[test]
fn test_apply_all_decay_updates_db() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "structs")?;
    let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
    conn.execute(
        "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'structs'",
        rusqlite::params![old_ts],
    )?;
    apply_all_decay(&mut conn)?;
    let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
    assert_eq!(sub.mastery_score.get(), 1.5);
    Ok(())
}

#[test]
fn test_apply_all_decay_no_change_when_recent() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pipes")?;
    let recent_ts = Utc::now().timestamp() - 5 * SECS_PER_DAY;
    conn.execute(
        "UPDATE subjects SET mastery_score = 3.0, last_practiced_at = ?1 WHERE name = 'pipes'",
        rusqlite::params![recent_ts],
    )?;
    apply_all_decay(&mut conn)?;
    let sub = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
    assert_eq!(sub.mastery_score.get(), 3.0);
    Ok(())
}

#[test]
fn test_apply_all_decay_updates_last_practiced_at() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "structs")?;
    let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
    conn.execute(
        "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'structs'",
        rusqlite::params![old_ts],
    )?;
    apply_all_decay(&mut conn)?;
    let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
    assert_eq!(sub.mastery_score.get(), 1.5, "score must decay by 0.5");
    // last_practiced_at must have advanced (not remain at old_ts)
    assert!(
        sub.last_practiced_at
            .expect("last_practiced_at should be set")
            > old_ts,
        "last_practiced_at must advance after decay"
    );
    Ok(())
}

#[test]
fn test_apply_all_decay_idempotent_in_db() -> clings::error::Result<()> {
    use chrono::Utc;
    use clings::constants::SECS_PER_DAY;
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pipes")?;
    let old_ts = Utc::now().timestamp() - 15 * SECS_PER_DAY;
    conn.execute(
        "UPDATE subjects SET mastery_score = 2.0, last_practiced_at = ?1 WHERE name = 'pipes'",
        rusqlite::params![old_ts],
    )?;
    apply_all_decay(&mut conn)?;
    let sub1 = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
    apply_all_decay(&mut conn)?;
    let sub2 = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
    assert_eq!(
        sub1.mastery_score.get(),
        sub2.mastery_score.get(),
        "decay must not compound on second call"
    );
    Ok(())
}

#[test]
fn test_record_attempt_persists_srs_fields() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pipes")?;
    let sub = record_attempt(&conn, "pipes", "pipe-01", true)?;
    // Après un succès, intervalle SRS = round(1 * 2.5) = 3
    assert_eq!(sub.srs_interval_days.get(), 3);
    assert!(sub.next_review_at.is_some());
    // Vérifier la persistance en DB
    let reloaded = get_subject(&conn, "pipes")?.expect("pipes should exist after insert");
    assert_eq!(reloaded.srs_interval_days, sub.srs_interval_days);
    assert_eq!(reloaded.next_review_at, sub.next_review_at);
    Ok(())
}

// ── Import/export (persistence) ────────────────────────────────────────────────

#[test]
fn test_import_progress_clamps_mastery_score() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "structs")?;

    let json = r#"
    {
        "subjects": [
            {
                "name": "structs",
                "mastery_score": 10.0,
                "last_practiced_at": null,
                "attempts_total": 0,
                "attempts_success": 0,
                "difficulty_unlocked": 1,
                "next_review_at": null,
                "srs_interval_days": 1
            }
        ]
    }
    "#;

    let (count, _warnings) = import_progress(&mut conn, json, true)?;
    assert_eq!(count, 1);

    let sub = get_subject(&conn, "structs")?.expect("structs should exist after insert");
    assert_eq!(sub.mastery_score.get(), clings::constants::MASTERY_MAX);
    Ok(())
}

#[test]
fn test_import_progress_clamps_negative_score() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;

    let json = r#"
    {
        "subjects": [
            {
                "name": "pointers",
                "mastery_score": -2.0,
                "last_practiced_at": null,
                "attempts_total": 0,
                "attempts_success": 0,
                "difficulty_unlocked": 1,
                "next_review_at": null,
                "srs_interval_days": 1
            }
        ]
    }
    "#;

    let (count, _warnings) = import_progress(&mut conn, json, true)?;
    assert_eq!(count, 1);

    let sub = get_subject(&conn, "pointers")?.expect("pointers should exist after insert");
    assert_eq!(sub.mastery_score.get(), clings::constants::MASTERY_MIN);
    Ok(())
}

#[test]
fn test_import_progress_preserves_valid_scores() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "memory_allocation")?;

    let json = r#"{"subjects": [{"name": "memory_allocation", "mastery_score": 2.5,
        "attempts_total": 5, "attempts_success": 3,
        "difficulty_unlocked": 2, "srs_interval_days": 7,
        "last_practiced_at": null, "next_review_at": null}]}"#;
    let (count, _warnings) = import_progress(&mut conn, json, true)?;
    assert_eq!(count, 1);
    let sub = get_subject(&conn, "memory_allocation")?
        .expect("memory_allocation should exist after insert");
    assert!((sub.mastery_score.get() - 2.5).abs() < f64::EPSILON);
    assert_eq!(sub.attempts_total, 5);
    Ok(())
}

#[test]
fn test_import_progress_clamps_all_fields() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "signals")?;

    let json = r#"
    {
        "subjects": [
            {
                "name": "signals",
                "mastery_score": 3.0,
                "last_practiced_at": null,
                "attempts_total": -3,
                "attempts_success": 100,
                "difficulty_unlocked": 99,
                "next_review_at": null,
                "srs_interval_days": 9999
            }
        ]
    }
    "#;

    let (count, _warnings) = import_progress(&mut conn, json, true)?;
    assert_eq!(count, 1);

    let sub = get_subject(&conn, "signals")?.expect("signals should exist after insert");
    assert_eq!(sub.difficulty_unlocked, 5, "difficulty clamped to 5");
    assert_eq!(
        sub.srs_interval_days.get(),
        clings::constants::SRS_MAX_INTERVAL_DAYS,
        "srs_interval clamped to SRS_MAX_INTERVAL_DAYS"
    );
    assert_eq!(
        sub.attempts_total, 0,
        "negative attempts_total clamped to 0"
    );
    assert_eq!(
        sub.attempts_success, 0,
        "attempts_success clamped to attempts_total"
    );
    Ok(())
}

#[test]
fn test_save_load_last_exam_session_roundtrip() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_last_exam_session(&conn, "nsy103-s1-2024")?;
    let loaded = load_last_exam_session(&conn)?;
    assert_eq!(loaded, Some("nsy103-s1-2024".to_string()));
    Ok(())
}

#[test]
fn test_load_last_exam_session_missing_returns_none() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    let loaded = load_last_exam_session(&conn)?;
    assert_eq!(loaded, None);
    Ok(())
}

#[test]
fn test_exercise_scores_upsert() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    record_attempt(&conn, "pointers", "ptr-deref-01", true)?;
    record_attempt(&conn, "pointers", "ptr-deref-01", false)?;
    let scores = get_exercise_scores(&conn, "pointers")?;
    assert_eq!(scores.len(), 1);
    let (id, successes, attempts) = &scores[0];
    assert_eq!(id, "ptr-deref-01");
    assert_eq!(*attempts, 2);
    assert_eq!(*successes, 1);
    Ok(())
}

#[test]
fn test_get_all_weakest_exercises() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    record_attempt(&conn, "subj_a", "ex_a1", false)?;
    record_attempt(&conn, "subj_a", "ex_a2", true)?;
    record_attempt(&conn, "subj_b", "ex_b1", true)?;
    let map = get_all_weakest_exercises(&conn)?;
    assert_eq!(map.get("subj_a").map(|s| s.as_str()), Some("ex_a1"));
    assert_eq!(map.get("subj_b").map(|s| s.as_str()), Some("ex_b1"));
    Ok(())
}

#[test]
fn test_import_progress_invalid_json() {
    let mut conn = open_test_db().expect("open_test_db");
    let result = import_progress(&mut conn, "not valid json { at all", false);
    assert!(result.is_err(), "invalid JSON should return Err");
}

#[test]
fn test_import_progress_empty_subjects() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    let (count, warnings) = import_progress(&mut conn, r#"{"subjects": []}"#, false)?;
    assert_eq!(count, 0);
    assert!(warnings.is_empty());
    Ok(())
}

#[test]
fn test_import_progress_inserts_subject() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    let json = r#"{"subjects": [{"name": "pointers", "mastery_score": 2.5,
        "attempts_total": 10, "attempts_success": 8,
        "difficulty_unlocked": 2, "srs_interval_days": 7,
        "last_practiced_at": null, "next_review_at": null}]}"#;
    let (count, warnings) = import_progress(&mut conn, json, false)?;
    assert_eq!(count, 1);
    assert!(warnings.is_empty());
    let sub = get_subject(&conn, "pointers")?.expect("subject should exist after import");
    assert!((sub.mastery_score.get() - 2.5).abs() < f64::EPSILON);
    assert_eq!(sub.attempts_total, 10);
    assert_eq!(sub.difficulty_unlocked, 2);
    Ok(())
}

#[test]
fn test_import_progress_clamps_difficulty_out_of_bounds() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    let json = r#"{"subjects": [{"name": "pointers", "mastery_score": 1.0,
        "attempts_total": 0, "attempts_success": 0,
        "difficulty_unlocked": 99, "srs_interval_days": 1,
        "last_practiced_at": null, "next_review_at": null}]}"#;
    let (count, warnings) = import_progress(&mut conn, json, false)?;
    assert_eq!(count, 1);
    assert!(!warnings.is_empty(), "should warn about clamped difficulty");
    assert!(
        warnings.iter().any(|w| w.contains("difficulty_unlocked")),
        "warning should mention difficulty_unlocked; got: {warnings:?}"
    );
    let sub = get_subject(&conn, "pointers")?.expect("subject should exist after import");
    assert_eq!(
        sub.difficulty_unlocked, 5,
        "difficulty should be clamped to 5"
    );
    Ok(())
}

#[test]
fn test_import_progress_future_timestamp_accepted() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    let json = r#"{"subjects": [{"name": "signals", "mastery_score": 3.0,
        "attempts_total": 5, "attempts_success": 4,
        "difficulty_unlocked": 3, "srs_interval_days": 30,
        "last_practiced_at": 4102444800, "next_review_at": 4102531200}]}"#;
    let (count, _warnings) = import_progress(&mut conn, json, false)?;
    assert_eq!(count, 1);
    let sub = get_subject(&conn, "signals")?.expect("subject should exist after import");
    assert_eq!(sub.last_practiced_at, Some(4102444800));
    Ok(())
}

#[test]
fn test_import_progress_overwrite_replaces_existing() -> clings::error::Result<()> {
    let mut conn = open_test_db()?;
    ensure_subject_for_test(&conn, "pointers")?;
    record_attempt(&conn, "pointers", "ptr-deref-01", true)?;

    let json = r#"{"subjects": [{"name": "pointers", "mastery_score": 4.0,
        "attempts_total": 20, "attempts_success": 18,
        "difficulty_unlocked": 3, "srs_interval_days": 14,
        "last_practiced_at": null, "next_review_at": null}]}"#;
    let (count, _) = import_progress(&mut conn, json, true)?;
    assert_eq!(count, 1);
    let sub = get_subject(&conn, "pointers")?.expect("subject should exist");
    assert!(
        (sub.mastery_score.get() - 4.0).abs() < f64::EPSILON,
        "overwrite should replace score"
    );
    Ok(())
}

#[test]
fn test_last_session_roundtrip() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_last_session(&conn, "watch", Some(7), 12)?;
    let loaded = load_last_session(&conn)?;
    assert_eq!(loaded, Some(("watch".to_string(), Some(7), 12)));
    Ok(())
}

#[test]
fn test_last_session_all_chapters() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    save_last_session(&conn, "piscine", None, 5)?;
    let loaded = load_last_session(&conn)?;
    assert_eq!(loaded, Some(("piscine".to_string(), None, 5)));
    Ok(())
}

#[test]
fn test_last_session_missing_returns_none() -> clings::error::Result<()> {
    let conn = open_test_db()?;
    let loaded = load_last_session(&conn)?;
    assert_eq!(loaded, None);
    Ok(())
}
