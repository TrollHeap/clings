//! Application state and event handling (TEA/Elm architecture).
//!
//! Phase 1: Core app structure without full rendering logic.

use std::path::PathBuf;
use std::time::Instant;

use crate::error::Result;

/// Mode d'affichage de l'application
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppMode {
    Watch {
        chapter: Option<u8>,
    },
    Piscine {
        chapter: Option<u8>,
        timed: Option<u64>,
    },
}

/// Messages traités par App::update()
#[allow(dead_code)]
#[derive(Debug)]
pub enum Msg {
    Key(ratatui::crossterm::event::KeyEvent),
    FileChanged(PathBuf),
    RunComplete,
    Tick,
    Quit,
}

/// État centralisé de l'application TUI
#[allow(dead_code)]
pub struct AppState {
    pub mode: AppMode,
    pub should_quit: bool,
    pub current_index: usize,
    pub hint_shown: bool,
    pub vis_active: bool,
    pub vis_step: usize,
    pub consecutive_failures: u8,
    pub already_recorded: bool,
    pub piscine_deadline: Option<Instant>,
}

#[allow(dead_code)]
impl AppState {
    pub fn new(mode: AppMode) -> Self {
        Self {
            mode,
            should_quit: false,
            current_index: 0,
            hint_shown: false,
            vis_active: false,
            vis_step: 0,
            consecutive_failures: 0,
            already_recorded: false,
            piscine_deadline: None,
        }
    }
}

/// Application TUI principale (TEA pattern)
#[allow(dead_code)]
pub struct App {
    pub state: AppState,
}

#[allow(dead_code)]
impl App {
    pub fn new(mode: AppMode) -> Self {
        Self {
            state: AppState::new(mode),
        }
    }

    /// Boucle principale TEA : handle_events → draw → repeat
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        use ratatui::crossterm::event::{self, Event, KeyEventKind};
        use std::time::Duration;

        loop {
            terminal.draw(|f| self.view(f))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.update(Msg::Key(key));
                    }
                }
            }

            if self.state.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Dispatch des messages vers l'état
    pub fn update(&mut self, msg: Msg) {
        use ratatui::crossterm::event::KeyCode;
        match msg {
            Msg::Key(key) => match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.state.should_quit = true;
                }
                _ => {}
            },
            Msg::Quit => {
                self.state.should_quit = true;
            }
            _ => {}
        }
    }

    /// Rendu (placeholder Phase 1 — sera remplacé en Phase 2)
    fn view(&self, f: &mut ratatui::Frame) {
        use ratatui::layout::{Constraint, Layout};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let area = f.area();
        let [header, body] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

        f.render_widget(
            Block::bordered().title("clings v3.0 — Migration Ratatui"),
            header,
        );
        f.render_widget(
            Paragraph::new("Phase 1 scaffold — appuyer sur [q] pour quitter")
                .block(Block::new().borders(Borders::ALL)),
            body,
        );
    }
}
