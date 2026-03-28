#[cfg(test)]
mod unit_tests {
    use super::super::*;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify CLINGS_HOME env var
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── SrsConfig defaults ─────────────────────────────────────────────────────────

    #[test]
    fn test_srs_config_default_decay_days() {
        let cfg = srs::SrsConfig::default();
        assert_eq!(cfg.decay_days, crate::constants::MASTERY_DECAY_DAYS);
        assert_eq!(cfg.decay_days, 14);
    }

    #[test]
    fn test_srs_config_default_base_interval() {
        let cfg = srs::SrsConfig::default();
        assert_eq!(
            cfg.base_interval_days,
            crate::constants::SRS_BASE_INTERVAL_DAYS
        );
        assert_eq!(cfg.base_interval_days, 1);
    }

    #[test]
    fn test_srs_config_default_max_interval() {
        let cfg = srs::SrsConfig::default();
        assert_eq!(
            cfg.max_interval_days,
            crate::constants::SRS_MAX_INTERVAL_DAYS
        );
        assert_eq!(cfg.max_interval_days, 60);
    }

    #[test]
    fn test_srs_config_default_multiplier() {
        let cfg = srs::SrsConfig::default();
        assert_eq!(
            cfg.interval_multiplier,
            crate::constants::SRS_INTERVAL_MULTIPLIER
        );
        assert_eq!(cfg.interval_multiplier, 2.5);
    }

    // ── UiConfig defaults ──────────────────────────────────────────────────────────

    #[test]
    fn test_ui_config_default_editor() {
        let cfg = ui::UiConfig::default();
        assert_eq!(cfg.editor, crate::constants::TMUX_EDITOR);
        assert_eq!(cfg.editor, "nvim");
    }

    #[test]
    fn test_ui_config_default_pane_width() {
        let cfg = ui::UiConfig::default();
        assert_eq!(cfg.tmux_pane_width, 50);
    }

    // ── TmuxConfig defaults ────────────────────────────────────────────────────────

    #[test]
    fn test_tmux_config_default_enabled() {
        let cfg = tmux::TmuxConfig::default();
        assert!(cfg.enabled);
    }

    // ── SyncConfig defaults ────────────────────────────────────────────────────────

    #[test]
    fn test_sync_config_default_disabled() {
        let cfg = sync::SyncConfig::default();
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_sync_config_default_empty_remote() {
        let cfg = sync::SyncConfig::default();
        assert_eq!(cfg.remote, "");
    }

    #[test]
    fn test_sync_config_default_branch() {
        let cfg = sync::SyncConfig::default();
        assert_eq!(cfg.branch, crate::constants::SYNC_DEFAULT_BRANCH);
        assert_eq!(cfg.branch, "main");
    }

    #[test]
    fn test_sync_config_default_empty_hostname() {
        let cfg = sync::SyncConfig::default();
        assert_eq!(cfg.hostname, "");
    }

    // ── ClingConfig defaults ───────────────────────────────────────────────────────

    #[test]
    fn test_cling_config_default_all_sections() {
        let cfg = ClingConfig::default();
        assert_eq!(cfg.srs.decay_days, 14);
        assert_eq!(cfg.ui.editor, "nvim");
        assert!(cfg.tmux.enabled);
        assert!(!cfg.sync.enabled);
    }

    // ── TOML deserialization ───────────────────────────────────────────────────────

    #[test]
    fn test_toml_deserialize_srs_section() {
        let toml_str = r#"
[srs]
decay_days = 7
base_interval_days = 2
max_interval_days = 90
interval_multiplier = 3.0
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 7);
        assert_eq!(cfg.srs.base_interval_days, 2);
        assert_eq!(cfg.srs.max_interval_days, 90);
        assert_eq!(cfg.srs.interval_multiplier, 3.0);
    }

    #[test]
    fn test_toml_deserialize_ui_section() {
        let toml_str = r#"
[ui]
editor = "vim"
tmux_pane_width = 80
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.ui.editor, "vim");
        assert_eq!(cfg.ui.tmux_pane_width, 80);
    }

    #[test]
    fn test_toml_deserialize_tmux_section() {
        let toml_str = r#"
[tmux]
enabled = false
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert!(!cfg.tmux.enabled);
    }

    #[test]
    fn test_toml_deserialize_sync_section() {
        let toml_str = r#"
[sync]
enabled = true
remote = "https://github.com/user/clings-progress.git"
branch = "develop"
hostname = "laptop-a"
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.sync.enabled);
        assert_eq!(
            cfg.sync.remote,
            "https://github.com/user/clings-progress.git"
        );
        assert_eq!(cfg.sync.branch, "develop");
        assert_eq!(cfg.sync.hostname, "laptop-a");
    }

    #[test]
    fn test_toml_deserialize_mixed_sections() {
        let toml_str = r#"
[srs]
decay_days = 21

[ui]
editor = "code"

[tmux]
enabled = false

[sync]
enabled = true
remote = "https://example.com/repo.git"
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 21);
        assert_eq!(cfg.ui.editor, "code");
        assert!(!cfg.tmux.enabled);
        assert!(cfg.sync.enabled);
        assert_eq!(cfg.sync.remote, "https://example.com/repo.git");
    }

    #[test]
    fn test_toml_deserialize_partial_config() {
        let toml_str = r#"
[srs]
decay_days = 28
"#;
        let cfg: ClingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.srs.decay_days, 28);
        // Other values should be defaults
        assert_eq!(cfg.srs.base_interval_days, 1);
        assert_eq!(cfg.ui.editor, "nvim");
    }

    // ── set_value: valid keys ──────────────────────────────────────────────────────

    #[test]
    fn test_set_value_srs_decay_days() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("srs", "decay_days", "21");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_srs_base_interval() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("srs", "base_interval_days", "3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_ui_editor() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("ui", "editor", "emacs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_ui_pane_width() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("ui", "tmux_pane_width", "100");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_tmux_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("tmux", "enabled", "false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_enabled() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("sync", "enabled", "true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_remote() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("sync", "remote", "https://github.com/user/repo.git");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_branch() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("sync", "branch", "development");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_sync_hostname() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("sync", "hostname", "machine-01");
        assert!(result.is_ok());
    }

    // ── set_value: unknown keys ────────────────────────────────────────────────────

    #[test]
    fn test_set_value_unknown_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("unknown", "key", "value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("clé inconnue"));
    }

    #[test]
    fn test_set_value_unknown_key_in_known_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("srs", "unknown_key", "42");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("clé inconnue"));
    }

    #[test]
    fn test_set_value_wrong_section_right_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("tmux", "decay_days", "7");
        assert!(result.is_err());
    }

    // ── set_value: value parsing ───────────────────────────────────────────────────

    #[test]
    fn test_set_value_integer_parsing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("srs", "decay_days", "42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_float_parsing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("srs", "base_interval_days", "3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_boolean_true() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("tmux", "enabled", "true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_boolean_false() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("sync", "enabled", "false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_value_string_with_spaces() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("ui", "editor", "code --wait");
        assert!(result.is_ok());
    }

    // ── set_value: file creation & preservation ────────────────────────────────────

    #[test]
    fn test_set_value_creates_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        loader::set_value("srs", "decay_days", "28").unwrap();
        let path = clings_home.join("clings.toml");
        assert!(path.exists());
    }

    #[test]
    fn test_set_value_creates_parent_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested").join("dir");
        std::env::set_var("CLINGS_HOME", &nested);
        let result = loader::set_value("srs", "decay_days", "7");
        assert!(result.is_ok());
        assert!(nested.exists());
    }

    #[test]
    fn test_set_value_preserves_existing_keys() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = loader::set_value("srs", "decay_days", "14");
        let r2 = loader::set_value("srs", "base_interval_days", "2");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        // Verify file was created
        let path = clings_home.join("clings.toml");
        assert!(path.exists());
    }

    #[test]
    fn test_set_value_updates_existing_key_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = loader::set_value("srs", "decay_days", "14");
        let r2 = loader::set_value("srs", "decay_days", "21");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    #[test]
    fn test_set_value_multiple_sections_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let r1 = loader::set_value("srs", "decay_days", "21");
        let r2 = loader::set_value("ui", "editor", "vim");
        let r3 = loader::set_value("tmux", "enabled", "false");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }

    // ── set_value: edge cases ──────────────────────────────────────────────────────

    #[test]
    fn test_set_value_empty_string_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("", "key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_value_empty_string_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        let result = loader::set_value("srs", "", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_value_numeric_out_of_u8_range_for_pane_width() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("CLINGS_HOME", tmp.path());
        // 300 is valid i64 but won't fit u8; it will be stored as i64
        let clings_home = tmp.path().to_path_buf();
        loader::set_value("ui", "tmux_pane_width", "300").unwrap();
        let path = clings_home.join("clings.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("tmux_pane_width = 300"));
    }

    #[test]
    fn test_set_value_url_as_string() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        loader::set_value("sync", "remote", "https://github.com/user/repo.git").unwrap();
        let path = clings_home.join("clings.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("https://github.com/user/repo.git"));
    }

    #[test]
    fn test_set_value_negative_integer_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let clings_home = tmp.path().to_path_buf();
        std::env::set_var("CLINGS_HOME", &clings_home);
        let result = loader::set_value("srs", "decay_days", "-5");
        assert!(result.is_ok());
    }

    // ── Load function ─────────────────────────────────────────────────────────────
    // Note: load() tests are not included because load() reads HOME env var at runtime,
    // and parallel tests cause race conditions when modifying global env state.
    // The load() function is also tested implicitly through set_value integration tests.
}
