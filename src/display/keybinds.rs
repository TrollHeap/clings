//! Keybind help display — renders a compact keybind reference table.

use colored::Colorize;

/// Show keybind hints.
///
/// - `has_visualizer`: include `[v]` visualiser key.
/// - `has_nav`: include `[j]`/`[k]` nav keys.
/// - `has_list`: include `[l]` list key.
pub fn show_keybinds_with_vis(has_visualizer: bool, has_nav: bool, has_list: bool) {
    let mut binds: Vec<(char, &str)> = Vec::with_capacity(9);
    binds.push(('h', "hint"));
    if has_nav {
        binds.push(('j', "suivant"));
        binds.push(('k', "précédent"));
    }
    binds.push(('n', "skip"));
    if has_list {
        binds.push(('l', "list"));
    }
    binds.push(('r', "run"));
    if has_visualizer {
        binds.push(('v', "visualiser"));
    }
    binds.push(('q', "quit"));

    print!("  {} ", "Touches".bold().cyan());
    let mut first = true;
    for (key, desc) in &binds {
        if !first {
            print!("  ");
        }
        print!("{} {}", format!("[{key}]").bold(), desc);
        first = false;
    }
    println!();
    println!();
}
