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
