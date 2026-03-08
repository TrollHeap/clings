use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Difficulty {
    Easy = 1,
    Medium = 2,
    Hard = 3,
}

impl TryFrom<u8> for Difficulty {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, String> {
        match v {
            1 => Ok(Difficulty::Easy),
            2 => Ok(Difficulty::Medium),
            3 => Ok(Difficulty::Hard),
            _ => Err(format!("invalid difficulty: {v}")),
        }
    }
}

impl From<Difficulty> for u8 {
    fn from(d: Difficulty) -> u8 {
        d as u8
    }
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "Easy"),
            Difficulty::Medium => write!(f, "Medium"),
            Difficulty::Hard => write!(f, "Hard"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Rust,
    C,
    Cpp,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lang::Rust => write!(f, "rust"),
            Lang::C => write!(f, "c"),
            Lang::Cpp => write!(f, "cpp"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationMode {
    Output,
    Test,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub mode: ValidationMode,
    pub expected_output: Option<String>,
    pub test_code: Option<String>,
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseType {
    #[default]
    Complete,
    FixBug,
    FillBlank,
    Refactor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseFile {
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub id: String,
    pub subject: String,
    pub lang: Lang,
    pub difficulty: Difficulty,
    pub title: String,
    pub description: String,
    pub starter_code: String,
    #[serde(skip_serializing)]
    pub solution_code: String,
    pub hints: Vec<String>,
    pub validation: ValidationConfig,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub files: Vec<ExerciseFile>,
    #[serde(default)]
    pub exercise_type: ExerciseType,
    #[serde(default)]
    pub key_concept: Option<String>,
    #[serde(default)]
    pub common_mistake: Option<String>,
    #[serde(default)]
    pub kc_ids: Vec<String>,
    #[serde(default)]
    pub starter_code_stages: Vec<String>,
    // Skip visualizer — not needed for CLI
    #[serde(default)]
    pub visualizer: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Subject {
    pub name: String,
    pub mastery_score: f64,
    pub last_practiced_at: Option<i64>,
    pub attempts_total: i64,
    pub attempts_success: i64,
    pub difficulty_unlocked: i32,
    pub next_review_at: Option<i64>,
    pub srs_interval_days: i64,
}

impl Subject {
    pub fn new(name: String) -> Self {
        Self {
            name,
            mastery_score: 0.0,
            last_practiced_at: None,
            attempts_total: 0,
            attempts_success: 0,
            difficulty_unlocked: 1,
            next_review_at: None,
            srs_interval_days: 1,
        }
    }
}
