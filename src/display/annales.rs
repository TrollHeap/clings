use std::io::Read;

use colored::Colorize;

use crate::models::Exercise;

use super::{hr, show_banner, AnnaleSession};

/// Affiche les annales NSY103 avec le mapping vers les exercices clings.
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
    let mut esc_buf: Vec<u8> = Vec::new();

    let _raw = crate::enable_raw_mode();

    loop {
        // Clear screen and redraw
        print!("\x1b[2J\x1b[H");
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

        // Read one byte
        let mut buf = [0u8; 1];
        if std::io::stdin().read_exact(&mut buf).is_err() {
            return None;
        }
        let byte = buf[0];

        // Accumulate ESC sequences
        if byte == 0x1b {
            esc_buf.clear();
            esc_buf.push(byte);
            // Try to read 2 more bytes with a short non-blocking window
            let mut b2 = [0u8; 1];
            if std::io::stdin().read_exact(&mut b2).is_ok() {
                esc_buf.push(b2[0]);
                let mut b3 = [0u8; 1];
                if std::io::stdin().read_exact(&mut b3).is_ok() {
                    esc_buf.push(b3[0]);
                }
            }
            // Arrow up: ESC [ A
            if esc_buf == [0x1b, b'[', b'A'] {
                cursor = cursor.saturating_sub(1);
            }
            // Arrow down: ESC [ B
            if esc_buf == [0x1b, b'[', b'B'] && cursor + 1 < sessions.len() {
                cursor += 1;
            }
            esc_buf.clear();
            continue;
        }

        match byte {
            b'k' | b'K' => cursor = cursor.saturating_sub(1),
            b'j' | b'J' => {
                if cursor + 1 < sessions.len() {
                    cursor += 1;
                }
            }
            b'\r' | b'\n' => {
                print!("\x1b[2J\x1b[H");
                return Some(sessions[cursor].id.clone());
            }
            b'q' | b'Q' => {
                print!("\x1b[2J\x1b[H");
                return None;
            }
            _ => {}
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
