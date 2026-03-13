//! Fuzzy exercise search using nucleo-matcher.

use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::models::Exercise;

/// Score un exercise contre une query. Retourne None si aucun match.
/// `buf` est réutilisé entre les appels pour éviter des allocations répétées.
fn score_exercise(
    matcher: &mut Matcher,
    query_buf: &[char],
    ex: &Exercise,
    buf: &mut Vec<char>,
) -> Option<u32> {
    let desc_preview: String = ex.description.chars().take(120).collect();
    let haystack = format!(
        "{} {} {} {} {}",
        ex.id,
        ex.title,
        ex.subject,
        ex.key_concept.as_deref().unwrap_or(""),
        desc_preview,
    );
    buf.clear();
    matcher
        .fuzzy_match(Utf32Str::new(&haystack, buf), Utf32Str::Unicode(query_buf))
        .map(|s| s as u32)
}

/// Recherche fuzzy sur la liste d'exercices.
/// Retourne les exercices triés par score décroissant (meilleur match en premier).
pub fn search_exercises<'a>(
    exercises: &'a [Exercise],
    query: &str,
    filter_subject: Option<&str>,
) -> Vec<(&'a Exercise, u32)> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let query_buf: Vec<char> = query.chars().collect();
    let mut buf: Vec<char> = Vec::new();

    let mut results: Vec<(&Exercise, u32)> = exercises
        .iter()
        .filter(|ex| filter_subject.is_none_or(|s| ex.subject == s))
        .filter_map(|ex| score_exercise(&mut matcher, &query_buf, ex, &mut buf).map(|s| (ex, s)))
        .collect();

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
        for (ex, _) in &filtered {
            assert_eq!(
                ex.subject, first_subject,
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
