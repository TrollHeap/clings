//! Application error type (`KfError`) and crate-level `Result` alias.

use thiserror::Error;

/// Erreurs applicatives de clings — enum des variantes possibles.
#[derive(Debug, Error)]
pub enum KfError {
    /// Erreur SQLite (DB verrouillée, table manquante, etc.).
    #[error("erreur de base de données : {0}")]
    Database(#[from] rusqlite::Error),

    /// Erreur d'entrée/sortie (fichier manquant, permission refusée, etc.).
    #[error("erreur I/O : {0}")]
    Io(#[from] std::io::Error),

    /// Erreur du file watcher (notify).
    #[error("erreur de surveillance fichier : {0}")]
    Watch(String),

    /// Exercice demandé introuvable (ID invalide).
    #[error("exercice introuvable : {0}")]
    ExerciseNotFound(String),

    /// Erreur de désérialisation JSON (exo invalide, annales mal formées).
    #[error("erreur JSON : {0}")]
    Json(#[from] serde_json::Error),

    /// Erreur de configuration ou chemin (message libre — HOME absent, path invalide, etc.).
    #[error("{0}")]
    Config(String),

    /// Erreur de synchronisation Git (clone, pull, push échoué).
    #[error("erreur de synchronisation : {0}")]
    Sync(String),
}

/// Alias de résultat avec `KfError` comme type d'erreur.
pub type Result<T> = std::result::Result<T, KfError>;

impl From<notify::Error> for KfError {
    fn from(e: notify::Error) -> Self {
        KfError::Watch(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exercise_not_found_display() {
        let e = KfError::ExerciseNotFound("ptr-deref-42".to_string());
        assert_eq!(e.to_string(), "exercice introuvable : ptr-deref-42");
    }

    #[test]
    fn test_config_error_display() {
        let e = KfError::Config("Variable $HOME non définie".to_string());
        assert_eq!(e.to_string(), "Variable $HOME non définie");
    }

    #[test]
    fn test_io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let kf_err = KfError::from(io_err);
        assert!(matches!(kf_err, KfError::Io(_)));
        assert!(kf_err.to_string().contains("erreur I/O"));
    }

    #[test]
    fn test_io_error_into_kferror() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let kf_err: KfError = io_err.into();
        assert!(matches!(kf_err, KfError::Io(_)));
    }

    #[test]
    fn test_config_error_is_not_io() {
        let e = KfError::Config("bad config".to_string());
        assert!(!matches!(e, KfError::Io(_)));
        assert!(!matches!(e, KfError::ExerciseNotFound(_)));
    }
}
