//! Snapshot tests for TUI widgets using insta + ratatui TestBackend.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;

use clings::models::{
    Difficulty, Exercise, ExerciseType, Lang, ValidationConfig, ValidationMode, Visualizer,
};
use clings::runner::RunResult;
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
    }
}

/// Build an AppState with a single exercise for snapshot testing.
fn make_app_state() -> AppState {
    let exercise = make_exercise("hello-01", "pointers", Difficulty::Easy);
    let mut state = AppState::new();
    state.exercises = vec![exercise];
    state.completed = vec![false];
    state.current_index = 0;
    state.mastery_map = HashMap::from([("pointers".to_owned(), 2.5)]);
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
    state.run_result = Some(RunResult {
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
    state.run_result = Some(RunResult {
        success: true,
        stdout: "Hello".to_owned(),
        stderr: String::new(),
        duration_ms: 5,
        compile_error: false,
        timeout: false,
        gcc_hint: None,
    });
    state.completed[0] = true;

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
    state.overlay.help_active = true;

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
    state.overlay.solution_active = true;

    terminal
        .draw(|f| {
            ui_watch::view(f, &mut state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    insta::assert_snapshot!("watch_view_solution_overlay", format!("{buffer:#?}"));
}
