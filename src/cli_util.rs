use std::io::{self, IsTerminal, Write};
use nu_ansi_term::Style;
use crate::BrainfuckReaderError;

/// Pretty-print structured BrainfuckReaderError with caret positioning.
/// If `program` is `Some("bf")`, prefix messages with "bf: ..." for CLI read mode
pub fn print_reader_error(program: Option<&str>, code: &str, err: &BrainfuckReaderError) {
    let prefix_program = |msg: &str| {
        if let Some(p) = program {
            format!("{p}: {msg}")
        } else {
            msg.to_string()
        }
    };

    match err {
        BrainfuckReaderError::PointerOutOfBounds { ip, ptr, op } => {
            let msg = prefix_program(&format!(
                "Runtime error: pointer out of bounds (ptr={ptr}, op={op})"
            ));
            print_error_with_context(&msg, code, *ip);
        }
        BrainfuckReaderError::InvalidCharacter { ch, ip } => {
            let msg = prefix_program(&format!("Parse error: invalid character '{ch}'"));
            print_error_with_context(&msg, code, *ip);
        }
        BrainfuckReaderError::UnmatchedBrackets { ip, kind } => {
            let msg = prefix_program(&format!("Parse error: unmatched bracket '{kind}'"));
            print_error_with_context(&msg, code, *ip);
        }
        BrainfuckReaderError::IoError { ip, source } => {
            let msg = prefix_program(&format!("I/O error: {source}"));
            print_error_with_context(&msg, code, *ip);
        }
        BrainfuckReaderError::StepLimitExceeded { limit } => {
            let msg = prefix_program(&format!("Step limit exceeded ({limit}"));
            print_error_with_context(&msg, code, 0);
        }
        BrainfuckReaderError::Canceled => {
            let msg = prefix_program("Wall-clock timeout exceeded");
            print_error_with_context(&msg, code, 0);
        }
    }
}

/// Print a concise error with instruction index and a caret context window,
/// working with UTF-8 by slicing using char indices.
pub fn print_error_with_context(prefix: &str, code: &str, pos: usize) {
    use crate::config::colors;

    let is_tty = io::stderr().is_terminal();
    
    if is_tty {
        let cfg = colors();
                let styled = Style::new().fg(rat_to_nu(cfg.dialog_error)).bold().paint(format!("{prefix} at instruction {pos}"));
        eprintln!("{styled}");
    } else {
        eprintln!("{prefix} at instruction {pos}");
    }

    // Show a short window around the position for context
    const WINDOW_CHARS: usize = 32;

    let total_chars = code.chars().count();
    let start_char = pos.saturating_sub(WINDOW_CHARS);
    let end_char = (pos + WINDOW_CHARS + 1).min(total_chars);

    let start_byte = char_to_byte_index(code, start_char);
    let end_byte = char_to_byte_index(code, end_char);
    let slice = &code[start_byte..end_byte];

    eprintln!("  {}", slice);

    // Caret under the exact position
    let caret_offset_chars = pos.saturating_sub(start_char);
    let mut spaces = String::new();
    for _ in 0..caret_offset_chars {
        spaces.push(' ');
    }
    
    if is_tty {
        let cfg = colors();
                let caret = Style::new().fg(rat_to_nu(cfg.editor_op_output)).bold().paint("^");
        eprintln!("  {}{}", spaces, caret);
    } else {
        eprintln!("  {}^", spaces);
    }
    let _ = io::stderr().flush();
}

/// Convert a char index into a byte index in the given UTF-8 string.
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 { return 0; }

    let mut count = 0usize;
    let mut byte_idx = 0usize;

    for ch in s.chars() {
        if count == char_idx {
            break;
        }
        byte_idx += ch.len_utf8();
        count += 1;
    }

    byte_idx
}

pub fn rat_to_nu(c: ratatui::style::Color) -> nu_ansi_term::Color {
    use ratatui::style::Color as RColor;
    use nu_ansi_term::Color as NColor;
    match c {
        RColor::Black => NColor::Black,
        RColor::Red => NColor::Red,
        RColor::Green => NColor::Green,
        RColor::Yellow => NColor::Yellow,
        RColor::Blue => NColor::Blue,
        RColor::Magenta => NColor::Purple,
        RColor::Cyan => NColor::Cyan,
        RColor::Gray => NColor::LightGray,
        RColor::DarkGray => NColor::DarkGray,
        RColor::LightRed => NColor::LightRed,
        RColor::LightGreen => NColor::LightGreen,
        RColor::LightBlue => NColor::LightBlue,
        RColor::LightMagenta => NColor::LightMagenta,
        RColor::LightCyan => NColor::LightCyan,
        RColor::White => NColor::White,
        RColor::Rgb(r,g,b) => NColor::Rgb(r,g,b),
        _ => NColor::Default,
    }
}

