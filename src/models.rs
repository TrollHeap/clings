//! Core data types: `Exercise`, `Subject`, `ValidationMode`, and supporting enums.

use serde::{Deserialize, Serialize};

/// Niveau de difficulté d'un exercice, de 1 (Facile) à 5 (Expert).
/// Déverrouillé progressivement via le score de maîtrise SRS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Difficulty {
    /// Niveau 1 — introduction au concept
    Easy = 1,
    /// Niveau 2 — déverrouillé à mastery ≥ 2.0
    Medium = 2,
    /// Niveau 3 — déverrouillé à mastery ≥ 4.0
    Hard = 3,
    /// Niveau 4 — déverrouillé à mastery ≥ 4.5
    Advanced = 4,
    /// Niveau 5 — déverrouillé à mastery = 5.0
    Expert = 5,
}

/// Erreur retournée par `Difficulty::try_from(u8)` pour une valeur hors [1, 5].
#[derive(Debug, thiserror::Error)]
#[error("difficulty invalide : {0} (attendu 1–5)")]
pub struct InvalidDifficultyError(pub u8);

impl TryFrom<u8> for Difficulty {
    type Error = InvalidDifficultyError;
    fn try_from(v: u8) -> Result<Self, InvalidDifficultyError> {
        match v {
            1 => Ok(Difficulty::Easy),
            2 => Ok(Difficulty::Medium),
            3 => Ok(Difficulty::Hard),
            4 => Ok(Difficulty::Advanced),
            5 => Ok(Difficulty::Expert),
            _ => Err(InvalidDifficultyError(v)),
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
            Difficulty::Advanced => write!(f, "Advanced"),
            Difficulty::Expert => write!(f, "Expert"),
        }
    }
}

/// Langage de programmation d'un exercice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Exercice en Rust
    Rust,
    /// Exercice en C
    C,
    /// Exercice en C++
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

/// Mode de validation d'un exercice.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Comparer stdout avec `expected_output` (défaut)
    #[default]
    Output,
    /// Exécuter le harnais de tests C et vérifier que tous les tests passent
    Test,
    /// Les deux : output ET tests doivent passer
    Both,
}

/// Configuration de validation d'un exercice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Mode de validation (output, test, both)
    #[serde(default)]
    pub mode: ValidationMode,
    /// Sortie attendue (comparaison stdout normalisée) ; supporte le préfixe `REGEX:`
    pub expected_output: Option<String>,
    /// Durée maximale d'exécution en millisecondes (remplace la limite globale de 10s)
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
    /// Code C du harnais de tests (inclus après `current.c` en mode Test/Both)
    #[serde(default)]
    pub test_code: Option<String>,
    /// Nombre de tests attendus (si None : tous les tests déclarés doivent passer)
    #[serde(default)]
    pub expected_tests_pass: Option<usize>,
}

/// Nature pédagogique d'un exercice.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseType {
    /// Écrire le code complet depuis le squelette fourni (défaut)
    #[default]
    Complete,
    /// Identifier et corriger un bug existant
    FixBug,
    /// Compléter les blancs laissés dans le code
    FillBlank,
    /// Réécrire le code en respectant des contraintes
    Refactor,
}

impl std::fmt::Display for ExerciseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExerciseType::Complete => write!(f, "Complete"),
            ExerciseType::FixBug => write!(f, "Fix Bug"),
            ExerciseType::FillBlank => write!(f, "Fill Blank"),
            ExerciseType::Refactor => write!(f, "Refactoring"),
        }
    }
}

/// Fichier auxiliaire fourni avec un exercice (en-tête, données…).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciseFile {
    /// Nom du fichier tel qu'il sera écrit dans `~/.clings/`
    pub name: String,
    /// Contenu textuel du fichier
    pub content: String,
    /// Si `true`, le fichier ne doit pas être modifié par l'apprenant
    #[serde(default)]
    pub readonly: bool,
}

/// Définition complète d'un exercice chargé depuis un fichier JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    /// Identifiant unique de l'exercice (ex. `ptr-deref-01`)
    pub id: String,
    /// Sujet auquel appartient l'exercice (ex. `pointers`, `signals`)
    pub subject: String,
    /// Langage de programmation de l'exercice
    pub lang: Lang,
    /// Niveau de difficulté
    pub difficulty: Difficulty,
    /// Titre court affiché dans la TUI
    pub title: String,
    /// Énoncé complet affiché à l'apprenant
    pub description: String,
    /// Code de départ écrit dans `~/.clings/current.c`
    pub starter_code: String,
    /// Corrigé (non sérialisé pour éviter la triche)
    #[serde(skip_serializing)]
    pub solution_code: String,
    /// Liste d'indices progressifs à débloquer avec `[h]`
    pub hints: Vec<String>,
    /// Règles de validation de la sortie ou des tests
    pub validation: ValidationConfig,
    /// Identifiants d'exercices requis avant celui-ci
    #[serde(default)]
    pub prerequisites: Vec<String>,
    /// Fichiers auxiliaires (en-têtes, données) copiés dans le répertoire de travail
    #[serde(default)]
    pub files: Vec<ExerciseFile>,
    /// Type pédagogique de l'exercice
    #[serde(default)]
    pub exercise_type: ExerciseType,
    /// Concept clé mis en avant par cet exercice
    #[serde(default)]
    pub key_concept: Option<String>,
    /// Erreur fréquente associée à cet exercice
    #[serde(default)]
    pub common_mistake: Option<String>,
    /// Identifiants de connaissances clés associées
    #[serde(default)]
    pub kc_ids: Vec<String>,
    /// Versions du code de départ par palier de maîtrise (S0–S4)
    #[serde(default)]
    pub starter_code_stages: Vec<String>,
    /// Visualiseur interactif d'étapes (stack/heap)
    #[serde(default)]
    pub visualizer: Visualizer,
}

/// Visualiseur d'exercice — séquence d'étapes annotées.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Visualizer {
    #[serde(rename = "type", default)]
    pub vis_type: String,
    #[serde(default)]
    pub steps: Vec<VisStep>,
}

/// Une étape du visualiseur avec snapshot mémoire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisStep {
    pub label: String,
    #[serde(default)]
    pub stack: Vec<VisVar>,
    #[serde(default)]
    pub heap: Vec<VisVar>,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub step_label: String,
}

/// Variable affichée dans le stack ou le heap.
/// Accepte `name` ou `address` pour le libellé, `value` ou `content` pour la donnée.
/// Les champs inconnus (arrows, call_frames, etc.) sont ignorés silencieusement.
#[derive(Debug, Clone, Serialize)]
pub struct VisVar {
    pub name: String,
    pub value: String,
}

impl<'de> serde::Deserialize<'de> for VisVar {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{IgnoredAny, MapAccess, Visitor};

        struct VisVarVisitor;

        impl<'de> Visitor<'de> for VisVarVisitor {
            type Value = VisVar;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "a VisVar object with name/address and value/content fields"
                )
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<VisVar, A::Error> {
                let mut name: Option<String> = None;
                let mut value: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            let v: String = map.next_value()?;
                            if name.is_none() {
                                name = Some(v);
                            }
                        }
                        "address" => {
                            let v: String = map.next_value()?;
                            if name.is_none() {
                                name = Some(v);
                            }
                        }
                        "value" => {
                            let v: String = map.next_value()?;
                            if value.is_none() {
                                value = Some(v);
                            }
                        }
                        "content" => {
                            let v: String = map.next_value()?;
                            if value.is_none() {
                                value = Some(v);
                            }
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                // unwrap_or_default: visualizer data is internal/best-effort —
                // missing name/value in JSON produces empty strings rather than a parse error.
                Ok(VisVar {
                    name: name.unwrap_or_default(),
                    value: value.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_map(VisVarVisitor)
    }
}

/// Score de maîtrise SRS, toujours dans [0.0, 5.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MasteryScore(f64);

impl MasteryScore {
    pub const FLOOR: f64 = 0.0;
    pub const CAP: f64 = 5.0;

    /// Construit en clampant `v` dans [0.0, 5.0].
    pub fn clamped(v: f64) -> Self {
        Self(v.clamp(Self::FLOOR, Self::CAP))
    }

    /// Accès à la valeur brute.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl Default for MasteryScore {
    fn default() -> Self {
        Self(0.0)
    }
}

impl std::fmt::Display for MasteryScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}", self.0)
    }
}

/// Intervalle SRS en jours, toujours ≥ 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SrsIntervalDays(i64);

impl SrsIntervalDays {
    pub const MIN: i64 = 1;

    /// Construit en clampant `v` à ≥ 1.
    pub fn clamped(v: i64) -> Self {
        Self(v.max(Self::MIN))
    }

    /// Accès à la valeur brute.
    pub fn get(self) -> i64 {
        self.0
    }
}

impl Default for SrsIntervalDays {
    fn default() -> Self {
        Self(1)
    }
}

fn deserialize_mastery_score<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<MasteryScore, D::Error> {
    let v = f64::deserialize(d)?;
    Ok(MasteryScore::clamped(v))
}

fn deserialize_srs_interval<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<SrsIntervalDays, D::Error> {
    let v = i64::deserialize(d)?;
    Ok(SrsIntervalDays::clamped(v))
}

/// État de maîtrise d'un sujet, persisté en base SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    /// Nom du sujet correspondant au champ `subject` des exercices
    pub name: String,
    /// Score SRS courant entre 0.0 et 5.0
    #[serde(deserialize_with = "deserialize_mastery_score")]
    pub mastery_score: MasteryScore,
    /// Horodatage Unix de la dernière pratique
    pub last_practiced_at: Option<i64>,
    /// Nombre total de tentatives enregistrées
    pub attempts_total: i64,
    /// Nombre de tentatives réussies
    pub attempts_success: i64,
    /// Niveau de difficulté maximal actuellement déverrouillé (1–5)
    pub difficulty_unlocked: i32,
    /// Horodatage Unix de la prochaine révision planifiée par le SRS
    pub next_review_at: Option<i64>,
    /// Intervalle SRS courant en jours
    #[serde(deserialize_with = "deserialize_srs_interval")]
    pub srs_interval_days: SrsIntervalDays,
}

impl Subject {
    /// Crée un sujet avec un score de maîtrise à zéro et la difficulté 1 déverrouillée.
    pub fn new(name: String) -> Self {
        Self {
            name,
            mastery_score: MasteryScore::default(),
            last_practiced_at: None,
            attempts_total: 0,
            attempts_success: 0,
            difficulty_unlocked: 1,
            next_review_at: None,
            srs_interval_days: SrsIntervalDays::default(),
        }
    }
}

/// Une question d'annale NSY103 avec le mapping vers les exercices clings.
#[derive(Debug, Deserialize)]
pub struct AnnaleQuestion {
    #[serde(default)]
    pub number: u32,
    pub points: f32,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub exercises: Vec<String>,
}

/// Une session d'annale (examen) NSY103/UTC502.
/// Utilisée à la fois pour l'affichage (`clings annales`) et le mode exam simulé.
#[derive(Debug, Deserialize)]
pub struct AnnaleSession {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub total_points: f32,
    pub questions: Vec<AnnaleQuestion>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_try_from_valid() {
        for v in 1..=5 {
            let result = Difficulty::try_from(v);
            assert!(result.is_ok(), "Difficulty::try_from({}) should succeed", v);
        }

        assert_eq!(Difficulty::try_from(1).unwrap(), Difficulty::Easy);
        assert_eq!(Difficulty::try_from(2).unwrap(), Difficulty::Medium);
        assert_eq!(Difficulty::try_from(3).unwrap(), Difficulty::Hard);
        assert_eq!(Difficulty::try_from(4).unwrap(), Difficulty::Advanced);
        assert_eq!(Difficulty::try_from(5).unwrap(), Difficulty::Expert);
    }

    #[test]
    fn test_difficulty_try_from_invalid() {
        assert!(
            Difficulty::try_from(0).is_err(),
            "Difficulty::try_from(0) should fail"
        );
        assert!(
            Difficulty::try_from(6).is_err(),
            "Difficulty::try_from(6) should fail"
        );
        assert!(
            Difficulty::try_from(255).is_err(),
            "Difficulty::try_from(255) should fail"
        );
    }

    #[test]
    fn test_difficulty_display() {
        assert_eq!(Difficulty::Easy.to_string(), "Easy");
        assert_eq!(Difficulty::Medium.to_string(), "Medium");
        assert_eq!(Difficulty::Hard.to_string(), "Hard");
        assert_eq!(Difficulty::Advanced.to_string(), "Advanced");
        assert_eq!(Difficulty::Expert.to_string(), "Expert");
    }

    #[test]
    fn test_difficulty_ordering() {
        assert!(Difficulty::Easy < Difficulty::Medium);
        assert!(Difficulty::Medium < Difficulty::Hard);
        assert!(Difficulty::Hard < Difficulty::Advanced);
        assert!(Difficulty::Advanced < Difficulty::Expert);
    }

    #[test]
    fn test_difficulty_roundtrip() {
        for original in [
            Difficulty::Easy,
            Difficulty::Medium,
            Difficulty::Hard,
            Difficulty::Advanced,
            Difficulty::Expert,
        ] {
            let u8_val: u8 = original.into();
            let recovered = Difficulty::try_from(u8_val).unwrap();
            assert_eq!(original, recovered);
        }
    }

    #[test]
    fn test_exercise_deserialize() {
        let json = r#"{
            "id": "test-ex-01",
            "subject": "pointers",
            "lang": "c",
            "difficulty": 1,
            "title": "Test Exercise",
            "description": "This is a test",
            "starter_code": "int main() { return 0; }",
            "solution_code": "int main() { printf(\"done\"); return 0; }",
            "hints": ["Hint 1", "Hint 2"],
            "validation": {
                "expected_output": "done"
            }
        }"#;

        let exercise: Exercise = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(exercise.id, "test-ex-01");
        assert_eq!(exercise.subject, "pointers");
        assert_eq!(exercise.lang, Lang::C);
        assert_eq!(exercise.difficulty, Difficulty::Easy);
        assert_eq!(exercise.title, "Test Exercise");
        assert_eq!(exercise.hints.len(), 2);
    }

    #[test]
    fn test_subject_new_defaults() {
        let subject = Subject::new("test_subject".to_owned());
        assert_eq!(subject.name, "test_subject");
        assert_eq!(subject.mastery_score.get(), 0.0);
        assert_eq!(subject.last_practiced_at, None);
        assert_eq!(subject.attempts_total, 0);
        assert_eq!(subject.attempts_success, 0);
        assert_eq!(subject.difficulty_unlocked, 1);
        assert_eq!(subject.next_review_at, None);
        assert_eq!(subject.srs_interval_days.get(), 1);
    }
}
