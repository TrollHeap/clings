//! TUI display for annales — past exam session selector and question mapping.

use colored::Colorize;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::models::Exercise;

use crate::constants::ANSI_CLEAR_SCREEN;

use super::{hr, show_banner, AnnaleSession};

/// Affiche les annales NSY103 avec le mapping vers les exercices clings.
#[allow(dead_code)]
pub fn show_annales(annales: &[AnnaleSession], exercises: &[Exercise]) {
    println!();
    show_banner();
    println!(
        "  {} {}\n",
        "Annales NSY103".bold().cyan(),
        "— correspondance exercices clings".dimmed()
    );

    for exam in annales {
        println!(
            "  {} {} — {} ({}pt)",
            "▸".bold().cyan(),
            exam.title.bold(),
            exam.date.dimmed(),
            exam.total_points
        );
        println!("  {}", hr().dimmed());

        for q in &exam.questions {
            let pts = format!("({:.0}pt)", q.points);
            println!(
                "  Q{} {} {} — {}",
                q.number,
                pts.dimmed(),
                q.title.bold(),
                q.summary.dimmed()
            );

            if !q.subjects.is_empty() {
                println!(
                    "    {} {}",
                    "Sujets:".dimmed(),
                    q.subjects.join(", ").cyan()
                );
            }

            // Prefer the curated exercise list from the annales map; fall back to subject filter.
            let ids: Vec<String> = if !q.exercises.is_empty() {
                // Curated list: show all (they're already hand-picked for this question)
                q.exercises.clone()
            } else {
                exercises
                    .iter()
                    .filter(|e| q.subjects.iter().any(|s| s == &e.subject))
                    .map(|e| e.id.clone())
                    .collect()
            };

            if ids.is_empty() {
                println!("    {}", "Aucun exercice associé.".dimmed());
            } else {
                let shown = &ids[..ids.len().min(5)];
                let more = if ids.len() > 5 {
                    format!(" +{} autres", ids.len() - 5)
                } else {
                    String::new()
                };
                println!(
                    "    {} {}{}",
                    "Exercices:".dimmed(),
                    shown.join(", ").green(),
                    more.dimmed()
                );
            }
            println!();
        }
    }

    println!(
        "  {} `clings list --subject <sujet>` pour voir tous les exercices d'un sujet.",
        "Astuce:".bold().yellow()
    );
    println!();
}

/// Sélecteur interactif TUI pour choisir une session d'exam (flèches + Entrée, q pour quitter).
/// Retourne l'ID de la session choisie, ou None si annulé.
/// DEPRECATED: Use crate::tui::ui_exam_selector::select_exam_session instead.
#[allow(dead_code)]
pub fn select_exam_session(
    sessions: &[AnnaleSession],
    last_session_id: Option<&str>,
) -> Option<String> {
    if sessions.is_empty() {
        return None;
    }

    let initial = last_session_id
        .and_then(|id| sessions.iter().position(|s| s.id == id))
        .unwrap_or(0);
    let mut cursor = initial;

    let _raw = crate::enable_raw_mode();

    loop {
        // Clear screen and redraw
        print!("{ANSI_CLEAR_SCREEN}");
        println!();
        println!("  {}", "Sélectionner une session d'exam".bold().cyan());
        println!(
            "  {} flèches/jk : naviguer  Entrée : lancer  q : annuler\n",
            "▸".dimmed()
        );

        for (i, s) in sessions.iter().enumerate() {
            if i == cursor {
                println!(
                    "  {} {} — {} ({} pts)",
                    "▶".bold().green(),
                    s.id.bold(),
                    s.title.cyan(),
                    s.total_points
                );
            } else {
                println!(
                    "    {} — {} ({} pts)",
                    s.id.dimmed(),
                    s.title.dimmed(),
                    s.total_points
                );
            }
        }
        println!();
        let _ = std::io::Write::flush(&mut std::io::stdout());

        if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            cursor = cursor.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            if cursor + 1 < sessions.len() {
                                cursor += 1;
                            }
                        }
                        KeyCode::Enter => {
                            print!("{ANSI_CLEAR_SCREEN}");
                            return Some(sessions[cursor].id.clone());
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            print!("{ANSI_CLEAR_SCREEN}");
                            return None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_exam_session_returns_none_on_empty_list() {
        // Empty list should return None without blocking
        let result = select_exam_session(&[], None);
        assert!(result.is_none());
    }
}
