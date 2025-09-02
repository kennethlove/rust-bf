use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use cross_xdg::BaseDirs;
use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Colors {
    pub editor_title_focused: Color,
    pub editor_title_unfocused: Color,
    pub gutter_text: Color,

    pub output_title_focused: Color,
    pub output_title_unfocused: Color,

    pub tape_border_focused: Color,
    pub tape_border_unfocused: Color,
    pub tape_cell_empty: Color,
    pub tape_cell_nonzero: Color,
    pub tape_cell_pointer: Color,

    pub status_text: Color,
    pub dialog_title: Color,
    pub dialog_bg: Color,
    pub dialog_error: Color,
    pub dialog_text: Color,
    pub help_hint: Color,

    pub editor_op_right: Color,     // '>'
    pub editor_op_left: Color,      // '<'
    pub editor_op_inc: Color,       // '+'
    pub editor_op_dec: Color,       // '-'
    pub editor_op_output: Color,    // '.'
    pub editor_op_input: Color,     // ','
    pub editor_op_bracket: Color,   // '[' and ']'
    pub editor_non_bf: Color,
}

impl Default for Colors {
    fn default() -> Self {
        // Reasonable defaults matching current hard-coded scheme
        Self {
            editor_title_focused: Color::Cyan,
            editor_title_unfocused: Color::Gray,
            gutter_text: Color::DarkGray,

            output_title_focused: Color::Cyan,
            output_title_unfocused: Color::Gray,

            tape_border_focused: Color::Cyan,
            tape_border_unfocused: Color::Gray,
            tape_cell_empty: Color::DarkGray,
            tape_cell_nonzero: Color::White,
            tape_cell_pointer: Color::Yellow,

            status_text: Color::White,
            dialog_title: Color::White,
            dialog_bg: Color::Black,
            dialog_error: Color::Red,
            dialog_text: Color::White,
            help_hint: Color::Gray,

            editor_op_right: Color::Cyan,
            editor_op_left: Color::Green,
            editor_op_inc: Color::LightGreen,
            editor_op_dec: Color::Red,
            editor_op_output: Color::Yellow,
            editor_op_input: Color::Magenta,
            editor_op_bracket: Color::LightMagenta,
            editor_non_bf: Color::Gray,
        }
    }
}

static COLORS: OnceLock<Colors> = OnceLock::new();

pub fn colors() -> &'static Colors {
    COLORS.get_or_init(|| load_from_toml().unwrap_or_default())
}

fn parse_color(value: &str) -> Option<Color> {
    let s = value.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Some(Color::Rgb(r, g, b));
            }
        }
    } else {
        // Try named colors matching ratatui::style::Color variants
        let name = s.to_ascii_lowercase();
        return Some(match name.as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" | "grey" => Color::Gray,
            "darkgray" | "dark_grey" | "darkgrey" | "dark_gray" => Color::DarkGray,
            "lightred" | "light_red" => Color::LightRed,
            "lightgreen" | "light_green" => Color::LightGreen,
            "lightblue" | "light_blue" => Color::LightBlue,
            "lightmagenta" | "light_magenta" => Color::LightMagenta,
            "lightcyan" | "light_cyan" => Color::LightCyan,
            "white" => Color::White,
            _ => return None,
        });
    }
    None
}

fn load_from_toml() -> Option<Colors> {
    // Look for ./config.toml in CWD
    let base_dirs = BaseDirs::new().unwrap();

    // On Linux: resolves to /home/<user>/.config
    // On Windows: resolves to C:\Users\<user>\.config
    // On macOS: resolves to /Users/<user>/.config
    let config_home = base_dirs.config_home();

    let mut path = PathBuf::from(config_home);
    path.push("bf.toml");

    let content = fs::read_to_string(path).ok()?;
    // Very small hand-rolled parser: look for [colors] section and key = value pairs
    // Values are strings like "#RRGGBB" or named colors.
    let mut in_colors = false;
    let mut map: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') && line.ends_with(']') {
            in_colors = &line[1..line.len()-1] == "colors";
            continue;
        }
        if !in_colors { continue; }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().to_string();
            let val_raw = line[eq+1..].trim();
            // Accept quoted or unquoted
            let val = if val_raw.starts_with('"') && val_raw.ends_with('"') && val_raw.len() >= 2 {
                val_raw[1..val_raw.len()-1].to_string()
            } else { val_raw.to_string() };
            map.insert(key, val);
        }
    }

    let mut cfg = Colors::default();

    macro_rules! set {
        ($field:ident, $key:literal) => {
            if let Some(v) = map.get($key).and_then(|s| parse_color(s)) { cfg.$field = v; }
        };
    }

    set!(editor_title_focused, "editor_title_focused");
    set!(editor_title_unfocused, "editor_title_unfocused");
    set!(gutter_text, "gutter_text");

    set!(output_title_focused, "output_title_focused");
    set!(output_title_unfocused, "output_title_unfocused");

    set!(tape_border_focused, "tape_border_focused");
    set!(tape_border_unfocused, "tape_border_unfocused");
    set!(tape_cell_empty, "tape_cell_empty");
    set!(tape_cell_nonzero, "tape_cell_nonzero");
    set!(tape_cell_pointer, "tape_cell_pointer");

    set!(status_text, "status_text");
    set!(dialog_title, "dialog_title");
    set!(dialog_bg, "dialog_bg");
    set!(dialog_error, "dialog_error");
    set!(dialog_text, "dialog_text");
    set!(help_hint, "help_hint");

    set!(editor_op_right, "editor_op_right");
    set!(editor_op_left, "editor_op_left");
    set!(editor_op_inc, "editor_op_inc");
    set!(editor_op_dec, "editor_op_dec");
    set!(editor_op_output, "editor_op_output");
    set!(editor_op_input, "editor_op_input");
    set!(editor_op_bracket, "editor_op_bracket");
    set!(editor_non_bf, "editor_non_bf");

    Some(cfg)
}
