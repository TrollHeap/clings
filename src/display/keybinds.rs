use colored::Colorize;

/// Render a keybind table from a list of (key char, description) pairs.
fn show_keybinds_list(binds: &[(char, &str)]) {
    print!("  {} ", "Keys".bold().cyan());
    let mut first = true;
    for (key, desc) in binds {
        if !first {
            print!("  ");
        }
        print!("{} {}", format!("[{key}]").bold(), desc);
        first = false;
    }
    println!();
    println!();
}

/// Show keybind hints.
pub fn show_keybinds() {
    show_keybinds_list(&[
        ('h', "hint"),
        ('n', "skip"),
        ('q', "quit"),
        ('l', "list"),
        ('r', "run"),
    ]);
}

/// Show keybind hints for piscine mode (no [l] list, shows [j] next + [k] prev).
pub fn show_keybinds_piscine(has_visualizer: bool) {
    if has_visualizer {
        show_keybinds_list(&[
            ('h', "hint"),
            ('j', "suivant"),
            ('k', "précédent"),
            ('n', "skip"),
            ('r', "run"),
            ('v', "visualiser"),
            ('q', "quit"),
        ]);
    } else {
        show_keybinds_list(&[
            ('h', "hint"),
            ('j', "suivant"),
            ('k', "précédent"),
            ('n', "skip"),
            ('r', "run"),
            ('q', "quit"),
        ]);
    }
}

/// Show keybind hints with optional visualizer key.
pub fn show_keybinds_with_vis(has_visualizer: bool) {
    if has_visualizer {
        show_keybinds_list(&[
            ('h', "hint"),
            ('n', "skip"),
            ('q', "quit"),
            ('l', "list"),
            ('r', "run"),
            ('v', "visualiser"),
        ]);
    } else {
        show_keybinds();
    }
}
