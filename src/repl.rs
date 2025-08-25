use std::env;
use std::io::{self, IsTerminal, Write};
use reedline::{Signal, DefaultPrompt, DefaultPromptSegment, HistoryItem, Highlighter, StyledText};
use nu_ansi_term::{Color, Style};

use crate::{cli_util, BrainfuckReader};

pub fn repl_loop() -> io::Result<()> {
    // Initialize interactive line editor
    let mut editor = init_line_editor()?;

    loop {
        // Prompt and read a multi-line submission via editor
        let submission = read_submission_interactive(&mut editor)?;
        if submission.is_none() {
            // EOF or editor closed. End the session cleanly to avoid hanging when stdin is closed
            println!();
            io::stdout().flush()?;
            return Ok(());
        }

        let submission = submission.unwrap();

        let trimmed = submission.trim();
        if trimmed.is_empty() {
            continue; // Ignore empty submissions
        }

        let filtered = bf_only(&trimmed);
        if filtered.is_empty() {
            continue;
        }

        // Execute the Brainfuck code buffer
        execute_bf_buffer(filtered);

        // Test hook: if BF_REPL_ONCE=1, exit after one execution
        if env::var("BF_REPL_ONCE").ok().as_deref() == Some("1") {
            return Ok(());
        }
    }
}

fn init_line_editor() -> io::Result<reedline::Reedline> {
    use reedline::{
        default_emacs_keybindings, EditCommand, Emacs, KeyCode, KeyModifiers, Reedline, ReedlineEvent,
    };

    // Start from default emacs-like bindings and adjust:
    // - Enter -> InsertNewLine (do not submit)
    // - Ctrl+D -> AcceptLine (submit)
    // - Ctrl+Z -> AcceptLine (submit, for Windows)
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(KeyModifiers::NONE, KeyCode::Enter, ReedlineEvent::Edit(vec![EditCommand::InsertNewline]));
    keybindings.add_binding(KeyModifiers::CONTROL, KeyCode::Char('d'), ReedlineEvent::Submit);
    keybindings.add_binding(KeyModifiers::CONTROL, KeyCode::Char('z'), ReedlineEvent::Submit);

    let history = reedline::FileBackedHistory::new(1_000).unwrap();

    let editor = Reedline::create()
        .with_highlighter(Box::new(BrainfuckHighlighter::default()))
        .with_history(Box::new(history))
        .with_edit_mode(Box::new(Emacs::new(keybindings)));

    Ok(editor)
}

pub fn read_submission<R: io::BufRead>(stdin: &mut R) -> Option<String> {
    // Collect all lines until EOF
    let mut buffer = String::new();

    loop {
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                // EOF
                break;
            }
            Ok(_) => {
                buffer.push_str(&line);
            }
            Err(_) => {
                // Read error, ignore
                return None;
            }
        }
    }

    if buffer.is_empty() {
        None
    } else {
        Some(buffer)
    }
}

fn read_submission_interactive(editor: &mut reedline::Reedline) -> io::Result<Option<String>> {
    // Minimal prompt
    let prompt = DefaultPrompt::new(DefaultPromptSegment::Basic("bf".to_string()), DefaultPromptSegment::Empty);

    // Render prompt and read until user submits with Ctrl+D or Ctrl+Z
    // Enter inserts a newline; history is in-memory and not browsed
    let res = editor.read_line(&prompt);

    match res {
        Ok(Signal::Success(buffer)) => {
            // Add one history item per submitted buffer (program-level)
            if !buffer.trim().is_empty() {
                let _ = editor.history_mut().save(HistoryItem::from_command_line(buffer.clone()));
            }
            Ok(Some(buffer))
        }
        Ok(Signal::CtrlC) => Ok(None), // Global SIGINT, exit immediately
        Ok(Signal::CtrlD) => Ok(None), // EOF, exit cleanly
        Err(e) => {
            // Print concise error and end session
            eprintln!("repl: editor error: {e}");
            let _ = io::stderr().flush();
            Ok(None)
        }
    }

}

/// Keep only Brainfuck instruction characters
fn bf_only(s: &str) -> String {
    s.chars()
        .filter(|c| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']'))
        .collect()
}

/// Executes a single Brainfuck program contained in `buffer`.
/// - Program output goes to stdout.
/// - Errors are printed concisely to stderr.
/// - A newline is always written to stdout after execution (success or error)
///   so that the prompt begins at column 0 on the next iteration.
fn execute_bf_buffer(buffer: String) {
    let mut bf = BrainfuckReader::new(buffer.clone());
    if let Err(err) = bf.run() {
        cli_util::print_reader_error(None, &buffer, &err);
        let _ = io::stderr().flush();
    }
    println!();
    let _ = io::stdout().flush(); // Ensure output is flushed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplMode {
    Bare,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeFlagOverride {
    None,
    Bare,
    Editor,
}

pub fn select_mode(flag: ModeFlagOverride) -> Result<ReplMode, String> {
    // Flag override
    match flag {
        ModeFlagOverride::Bare => return Ok(ReplMode::Bare),
        ModeFlagOverride::Editor => {
            if !io::stdin().is_terminal() {
                return Err("cannot start editor: stdin is not a TTY (use --bare or BF_REPL_MODE=bare)".to_string());
            }
            return Ok(ReplMode::Editor);
        }
        ModeFlagOverride::None => {}
    }
    
    // Environment override
    if let Ok(val) = env::var("BF_REPL_MODE") {
        let v = val.trim().to_ascii_lowercase();
        return match v.as_str() {
            "bare" => Ok(ReplMode::Bare),
            "editor" => {
                if !io::stdin().is_terminal() {
                    return Err("cannot start editor: stdin is not a TTY (use BF_REPL_MODE=bare)".to_string());
                }
                Ok(ReplMode::Editor)
            }
            _ => Err(format!("invalid BF_REPL_MODE value: {val}, must be 'bare' or 'editor'")),
        }
    }

    // Auto-detect
    if io::stdin().is_terminal() {
        Ok(ReplMode::Editor)
    } else {
        Ok(ReplMode::Bare)
    }
}

pub fn execute_bare_once() -> io::Result<()> {
    let mut locked = io::BufReader::new(io::stdin().lock());
    let submission = read_submission(&mut locked);
    if let Some(s) = submission {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            let filtered = bf_only(trimmed);
            if !filtered.is_empty() {
                execute_bf_buffer(filtered);
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct BrainfuckHighlighter;

impl Highlighter for BrainfuckHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let bf_style = Style::new().fg(Color::Cyan).bold();
        let non_style = Style::new().fg(Color::DarkGray);
        
        let mut out: StyledText = StyledText::new();
        
        // Group contiguous BF and non-BF characters for styling
        let mut current_is_bf: Option<bool> = None;
        let mut current_buf = String::new();
        
        for ch in line.chars() {
            let is_bf = matches!(ch, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']');
            
            match current_is_bf {
                None => {
                    current_is_bf = Some(is_bf);
                    current_buf.push(ch);
                }
                Some(cur) if cur == is_bf => {
                    current_buf.push(ch);
                }
                Some(_) => {
                    // Flush previous span
                    let style = if current_is_bf.unwrap() { bf_style } else { non_style };
                    out.push((style, std::mem::take(&mut current_buf)));
                    current_is_bf = Some(is_bf);
                    current_buf.push(ch);
                }
            }
        }
        
        if !current_buf.is_empty() {
            let style = if current_is_bf.unwrap_or(false) { bf_style } else { non_style };
            out.push((style, current_buf));
        }
        
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_submission_reads_until_eof_multiple_lines() {
        let input = b"+++\n>+.\n";
        let mut cursor = Cursor::new(&input[..]);
        let got = read_submission(&mut cursor);
        assert_eq!(got.as_deref(), Some("+++\n>+.\n"));
    }

    #[test]
    fn read_submission_empty_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let got = read_submission(&mut cursor);
        assert!(got.is_none());
    }
}
