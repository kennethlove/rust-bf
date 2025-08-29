use std::io::{self, Write};
use std::path::PathBuf;
use clap::Args;
use crate::tui::run_with_file;

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
pub struct TuiArgs {
    /// Accept a file name to load on startup
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub filename: Option<String>,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    pub help: bool,
}


// Public entry point for the TUI from main.rs
pub fn run(program: &str, help: bool, filename: Option<PathBuf>) -> i32 {
    if help {
        usage_and_exit(program, 0);
    } else {
        let _ = run_with_file(filename);
    }
    0
}

fn usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} tui   # Start a Brainfuck Terminal IDE (read-eval-print loop)

Options:
  --help,   -h        Show this help
  --file,   -f        Optional file to load on startup

Description:
  Starts a terminal IDE where you can enter Brainfuck code and execute it live.

Notes:
    - Non-Brainfuck characters are ignored; only valid instructions are executed.
    - Ctrl+R executes the current buffer
    - Ctrl+Q exits the IDE immediately.
"#,
        program
    );
    let _ = io::stderr().flush();
    std::process::exit(code);
}
