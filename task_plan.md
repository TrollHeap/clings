# task_plan.md — Modale de confirmation `q` + retour au launcher

## Contexte

Actuellement, `q` en session watch/piscine quitte immédiatement le programme.
L'utilisateur veut : `q` → modale "Revenir au menu ?" → (o/Enter) retour au launcher, (autre) rester.

Deux changements couplés :

1. **Modale `quit_confirm`** dans les sessions (même pattern que `nav_confirm`)
2. **Loop dans `main.rs`** pour retourner au launcher après chaque fin de session

---

## Fichiers à modifier

| Fichier                 | Rôle                                                                 |
| ----------------------- | -------------------------------------------------------------------- |
| `src/tui/app.rs`        | Ajouter `quit_confirm_active` à OverlayState, modifier handlers Quit |
| `src/tui/overlays.rs`   | Ajouter `render_quit_confirm_overlay`                                |
| `src/tui/ui_watch.rs`   | Appeler `render_quit_confirm_overlay` (dernier, au-dessus de tout)   |
| `src/tui/ui_piscine.rs` | Idem                                                                 |
| `src/main.rs`           | Wrapper le dispatch en `loop { ... }`                                |

---

## T1 : `src/tui/app.rs` — OverlayState + handlers

### T1a : Ajouter `quit_confirm_active` à OverlayState (après `nav_confirm_next`, ~ligne 141)

```rust
/// Modal de confirmation avant de quitter la session.
pub quit_confirm_active: bool,
```

### T1b : `handle_overlay_dispatch` (~ligne 903) — intercepter avant nav_confirm

Ajouter EN PREMIER dans la fonction (avant le bloc `nav_confirm_active`) :

```rust
if self.state.overlay.quit_confirm_active {
    use ratatui::crossterm::event::KeyCode;
    self.state.overlay.quit_confirm_active = false;
    if matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter) {
        self.state.session.should_quit = true;
    }
    return true;
}
```

### T1c : `update_watch` (~ligne 1218) — ne plus quitter directement

```rust
// AVANT
Command::Quit => self.state.session.should_quit = true,

// APRÈS
Command::Quit => self.state.overlay.quit_confirm_active = true,
```

### T1d : `update_piscine` (~ligne 1344) — checkpoint avant modale

```rust
// AVANT
Command::Quit => {
    let idx = self.state.ex.current_index;
    self.save_checkpoint(conn, session_id, idx);
    self.state.session.should_quit = true;
}

// APRÈS
Command::Quit => {
    let idx = self.state.ex.current_index;
    self.save_checkpoint(conn, session_id, idx);
    self.state.overlay.quit_confirm_active = true;
}
```

---

## T2 : `src/tui/overlays.rs` — `render_quit_confirm_overlay`

Ajouter après `render_nav_confirm_overlay` (~ligne 465) :

```rust
pub fn render_quit_confirm_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_popup(area, 38, 28);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::raw(""),
        Line::styled(
            "La session sera interrompue.",
            Style::default().fg(C_TEXT_DIM),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("[o] ", Style::default().fg(C_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled("retour au menu   ", Style::default().fg(C_TEXT)),
            Span::styled("[autre] ", Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD)),
            Span::styled("continuer", Style::default().fg(C_TEXT)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(span!(
                        Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD);
                        "Quitter la session ?"
                    ))
                    .style(Style::default().bg(C_SURFACE))
                    .border_style(Style::default().fg(C_WARNING)),
            )
            .alignment(Alignment::Center),
        popup,
    );
}
```

Re-exporté via `common.rs` automatiquement si `common.rs` fait `pub use overlays::*`.

---

## T3 : `src/tui/ui_watch.rs` — Rendu modale

Ajouter APRÈS le bloc `nav_confirm_active` (~ligne 53) :

```rust
if state.overlay.quit_confirm_active {
    common::render_quit_confirm_overlay(f, body_area);
}
```

**Ordre final (bas → haut) :**

1. body / active_overlay
2. success_overlay
3. nav_confirm_active
4. quit_confirm_active ← au-dessus de tout

---

## T4 : `src/tui/ui_piscine.rs` — Rendu modale

Ajouter après `success_overlay` (~ligne 65) :

```rust
if state.overlay.quit_confirm_active {
    common::render_quit_confirm_overlay(f, body_rest);
}
```

---

## T5 : `src/main.rs` — Loop retour au launcher

Wrapper le dispatch `None =>` en `loop { ... break }`. Chaque branche session devient `...?` (propage erreurs), `Quit => break`.

```rust
None => (|| {
    let conn = progress::open_db()?;
    loop {
        match tui::ui_launcher::select_launch(&conn)? {
            tui::ui_launcher::LaunchChoice::Continue => {
                let (mode, chapter, _index) = progress::load_last_session(&conn)?
                    .unwrap_or_else(|| ("watch".to_string(), None, 0));
                match mode.as_str() {
                    "piscine" => piscine::cmd_piscine(chapter, None)?,
                    _ => cmd_watch(chapter, false)?,
                }
            }
            tui::ui_launcher::LaunchChoice::Start {
                mode: tui::ui_launcher::LaunchMode::Watch,
                chapter,
            } => cmd_watch(chapter, false)?,
            tui::ui_launcher::LaunchChoice::Start {
                mode: tui::ui_launcher::LaunchMode::Piscine,
                chapter,
            } => piscine::cmd_piscine(chapter, None)?,
            tui::ui_launcher::LaunchChoice::Start {
                mode: tui::ui_launcher::LaunchMode::Nsy103,
                chapter: _,
            } => cmd_watch(None, true)?,
            tui::ui_launcher::LaunchChoice::Start {
                mode: tui::ui_launcher::LaunchMode::ExamNsy103,
                chapter: _,
            } => exam::cmd_exam(None, false)?,
            tui::ui_launcher::LaunchChoice::Quit => break,
        }
    }
    Ok(())
})()
```

---

## Vérification

```bash
cc-run build cargo build
cc-run tests cargo test
cargo clippy -- -D warnings
```

### Scénarios manuels

1. **Watch → `q` → Esc** : modale apparaît, Esc → reste dans la session
2. **Watch → `q` → `o`** : modale → confirme → retour au launcher (4 modes)
3. **Piscine → `q` → `o`** : checkpoint sauvé → retour launcher
4. **Piscine → `q` → Esc** : reste en piscine
5. **Depuis launcher → `q`** : quitte le programme (`LaunchChoice::Quit` inchangé)
6. **Ctrl+C en session** : quitte sans modale (comportement conservé)
7. **Fin naturelle d'une session** : retour automatique au launcher via loop

### Edge cases

- `quit_confirm` + `nav_confirm` simultanés : impossible (quit_confirm intercepte en premier, `return true`)
- `cargo run -- watch` (subcommande directe) : ne passe pas par le loop, quitte après session (inchangé)
