//! Snapshot tests for TUI widgets using insta + ratatui TestBackend.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;

use clings::models::{
    Difficulty, Exercise, ExerciseType, Lang, ValidationConfig, ValidationMode, Visualizer,
};
use clings::runner::RunResult;
use clings::tui::app::ActiveOverlay;
use clings::tui::app::AppState;
use clings::tui::ui_watch;

/// Build a minimal exercise for testing.
fn make_exercise(id: &str, subject: &str, difficulty: Difficulty) -> Exercise {
    Exercise {
        id: id.to_owned(),
        subject: subject.to_owned(),
        lang: Lang::C,
        difficulty,
        title: format!("Exercise: {id}"),
        description: "Write a C program that prints Hello.".to_owned(),
        starter_code: "int main() { return 0; }".to_owned(),
        solution_code: "#include <stdio.h>\nint main() { puts(\"Hello\"); }".to_owned(),
        hints: vec!["Use puts()".to_owned(), "Don't forget stdio.h".to_owned()],
        validation: ValidationConfig {
            mode: ValidationMode::Output,
            expected_output: Some("Hello".to_owned()),
            max_duration_ms: None,
            test_code: None,
            expected_tests_pass: None,
        },
        prerequisites: vec![],
        files: vec![],
        exercise_type: ExerciseType::Complete,
        key_concept: Some("stdout".to_owned()),
        common_mistake: Some("Forgetting newline".to_owned()),
        kc_ids: vec![],
        starter_code_stages: vec![],
        visualizer: Visualizer::default(),
        libsys_module: None,
        libsys_function: None,
        libsys_unlock: None,
        header_code: None,
    }
}

/// Build an AppState with a single exercise for snapshot testing.
fn make_app_state() -> AppState {
    let exercise = make_exercise("hello-01", "pointers", Difficulty::Easy);
    let mut state = AppState::new();
    state.ex.exercises = vec![exercise];
    state.ex.completed = vec![false];
    state.ex.current_index = 0;
    state.progress.mastery_map = HashMap::from([("pointers".to_owned(), 2.5)]);
    state
}

#[test]
fn snapshot_watch_view_initial() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_initial", format!("{buffer:#?}"));
}

#[test]
fn snapshot_watch_view_with_compile_error() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.ex.run_result = Some(RunResult {
        success: false,
        stdout: String::new(),
        stderr: "error: expected ';' before '}' token".to_owned(),
        duration_ms: 42,
        compile_error: true,
        timeout: false,
        gcc_hint: Some("Point-virgule manquant — repérez la ligne indiquée par gcc".to_owned()),
    });

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_compile_error", format!("{buffer:#?}"));
}

#[test]
fn snapshot_watch_view_success() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.ex.run_result = Some(RunResult {
        success: true,
        stdout: "Hello".to_owned(),
        stderr: String::new(),
        duration_ms: 5,
        compile_error: false,
        timeout: false,
        gcc_hint: None,
    });
    state.ex.completed[0] = true;

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_success", format!("{buffer:#?}"));
}

#[test]
fn snapshot_watch_view_help_overlay() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.overlay.active = ActiveOverlay::Help;

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_help_overlay", format!("{buffer:#?}"));
}

#[test]
fn snapshot_watch_view_solution_overlay() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.overlay.active = ActiveOverlay::Solution;

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_solution_overlay", format!("{buffer:#?}"));
}

#[test]
fn snapshot_libsys_overlay_empty() {
    use clings::tui::ui_libsys;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.overlay.active = ActiveOverlay::Libsys;
    state.overlay.libsys_portfolio = vec![]; // Empty portfolio

    terminal
        .draw(|f| {
            ui_libsys::render_libsys_overlay(f, f.area(), &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("libsys_overlay_empty", format!("{buffer:#?}"));
}

#[test]
fn snapshot_libsys_overlay_with_modules() {
    use clings::libsys::{ExportedFn, ModuleStatus};
    use clings::tui::ui_libsys;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = make_app_state();
    state.overlay.active = ActiveOverlay::Libsys;

    // Populate with sample modules and functions
    state.overlay.libsys_portfolio = vec![
        ModuleStatus {
            name: "my_string".to_string(),
            functions: vec![
                ExportedFn {
                    name: "my_strdup".to_string(),
                    commit_hash: "abc12345".to_string(),
                },
                ExportedFn {
                    name: "my_strlen".to_string(),
                    commit_hash: "def67890".to_string(),
                },
            ],
            unlock_subject: None,
        },
        ModuleStatus {
            name: "my_process".to_string(),
            functions: vec![ExportedFn {
                name: "my_fork".to_string(),
                commit_hash: "ghi11111".to_string(),
            }],
            unlock_subject: Some("processes".to_string()),
        },
    ];

    terminal
        .draw(|f| {
            ui_libsys::render_libsys_overlay(f, f.area(), &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("libsys_overlay_with_modules", format!("{buffer:#?}"));
}
