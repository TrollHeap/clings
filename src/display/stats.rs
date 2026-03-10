use colored::Colorize;

use crate::models::Subject;

use super::{footer_box, header_box, hr, mastery_bar, show_banner};

/// Show global statistics: streak, average mastery, top/bottom subjects.
pub fn show_stats(subjects: &[Subject], streak: u32) {
    println!();
    show_banner();

    println!("  {}", header_box("clings — Statistiques").cyan());
    println!();

    // Streak
    println!(
        "  {} {}  {}",
        "Série:".bold().cyan(),
        streak.to_string().bold().yellow(),
        "jours consécutifs".dimmed()
    );

    // Average mastery
    if subjects.is_empty() {
        println!("  {}", "Aucun sujet pratiqué pour l'instant.".dimmed());
        println!();
        println!("  {}", footer_box().cyan());
        return;
    }

    let total_mastery: f64 = subjects.iter().map(|s| s.mastery_score).sum();
    let avg = total_mastery / subjects.len() as f64;
    println!(
        "  {} {}",
        "Maîtrise moyenne:".bold().cyan(),
        mastery_bar(avg)
    );
    println!();

    // Sort by mastery descending
    let mut sorted: Vec<&Subject> = subjects.iter().collect();
    sorted.sort_by(|a, b| {
        b.mastery_score
            .partial_cmp(&a.mastery_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Header row
    println!("  {}", hr().dimmed());
    println!(
        "  {:<22} {:<16} {}",
        "Sujet".bold(),
        "Mastery".bold(),
        "Bar".bold()
    );
    println!("  {}", hr().dimmed());

    const TOP_N: usize = 5;

    // Top subjects
    if sorted.len() > TOP_N * 2 {
        println!("  {}", "── Top sujets ──".dimmed());
        for sub in sorted.iter().take(TOP_N) {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
        println!("  {}", "── À renforcer ──".dimmed());
        for sub in sorted.iter().rev().take(TOP_N) {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
    } else {
        for sub in &sorted {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score,
                mastery_bar(sub.mastery_score)
            );
        }
    }

    println!("  {}", hr().dimmed());
    println!();
    println!("  {}", "Continuez à pratiquer !".bold().green());
    println!();
    println!("  {}", footer_box().cyan());
    println!();
}
