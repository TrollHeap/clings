//! Rapport d'apprentissage pédagogique par chapitre.

use crate::chapters::CHAPTERS;
use crate::error::Result;
use crate::progress;
use crate::reporting;

/// Display learning report. If chapter is provided, shows report for that chapter only.
pub fn cmd_report(chapter: Option<u8>) -> Result<()> {
    let mut conn = progress::open_db()?;
    progress::apply_all_decay(&mut conn)?;

    let all_reports = reporting::build_chapter_reports(&conn, CHAPTERS)?;

    if let Some(ch_num) = chapter {
        // Show single chapter report
        if let Some(report) = all_reports.iter().find(|r| r.chapter_num == ch_num) {
            print_chapter_report(report);
            Ok(())
        } else {
            Err(crate::error::KfError::ExerciseNotFound(format!(
                "Chapitre {} non trouvé",
                ch_num
            )))
        }
    } else {
        // Show all chapters
        print_full_report(&all_reports);
        Ok(())
    }
}

fn print_full_report(reports: &[reporting::ChapterReport]) {
    println!();
    println!("{}", "═".repeat(50));
    println!("{:<50}", "Rapport d'apprentissage clings");
    println!("{}", "═".repeat(50));

    for report in reports {
        print_chapter_report(report);
    }

    // Summary section
    println!();
    println!("{}", "─".repeat(50));

    let strong_subjects: Vec<_> = reports
        .iter()
        .flat_map(|ch| &ch.subjects)
        .filter(|s| s.mastery >= 4.0)
        .map(|s| s.subject.clone())
        .collect();

    let weak_subjects: Vec<_> = reports
        .iter()
        .flat_map(|ch| &ch.subjects)
        .filter(|s| s.mastery < 2.5)
        .map(|s| s.subject.clone())
        .collect();

    if !strong_subjects.is_empty() {
        println!("Points forts (≥4.0) : {}", strong_subjects.join(", "));
    }

    if !weak_subjects.is_empty() {
        println!("Points faibles (<2.5) : {}", weak_subjects.join(", "));
    }

    println!("{}", "═".repeat(50));
    println!();
}

fn print_chapter_report(report: &reporting::ChapterReport) {
    let bar_width = 10usize;
    let filled = (report.avg_mastery / 5.0 * bar_width as f64) as usize;
    let empty = bar_width.saturating_sub(filled);

    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    println!();
    println!("Ch.{} — {}", report.chapter_num, report.chapter_title);

    println!(
        "  Maîtrise : {} {:.1}/5.0  │  Succès : {:.0}%  │  Tentatives : {}",
        bar, report.avg_mastery, report.success_rate, report.total_attempts
    );

    let weak: Vec<_> = report
        .subjects
        .iter()
        .filter(|s| s.mastery < 2.0 && s.attempts > 0)
        .collect();

    if !weak.is_empty() {
        let weak_str = weak
            .iter()
            .map(|s| format!("{} ({:.1})", s.subject, s.mastery))
            .collect::<Vec<_>>()
            .join(" · ");
        println!("  Faibles  : {}", weak_str);
    } else {
        println!("  ─ Aucun point faible critique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_report_function_exists() {
        // Verify the function signature is valid
        let _f: fn(Option<u8>) -> Result<()> = cmd_report;
    }
}
