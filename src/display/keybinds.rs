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
///
/// - `has_visualizer`: include `[v]` visualiser key.
/// - `has_nav`: include `[j]`/`[k]` nav keys and omit `[l]` list key (piscine mode).
pub fn show_keybinds_with_vis(has_visualizer: bool, has_nav: bool) {
    let mut binds: Vec<(char, &str)> = Vec::with_capacity(8);
    binds.push(('h', "hint"));
    if has_nav {
        binds.push(('j', "suivant"));
        binds.push(('k', "précédent"));
    }
    binds.push(('n', "skip"));
    if !has_nav {
        binds.push(('l', "list"));
    }
    binds.push(('r', "run"));
    if has_visualizer {
        binds.push(('v', "visualiser"));
    }
    binds.push(('q', "quit"));
    show_keybinds_list(&binds);
}
