//! Pédagogical reporting — generate learning analytics by chapter and subject.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::chapters::Chapter;
use crate::error::Result;

/// Rapport d'apprentissage pour un sujet au sein d'un chapitre.
#[derive(Debug, Clone)]
pub struct SubjectReport {
    /// Nom du sujet (ex. "pointers", "signals").
    pub subject: String,
    /// Score moyen de maîtrise pour ce sujet.
    pub mastery: f64,
    /// Nombre total de tentatives.
    pub attempts: u64,
    /// Nombre de tentatives réussies.
    #[allow(dead_code)]
    pub successes: u64,
    /// IDs des exercices avec mastery < 2.0 (points faibles).
    #[allow(dead_code)]
    pub weakest_exercises: Vec<String>,
}

/// Rapport d'apprentissage pour un chapitre entier.
#[derive(Debug, Clone)]
pub struct ChapterReport {
    /// Numéro du chapitre (1–16).
    pub chapter_num: u8,
    /// Titre du chapitre.
    pub chapter_title: String,
    /// Rapports par sujet.
    pub subjects: Vec<SubjectReport>,
    /// Moyenne des mastery scores pour tous les sujets.
    pub avg_mastery: f64,
    /// Nombre total de tentatives dans le chapitre.
    pub total_attempts: u64,
    /// Taux de réussite en pourcentage.
    pub success_rate: f64,
}

/// Build reports for all chapters.
pub fn build_chapter_reports(
    progress: &Connection,
    chapters: &[Chapter],
) -> Result<Vec<ChapterReport>> {
    let all_subjects = crate::progress::get_all_subjects(progress)?;
    let all_attempts = crate::progress::get_subject_attempts(progress)?;
    let all_weakest = crate::progress::get_all_weakest_exercises(progress)?;

    // Map subject name → attempts (successes, total)
    let attempts_map: HashMap<String, (u64, u64)> = all_attempts
        .iter()
        .map(|(s, successes, total)| (s.clone(), (*successes as u64, *total as u64)))
        .collect();

    let mut chapter_reports = Vec::new();

    for chapter in chapters {
        let mut subject_reports = Vec::new();
        let mut chapter_total_attempts = 0u64;
        let mut chapter_total_successes = 0u64;

        for subject_name in chapter.subjects {
            let subject_data = all_subjects.iter().find(|s| s.name == *subject_name);
            let mastery = subject_data.map(|s| s.mastery_score.get()).unwrap_or(0.0);

            let (successes, total) = attempts_map.get(*subject_name).copied().unwrap_or((0, 0));

            chapter_total_attempts += total;
            chapter_total_successes += successes;

            let weakest_exercises = if let Some(exercise_id) = all_weakest.get(*subject_name) {
                vec![exercise_id.clone()]
            } else {
                Vec::new()
            };

            subject_reports.push(SubjectReport {
                subject: subject_name.to_string(),
                mastery,
                attempts: total,
                successes,
                weakest_exercises,
            });
        }

        let avg_mastery = if subject_reports.is_empty() {
            0.0
        } else {
            let sum: f64 = subject_reports.iter().map(|r| r.mastery).sum();
            sum / subject_reports.len() as f64
        };

        let success_rate = if chapter_total_attempts > 0 {
            (chapter_total_successes as f64 / chapter_total_attempts as f64) * 100.0
        } else {
            0.0
        };

        chapter_reports.push(ChapterReport {
            chapter_num: chapter.number,
            chapter_title: chapter.title.to_string(),
            subjects: subject_reports,
            avg_mastery,
            total_attempts: chapter_total_attempts,
            success_rate,
        });
    }

    Ok(chapter_reports)
}

/// Build a single chapter report by chapter number.
#[allow(dead_code)]
pub fn build_chapter_report_by_number(
    progress: &Connection,
    chapters: &[Chapter],
    chapter_num: u8,
) -> Result<Option<ChapterReport>> {
    let all_reports = build_chapter_reports(progress, chapters)?;
    Ok(all_reports
        .into_iter()
        .find(|r| r.chapter_num == chapter_num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chapter_report_structure() {
        // Verify ChapterReport can be constructed
        let subject_report = SubjectReport {
            subject: "pointers".to_string(),
            mastery: 3.5,
            attempts: 10,
            successes: 8,
            weakest_exercises: vec!["ptr_arith_02".to_string()],
        };

        let chapter_report = ChapterReport {
            chapter_num: 1,
            chapter_title: "Fondamentaux C".to_string(),
            subjects: vec![subject_report],
            avg_mastery: 3.5,
            total_attempts: 10,
            success_rate: 80.0,
        };

        assert_eq!(chapter_report.chapter_num, 1);
        assert_eq!(chapter_report.avg_mastery, 3.5);
        assert_eq!(chapter_report.success_rate, 80.0);
    }
}
