use colored::Colorize;

use crate::models::Subject;

use super::{footer_box, header_box, hr, mastery_bar, show_banner};

const SPARK_BARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Build an ASCII sparkline from a slice of counts.
/// Empty input returns an empty string. All-zero input returns `▁` repeated.
pub fn sparkline(data: &[u32]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let max = data.iter().copied().max().unwrap_or(0);
    data.iter()
        .map(|&v| {
            let idx = if max == 0 {
                0
            } else {
                ((v as f64 / max as f64) * (SPARK_BARS.len() - 1) as f64).round() as usize
            };
            SPARK_BARS[idx.min(SPARK_BARS.len() - 1)]
        })
        .collect()
}

/// Show detailed statistics: per-subject attempt breakdown + 30-day activity sparkline.
pub fn show_stats_detailed(
    subjects: &[Subject],
    streak: u32,
    attempts: &[(String, u32, u32)],
    daily: &[(String, u32)],
) {
    println!();
    show_banner();

    println!(
        "  {}",
        header_box("clings — Statistiques détaillées").cyan()
    );
    println!();

    println!(
        "  {} {}  {}",
        "Série:".bold().cyan(),
        streak.to_string().bold().yellow(),
        "jours consécutifs".dimmed()
    );

    if subjects.is_empty() {
        println!("  {}", "Aucun sujet pratiqué pour l'instant.".dimmed());
        println!();
        println!("  {}", footer_box().cyan());
        return;
    }

    let total_mastery: f64 = subjects.iter().map(|s| s.mastery_score.get()).sum();
    let avg = total_mastery / subjects.len() as f64;
    println!(
        "  {} {}",
        "Maîtrise moyenne:".bold().cyan(),
        mastery_bar(avg)
    );
    println!();

    // ── Activité 30 jours ────────────────────────────────────────────────
    if !daily.is_empty() {
        let counts: Vec<u32> = daily.iter().map(|(_, c)| *c).collect();
        let spark = sparkline(&counts);
        let total_attempts: u32 = counts.iter().sum();
        println!("  {}", "── Activité (30 jours) ──".dimmed());
        println!("  {}  {} tentatives", spark.yellow(), total_attempts);
        println!();
    }

    // ── Tentatives par sujet ─────────────────────────────────────────────
    if !attempts.is_empty() {
        println!("  {}", hr().dimmed());
        println!(
            "  {:<22} {:>8} {:>8} {:>8}",
            "Sujet".bold(),
            "Succès".bold().green(),
            "Échecs".bold().red(),
            "Total".bold()
        );
        println!("  {}", hr().dimmed());
        for (name, succ, fail) in attempts {
            let total = succ + fail;
            println!("  {:<22} {:>8} {:>8} {:>8}", name, succ, fail, total);
        }
        println!("  {}", hr().dimmed());
        println!();
    }

    println!("  {}", "Continuez à pratiquer !".bold().green());
    println!();
    println!("  {}", footer_box().cyan());
    println!();
}

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

    let total_mastery: f64 = subjects.iter().map(|s| s.mastery_score.get()).sum();
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
            .get()
            .partial_cmp(&a.mastery_score.get())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Header row
    println!("  {}", hr().dimmed());
    println!(
        "  {:<22} {:<16} {}",
        "Sujet".bold(),
        "Maîtrise".bold(),
        "Barre".bold()
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
                sub.mastery_score.get(),
                mastery_bar(sub.mastery_score.get())
            );
        }
        println!("  {}", "── À renforcer ──".dimmed());
        for sub in sorted.iter().rev().take(TOP_N) {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score.get(),
                mastery_bar(sub.mastery_score.get())
            );
        }
    } else {
        for sub in &sorted {
            println!(
                "  {:<22} {:<6.1}  {}",
                sub.name,
                sub.mastery_score.get(),
                mastery_bar(sub.mastery_score.get())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_empty() {
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn sparkline_uniform_data() {
        let result = sparkline(&[5, 5, 5]);
        // All same value → all same character (the max → last bar '█')
        assert_eq!(result.chars().count(), 3);
        assert!(result.chars().all(|c| c == result.chars().next().unwrap()));
    }

    #[test]
    fn sparkline_ascending() {
        let result = sparkline(&[0, 4, 8]);
        let chars: Vec<char> = result.chars().collect();
        assert_eq!(chars.len(), 3);
        // Each char should be ≥ the previous (ascending values → ascending bars)
        assert!(
            chars[0] <= chars[1] && chars[1] <= chars[2],
            "ascending data should produce non-decreasing sparkline: {result}"
        );
    }

    #[test]
    fn sparkline_all_zeros() {
        let result = sparkline(&[0, 0, 0]);
        // All zeros → all minimum bar
        assert!(result.chars().all(|c| c == SPARK_BARS[0]));
    }

    #[test]
    fn sparkline_single_value() {
        let result = sparkline(&[42]);
        assert_eq!(result.chars().count(), 1);
        // Single value is maximum → should be '█'
        assert_eq!(
            result.chars().next().unwrap(),
            SPARK_BARS[SPARK_BARS.len() - 1]
        );
    }
}
