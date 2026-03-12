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
