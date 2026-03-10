use thiserror::Error;

/// Erreurs applicatives de clings.
#[derive(Debug, Error)]
pub enum KfError {
    /// Erreur SQLite propagée depuis rusqlite
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Erreur d'entrée/sortie standard
    #[error("{0}")]
    Io(#[from] std::io::Error),

    /// Erreur du système de surveillance de fichiers (`notify`)
    #[error("watcher error: {0}")]
    Watch(#[from] notify::Error),

    /// L'identifiant d'exercice demandé est introuvable
    #[error("exercise not found: {0}")]
    ExerciseNotFound(String),

    /// Erreur de configuration ou de chemin (message libre)
    #[error("{0}")]
    Config(String),
}

/// Alias de résultat avec `KfError` comme type d'erreur.
pub type Result<T> = std::result::Result<T, KfError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exercise_not_found_display() {
        let e = KfError::ExerciseNotFound("ptr-deref-42".to_string());
        assert_eq!(e.to_string(), "exercise not found: ptr-deref-42");
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
        assert!(kf_err.to_string().contains("file missing"));
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
