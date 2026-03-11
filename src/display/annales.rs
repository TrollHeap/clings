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
