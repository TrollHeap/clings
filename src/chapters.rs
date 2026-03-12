//! NSY103 chapter definitions and curriculum-based exercise ordering.

use crate::models::{Exercise, Subject};

/// A chapter in the NSY103 learning path.
#[derive(Debug, Clone)]
pub struct Chapter {
    /// Numéro du chapitre dans la progression NSY103 (1-based)
    pub number: u8,
    /// Intitulé affiché dans la TUI
    pub title: &'static str,
    /// Noms des sujets appartenant à ce chapitre
    pub subjects: &'static [&'static str],
}

// Compile-time guarantee that CHAPTERS.len() fits in u8 (used throughout for chapter_number).
const _: () = assert!(CHAPTERS.len() <= 255, "chapter count must fit in u8");

const UNCATEGORIZED_CHAPTER: Chapter = Chapter {
    number: 0,
    title: "Divers",
    subjects: &[],
};

/// NSY103 "Linux: noyau et programmation système" chapter progression.
pub const CHAPTERS: &[Chapter] = &[
    Chapter {
        number: 1,
        title: "Fondamentaux C",
        subjects: &["structs", "pointers"],
    },
    Chapter {
        number: 2,
        title: "Chaînes & bits",
        subjects: &["string_formatting", "bitwise_ops"],
    },
    Chapter {
        number: 3,
        title: "Allocation mémoire & errno",
        subjects: &["memory_allocation", "errno"],
    },
    Chapter {
        number: 4,
        title: "Entrées/sorties & descripteurs",
        subjects: &["file_io", "fd_basics"],
    },
    Chapter {
        number: 5,
        title: "Système de fichiers",
        subjects: &["filesystem"],
    },
    Chapter {
        number: 6,
        title: "Ordonnancement",
        subjects: &["scheduling"],
    },
    Chapter {
        number: 7,
        title: "Processus",
        subjects: &["processes"],
    },
    Chapter {
        number: 8,
        title: "Signaux",
        subjects: &["signals"],
    },
    Chapter {
        number: 9,
        title: "Tubes",
        subjects: &["pipes"],
    },
    Chapter {
        number: 10,
        title: "Files de messages",
        subjects: &["message_queues"],
    },
    Chapter {
        number: 11,
        title: "Mémoire partagée",
        subjects: &["shared_memory"],
    },
    Chapter {
        number: 12,
        title: "Sémaphores",
        subjects: &["semaphores"],
    },
    Chapter {
        number: 13,
        title: "Threads POSIX & synchronisation",
        subjects: &["pthreads", "sync_concepts"],
    },
    Chapter {
        number: 14,
        title: "Sockets",
        subjects: &["sockets"],
    },
    Chapter {
        number: 15,
        title: "Mémoire virtuelle",
        subjects: &["proc_memory", "virtual_memory"],
    },
    Chapter {
        number: 16,
        title: "Projets intégrateurs",
        subjects: &["capstones"],
    },
];

/// Order exercises following NSY103 chapter progression.
/// Within each chapter: difficulty ascending, then SRS priority (lowest mastery first).
pub fn order_by_chapters<'a>(
    exercises: &'a [Exercise],
    subjects: &[Subject],
) -> Vec<ChapterBlock<'a>> {
    let subject_mastery: std::collections::HashMap<&str, f64> = subjects
        .iter()
        .map(|s| (s.name.as_str(), s.mastery_score.get()))
        .collect();

    // Build subject→chapter_index map once (O(chapters) instead of O(chapters×exercises))
    let subject_to_chapter: std::collections::HashMap<&str, usize> = CHAPTERS
        .iter()
        .enumerate()
        .flat_map(|(i, ch)| ch.subjects.iter().map(move |&s| (s, i)))
        .collect();

    // Partition exercises into chapter buckets in a single pass
    let mut buckets: Vec<Vec<&'a Exercise>> = vec![Vec::new(); CHAPTERS.len()];
    let mut uncategorized: Vec<&'a Exercise> = Vec::new();

    for ex in exercises {
        if let Some(&idx) = subject_to_chapter.get(ex.subject.as_str()) {
            buckets[idx].push(ex);
        } else {
            uncategorized.push(ex);
        }
    }

    let mut blocks = Vec::new();
    for (i, mut bucket) in buckets
        .into_iter()
        .enumerate()
        .filter(|(_, b)| !b.is_empty())
    {
        bucket.sort_by(|a, b| {
            a.difficulty.cmp(&b.difficulty).then_with(|| {
                let ma = subject_mastery.get(a.subject.as_str()).unwrap_or(&0.0);
                let mb = subject_mastery.get(b.subject.as_str()).unwrap_or(&0.0);
                ma.partial_cmp(mb).unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        blocks.push(ChapterBlock {
            chapter: &CHAPTERS[i],
            exercises: bucket,
        });
    }

    if !uncategorized.is_empty() {
        uncategorized.sort_by_key(|e| e.difficulty);
        blocks.push(ChapterBlock {
            chapter: &UNCATEGORIZED_CHAPTER,
            exercises: uncategorized,
        });
    }

    blocks
}

/// Groupe d'exercices appartenant à un chapitre, triés par difficulté puis priorité SRS.
#[derive(Debug)]
pub struct ChapterBlock<'a> {
    /// Métadonnées du chapitre
    pub chapter: &'a Chapter,
    /// Exercices ordonnés du chapitre
    pub exercises: Vec<&'a Exercise>,
}

/// Aplatit les blocs de chapitres en une liste linéaire pour le mode `watch`.
pub fn flatten_chapters<'a>(blocks: &[ChapterBlock<'a>]) -> Vec<&'a Exercise> {
    blocks
        .iter()
        .flat_map(|b| b.exercises.iter().copied())
        .collect()
}

/// Informations de position d'un exercice dans sa progression par chapitre.
pub struct ChapterContext {
    /// Numéro du chapitre courant
    pub chapter_number: u8,
    /// Intitulé du chapitre courant
    pub chapter_title: &'static str,
    /// Nombre total de chapitres dans la progression
    pub total_chapters: u8,
    /// Position de l'exercice dans son chapitre (1-based)
    pub exercise_in_chapter: usize,
    /// Nombre total d'exercices dans ce chapitre
    pub chapter_size: usize,
}

/// Get chapter context for a given exercise index in the flattened list.
#[allow(dead_code)]
pub fn chapter_context_at(blocks: &[ChapterBlock], flat_index: usize) -> ChapterContext {
    let mut offset = 0;
    for block in blocks {
        if flat_index < offset + block.exercises.len() {
            return ChapterContext {
                chapter_number: block.chapter.number,
                chapter_title: block.chapter.title,
                total_chapters: u8::try_from(CHAPTERS.len())
                    .expect("compile-time assert guarantees CHAPTERS.len() ≤ 255"),
                exercise_in_chapter: flat_index - offset + 1,
                chapter_size: block.exercises.len(),
            };
        }
        offset += block.exercises.len();
    }
    // Fallback
    ChapterContext {
        chapter_number: 0,
        chapter_title: "???",
        total_chapters: u8::try_from(CHAPTERS.len())
            .expect("compile-time assert guarantees CHAPTERS.len() ≤ 255"),
        exercise_in_chapter: 0,
        chapter_size: 0,
    }
}

/// Filtre les blocs par numéro de chapitre. Retourne `false` si le résultat est vide.
pub fn filter_by_chapter(blocks: &mut Vec<ChapterBlock<'_>>, chapter: Option<u8>) -> bool {
    if let Some(n) = chapter {
        blocks.retain(|b| b.chapter.number == n);
        return !blocks.is_empty();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chapters_not_empty() {
        assert!(!CHAPTERS.is_empty(), "CHAPTERS array should not be empty");
    }

    #[test]
    fn test_all_chapters_have_subjects() {
        for chapter in CHAPTERS {
            assert!(
                !chapter.subjects.is_empty(),
                "Chapter {} '{}' has no subjects",
                chapter.number,
                chapter.title
            );
        }
    }

    #[test]
    fn test_flatten_chapters_preserves_order() {
        let ex1 = Exercise {
            id: "ex1".to_string(),
            subject: "structs".to_string(),
            lang: crate::models::Lang::C,
            difficulty: crate::models::Difficulty::Easy,
            title: "Exercise 1".to_string(),
            description: "Test".to_string(),
            starter_code: "".to_string(),
            solution_code: "".to_string(),
            hints: vec![],
            validation: crate::models::ValidationConfig {
                expected_output: Some("test".to_string()),
                ..Default::default()
            },
            prerequisites: vec![],
            files: vec![],
            exercise_type: Default::default(),
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Default::default(),
        };

        let ex2 = Exercise {
            id: "ex2".to_string(),
            subject: "pointers".to_string(),
            ..ex1.clone()
        };

        let blocks = vec![ChapterBlock {
            chapter: &CHAPTERS[0], // Fondamentaux C
            exercises: vec![&ex1, &ex2],
        }];

        let flattened = flatten_chapters(&blocks);
        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].id, "ex1");
        assert_eq!(flattened[1].id, "ex2");
    }

    #[test]
    fn test_chapter_context_at_valid() {
        let ex1 = Exercise {
            id: "ex1".to_string(),
            subject: "structs".to_string(),
            lang: crate::models::Lang::C,
            difficulty: crate::models::Difficulty::Easy,
            title: "Exercise 1".to_string(),
            description: "Test".to_string(),
            starter_code: "".to_string(),
            solution_code: "".to_string(),
            hints: vec![],
            validation: crate::models::ValidationConfig {
                expected_output: Some("test".to_string()),
                ..Default::default()
            },
            prerequisites: vec![],
            files: vec![],
            exercise_type: Default::default(),
            key_concept: None,
            common_mistake: None,
            kc_ids: vec![],
            starter_code_stages: vec![],
            visualizer: Default::default(),
        };

        let ex2 = Exercise {
            id: "ex2".to_string(),
            ..ex1.clone()
        };

        let blocks = vec![ChapterBlock {
            chapter: &CHAPTERS[0], // Fondamentaux C, chapter_number = 1
            exercises: vec![&ex1, &ex2],
        }];

        let ctx = chapter_context_at(&blocks, 0);
        assert_eq!(ctx.chapter_number, 1);
        assert_eq!(ctx.chapter_title, "Fondamentaux C");
        assert_eq!(ctx.exercise_in_chapter, 1);
        assert_eq!(ctx.chapter_size, 2);
        assert_eq!(
            ctx.total_chapters,
            u8::try_from(CHAPTERS.len())
                .expect("compile-time assert guarantees CHAPTERS.len() ≤ 255")
        );

        let ctx = chapter_context_at(&blocks, 1);
        assert_eq!(ctx.chapter_number, 1);
        assert_eq!(ctx.exercise_in_chapter, 2);
    }

    #[test]
    fn test_chapter_context_at_invalid() {
        let blocks = vec![];
        let ctx = chapter_context_at(&blocks, 0);
        assert_eq!(ctx.chapter_number, 0);
        assert_eq!(ctx.chapter_title, "???");
        assert_eq!(ctx.exercise_in_chapter, 0);
        assert_eq!(ctx.chapter_size, 0);
    }
}
