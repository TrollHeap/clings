//! Fuzzy exercise search using nucleo-matcher.

use std::fmt::Write as _;

use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::models::Exercise;

/// Score un exercise contre une query. Retourne None si aucun match.
/// `char_buf` et `str_buf` sont réutilisés entre appels pour éviter les allocations.
fn score_exercise(
    matcher: &mut Matcher,
    query_buf: &[char],
    ex: &Exercise,
    char_buf: &mut Vec<char>,
    str_buf: &mut String,
) -> Option<u32> {
    let desc_end = ex
        .description
        .char_indices()
        .nth(120)
        .map(|(i, _)| i)
        .unwrap_or(ex.description.len());
    str_buf.clear();
    let _ = write!(
        str_buf,
        "{} {} {} {} {}",
        ex.id,
        ex.title,
        ex.subject,
        ex.key_concept.as_deref().unwrap_or(""),
        &ex.description[..desc_end],
    );
    char_buf.clear();
    matcher
        .fuzzy_match(
            Utf32Str::new(str_buf, char_buf),
            Utf32Str::Unicode(query_buf),
        )
        .map(|s| s as u32)
}

/// Recherche fuzzy sur la liste d'exercices.
/// Retourne les indices dans `exercises` triés par score décroissant (meilleur match en premier).
pub fn search_exercises(
    exercises: &[Exercise],
    query: &str,
    filter_subject: Option<&str>,
) -> Vec<(usize, u32)> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let query_buf: Vec<char> = query.chars().collect();
    let mut char_buf: Vec<char> = Vec::new();
    let mut str_buf = String::new();

    let mut results: Vec<(usize, u32)> = Vec::with_capacity(exercises.len());
    for (idx, ex) in exercises.iter().enumerate() {
        if filter_subject.is_some_and(|s| ex.subject != s) {
            continue;
        }
        if let Some(score) =
            score_exercise(&mut matcher, &query_buf, ex, &mut char_buf, &mut str_buf)
        {
            results.push((idx, score));
        }
    }

    results.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercises::load_all_exercises;

    #[test]
    fn test_search_empty_query_matches_all() {
        let (exercises, _) = load_all_exercises().unwrap();
        // query vide → nucleo matche tout (pattern vide = match universel)
        let results = search_exercises(&exercises, "", None);
        assert_eq!(
            results.len(),
            exercises.len(),
            "query vide doit matcher tous les exercices"
        );
    }

    #[test]
    fn test_search_known_subject_returns_results() {
        let (exercises, _) = load_all_exercises().unwrap();
        let results = search_exercises(&exercises, "pointer", None);
        assert!(
            !results.is_empty(),
            "\"pointer\" doit trouver au moins un exercice"
        );
    }

    #[test]
    fn test_search_filter_subject_limits_results() {
        let (exercises, _) = load_all_exercises().unwrap();
        let all = search_exercises(&exercises, "malloc", None);
        let first_subject = exercises[0].subject.as_str();
        let filtered = search_exercises(&exercises, "malloc", Some(first_subject));
        assert!(
            filtered.len() <= all.len(),
            "filtrage par sujet ne doit pas augmenter le nombre de résultats"
        );
        for (idx, _) in &filtered {
            assert_eq!(
                exercises[*idx].subject, first_subject,
                "tous les résultats doivent appartenir au sujet filtré"
            );
        }
    }

    #[test]
    fn test_search_results_sorted_by_score_descending() {
        let (exercises, _) = load_all_exercises().unwrap();
        let results = search_exercises(&exercises, "fork", None);
        for window in results.windows(2) {
            assert!(
                window[0].1 >= window[1].1,
                "résultats doivent être triés par score décroissant"
            );
        }
    }
}
