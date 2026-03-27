//! Exam simulation — loads annales sessions and launches a timed piscine.

use colored::Colorize;

use crate::error::{KfError, Result};
use crate::exercises;
use crate::models::{AnnaleQuestion, AnnaleSession};
use crate::progress;

/// Durée par défaut selon le type de session (minutes)
fn default_duration(session_id: &str) -> u64 {
    if session_id.starts_with("utc502") {
        crate::constants::EXAM_UTC502_DURATION_MINS
    } else {
        crate::constants::EXAM_NSY103_DURATION_MINS
    }
}

/// Collect deduplicated exercise IDs from session questions, preserving order.
fn collect_unique_ids(questions: &[AnnaleQuestion]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for q in questions {
        for eid in &q.exercises {
            if seen.insert(eid.as_str()) {
                ids.push(eid.clone());
            }
        }
    }
    ids
}

/// Launch exam simulation: select an annales session, display exam info, and run a timed piscine with exam exercises.
/// If list_sessions=true, prints available sessions and exits. Otherwise opens interactive session selector if no session_id provided.
pub fn cmd_exam(session_id: Option<&str>, list_sessions: bool) -> Result<()> {
    // 1. Charger annales_map.json
    let sessions: Vec<AnnaleSession> = exercises::load_annales_map()?;

    // 2. Si --list : afficher les sessions textuellement
    if list_sessions {
        println!(
            "\n  {} Sessions disponibles :\n",
            "Exam simulé —".bold().cyan()
        );
        for s in &sessions {
            println!(
                "    {}  {}  ({} pts)",
                s.id.bold(),
                s.title.dimmed(),
                s.total_points
            );
        }
        println!();
        println!("  Lancer : {}", "clings exam --session <id>".bold());
        println!();
        return Ok(());
    }

    // 3. Si pas de session : ouvrir le sélecteur TUI interactif
    let session_id_owned: String;
    let sid: &str = if let Some(id) = session_id {
        id
    } else {
        let conn = progress::open_db()?;
        let last = progress::load_last_exam_session(&conn)?;
        let chosen = crate::tui::ui_exam_selector::select_exam_session(&sessions, last.as_deref());
        match chosen {
            Some(id) => {
                progress::save_last_exam_session(&conn, &id)?;
                session_id_owned = id;
                &session_id_owned
            }
            None => {
                println!("  {}", "Annulé.".dimmed());
                return Ok(());
            }
        }
    };
    let session = sessions
        .iter()
        .find(|s| s.id == sid)
        .ok_or_else(|| KfError::Config(format!("Session introuvable : '{sid}'")))?;

    // 4. Collecter les exercise IDs (dédupliqués, dans l'ordre)
    let exercise_ids = collect_unique_ids(&session.questions);

    let total_ex = exercise_ids.len();
    let duration = default_duration(&session.id);

    // 5. Afficher l'introduction
    println!();
    println!(
        "  {} EXAM SIMULÉ — {}",
        "▶".bold().cyan(),
        session.title.bold().cyan()
    );
    println!();
    println!(
        "  Questions: {}   Exercices: {}   Durée: {}h{:02}min",
        session.questions.len().to_string().bold(),
        total_ex.to_string().bold(),
        duration / 60,
        duration % 60,
    );
    println!();

    // Résumé des questions
    for q in &session.questions {
        println!(
            "  {} ({} pts) — {} exercices",
            q.title.dimmed(),
            q.points,
            q.exercises.len()
        );
    }
    println!();

    // Confirmation
    print!(
        "  {} Appuyez sur [Entrée] pour démarrer ou [q] pour annuler : ",
        "▶".bold().green()
    );
    use std::io::Write;
    std::io::stdout().flush().ok(); // best-effort flush — non-critique
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().eq_ignore_ascii_case("q") {
        println!("  {}", "Annulé.".dimmed());
        return Ok(());
    }

    // 6. Charger les exercices filtrés
    let (all_exercises, _) = exercises::load_all_exercises()?;

    // Filtrer dans l'ordre des IDs de la session
    let ex_by_id: std::collections::HashMap<&str, &crate::models::Exercise> =
        all_exercises.iter().map(|e| (e.id.as_str(), e)).collect();
    let exam_exercises: Vec<crate::models::Exercise> = exercise_ids
        .iter()
        .filter_map(|id| ex_by_id.get(id.as_str()).map(|e| (*e).clone()))
        .collect();

    if exam_exercises.is_empty() {
        println!(
            "  {} Aucun exercice trouvé pour cette session.",
            "⚠".yellow()
        );
        return Ok(());
    }

    println!(
        "  {} Lancement avec {} exercices sélectionnés...",
        "→".bold().green(),
        exam_exercises.len()
    );
    println!();

    // Lancer une session piscine avec ces exercices uniquement
    crate::piscine::run_exam_piscine(exam_exercises, Some(duration), Some(sid))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(exercises: &[&str]) -> AnnaleQuestion {
        AnnaleQuestion {
            exercises: exercises.iter().map(|s| s.to_string()).collect(),
            points: 1.0,
            title: "Q".to_string(),
            number: 0,
            summary: String::new(),
            subjects: vec![],
        }
    }

    #[test]
    fn test_collect_unique_ids_deduplicates() {
        // "ptr-1" appears in both questions — should appear only once, first occurrence wins
        let questions = vec![q(&["ptr-1", "ptr-2"]), q(&["ptr-2", "ptr-3", "ptr-1"])];
        let ids = collect_unique_ids(&questions);
        assert_eq!(ids, vec!["ptr-1", "ptr-2", "ptr-3"]);
    }

    #[test]
    fn test_collect_unique_ids_preserves_order() {
        let questions = vec![q(&["c", "a"]), q(&["b"])];
        let ids = collect_unique_ids(&questions);
        assert_eq!(ids, vec!["c", "a", "b"]);
    }

    #[test]
    fn test_collect_unique_ids_empty() {
        assert!(collect_unique_ids(&[]).is_empty());
        assert!(collect_unique_ids(&[q(&[])]).is_empty());
    }

    #[test]
    fn test_default_duration_nsy103() {
        assert_eq!(default_duration("nsy103-s1-2024"), 150);
    }

    #[test]
    fn test_default_duration_utc502() {
        assert_eq!(default_duration("utc502-s2-2024"), 180);
    }
}
