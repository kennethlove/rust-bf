use std::io::{self, Write};
use std::path::PathBuf;
use clap::Args;
use crate::ide::run_with_options;

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
pub struct IdeArgs {
    /// Accept a file name to load on startup
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub filename: Option<String>,

    /// Enable Vi mode (default is Emacs mode)
    #[arg(short = 'v', long = "vi", action = clap::ArgAction::SetTrue)]
    pub vi_mode: bool,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    pub help: bool,
}


// Public entry point for the TUI from main.rs
pub fn run(program: &str, help: bool, filename: Option<PathBuf>, vi_mode: bool) -> i32 {
    if help {
        usage_and_exit(program, 0);
    } else {
        let _ = run_with_options(filename, vi_mode);
    }
    0
}

fn usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} ide   # Start a Brainfuck Terminal IDE (read-eval-print loop)

Options:
  --help,   -h        Show this help
  --file,   -f        Optional file to load on startup
  --vi,     -v        Enable Vi mode (default is Emacs mode)

Description:
  Starts a terminal IDE where you can enter Brainfuck code and execute it live.

Notes:
    - Non-Brainfuck characters are ignored; only valid instructions are executed.
    - Ctrl+R executes the current buffer
    - Ctrl+S saves the current buffer to a file
    - Ctrl+O opens a file into the current buffer
    - Ctrl+L toggles line numbers on/off (on by default)
    - Ctrl+N creates a new empty buffer
    - Ctrl+P jumps to matching bracket
    - Ctrl+Q exits the IDE; if there are unsaved changes, you will be asked to confirm.
"#,
        program
    );
    let _ = io::stderr().flush();
    std::process::exit(code);
}
