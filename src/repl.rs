use std::{env, thread};
use std::io::{self, IsTerminal, Write};
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use reedline::{Signal, DefaultPrompt, DefaultPromptSegment, HistoryItem, Highlighter, StyledText};
use nu_ansi_term::Style;
use crate::{cli_util, BrainfuckReader, BrainfuckReaderError};
use crate::reader::StepControl;

pub fn repl_loop() -> io::Result<()> {
    // Initialize interactive line editor
    let mut editor = init_line_editor()?;

    // Track the "current editing buffer" across prompts for `:dump`
    let mut current_buffer: String = String::new();

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

        // Meta-command recognition: line starting with `:`
        if let Some(meta) = parse_meta_command(&submission) {
            match handle_meta_command(&mut editor, &meta, &current_buffer)? {
                MetaAction::Exit => return Ok(()),
                MetaAction::Continue => {},
                MetaAction::ResetState => {
                    // Clear any pending state we keep in the loop; editor buffer will be fresh next prompt
                    current_buffer = String::new();
                }
            }
            continue; // Do not execute or add to history
        }

        // Update the current buffer snapshot with what was just submitted
        current_buffer = submission.clone();

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
    
    // Default edit-mode navigation.
    // Up/down move within the current multiline buffer, not history.
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Up,
        ReedlineEvent::Up
    );
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Down,
        ReedlineEvent::Down
    );
    
    // Explicit history-mode convenience bindings
    // Alt+Up/Alt+Down or Ctrl+Up/Ctrl+Down to navigate history items.
    keybindings.add_binding(KeyModifiers::ALT, KeyCode::Up, ReedlineEvent::PreviousHistory);
    keybindings.add_binding(KeyModifiers::CONTROL, KeyCode::Up, ReedlineEvent::PreviousHistory);
    keybindings.add_binding(KeyModifiers::ALT, KeyCode::Down, ReedlineEvent::NextHistory);
    keybindings.add_binding(KeyModifiers::CONTROL, KeyCode::Down, ReedlineEvent::NextHistory);

    let history = reedline::FileBackedHistory::new(1_000).unwrap();

    let editor = Reedline::create()
        .with_highlighter(Box::new(BrainfuckHighlighter::new_catppuccin_mocha()))
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

    // Render prompt and read until EOD with Ctrl+D or Ctrl+Z
    // Enter inserts a newline; history is in-memory and not browsed
    let res = editor.read_line(&prompt);

    match res {
        Ok(Signal::Success(buffer)) => {
            // Add one history item per submitted buffer (program-level)
            if !buffer.trim().is_empty() && !buffer.trim_start().starts_with(':') {
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
    // Limits from environment variables
    let timeout_ms = env::var("BF_TIMEOUT_MS").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(2_000);
    let max_steps = env::var("BF_MAX_STEPS").ok().and_then(|s| s.parse::<usize>().ok());

    // Cooperative cancellation flag
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Result<(), BrainfuckReaderError>>();
    let program = buffer.clone();
    let cancel_flag_clone = cancel_flag.clone();

    thread::spawn(move || {
        let mut bf = BrainfuckReader::new(program);
        let ctrl = StepControl::new(max_steps, cancel_flag_clone);
        // Run with cooperative cancellation
        let res = bf.run_with_control(ctrl);
        let _ = tx.send(res);
    });

    let timeout = Duration::from_millis(timeout_ms as u64);
    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => { } // Success
        Ok(Err(BrainfuckReaderError::StepLimitExceeded { limit })) => {
            eprintln!("Execution aborted: step limit exceeded ({limit})");
            let _ = io::stderr().flush();
        }
        Ok(Err(BrainfuckReaderError::Canceled)) => {
            eprintln!("Execution aborted: wall-clock timeout ({timeout_ms} ms)");
            let _ = io::stderr().flush();
        }
        Ok(Err(other)) => {
            cli_util::print_reader_error(None, &buffer, &other);
            let _ = io::stderr().flush();
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Signal cancel and inform the user
            cancel_flag.store(true, Ordering::Relaxed);
            eprintln!("Execution aborted: wall-clock timeout ({} ms)", timeout_ms);
            let _ = io::stderr().flush();
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {} // Worker ended unexpectedly; nothing to add
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
struct BrainfuckHighlighter {
    // Per-char styles for BF commands, and a fallback for non-commands
    map_plus: Style,
    map_minus: Style,
    map_lt: Style,
    map_gt: Style,
    map_dot: Style,
    map_comma: Style,
    map_lbracket: Style,
    map_rbracket: Style,
    map_other: Style,
}

impl BrainfuckHighlighter {
    fn new_catppuccin_mocha() -> Self {
        use crate::theme::catppuccin::Mocha as P;

        // Character mapping
        // > <   => SKY/TEAL (movement)
        // + ,   => GREEN/RED (data modification)
        // . ,   => YELLOW/PEACH (I/O)
        // [ ]   => MAUVE (flow control)
        let mut s = Self::default();
        s.map_gt = Style::new().fg(P::SKY).bold();
        s.map_lt = Style::new().fg(P::TEAL).bold();
        s.map_plus = Style::new().fg(P::GREEN).bold();
        s.map_minus = Style::new().fg(P::RED).bold();
        s.map_dot = Style::new().fg(P::YELLOW).bold();
        s.map_comma = Style::new().fg(P::PEACH).bold();
        s.map_lbracket = Style::new().fg(P::MAUVE).bold();
        s.map_rbracket = Style::new().fg(P::MAUVE).bold();
        s.map_other = Style::new().fg(P::SURFACE2).bold();
        s
    }

    #[inline]
    fn style_for(&self, ch: char) -> Style {
        match ch {
            '>' => self.map_gt,
            '<' => self.map_lt,
            '+' => self.map_plus,
            '-' => self.map_minus,
            '.' => self.map_dot,
            ',' => self.map_comma,
            '[' => self.map_lbracket,
            ']' => self.map_rbracket,
            _ => self.map_other,
        }
    }
}

impl Highlighter for BrainfuckHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut out: StyledText = StyledText::new();
        let mut current_style: Option<Style> = None;
        let mut buffer = String::new();

        for ch in line.chars() {
            let style = self.style_for(ch);

            match current_style {
                None => {
                    current_style = Some(style);
                    buffer.push(ch);
                }
                Some(s) if s == style => {
                    buffer.push(ch);
                }
                Some(s) => {
                    out.push((s, std::mem::take(&mut buffer)));
                    current_style = Some(style);
                    buffer.push(ch);
                }
            }
        }

        if let Some(s) = current_style {
            if !buffer.is_empty() {
                out.push((s, buffer));
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MetaCommand {
    Exit,
    Help,
    Reset,
    Dump {
        with_line_numbers: bool,
        all_to_stderr: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaAction {
    Continue,
    Exit,
    ResetState,
}

fn parse_meta_command(input: &str) -> Option<MetaCommand> {
    let line = input.trim();
    if !line.starts_with(':') {
        return None;
    }
    let mut parts = line.split_whitespace();
    let head = parts.next().unwrap_or("");
    match head {
        ":exit" | ":quit" => Some(MetaCommand::Exit),
        ":help" => Some(MetaCommand::Help),
        ":reset" => Some(MetaCommand::Reset),
        ":dump" => {
            let mut with_line_numbers = false;
            let mut all_to_stderr = false;
            for arg in parts {
                match arg {
                    "--line-numbers" | "-n" => with_line_numbers = true,
                    "--stderr" | "-e" => all_to_stderr = true,
                    _ => {}
                }
            }
            Some(MetaCommand::Dump { with_line_numbers, all_to_stderr })
        }
        _ => Some(MetaCommand::Help),
    }
}

fn handle_meta_command(editor: &mut reedline::Reedline, cmd: &MetaCommand, current_buffer_snapshot: &str) -> io::Result<MetaAction> {
    use reedline::EditCommand;

    match cmd {
        MetaCommand::Exit => Ok(MetaAction::Exit),
        MetaCommand::Help => {
            print_meta_help_text()?;
            Ok(MetaAction::Continue)
        }
        MetaCommand::Reset => {
            let _ = editor.run_edit_commands(&[EditCommand::Clear]);
            eprintln!("buffer reset");
            let _ = io::stderr().flush();
            Ok(MetaAction::ResetState)
        }
        MetaCommand::Dump { with_line_numbers, all_to_stderr } => {
            dump_buffer(current_buffer_snapshot, *with_line_numbers, *all_to_stderr)?;
            Ok(MetaAction::Continue)
        }
    }
}

fn print_meta_help_text() -> io::Result<()> {
    let mut err = io::stderr();
    writeln!(err, "Meta commands:")?;
    writeln!(err, "  :help                Show this help")?;
    writeln!(err, "  :exit                Exit immediately (code 0)")?;
    writeln!(err, "  :reset               Clear the current buffer")?;
    writeln!(err, "  :dump [-n|--stderr]  Print the current buffer (approx: last executed)")?;
    writeln!(err)?;
    writeln!(err, "Editing: Enter inserts newline; Ctrl+D (or Ctrl+Z on Windows) submits the buffer")?;
    writeln!(err, "Streams: program output -> stdout; prompts/meta/errors -> stderr")?;
    err.flush()?;
    Ok(())
}

fn dump_buffer(buf: &str, with_line_numbers: bool, all_to_stderr: bool) -> io::Result<()> {
    let mut out_stdout = io::stdout();
    let mut out_stderr = io::stderr();

    let lines: Vec<&str> = if buf.is_empty() { Vec::new() } else { buf.split_inclusive("\n").collect() };
    let line_count = if lines.is_empty() {
        if buf.is_empty() { 0 } else { 1 }
    } else {
        // split_inclusive keeps newlines and yields at least one element when non-empty
        let mut c = 0usize;
        for l in &lines {
            if l.ends_with('\n') {
                c += 1;
            }
        }
        if buf.ends_with('\n') { c } else { c + 1 }
    };

    if all_to_stderr {
        writeln!(out_stderr, "- dump ({} lines) -", line_count)?;
        write_dump_lines(&mut out_stderr, buf, with_line_numbers)?;
        writeln!(out_stderr, "- end dump -")?;
        out_stderr.flush()?;
    } else {
        writeln!(out_stderr, "- dump ({} lines) -", line_count)?;
        write_dump_lines(&mut out_stdout, buf, with_line_numbers)?;
        out_stdout.flush()?;
        writeln!(out_stdout, "")?;
        writeln!(out_stderr, "- end dump -")?;
        out_stderr.flush()?;
    }

    Ok(())
}

fn write_dump_lines<W: Write>(mut w: W, buf: &str, with_line_numbers: bool) -> io::Result<()> {
    if !with_line_numbers {
        write!(w, "{}", buf)?;
        return Ok(());
    }

    // Numbered lines
    if buf.is_empty() {
        return Ok(());
    }
    for (i, line) in buf.split_inclusive('\n').enumerate() {
        // Keep newline as-is; line numbers on stdout when requested
        write!(w, "{:>4} | {}", i + 1, line)?;
        if !line.ends_with('\n') {
            // If the last line lacks a newline, still print without forcing one
        }
    }
    Ok(())
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
