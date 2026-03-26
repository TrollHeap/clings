/// Build script — déclare les dépendances de recompilation pour les exercices embarqués.
///
/// Sans ce fichier, Cargo ne sait pas que les fichiers `.json` dans `exercises/`
/// sont des entrées du binaire (via rust-embed). Résultat : `cargo install` peut
/// produire un binaire avec des exercices périmés si seuls les JSON ont changé.
///
/// Ce script émet `cargo:rerun-if-changed` pour chaque fichier JSON, forçant
/// Cargo à recompiler dès qu'un exercice est ajouté, modifié ou supprimé.
fn main() {
    emit_rerun_for_dir(std::path::Path::new("exercises"));
}

fn emit_rerun_for_dir(dir: &std::path::Path) {
    // Surveille le répertoire lui-même (ajout/suppression de fichiers)
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_for_dir(&path);
        } else if path.extension().is_some_and(|e| e == "json") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
