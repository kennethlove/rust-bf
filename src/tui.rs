use std::collections::HashMap;
use std::io::{self};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::{backend::CrosstermBackend, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::{Block, Borders, Paragraph, Wrap}, Frame, Terminal};
use crate::{BrainfuckReader, BrainfuckReaderError, bf_only};
use crate::reader::StepControl;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Focus {
    Editor,
    Output,
    Tape,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OutputMode {
    Raw,
    Escaped,
}

// Runner wiring: messages and commands between UI and runner
#[derive(Debug)]
enum RunnerMsg {
    // Program produced output bytes (batch as needed)
    Output(Vec<u8>),
    // Snapshot of current tape state (ptr index and 128-cell window)
    Tape { ptr: usize, base: usize, window: [u8; 128] },
    // Runner is awaiting input for `,` instruction
    NeedsInput,
    // Program finished (Ok) or errored
    Halted(Result<(), BrainfuckReaderError>),
}

#[derive(Debug)]
enum UiCmd {
    // Provide input byte for `,` instruction; None = EOF
    ProvideInput(Option<u8>),
    // Request to stop the program
    Stop,
}

struct RunnerHandle {
    // Send commands to the runner
    tx_cmd: mpsc::Sender<UiCmd>,
    // Receive messages from the runner
    rx_msg: mpsc::Receiver<RunnerMsg>,
    // Cooperative cancellation flag (also flipped by Stop)
    cancel: Arc<AtomicBool>,
    // Join handle is kept in worker (detached); we just hold channels and flag
}

pub struct App {
    // editor
    buffer: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    scroll_row: usize,

    // output pane
    output: Vec<u8>,

    // tape pane
    tape_ptr: usize,
    tape_window_base: usize,
    tape_window: [u8; 128],

    // status
    focused: Focus,
    dirty: bool,
    filename: Option<String>,
    running: bool,
    output_mode: OutputMode,

    // help
    show_help: bool,

    // timing
    last_tick: Instant,

    // Runner wiring
    runner: Option<RunnerHandle>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            buffer: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            output: Vec::new(),
            tape_ptr: 0,
            tape_window_base: 0,
            tape_window: [0u8; 128],
            focused: Focus::Editor,
            dirty: false,
            filename: None,
            running: false,
            output_mode: OutputMode::Raw,
            show_help: false,
            last_tick: Instant::now(),
            runner: None,
        }
    }
}

pub fn run() -> io::Result<()> {
    // terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let res = run_app(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    res
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    let mut app = App::default();
    let tick_rate = Duration::from_millis(33);

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_key(&mut app, key)? {
                        break;
                    }
                }
            }
        }

        let mut should_clear_runner = false;

        // Drain runner messages without blocking
        if let Some(handle) = app.runner.as_mut() {
            while let Ok(msg) = handle.rx_msg.try_recv() {
                match msg {
                    RunnerMsg::Output(bytes) => {
                        app.output.extend_from_slice(&bytes);
                    }
                    RunnerMsg::Tape { ptr, base, window } => {
                        app.tape_ptr = ptr;
                        app.tape_window_base = base;
                        app.tape_window.copy_from_slice(&window);
                        app.dirty = true;
                    }
                    RunnerMsg::NeedsInput => {
                        // TODO: gather input
                        let _ = handle.tx_cmd.send(UiCmd::ProvideInput(None));
                    }
                    RunnerMsg::Halted(res) => {
                        app.running = false;
                        should_clear_runner = true;
                        let _ = res;
                    }
                }
            }
        }

        if should_clear_runner {
            // Now it's safe to clear the runner
            app.runner = None;
        }

        if app.last_tick.elapsed() >= tick_rate {
            app.last_tick = Instant::now();
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Root: vertical layout -> main area + status bar
    let root = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(size);

    let main_area = root[0];
    let status_area = root[1];

    // Main area: two columns (left, right)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(main_area);

    let left = cols[0];
    let right = cols[1];

    // Left: editor (top), output (bottom)
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(8)].as_ref())
        .split(left);

    let editor_area = left_rows[0];
    let output_area = left_rows[1];

    draw_editor(f, editor_area, app);
    draw_output(f, output_area, app);
    draw_tape(f, right, app);
    draw_status(f, status_area, app);

    if app.show_help {
        draw_help_overlay(f, size);
    }
}

fn draw_editor(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.filename.as_deref() {
        Some(path) => format!("Editor - {}{}", path, if app.dirty { " *" } else { "" }),
        None => format!("Editor - <untitled>{}", if app.dirty { " *" } else { "" },),
    };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(if app.focused == Focus::Editor { Color::Cyan } else { Color::Gray }),
        ))
        .borders(Borders::ALL);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Prepare highlighted lines within visible window
    let mut lines: Vec<Line> = Vec::new();

    let max_lines = (inner.height as usize).saturating_sub(0);
    let start = app.scroll_row.min(app.buffer.len().saturating_sub(1));
    let end = (start + max_lines).min(app.buffer.len());

    for (idx, line) in app.buffer[start..end].iter().enumerate() {
        lines.push(highlight_bf_line(line, app, start + idx));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);

    // Cursor rendering (if editor is focused)
    if app.focused == Focus::Editor {
        let row = app
            .cursor_row
            .saturating_sub(app.scroll_row)
            .min(inner.height.saturating_sub(1) as usize);
        let col = app.cursor_col.min(inner.width.saturating_sub(1) as usize);
        f.set_cursor_position(Position::new(inner.x + col as u16, inner.y + row as u16));
    }
}

fn draw_output(f: &mut Frame, area: Rect, app: &App) {
    let mode = match app.output_mode {
        OutputMode::Raw => "Raw",
        OutputMode::Escaped => "Esc",
    };
    let block = Block::default()
        .title(Span::styled(
            format!("Output - {mode}"),
            Style::default().fg(if app.focused == Focus::Output { Color::Cyan } else { Color::Gray }),
        ))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let paragraph = if app.output.is_empty() {
        Paragraph::new("<no output yet>")
    } else {
        match app.output_mode {
            OutputMode::Raw => {
                // Best-effort: display bytes as UTF-8 (lossy) to avoid panics on invalid UTF-8
                let s = String::from_utf8_lossy(&app.output);
                Paragraph::new(s.into_owned())
            }
            OutputMode::Escaped => {
                let s = bytes_to_escaped(&app.output);
                Paragraph::new(s)
            }
        }
    };
    f.render_widget(paragraph, inner);
}

fn draw_tape(f: &mut Frame, area: Rect, app: &App) {
    let border_style = if app.focused == Focus::Tape {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };
    let block = Block::default()
        .title(Line::raw("Tape (128 cells)"))
        .borders(Borders::ALL)
        .border_style(border_style);

    // Build the tape line; content only contains cells, not the title.
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(app.tape_window.len());
    for (i, byte) in app.tape_window.iter().enumerate() {
        let abs_idx = app.tape_window_base + i;
        let cell_text = format!("[{:02X}]", byte);
        let cell_style = if abs_idx == app.tape_ptr {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(cell_text, cell_style));
    }

    // Enable wrapping within the Tape area and preserve spaces
    let paragraph = Paragraph::new(Line::from(spans))
        .wrap(Wrap { trim: false })
        .block(block);

    f.render_widget(paragraph, area);
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let filename = app
        .filename
        .as_deref()
        .unwrap_or("<untitled>");
    let dirty = if app.dirty { "*" } else { "" };
    let run_state = if app.running { "Running" } else { "Stopped" };
    let output_mode = match app.output_mode {
        OutputMode::Raw => "Raw",
        OutputMode::Escaped => "Esc",
    };
    let status = format!(
        " {}{} | {} | Ptr: {} | Cell: -- | Output: {} | F1 for Help ",
        filename, dirty, run_state, app.tape_ptr, output_mode
    );
    let block = Block::default().borders(Borders::TOP);
    f.render_widget(block, area);
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    let line = Line::from(Span::styled(status, Style::default().fg(Color::White)));
    f.render_widget(Paragraph::new(line), inner);
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL);

    let w = area.width.saturating_sub(area.width / 4);
    let h = area.height.saturating_sub(area.height / 3);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(block, rect);

    let text = vec![
        Line::raw("F5/Ctrl+R: Run  Shift+F5/Ctrl+.: Stop"),
        Line::raw("Ctrl+O: Open  Ctrl+S: Save"),
        Line::raw("Tab/Shift+Tab: Switch pane focus"),
        Line::raw("Ctrl+E: Toggle output mode (Raw/Esc)"),
        Line::raw("F1/Ctrl+H: Toggle this help"),
        Line::raw("Editor: Arrows, PageUp/PageDown, Home/End, typing, Enter, Backspace"),
        Line::raw("Tape pane: [ and ] to shift window; Left/Right to move highlight"),
        Line::raw("q/Esc: Quit"),
    ];
    let inner = Rect {
        x: rect.x + 2,
        y: rect.y + 2,
        width: rect.width.saturating_sub(4),
        height: rect.height.saturating_sub(4),
    };
    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
}

fn handle_key(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    // Global keys
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q') => return Ok(true), // Quit
            KeyCode::Char('h') | KeyCode::F(1) => {
                app.show_help = !app.show_help;
                return Ok(false);
            }
            KeyCode::Char('r') | KeyCode::F(5) => {
                // Start runner
                start_runner(app);
                return Ok(false);
            }
            KeyCode::Char('o') => {
                // Open file (not implemented)
                return Ok(false);
            }
            KeyCode::Char('s') => {
                // Save file (not implemented)
                return Ok(false);
            }
            KeyCode::Char('e') => {
                app.output_mode = match app.output_mode {
                    OutputMode::Raw => OutputMode::Escaped,
                    OutputMode::Escaped => OutputMode::Raw,
                };
                return Ok(false);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::F(1) => {
            app.show_help = !app.show_help;
            Ok(false)
        }
        KeyCode::F(5) => {
            // Start runner
            start_runner(app);
            Ok(false)
        }
        KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(h) = app.runner.as_ref() {
                h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = h.tx_cmd.send(UiCmd::Stop);
            }
            app.running = false;
            Ok(false)
        }
        KeyCode::F(17) /* Shift+F5 */ => {
            if let Some(h) = app.runner.as_ref() {
                h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = h.tx_cmd.send(UiCmd::Stop);
            }
            app.running = false;
            Ok(false)
        }
        KeyCode::Tab => {
            app.focused = match app.focused {
                Focus::Editor => Focus::Output,
                Focus::Output => Focus::Tape,
                Focus::Tape => Focus::Editor,
            };
            Ok(false)
        }
        KeyCode::BackTab => {
            app.focused = match app.focused {
                Focus::Editor => Focus::Tape,
                Focus::Output => Focus::Editor,
                Focus::Tape => Focus::Output,
            };
            Ok(false)
        }
        KeyCode::Esc => {
            // Quit when help is open; otherwise quit
            if app.show_help {
                app.show_help = false;
                Ok(false)
            } else {
                Ok(true)
            }
        }
        _ => match app.focused {
            Focus::Editor => {
                handle_editor_key(app, key);
                Ok(false)
            },
            Focus::Output => Ok(false), // No keys handled in output pane
            Focus::Tape => {
                handle_tape_key(app, key);
                Ok(false)
            },
        },
    }
}

fn handle_tape_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Page to previous/next 128-cell chunk
        KeyCode::Char('[') | KeyCode::Left | KeyCode::PageUp => {
            let page = 128usize;
            let new_base = app.tape_window_base.saturating_sub(page);
            if new_base != app.tape_window_base {
                app.tape_window_base = new_base;
                app.dirty = true;
            }
        }
        KeyCode::Char(']') | KeyCode::Right | KeyCode::PageDown => {
            let page = 128usize;
            // avoid overflow / beyond memory size; if you have memory size available, clamp to it
            app.tape_window_base = app.tape_window_base.saturating_add(page);
            app.dirty = true;
        }
        // Center current pointer into its page
        KeyCode::Char('c') if key.modifiers.is_empty() => {
            let page = 128usize;
            app.tape_window_base = app.tape_ptr - (app.tape_ptr % page);
            app.dirty = true;
        }
        _ => {}
    }
}

fn handle_editor_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Left => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
            } else if app.cursor_row > 0 {
                app.cursor_row -= 1;
                app.cursor_col = app.buffer[app.cursor_row].len();
                ensure_cursor_visible(app);
            }
        }
        KeyCode::Right => {
            let len = app.buffer[app.cursor_row].len();
            if app.cursor_col < len {
                app.cursor_col += 1;
            } else if app.cursor_row + 1 < app.buffer.len() {
                app.cursor_row += 1;
                app.cursor_col = 0;
                ensure_cursor_visible(app);
            }
        }
        KeyCode::Up => {
            if app.cursor_row > 0 {
                app.cursor_row -= 1;
                app.cursor_col = app.cursor_col.min(app.buffer[app.cursor_row].len());
                ensure_cursor_visible(app);
            }
        }
        KeyCode::Down => {
            if app.cursor_row + 1 < app.buffer.len() {
                app.cursor_row += 1;
                app.cursor_col = app.cursor_col.min(app.buffer[app.cursor_row].len());
                ensure_cursor_visible(app);
            }
        }
        KeyCode::Home => { app.cursor_col = 0; }
        KeyCode::End => { app.cursor_col = app.buffer[app.cursor_row].len(); }
        KeyCode::PageUp => {
            let jump = 10usize;
            app.cursor_row = app.cursor_row.saturating_sub(jump);
            app.cursor_col = app.cursor_col.min(app.buffer[app.cursor_row].len());
            ensure_cursor_visible(app);
        }
        KeyCode::PageDown => {
            let jump = 10usize;
            app.cursor_row = (app.cursor_row + jump).min(app.buffer.len().saturating_sub(1));
            app.cursor_col = app.cursor_col.min(app.buffer[app.cursor_row].len());
            ensure_cursor_visible(app);
        }
        KeyCode::Enter => {
            let line = app.buffer[app.cursor_row].clone();
            let (left, right) = line.split_at(app.cursor_col);
            app.buffer[app.cursor_row] = left.to_string();
            app.buffer.insert(app.cursor_row + 1, right.to_string());
            app.cursor_row += 1;
            app.cursor_col = 0;
            app.dirty = true;
            ensure_cursor_visible(app);
        }
        KeyCode::Backspace => {
            if app.cursor_col > 0 {
                let line = &mut app.buffer[app.cursor_row];
                let prev_byte_idx = nth_char_to_byte_idx(line, app.cursor_col - 1);
                line.drain(prev_byte_idx..nth_char_to_byte_idx(line, app.cursor_col));
                app.cursor_col -= 1;
                app.dirty = true;
            } else if app.cursor_row > 0 {
                let cur = app.buffer.remove(app.cursor_row);
                app.cursor_row -= 1;
                let prev_len_chars = app.buffer[app.cursor_row].chars().count();
                app.buffer[app.cursor_row].push_str(&cur);
                app.cursor_row = prev_len_chars;
                app.dirty = true;
                ensure_cursor_visible(app);
            } else {
                // At start of file, do nothing
            }
        }
        KeyCode::Delete => {
            let len_chars = app.buffer[app.cursor_row].chars().count();
            if app.cursor_col < len_chars {
                let line = &mut app.buffer[app.cursor_row];
                let start = nth_char_to_byte_idx(line, app.cursor_col);
                let end = nth_char_to_byte_idx(line, app.cursor_col + 1);
                line.drain(start..end);
                app.dirty = true;
            } else if app.cursor_row + 1 < app.buffer.len() {
                let next = app.buffer.remove(app.cursor_row + 1);
                app.buffer[app.cursor_row].push_str(&next);
                app.dirty = true;
            }
        }
        KeyCode::Char(ch) => {
            // Only insert when no modifiers are held; avoid inserting on Ctrl/Alt/Shift combos
            if key.modifiers.is_empty() && !is_control_char(ch) {
                app.buffer[app.cursor_row].insert(app.cursor_col, ch);
                app.cursor_col += 1;
                app.dirty = true;
                ensure_cursor_visible(app);
            }
        }
        _ => {}
    }

    if app.buffer.is_empty() {
        app.buffer.push(String::new());
        app.cursor_row = 0;
        app.cursor_col = 0;
        app.scroll_row = 0;
    } else {
        app.cursor_row = app.cursor_row.min(app.buffer.len() - 1);
        let cur_len = app.buffer[app.cursor_row].chars().count();
        app.cursor_col = app.cursor_col.min(cur_len);
    }
}

fn nth_char_to_byte_idx(s: &str, nth: usize) -> usize {
    if nth == 0 {
        return 0;
    }
    match s.char_indices().nth(nth) {
        Some((i, _)) => i,
        None => s.len(),
    }
}

fn is_control_char(ch: char) -> bool {
    ch.is_control()
}

fn ensure_cursor_visible(app: &mut App) {
    let margin = 3usize;
    if app.cursor_row < app.scroll_row.saturating_add(margin) {
        app.scroll_row = app.cursor_row.saturating_sub(margin);
    }
    let end = app.scroll_row + margin * 2;
    if app.cursor_row > end {
        app.scroll_row = app.cursor_row.saturating_sub(margin);
    }
}

// Syntax highlighting for BF tokens + matching bracket highlighting
fn highlight_bf_line(line: &str, app: &App, row: usize) -> Line<'static> {
    let (match_row_col, cursor_on_bracket) = if app.focused == Focus::Editor
        && row == app.cursor_row
        && app.cursor_col < line.chars().count()
    {
        let ch = line.chars().nth(app.cursor_col).unwrap_or('\0');
        if ch == '[' || ch == ']' {
            (find_matching_bracket(app, (app.cursor_row, app.cursor_col)), true)
        } else {
            (None, false)
        }
    } else {
        (None, false)
    };

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.len().max(1));
    for (i, ch) in line.chars().enumerate() {
        let base = match ch {
            '>' => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            '<' => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            '+' => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            '-' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            '.' => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ',' => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            '[' | ']' => Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::Gray),
        };

        // Highlight current bracket and its match
        let styled = if cursor_on_bracket && (row, i) == (app.cursor_row, app.cursor_col) {
            base.add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else if let Some((mr, mc)) = match_row_col {
            if (row, i) == (mr, mc) {
                base.add_modifier(Modifier::REVERSED)
            } else {
                base
            }
        } else {
            base
        };

        spans.push(Span::styled(ch.to_string(), styled));
    }

    if spans.is_empty() {
        spans.push(Span::raw(" "))
    }
    Line::from(spans)
}

fn find_matching_bracket(app: &App, pos: (usize, usize)) -> Option<(usize, usize)> {
    let Mapping {
        bf_seq,
        orig_to_bf_idx,
        bf_idx_to_orig,
    } = build_bf_mapping(&app.buffer);

    let bf_idx = *orig_to_bf_idx.get(&pos)?;

    let chars: Vec<char> = bf_seq.chars().collect();
    let cur = *chars.get(bf_idx)?;
    if cur != '[' && cur != ']' {
        return None;
    }

    if cur == '[' {
        let mut depth: isize = 0;
        for i in (bf_idx + 1)..chars.len() {
            match chars[i] {
                '[' => depth += 1,
                ']' => {
                    if depth == 0 {
                        return bf_idx_to_orig.get(&i).copied();
                    } else {
                        depth -= 1;
                    }
                }
                _ => {}
            }
        }
    } else {
        let mut depth: isize = 0;
        let mut i = bf_idx;
        while i > 0 {
            i -= 1;
            match chars[i] {
                ']' => depth += 1,
                '[' => {
                    if depth == 0 {
                        return bf_idx_to_orig.get(&i).copied();
                    } else {
                        depth -= 1;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

struct Mapping {
    bf_seq: String,
    // Original (row, col) -> index in bf_seq (only for BF tokens)
    orig_to_bf_idx: HashMap<(usize, usize), usize>,
    // Index in bf_seq -> Original (row, col)
    bf_idx_to_orig: HashMap<usize, (usize, usize)>,
}

fn build_bf_mapping(lines: &[String]) -> Mapping {
    let mut bf_seq = String::new();
    let mut orig_to_bf_idx: HashMap<(usize, usize), usize>  = HashMap::new();
    let mut bf_idx_to_orig: HashMap<usize, (usize, usize)>  = HashMap::new();

    let is_bf = |c: char| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']');

    let mut idx = 0usize;
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            if is_bf(ch) {
                bf_seq.push(ch);
                orig_to_bf_idx.insert((r, c), idx);
                bf_idx_to_orig.insert(idx, (r, c));
                idx += 1;
            }
        }
    }

    Mapping {
        bf_seq,
        orig_to_bf_idx,
        bf_idx_to_orig,
    }
}

// Start the Brainfuck runner thread with cooperative cancellation and channels
fn start_runner(app: &mut App) {
    // If a runner is already active, ignore
    if app.runner.is_some() {
        return;
    }

    // Prepare source (keep only BF tokens)
    let source = app_current_source(app);
    let filtered = bf_only(&source);
    if filtered.trim().is_empty() {
        // No BF code to run
        return;
    }

    // Channels
    let (tx_msg, rx_msg) = mpsc::channel::<RunnerMsg>();
    let (tx_cmd, rx_cmd) = mpsc::channel::<UiCmd>();

    // Cancel flag and step control
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_timer = cancel.clone();

    // Limits from environment
    let timeout_ms = std::env::var("BF_TIMEOUT_MS").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(2_000);
    let max_steps = std::env::var("BF_MAX_STEPS").ok().and_then(|s| s.parse::<usize>().ok());

    // Make rx_cmd accessible from callbacks invoked during execution
    let rx_cmd_shared = Arc::new(Mutex::new(rx_cmd));

    // Spawn worker thread
    let program = filtered.clone();
    thread::spawn(move || {
        // Timer thread: flip cancel after wall-clock timeout
        let cancel_for_timer = cancel_for_timer.clone();
        let cancel_clone = cancel_for_timer.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(timeout_ms as u64));
            cancel_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        // Build the reader and wire callbacks
        let mut bf = BrainfuckReader::new(program);

        // Output: forward produced bytes to UI
        let tx_out = tx_msg.clone();
        bf.set_output_sink(Box::new(move |bytes: &[u8]| {
            // Send as a batch; UI appends to its buffer
            let _ = tx_out.send(RunnerMsg::Output(bytes.to_vec()));
        }));

        // Input: ask UI, block until ProvideInput arrives (or channel closes)
        let tx_needs_input = tx_msg.clone();
        let rx_input = rx_cmd_shared.clone();
        bf.set_input_provider(Box::new(move || {
            let _ = tx_needs_input.send(RunnerMsg::NeedsInput);
            // Wait for a ProvideInput command; None if UI side dropped
            let recv_res = {
                let lock = rx_input.lock().expect("rx_cmd mutex poisoned");
                lock.recv()
            };
            match recv_res {
                Ok(UiCmd::ProvideInput(b)) => b,
                Ok(UiCmd::Stop) => None, // treat Stop as EOF for input
                Err(_) => None, // channel closed
            }
        }));

        // Tape observer: emit 128-cell window snapshots
        let tx_tape = tx_msg.clone();
        bf.set_tape_observer(
            128, { // Window size requested from the engine
                let tx = tx_msg.clone();
                move |ptr, base, window| {
                    // copy to fixed array expected by UI
                    let mut buf = [0u8; 128];
                    buf[..window.len().min(128)].copy_from_slice(&window[..window.len().min(128)]);
                    let _ = tx.send(RunnerMsg::Tape { ptr, base, window: buf });
                }
            }
        );

        // Run BF with cooperative cancellation
        let ctrl = StepControl::new(max_steps, cancel_for_timer.clone());
        let res = {
            bf.run_with_control(ctrl)
        };

        // Report completion
        let _ = tx_msg.send(RunnerMsg::Halted(res));
    });

    // Save handle in app
    app.runner = Some(RunnerHandle {
        tx_cmd,
        rx_msg,
        cancel,
    });
    app.running = true;

    // Reset previous output buffer for a fresh run
    app.output.clear();
}

// Helper: get the current editor buffer as a newline-joined string
fn app_current_source(app: &App) -> String {
    if app.buffer.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        for (i, line) in app.buffer.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(line);
        }
        s
    }
}

// Helper: convert bytes into "escaped" string: printable ASCII as-is, others as \xHH
fn bytes_to_escaped(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x20..=0x7E => out.push(b as char), // Printable ASCII
            b'\n' => out.push('\n'),
            b'\r' => out.push('\r'),
            b'\t' => out.push('\t'),
            _ => {
                use std::fmt::Write as _;
                let _ = write!(&mut out, "\\x{:02X}", b);
            }
        }
    }
    out
}
