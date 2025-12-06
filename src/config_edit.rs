use ncurses::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CATEGORY_PAIR: i16 = 1;
const PAIR_BLACK: i16 = 2;
const PAIR_RED: i16 = 3;
const PAIR_GREEN: i16 = 4;
const PAIR_YELLOW: i16 = 5;
const PAIR_BLUE: i16 = 6;
const PAIR_MAGENTA: i16 = 7;
const PAIR_CYAN: i16 = 8;
const PAIR_WHITE: i16 = 9;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Value {
    /// Free-text string
    Text {
        value: String,
    },

    /// Choice in a list of options
    Choice {
        options: Vec<String>,
        selected: usize,
    },

    /// Non-editable category/header
    Category,

    /// Color choice, works like Choice but rendered with ncurses colors
    Color {
        options: Vec<String>,
        selected: usize,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Entry {
    key: String,
    value: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    entries: Vec<Entry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            entries: vec![
                Entry {
                    key: "username".into(),
                    value: Value::Text {
                        value: "alice".into(),
                    },
                },
                Entry {
                    key: "visual parameters".into(),
                    value: Value::Category,
                },
                Entry {
                    key: "theme".into(),
                    value: Value::Choice {
                        options: vec!["light".into(), "dark".into(), "solarized".into()],
                        selected: 1,
                    },
                },
                Entry {
                    key: "theme color".into(),
                    value: Value::Color {
                        options: vec!["RED".into(), "BLUE".into(), "BLACK".into()],
                        selected: 1, // BLUE
                    },
                },
                Entry {
                    key: "log_level".into(),
                    value: Value::Choice {
                        options: vec!["error".into(), "warn".into(), "info".into(), "debug".into()],
                        selected: 2,
                    },
                },
            ],
        }
    }
}

fn load_config(path: &str) -> Config {
    if Path::new(path).exists() {
        match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(cfg) => cfg,
                Err(err) => {
                    eprintln!("Failed to parse JSON (using defaults): {err}");
                    Config::default()
                }
            },
            Err(err) => {
                eprintln!("Failed to read config (using defaults): {err}");
                Config::default()
            }
        }
    } else {
        Config::default()
    }
}

fn save_config_to_disk(cfg: &Config, path: &str) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn color_name_to_pair(name: &str) -> Option<i16> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "BLACK" => Some(PAIR_BLACK),
        "RED" => Some(PAIR_RED),
        "GREEN" => Some(PAIR_GREEN),
        "YELLOW" => Some(PAIR_YELLOW),
        "BLUE" => Some(PAIR_BLUE),
        "MAGENTA" => Some(PAIR_MAGENTA),
        "CYAN" => Some(PAIR_CYAN),
        "WHITE" => Some(PAIR_WHITE),
        _ => None,
    }
}

/// Draw the whole screen: header, list (scrolling, selected centered), and bottom status line.
fn draw_screen(cfg: &Config, selected: usize, path: &str) {
    clear();

    let mut max_y = 0;
    let mut max_x = 0;
    getmaxyx(stdscr(), &mut max_y, &mut max_x);

    // Header at the top
    let header = format!("Key/Value editor  |  file: {}", path);
    mvprintw(0, 0, &header);
    clrtoeol();

    // Instruction line (with Unicode arrows)
    mvprintw(
        1,
        0,
        "↑/↓: move   Enter/e: edit text / next choice   ←/→: change choice/color   s: save   q: quit",
    );
    clrtoeol();

    // Area reserved for the list
    let list_top = 3;
    let list_bottom = max_y - 3; // keep a few lines at the bottom for status/edit

    // Pre-render entry lines to compute max width for horizontal centering
    let mut rendered_lines = Vec::with_capacity(cfg.entries.len());
    let mut max_width: usize = 0;

    for entry in &cfg.entries {
        let line = match &entry.value {
            Value::Text { value } => {
                let value_str = format!("\"{}\"", value);
                format!("{:<20} = {}", entry.key, value_str)
            }
            Value::Choice { options, selected } => {
                let current = options
                    .get(*selected)
                    .map(|s| s.as_str())
                    .unwrap_or("<?>");
                let value_str = format!("[{}]", current);
                format!("{:<20} = {}", entry.key, value_str)
            }
            Value::Category => {
                // For width computation, just consider the key as the "line"
                entry.key.clone()
            }
            Value::Color { options, selected } => {
                let current = options
                    .get(*selected)
                    .map(|s| s.as_str())
                    .unwrap_or("<?>");
                let value_str = format!("[{}]", current);
                format!("{:<20} = {}", entry.key, value_str)
            }
        };

        let width = line.chars().count();
        if width > max_width {
            max_width = width;
        }
        rendered_lines.push(line);
    }

    // Horizontally center
    let start_col: i32 = if (max_x as usize) > max_width {
        ((max_x as usize - max_width) / 2) as i32
    } else {
        0
    };

    // Vertically: the selected entry is always on the "center row"
    let mut center_row = max_y / 2;
    if center_row < list_top {
        center_row = list_top;
    }
    if center_row > list_bottom {
        center_row = list_bottom;
    }

    let has_color = has_colors();

    // Draw each entry with a row based on its offset from the selected index
    for (i, line) in rendered_lines.iter().enumerate() {
        let offset = i as i32 - selected as i32;
        let row = center_row + offset;

        // Only draw entries that fit in the visible list window
        if row < list_top || row > list_bottom {
            continue;
        }

        let entry = &cfg.entries[i];

        match &entry.value {
            Value::Category => {
                // Category: full-width bar (max_width), centered key, green + reverse
                let bar_width = max_width.max(entry.key.chars().count());
                let key = &entry.key;
                let key_len = key.chars().count();

                let mut cat_line = String::new();
                let padding_left = if bar_width > key_len {
                    (bar_width - key_len) / 2
                } else {
                    0
                };

                // Left padding
                for _ in 0..padding_left {
                    cat_line.push(' ');
                }

                // Add key
                let mut current_len = padding_left;
                for c in key.chars() {
                    if current_len >= bar_width {
                        break;
                    }
                    cat_line.push(c);
                    current_len += 1;
                }

                // Right padding
                while current_len < bar_width {
                    cat_line.push(' ');
                    current_len += 1;
                }

                if has_color {
                    attron(COLOR_PAIR(CATEGORY_PAIR));
                }
                attron(A_REVERSE());
                mvprintw(row, start_col, &cat_line);
                attroff(A_REVERSE());
                if has_color {
                    attroff(COLOR_PAIR(CATEGORY_PAIR));
                }

                mv(row, start_col + bar_width as i32);
                clrtoeol();
            }
            Value::Color { options, selected: color_idx } => {
                // Color entry: key field + " = [" + colored name + "]"
                let current = options
                    .get(*color_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("<?>");

                let prefix = format!("{:<20} = [", entry.key);
                let suffix = "]";

                // For clearing, use the precomputed line width
                let line_width = line.chars().count() as i32;

                // Prefix (with selection highlight if selected)
                if i == selected {
                    attron(A_REVERSE());
                    mvprintw(row, start_col, &prefix);
                    attroff(A_REVERSE());
                } else {
                    mvprintw(row, start_col, &prefix);
                }

                let mut col = start_col + prefix.chars().count() as i32;

                // Colored name, no reverse, just color pair
                if has_color {
                    if let Some(pair) = color_name_to_pair(current) {
                        attron(COLOR_PAIR(pair));
                        mvprintw(row, col, current);
                        attroff(COLOR_PAIR(pair));
                    } else {
                        mvprintw(row, col, current);
                    }
                } else {
                    mvprintw(row, col, current);
                }

                col += current.chars().count() as i32;

                // Suffix (with selection highlight if selected)
                if i == selected {
                    attron(A_REVERSE());
                    mvprintw(row, col, &suffix);
                    attroff(A_REVERSE());
                } else {
                    mvprintw(row, col, &suffix);
                }

                // Clear to end of line
                mv(row, start_col + line_width);
                clrtoeol();
            }
            _ => {
                // Text or Choice
                if i == selected && !matches!(entry.value, Value::Category) {
                    attron(A_REVERSE());
                    mvprintw(row, start_col, line);
                    attroff(A_REVERSE());
                } else {
                    mvprintw(row, start_col, line);
                }

                mv(row, start_col + line.chars().count() as i32);
                clrtoeol();
            }
        }
    }

    // Status/help line at the very bottom; content is updated by show_status()
    mvprintw(max_y - 1, 0, "Press q to quit, s to save…");
    clrtoeol();

    refresh();
}

fn show_status(msg: &str) {
    let mut max_y = 0;
    let mut max_x = 0;
    getmaxyx(stdscr(), &mut max_y, &mut max_x);

    let width = if max_x > 1 { (max_x - 1) as usize } else { 1 };
    let text: String = msg.chars().take(width).collect();

    mv(max_y - 2, 0);
    clrtoeol();
    mvprintw(max_y - 2, 0, &text);
    refresh();
}

/// Edit a text value in-place at the bottom of the screen.
fn edit_text_value(key: &str, value: &mut String) {
    let mut max_y = 0;
    let mut max_x = 0;
    getmaxyx(stdscr(), &mut max_y, &mut max_x);

    let prompt = format!("Editing '{}': Enter=save, Esc=cancel", key);
    mv(max_y - 3, 0);
    clrtoeol();
    mvprintw(max_y - 3, 0, &prompt);

    mv(max_y - 2, 0);
    clrtoeol();
    mvprintw(max_y - 2, 0, "Current value (editable):");

    mv(max_y - 1, 0);
    clrtoeol();

    let mut input = value.clone();
    curs_set(CURSOR_VISIBILITY::CURSOR_VISIBLE);

    loop {
        // Display current input (truncate if needed)
        mv(max_y - 1, 0);
        clrtoeol();

        let max_len = if max_x > 1 { (max_x - 1) as usize } else { 1 };
        let visible = if input.len() > max_len {
            let start = input.len().saturating_sub(max_len);
            &input[start..]
        } else {
            &input
        };

        mvprintw(max_y - 1, 0, visible);
        refresh();

        let ch = getch();

        match ch {
            // Enter
            10 | 13 => {
                *value = input.clone();
                break;
            }
            // Esc
            27 => {
                // Cancel, keep old value
                break;
            }
            // Backspace (handle a couple of common codes)
            KEY_BACKSPACE | 127 | 8 => {
                input.pop();
            }
            _ => {
                // Printable ASCII (for simplicity)
                if ch >= 32 && ch <= 126 {
                    if let Some(c) = std::char::from_u32(ch as u32) {
                        if input.len() < 4096 {
                            input.push(c);
                        }
                    }
                }
            }
        }
    }

    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

    // Clear edit area
    mv(max_y - 3, 0);
    clrtoeol();
    mv(max_y - 2, 0);
    clrtoeol();
    mv(max_y - 1, 0);
    clrtoeol();
    refresh();
}

fn edit_entry(entry: &mut Entry) {
    let key = entry.key.clone(); // avoid borrow issues

    match entry.value {
        Value::Text { ref mut value } => {
            edit_text_value(&key, value);
        }
        // Choice and Color are edited directly with ←/→
        Value::Choice { .. } => {
            show_status("Use ←/→ or Enter to change this choice.");
        }
        Value::Color { .. } => {
            show_status("Use ←/→ or Enter to change this color.");
        }
        Value::Category => {
            show_status("Category header (not editable).");
        }
    }
}

/// Public entry point: edit a JSON config file in a terminal ncurses UI.
pub fn terminal_edit_json(path: &str) {
    let mut cfg = load_config(path);

    // Enable UTF-8 / wide-character support based on current locale.
    setlocale(LcCategory::all, "");

    // Init ncurses
    initscr();
    cbreak();
    noecho();
    keypad(stdscr(), true);
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

    // Initialize colors (for category lines and color entries)
    if has_colors() {
        start_color();
        use_default_colors();
        init_pair(CATEGORY_PAIR, COLOR_GREEN, -1);

        // Color name pairs
        init_pair(PAIR_BLACK, COLOR_BLACK, COLOR_WHITE); // black text on white background
        init_pair(PAIR_RED, COLOR_RED, -1);
        init_pair(PAIR_GREEN, COLOR_GREEN, -1);
        init_pair(PAIR_YELLOW, COLOR_YELLOW, -1);
        init_pair(PAIR_BLUE, COLOR_BLUE, -1);
        init_pair(PAIR_MAGENTA, COLOR_MAGENTA, -1);
        init_pair(PAIR_CYAN, COLOR_CYAN, -1);
        init_pair(PAIR_WHITE, COLOR_WHITE, -1);
    }

    // Initial selection: first non-category entry, if any
    let mut selected: usize = 0;
    if let Some(idx) = cfg
        .entries
        .iter()
        .position(|e| !matches!(e.value, Value::Category))
    {
        selected = idx;
    }

    loop {
        draw_screen(&cfg, selected, path);
        let ch = getch();

        match ch {
            KEY_UP => {
                if cfg.entries.is_empty() {
                    continue;
                }
                // Move up, skipping categories
                let mut new = selected;
                while new > 0 {
                    new -= 1;
                    if !matches!(cfg.entries[new].value, Value::Category) {
                        selected = new;
                        break;
                    }
                }
            }
            KEY_DOWN => {
                if cfg.entries.is_empty() {
                    continue;
                }
                // Move down, skipping categories
                let mut new = selected;
                while new + 1 < cfg.entries.len() {
                    new += 1;
                    if !matches!(cfg.entries[new].value, Value::Category) {
                        selected = new;
                        break;
                    }
                }
            }
            // Enter: for text, open editor; for choice/color, same as Right Arrow
            10 | 13 => {
                if let Some(entry) = cfg.entries.get_mut(selected) {
                    let is_choice_or_color = matches!(
                        entry.value,
                        Value::Choice { .. } | Value::Color { .. }
                    );

                    if is_choice_or_color {
                        // Same logic as KEY_RIGHT
                        match &mut entry.value {
                            Value::Choice {
                                ref options,
                                ref mut selected,
                            }
                            | Value::Color {
                                ref options,
                                ref mut selected,
                            } => {
                                if options.is_empty() {
                                    continue;
                                }
                                let len = options.len();
                                *selected = (*selected + 1) % len;
                            }
                            _ => {}
                        }
                    } else {
                        // Text or Category -> use regular edit_entry behavior
                        edit_entry(entry);
                    }
                }
            }
            // 'e' -> same as before: edit_entry (text editor or status messages)
            101 => {
                if let Some(entry) = cfg.entries.get_mut(selected) {
                    edit_entry(entry);
                }
            }
            // Left / Right to change a choice or color
            KEY_LEFT | KEY_RIGHT => {
                if let Some(entry) = cfg.entries.get_mut(selected) {
                    match &mut entry.value {
                        Value::Choice {
                            ref options,
                            ref mut selected,
                        }
                        | Value::Color {
                            ref options,
                            ref mut selected,
                        } => {
                            if options.is_empty() {
                                continue;
                            }
                            let len = options.len();
                            if ch == KEY_LEFT {
                                if *selected == 0 {
                                    *selected = len - 1;
                                } else {
                                    *selected -= 1;
                                }
                            } else {
                                *selected = (*selected + 1) % len;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // 's' -> save
            115 => {
                match save_config_to_disk(&cfg, path) {
                    Ok(()) => show_status("Saved configuration."),
                    Err(err) => show_status(&format!("Save failed: {err}")),
                }
            }
            // 'q' -> quit
            113 => {
                break;
            }
            _ => {}
        }
    }

    endwin();
}
