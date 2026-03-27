//! Migration one-shot : JSON → TOML pour les fichiers d'exercices clings.
//!
//! Usage :
//!   CLINGS_EXERCISES=./exercises cargo run --bin migrate-exercises
//!
//! Convertit tous les *.json (sauf annales_map.json et kc_error_map.json)
//! en *.toml dans le même répertoire.
//! Valide le round-trip TOML → Exercise pour chaque fichier converti.
//! Les fichiers JSON d'origine sont conservés ; supprimer manuellement après validation.

use std::path::{Path, PathBuf};

fn main() {
    let exercises_dir = std::env::var("CLINGS_EXERCISES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("exercises"));

    if !exercises_dir.is_dir() {
        eprintln!(
            "Erreur : répertoire introuvable — {}",
            exercises_dir.display()
        );
        eprintln!("Définir CLINGS_EXERCISES ou lancer depuis la racine du projet.");
        std::process::exit(1);
    }

    let mut converted = 0u32;
    let mut errors = 0u32;

    convert_dir(&exercises_dir, &mut converted, &mut errors);

    println!("\n── Résumé ──────────────────────────────────────");
    println!("  Convertis : {converted}");
    println!("  Erreurs   : {errors}");
    if errors == 0 {
        println!("  Tous les fichiers ont été migrés et validés.");
        println!("  Supprimer les JSON avec :");
        println!(
            "    fd -e json . {} --exclude annales_map.json --exclude kc_error_map.json -x rm {{}}",
            exercises_dir.display()
        );
    } else {
        println!("  Corriger les erreurs avant de supprimer les JSON.");
        std::process::exit(1);
    }
}

const SKIP_FILES: &[&str] = &["annales_map.json", "kc_error_map.json"];

fn convert_dir(dir: &Path, converted: &mut u32, errors: &mut u32) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Impossible de lire {} : {e}", dir.display());
            *errors += 1;
            return;
        }
    };

    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            convert_dir(&path, converted, errors);
        } else if path.extension().is_some_and(|e| e == "json") {
            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if SKIP_FILES.contains(&filename) {
                continue;
            }
            match convert_file(&path) {
                Ok(toml_path) => {
                    println!("  ✓  {}", toml_path.display());
                    *converted += 1;
                }
                Err(e) => {
                    eprintln!("  ✗  {} : {e}", path.display());
                    *errors += 1;
                }
            }
        }
    }
}

fn convert_file(json_path: &Path) -> Result<PathBuf, String> {
    let content = std::fs::read_to_string(json_path).map_err(|e| format!("lecture : {e}"))?;

    // Parse JSON comme valeur générique (préserve solution_code malgré skip_serializing)
    let json_val: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("JSON invalide : {e}"))?;

    // Convertir en toml::Value en omettant les null
    let toml_val = json_to_toml(json_val)
        .ok_or_else(|| "conversion JSON→TOML échouée (objet vide ?)".to_string())?;

    // Sérialiser en TOML (to_string_pretty génère des multiline strings pour les \n)
    let toml_str =
        toml::to_string_pretty(&toml_val).map_err(|e| format!("sérialisation TOML : {e}"))?;

    // Chemin de sortie : même nom, extension .toml
    let toml_path = json_path.with_extension("toml");

    // Valider round-trip : le TOML doit être parseable comme Exercise
    validate_roundtrip(&toml_str, json_path)?;

    std::fs::write(&toml_path, &toml_str)
        .map_err(|e| format!("écriture {} : {e}", toml_path.display()))?;

    Ok(toml_path)
}

fn validate_roundtrip(toml_str: &str, source: &Path) -> Result<(), String> {
    // Vérifier que le TOML se parse en Exercise sans erreur
    toml::from_str::<clings::models::Exercise>(toml_str).map_err(|e| {
        format!(
            "round-trip TOML→Exercise échoué pour {} : {e}",
            source.display()
        )
    })?;
    Ok(())
}

/// Convertit récursivement un `serde_json::Value` en `toml::Value`.
/// Les valeurs `null` et les objets/tableaux vides sont omis (retourne `None`).
fn json_to_toml(v: serde_json::Value) -> Option<toml::Value> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else {
                n.as_f64().map(toml::Value::Float)
            }
        }
        serde_json::Value::String(s) => Some(toml::Value::String(s)),
        serde_json::Value::Array(arr) => {
            let items: Vec<toml::Value> = arr.into_iter().filter_map(json_to_toml).collect();
            // Garder les tableaux vides (ex: hints=[], steps=[]) pour les champs requis
            Some(toml::Value::Array(items))
        }
        serde_json::Value::Object(obj) => {
            let mut map = toml::map::Map::new();
            for (k, v) in obj {
                if let Some(tv) = json_to_toml(v) {
                    map.insert(k, tv);
                }
            }
            if map.is_empty() {
                None
            } else {
                Some(toml::Value::Table(map))
            }
        }
    }
}
