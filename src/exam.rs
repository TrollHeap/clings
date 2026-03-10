use colored::Colorize;

use crate::error::{KfError, Result};
use crate::exercises;

#[derive(Debug, serde::Deserialize)]
struct AnnaleQuestion {
    pub exercises: Vec<String>,
    #[serde(default)]
    pub points: f32,
    pub title: String,
}

#[derive(Debug, serde::Deserialize)]
struct AnnaleSession {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub total_points: f32,
    pub questions: Vec<AnnaleQuestion>,
}

/// Durée par défaut selon le type de session (minutes)
fn default_duration(session_id: &str) -> u64 {
    if session_id.starts_with("utc502") {
        180
    } else {
        150 // NSY103 = 2h30
    }
}

pub fn cmd_exam(session_id: Option<&str>, list_sessions: bool) -> Result<()> {
    // 1. Charger annales_map.json
    let exercises_dir = exercises::resolve_exercises_dir()?;
    let map_path = exercises_dir.join("annales_map.json");
    let raw = std::fs::read_to_string(&map_path)?;
    let sessions: Vec<AnnaleSession> = serde_json::from_str(&raw)
        .map_err(|e| KfError::Config(format!("annales_map.json: {e}")))?;

    // 2. Si list ou pas de session : lister les sessions
    if list_sessions || session_id.is_none() {
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

    // 3. Trouver la session
    let sid = session_id.expect("session_id is Some: guarded by is_none() check above");
    let session = sessions
        .iter()
        .find(|s| s.id == sid)
        .ok_or_else(|| KfError::Config(format!("Session introuvable : '{sid}'")))?;

    // 4. Collecter les exercise IDs (dédupliqués, dans l'ordre)
    let mut seen = std::collections::HashSet::new();
    let mut exercise_ids: Vec<&str> = Vec::new();
    for q in &session.questions {
        for eid in &q.exercises {
            if seen.insert(eid.as_str()) {
                exercise_ids.push(eid.as_str());
            }
        }
    }

    let total_ex = exercise_ids.len();
    let duration = default_duration(&session.id);

    // 5. Afficher l'introduction
    println!();
    println!(
        "  {}",
        "╔════════════════════════════════════════════════════════╗"
            .bold()
            .cyan()
    );
    println!(
        "  {}  {}  {}",
        "║".bold().cyan(),
        format!(" EXAM SIMULÉ — {} ", session.title).bold().cyan(),
        "║".bold().cyan()
    );
    println!(
        "  {}",
        "╚════════════════════════════════════════════════════════╝"
            .bold()
            .cyan()
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
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().eq_ignore_ascii_case("q") {
        println!("  {}", "Annulé.".dimmed());
        return Ok(());
    }

    // 6. Charger les exercices filtrés
    let (all_exercises, _) = exercises::load_all_exercises()?;

    // Filtrer dans l'ordre des IDs de la session
    let exam_exercises: Vec<crate::models::Exercise> = exercise_ids
        .iter()
        .filter_map(|id| all_exercises.iter().find(|e| e.id == *id).cloned())
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
    crate::piscine::run_exam_piscine(exam_exercises, Some(duration))
}
