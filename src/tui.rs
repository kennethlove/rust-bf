use std::collections::HashMap;
use std::io::{self};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::{fs, thread};
use std::path::{Path, PathBuf};
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
use ratatui::widgets::{Cell, Clear, Row, Table};
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

    // Save dialog
    show_save_dialog: bool,
    save_name_input: String,
    save_error: Option<String>,

    // Open dialog
    show_open_dialog: bool,
    open_name_input: String,
    open_error: Option<String>,

    // Confirm dialog (for destructive actions like "open" with unsaved changes)
    show_confirm_dialog: bool,
    confirm_message: String,
    confirm_pending_open: Option<PathBuf>,

    // Last status message (auto-expires)
    status_message: Option<(String, Instant)>,
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

            show_save_dialog: false,
            save_name_input: String::new(),
            save_error: None,

            show_open_dialog: false,
            open_name_input: String::new(),
            open_error: None,

            show_confirm_dialog: false,
            confirm_message: String::new(),
            confirm_pending_open: None,

            status_message: None,
        }
    }
}

pub fn run() -> io::Result<()> {
    // For backwards compatibility, delegate to run_with_file(None)
    run_with_file(None)
}

// Entry point that accepts an optional initial file to open
pub fn run_with_file(initial_file: Option<PathBuf>) -> io::Result<()> {
    // terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let res = run_app(&mut terminal, initial_file);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    initial_file: Option<PathBuf>,
) -> io::Result<()> {
    let mut app = App::default();
    let tick_rate = Duration::from_millis(33);

    // If an initial file was provided, attempt to open it
    if let Some(path) = initial_file {
        if let Err(err) = app_open_file(&mut app, &path) {
            // If opening fails, leave app in default state
            set_status(&mut app, &format!("Failed to open {}: {}", path.display(), err));
            eprintln!("Failed to open {}: {}", path.display(), err);
        }
    }

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

        // We store deferred actions here
        let mut deferred_status: Option<String> = None;
        let mut deferred_send_auto_eof: bool = false;
        let mut saw_halted: bool = false;

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
                        deferred_send_auto_eof = true;
                        deferred_status = Some("Program requested input (auto-EOF sent)".to_string());
                    }
                    RunnerMsg::Halted(res) => {
                        app.running = false;
                        should_clear_runner = true;
                        saw_halted = true;
                        match res {
                            Ok(()) => {
                                deferred_status = Some("Program finished".to_string());
                            }
                            Err(e) => {
                                deferred_status = Some(format!("Error: {}", e));
                            }
                        }
                    }
                }
            }
        }

        // Now, with no active mutable borrow of app.runner, perform deferred actions
        if deferred_send_auto_eof {
            if let Some(h) = app.runner.as_ref() {
                let _ = h.tx_cmd.send(UiCmd::ProvideInput(None));
            }
        }
        if let Some(msg) = deferred_status.take() {
            set_status(&mut app, &msg);
        }

        if should_clear_runner || saw_halted {
            // Now it's safe to clear the runner
            app.runner = None;
        }

        if app.last_tick.elapsed() >= tick_rate {
            app.last_tick = Instant::now();

            // Expire status messages after 5 seconds
            if let Some((_, since)) = app.status_message.as_ref() {
                if since.elapsed() >= Duration::from_secs(5) {
                    app.status_message = None;
                }
            }
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Root: vertical layout -> editor + status bar
    let root = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(size);

    let main_area = root[0];
    let status_bar = root[1];

    // Main area: two columns (left, right)
    // Right (Tape) is fixed to 25% of the width; Left gets the remaining 75%
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(main_area);

    let left = cols[0];
    let right = cols[1];

    // Determine output height
    // - Use as few lines as required by the current output (count lines)
    // - Add 2 for the output block borders
    // - Cap to at most 50% of the available vertical space
    // - Ensure a minimal height (3 rows including borders) so the block is visible
    let output_inner_lines: u16 = output_display_lines(app);
    let output_block_height: u16 = output_inner_lines.saturating_add(2);
    let max_output_block_height: u16 = (main_area.height / 2).max(3);
    let desired_output_block_height: u16 = output_block_height
        .min(max_output_block_height)
        .max(3);

    // Left: editor (top), output (bottom)
    // Editor takes the rest, Output gets the computed fixed height
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(desired_output_block_height)].as_ref())
        .split(left);

    let editor_area = left_rows[0];
    let output_area = left_rows[1];

    draw_editor(f, editor_area, app);
    draw_output(f, output_area, app);
    draw_tape(f, right, app);
    draw_status(f, status_bar, app);

    if app.show_help {
        draw_help_overlay(f, size);
    }
    if app.show_save_dialog {
        draw_save_dialog(f, size, app);
    }
    if app.show_open_dialog {
        draw_open_dialog(f, size, app);
    }
    if app.show_confirm_dialog {
        draw_confirm_dialog(f, size, app);
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

    // Compute inner area to estimate available width/height for the table
    let inner = block.inner(area);

    // "Responsive" grid sizing
    // Each cell renders like "[XX]" (4 chars). We give it width 4 and 1 column space between cells
    let cell_content_width: u16 = 4;

    let mut cols = (inner.width / cell_content_width).max(1) as usize;
    cols = cols.min(128);
    if cols == 0 { cols = 1; }

    let rows = ((128 + cols - 1) / cols).max(1);

    // Build rows
    let mut table_rows: Vec<Row> = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut cells: Vec<Cell> = Vec::with_capacity(cols + 1);
        for c in 0..cols {
            let idx = r * cols + c;
            if idx < 128 {
                let byte = app.tape_window[idx];
                let abs_idx = app.tape_window_base + idx;

                let mut style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
                if byte > 0 {
                    style = style.fg(Color::White);
                }

                if abs_idx == app.tape_ptr {
                    style = style.fg(Color::Yellow);
                }

                cells.push(Cell::from(format!("[{byte:02X}]")).style(style));
            } else {
                // Pad remaining cells in the last row to keep grid aligned
                cells.push(Cell::from("    "));
            }

        }
        table_rows.push(Row::new(cells));
    }

    // Constraints
    // - First cols - columns are fixed width
    // - Last column expands by the exact leftover (ignoring spacing)
    let base_width_no_spacing = (cols as u16) * cell_content_width;
    let leftover_no_spacing = inner.width.saturating_sub(base_width_no_spacing);

    // Column width constraints for the table
    let mut constraints: Vec<Constraint> =
        std::iter::repeat(Constraint::Length(cell_content_width))
            .take(cols.saturating_sub(1))
            .collect();

    let last_width = cell_content_width + leftover_no_spacing;
    constraints.push(Constraint::Length(last_width));

    let table = Table::new(table_rows, constraints)
        .block(block)
        .column_spacing(0);
    f.render_widget(table, area);
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
    let cell_val = current_cell_value(app)
        .map(|v| format!("{v}"))
        .unwrap_or_else(|| "--".to_string());
    let msg = app
        .status_message
        .as_ref()
        .map(|(m, _)| m.as_str())
        .unwrap_or("");

    let status = format!(
        " {}{} | {} | Ptr: {} | Cell: {} | Output: {} | {} ",
        filename, dirty, run_state, app.tape_ptr, cell_val, output_mode, msg
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
        Line::raw("F5/Ctrl+R: Run"),
        Line::raw("Ctrl+O: Open  Ctrl+S: Save"),
        Line::raw("Tab/Shift+Tab: Switch pane focus"),
        Line::raw("Ctrl+E: Toggle output mode (Raw/Esc)"),
        Line::raw("F1/Ctrl+H: Toggle this help"),
        Line::raw(""),
        Line::raw("Editor: Arrows, PageUp/PageDown, Home/End, typing, Enter, Backspace"),
        Line::raw("Tape pane: [ and ] to shift window"),
        Line::raw(""),
        Line::raw("Input on ',': prompts for input; Esc at prompt sends EOF"),
        Line::raw("Output Raw mode may render control bytes; switch to Escaped mode if your terminal glitches"),
        Line::raw(""),
        Line::raw("Ctrl+q/Esc: Quit"),
    ];

    let inner = Rect {
        x: rect.x + 2,
        y: rect.y + 2,
        width: rect.width.saturating_sub(4),
        height: rect.height.saturating_sub(4),
    };

    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
}

fn draw_save_dialog(f: &mut Frame, area: Rect, app: &App) {
    // Content to display
    let title = "Save As";
    let prompt = "Enter file name (Esc to cancel):";
    let input_line = format!("> {}", app.save_name_input);
    let err_line = app.save_error.as_deref().unwrap_or("");

    // Compute minimal dialog size based on content
    let mut longest = prompt.len().max(input_line.len()).max(title.len());
    if !err_line.is_empty() {
        longest = longest.max(err_line.len());
    }

    // Borders add 2 columns; add a tiny horizontal padding of 1 char per side
    let horizontal_padding = 2u16;
    let min_w = 10u16;
    let max_w = area.width.saturating_sub(2);
    let w = ((longest as u16) + 2 /* borders */ + horizontal_padding).clamp(min_w, max_w);

    // Lines:
    // - 1: prompt
    // - 2: input
    // - 3: error(optional)

    let base_lines = 2u16;
    let lines = base_lines + if err_line.is_empty() { 0 } else { 1 };
    // Borders add 2 rows; add a tiny vertical padding of 0 (keep minimal)
    let min_h = 4u16;
    let max_h = area.height.saturating_sub(2);
    let h = (lines + 2 /* borders */).clamp(min_h, max_h);

    // Center dialog
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect { x, y, width: w, height: h };

    // Ensure a solid background and then draw the block
    f.render_widget(Clear, rect);

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::White)))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    f.render_widget(block.clone(), rect);

    // Inner area
    let inner = block.inner(rect);

    // Render content with minimal padding: one leading space
    let left_pad = " ";
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(format!("{left_pad}{prompt}")));
    lines.push(Line::raw(format!("{left_pad}{input_line}")));
    if !err_line.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("{left_pad}{err_line}"),
            Style::default().fg(Color::Red),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(paragraph, inner);

    // Show the text cursor at the end of the input line
    let cursor_x = inner
        .x
        .saturating_add(1) // left_pad
        .saturating_add(2) // "> "
        .saturating_add(app.save_name_input.len() as u16)
        .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
    let cursor_y = inner
        .y
        .saturating_add(1) // second rendered line
        .min(inner.y.saturating_add(area.height.saturating_sub(1)));
    f.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn draw_open_dialog(f: &mut Frame, area: Rect, app: &App) {
    // Content to display
    let title = "Open File";
    let prompt = "Enter file name to open (Esc to cancel):";
    let input_line = format!("> {}", app.open_name_input);
    let err_line = app.open_error.as_deref().unwrap_or("");

    // Compute minimal dialog size based on content
    let mut longest = prompt.len().max(input_line.len()).max(title.len());
    if !err_line.is_empty() {
        longest = longest.max(err_line.len());
    }

    // Borders add 2 columns; add a tiny horizontal padding of 1 char per side
    let horizontal_padding = 2u16;
    let min_w = 10u16;
    let max_w = area.width.saturating_sub(2);
    let w = ((longest as u16) + 2 /* borders */ + horizontal_padding).clamp(min_w, max_w);

    // Lines:
    // - 1: prompt
    // - 2: input
    // - 3: error(optional)

    let base_lines = 2u16;
    let lines = base_lines + if err_line.is_empty() { 0 } else { 1 };
    // Borders add 2 rows; add a tiny vertical padding of 0 (keep minimal)
    let min_h = 4u16;
    let max_h = area.height.saturating_sub(2);
    let h = (lines + 2 /* borders */).clamp(min_h, max_h);

    // Center dialog
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect { x, y, width: w, height: h };

    // Ensure a solid background and then draw the block
    f.render_widget(Clear, rect);

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::White)))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    f.render_widget(block.clone(), rect);

    // Inner area
    let inner = block.inner(rect);

    // Render content with minimal padding: one leading space
    let left_pad = " ";
    let mut lines_vec: Vec<Line> = Vec::new();
    lines_vec.push(Line::raw(format!("{left_pad}{prompt}")));
    lines_vec.push(Line::raw(format!("{left_pad}{input_line}")));
    if !err_line.is_empty() {
        lines_vec.push(Line::from(Span::styled(
            format!("{left_pad}{err_line}"),
            Style::default().fg(Color::Red),
        )));
    }

    let paragraph = Paragraph::new(lines_vec)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(paragraph, inner);

    let cursor_x = inner
        .x
        .saturating_add(1)
        .saturating_add(2)
        .saturating_add(app.open_name_input.len() as u16)
        .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
    let cursor_y = inner
        .y
        .saturating_add(1)
        .min(inner.y.saturating_add(area.height.saturating_sub(1)));
    f.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn draw_confirm_dialog(f: &mut Frame, area: Rect, app: &App) {
    let title = "Confirm";
    let hint = "(Enter = Yes, Esc = No)";
    let longest = title.len().max(app.confirm_message.len()).max(hint.len());

    let horizontal_padding = 2u16;
    let min_w = 20u16;
    let max_w = area.width.saturating_sub(2);
    let w = ((longest as u16) + 2  + horizontal_padding).clamp(min_w, max_w);

    let h = 5u16; // title + message + hint + borders
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, rect);

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::White)))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    f.render_widget(block.clone(), rect);

    let inner = block.inner(rect);

    // Center the hint within the inner width
    let hint_centered = if (hint.len() as u16) < inner.width {
        let pad = ((inner.width as usize).saturating_sub(hint.len())) / 2;
        format!("{}{}", " ".repeat(pad), hint)
    } else {
        hint.to_string()
    };

    let lines = vec![
        Line::raw(format!(" {}", app.confirm_message)),
        Line::from(Span::styled(hint_centered, Style::default().fg(Color::Gray))),
    ];
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(paragraph, inner);
}

fn handle_key(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    // When modal is open, it captures all keys
    if app.show_save_dialog {
        handle_save_dialog_key(app, key)?;
        return Ok(false);
    }
    if app.show_open_dialog {
        handle_open_dialog_key(app, key)?;
        return Ok(false);
    }
    if app.show_confirm_dialog {
        handle_confirm_dialog_key(app, key)?;
        return Ok(false);
    }

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
                // Open file: show path prompt
                app.show_open_dialog = true;
                app.open_error = None;
                // Prefill with current filename if present
                app.open_name_input = app.filename.clone().unwrap_or_default();
                return Ok(false);
            }
            KeyCode::Char('s') => {
                // Save current file
                if app.filename.is_none() {
                    app.show_save_dialog = true;
                    app.save_name_input = "untitled.bf".to_string();
                    app.save_error = None;
                } else {
                    match app_save_current(app) {
                        Ok(_) => { /* saved; dirty cleared */ }
                        Err(err) => {
                            // TODO: show status message
                            eprintln!("Save failed: {}", err);
                        }
                    }
                }
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
        set_status(app, "Nothing to run");
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
    set_status(app, "Running...");
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

fn output_display_lines(app: &App) -> u16 {
    if app.output.is_empty() {
        return 1;
    }
    let line_count = match app.output_mode {
        OutputMode::Raw => {
            let s = String::from_utf8_lossy(&app.output);
            let n = s.lines().count();
            n.max(1)
        }
        OutputMode::Escaped => {
            let s = bytes_to_escaped(&app.output);
            let n = s.lines().count();
            n.max(1)
        }
    };

    line_count as u16
}

// Open a file into the editor buffer
// Set filename and clear dirty flag on success
fn app_open_file(app: &mut App, path: &Path) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    // Split preserving empty final line if present
    let mut lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    app.buffer = lines;
    app.cursor_row = 0;
    app.cursor_col = 0;
    app.scroll_row = 0;
    app.filename = Some(path.to_string_lossy().to_string());
    app.dirty = false;

    // Clear runtime/output state for new file
    app.output.clear();
    app.tape_ptr = 0;
    app.tape_window_base = 0;
    app.tape_window = [0u8; 128];

    set_status(app, &format!("Opened {}", path.display()));
    Ok(())
}

// Save the current editor buffer to the existing filename
// Errors if no filename is set or on I/O errors
fn app_save_current(app: &mut App) -> io::Result<()> {
    let filename_owned: String;
    let filename = match app.filename.as_deref() {
        Some(p) => p,
        None => {
            // No filename set
            let new_path = generate_new_filename()?;
            let s = new_path.to_string_lossy().to_string();
            app.filename = Some(s.clone());
            filename_owned = s;
            &filename_owned
        }
    };
    let content = app_current_source(app);
    // Ensure parent directory exists or let fs::write return an error
    fs::write(Path::new(filename), content)?;
    app.dirty = false;
    set_status(app, &format!("Saved {}", filename));
    Ok(())
}

// Helper: choose a new default filename in the current directory.
// Tries "untitled.bf", "untitled1.bf", "untitled2.bf", ...
fn generate_new_filename() -> io::Result<PathBuf> {
    let base = std::env::current_dir()?;
    // Start with untitled.bf
    let stem = "untitled";
    let ext = "bf";

    let candidates = {
        let mut p = base.clone();
        p.push(format!("{stem}.{ext}"));
        p
    };

    if !candidates.exists() {
        return Ok(candidates);
    }

    // Try with numeric suffixes
    for i in 1..10_000 {
        let mut p = base.clone();
        p.push(format!("{stem}{i}.{ext}"));
        if !p.exists() {
            return Ok(p);
        }
    }

    // If we ran out of attempts, return an error
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "Unable to generate new filename"))
}

// Helper to save to a provided filename (relative or absolute)
fn save_to_filename(app: &mut App, name: &str) -> io::Result<()> {
    let mut path = PathBuf::from(name);
    if path.is_relative() {
        path = std::env::current_dir()?.join(path);
    }
    let content = app_current_source(app);
    fs::write(&path, content)?;
    app.filename = Some(path.to_string_lossy().to_string());
    app.dirty = false;
    Ok(())
}

fn handle_save_dialog_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.show_save_dialog = false;
            app.save_error = None;
        }
        KeyCode::Enter => {
            let name = app.save_name_input.trim().to_string();
            if name.is_empty() {
                app.save_error = Some("File name cannot be empty".to_string());
            } else {
                match save_to_filename(app, &name) {
                    Ok(_) => {
                        set_status(app, &format!("Saved {}", name));
                        app.show_save_dialog = false;
                        app.save_error = None;
                    }
                    Err(err) => {
                        app.save_error = Some(format!("Save failed: {}", err));
                        set_status(app, "Save failed");
                    }
                }
            }
        }
        KeyCode::Backspace => {
            app.save_name_input.pop();
        }
        KeyCode::Delete => {
            // no-op (simple input)
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
            // no-op (simple input)
        }
        KeyCode::Char(ch) => {
            if key.modifiers.is_empty() && !is_control_char(ch) {
                app.save_name_input.push(ch);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_open_dialog_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.show_open_dialog = false;
            app.open_error = None;
        }
        KeyCode::Enter => {
            let name = app.open_name_input.trim().to_string();
            if name.is_empty() {
                app.open_error = Some("Path cannot be empty".to_string());
            } else {
                // Resolve to absolute path for consistent filename display
                let mut path = PathBuf::from(&name);
                if path.is_relative() {
                    path = std::env::current_dir()?.join(path);
                }

                // If there are unsaved changes, ask for confirmation first
                if app.dirty {
                    app.confirm_message = "You have unsaved changes. Open anyway? Unsaved changes will be lost.".to_string();
                    app.confirm_pending_open = Some(path);
                    app.show_open_dialog = false;
                    app.show_confirm_dialog = true;
                } else {
                    match app_open_file(app, &path) {
                        Ok(_) => {
                            app.show_open_dialog = false;
                            app.open_error = None;
                        }
                        Err(err) => {
                            app.open_error = Some(format!("Open failed: {}", err));
                            set_status(app, "Open failed");
                        }
                    }
                }
            }
        }
        KeyCode::Backspace => {
            app.open_name_input.pop();
        }
        KeyCode::Delete => {
            // no-op (simple input)
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
            // no-op (simple input)
        }
        KeyCode::Char(ch) => {
            if key.modifiers.is_empty() && !is_control_char(ch) {
                app.open_name_input.push(ch);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_dialog_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => {
            if let Some(path) = app.confirm_pending_open.take() {
                // Attempt to open; on error, return to open dialog with error message
                match app_open_file(app, &path) {
                    Ok(_) => {
                        app.show_confirm_dialog = false;
                        app.open_error = None;
                        app.show_confirm_dialog = false;
                    }
                    Err(err) => {
                        app.open_error = Some(format!("Open failed: {}", err));
                        app.show_confirm_dialog = false;
                        app.show_open_dialog = true;
                        set_status(app, "Open failed");
                    }
                }
            } else {
                // No pending action; just close
                app.show_confirm_dialog = false;
            }

        }
        KeyCode::Esc => {
            // Cancel; return to the open dialog if we came from there
            app.show_confirm_dialog = false;
            if app.confirm_pending_open.is_some() {
                app.show_open_dialog = true;
            }
            // Keep the pending path so the user can adjust; or clear it
            // Clear to avoid accidental reuse
            app.confirm_pending_open = None;
        }
        _ => {}
    }
    Ok(())
}

// Compute current cell value if the pointer is within the current 128-cell window
fn current_cell_value(app: &App) -> Option<u8> {
    let base = app.tape_window_base;
    let end = base.saturating_add(128);
    if app.tape_ptr >= base && app.tape_ptr < end {
        let idx = app.tape_ptr - base;
        Some(app.tape_window[idx])
    } else {
        None
    }
}

// Helper: set a status message
fn set_status(app: &mut App, status: &str) {
    app.status_message = Some((status.to_string(), Instant::now()));
}
