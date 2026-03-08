use crate::models::{Exercise, Subject};

/// A chapter in the NSY103 learning path.
#[derive(Debug, Clone)]
pub struct Chapter {
    pub number: u8,
    pub title: &'static str,
    pub subjects: &'static [&'static str],
}

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
        title: "Allocation mémoire",
        subjects: &["memory_allocation"],
    },
    Chapter {
        number: 4,
        title: "Entrées/sorties",
        subjects: &["file_io"],
    },
    Chapter {
        number: 5,
        title: "Processus",
        subjects: &["processes"],
    },
    Chapter {
        number: 6,
        title: "Signaux",
        subjects: &["signals"],
    },
    Chapter {
        number: 7,
        title: "Tubes",
        subjects: &["pipes"],
    },
    Chapter {
        number: 8,
        title: "Files de messages",
        subjects: &["message_queues"],
    },
    Chapter {
        number: 9,
        title: "Mémoire partagée",
        subjects: &["shared_memory"],
    },
    Chapter {
        number: 10,
        title: "Sémaphores",
        subjects: &["semaphores"],
    },
    Chapter {
        number: 11,
        title: "Threads POSIX",
        subjects: &["pthreads"],
    },
    Chapter {
        number: 12,
        title: "Sockets",
        subjects: &["sockets"],
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
        .map(|s| (s.name.as_str(), s.mastery_score))
        .collect();

    let mut blocks = Vec::new();

    for chapter in CHAPTERS {
        let mut chapter_exercises: Vec<&'a Exercise> = exercises
            .iter()
            .filter(|e| chapter.subjects.iter().any(|&s| s == e.subject))
            .collect();

        chapter_exercises.sort_by(|a, b| {
            a.difficulty.cmp(&b.difficulty).then_with(|| {
                let ma = subject_mastery.get(a.subject.as_str()).unwrap_or(&0.0);
                let mb = subject_mastery.get(b.subject.as_str()).unwrap_or(&0.0);
                ma.partial_cmp(mb).unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        if !chapter_exercises.is_empty() {
            blocks.push(ChapterBlock {
                chapter,
                exercises: chapter_exercises,
            });
        }
    }

    // Exercises not in any chapter go at the end
    let known_subjects: std::collections::HashSet<&str> = CHAPTERS
        .iter()
        .flat_map(|ch| ch.subjects.iter().copied())
        .collect();

    let mut uncategorized: Vec<&'a Exercise> = exercises
        .iter()
        .filter(|e| !known_subjects.contains(e.subject.as_str()))
        .collect();

    if !uncategorized.is_empty() {
        uncategorized.sort_by_key(|e| e.difficulty);
        blocks.push(ChapterBlock {
            chapter: &UNCATEGORIZED_CHAPTER,
            exercises: uncategorized,
        });
    }

    blocks
}

/// A chapter with its ordered exercises.
#[derive(Debug)]
pub struct ChapterBlock<'a> {
    pub chapter: &'a Chapter,
    pub exercises: Vec<&'a Exercise>,
}

/// Flatten chapter blocks into a single ordered list for watch mode.
pub fn flatten_chapters<'a>(blocks: &[ChapterBlock<'a>]) -> Vec<&'a Exercise> {
    blocks
        .iter()
        .flat_map(|b| b.exercises.iter().copied())
        .collect()
}

/// Info about the current exercise's chapter position.
pub struct ChapterContext {
    pub chapter_number: u8,
    pub chapter_title: String,
    pub total_chapters: u8,
    pub exercise_in_chapter: usize,
    pub chapter_size: usize,
}

/// Get chapter context for a given exercise index in the flattened list.
pub fn chapter_context_at(blocks: &[ChapterBlock], flat_index: usize) -> ChapterContext {
    let mut offset = 0;
    for block in blocks {
        if flat_index < offset + block.exercises.len() {
            return ChapterContext {
                chapter_number: block.chapter.number,
                chapter_title: block.chapter.title.to_string(),
                total_chapters: CHAPTERS.len() as u8,
                exercise_in_chapter: flat_index - offset + 1,
                chapter_size: block.exercises.len(),
            };
        }
        offset += block.exercises.len();
    }
    // Fallback
    ChapterContext {
        chapter_number: 0,
        chapter_title: "???".to_string(),
        total_chapters: CHAPTERS.len() as u8,
        exercise_in_chapter: 0,
        chapter_size: 0,
    }
}
