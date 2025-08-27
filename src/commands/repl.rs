use std::io::{self, IsTerminal, Write};
use clap::Args;

use crate::repl::{execute_bare_once, repl_loop, select_mode, ReplMode, ModeFlagOverride};

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
pub struct ReplArgs {
    /// Force non-interactive bare mode
    #[arg(long = "bare", conflicts_with = "editor")]
    pub bare: bool,

    /// Force interactive mode (errors if stdin is not a TTY)
    #[arg(long = "editor", conflicts_with = "bare")]
    pub editor: bool,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,
}


// Public entry point for the REPL from main.rs
pub fn run(program: &str, help: bool, mode_flag: ModeFlagOverride) -> i32 {
    if help {
        usage_and_exit(program, 0);
    }

    // Determine mode: flags -> env -> auto-detect via is_terminal()
    let mode = match select_mode(mode_flag) {
        Ok(m) => m,
        Err(msg) => {
            eprintln!("{program}: {msg}");
            let _ = io::stderr().flush();
            return 1;
        }
    };

    // Install SIGINT (ctrl+c) handler to flush and exit(0) immediately
    if let Err(e) = ctrlc::set_handler(|| {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        std::process::exit(0);
    }) {
        eprintln!("{program}: failed to set ctrl+c handler: {e}");
        let _ = io::stderr().flush();
        return 1;
    }

    match mode {
        ReplMode::Editor => {
            // Print banners/prompts only if stderr is a TTY
            if io::stderr().is_terminal() {
                eprintln!("Brainfuck REPL (interactive editor mode)");
                eprintln!("Ctrl+d/Ctrl+z Enter (Windows) executes the current buffer. Press ctrl+c to exit");
                let _ = io::stderr().flush();
            }

            if let Err(e) = repl_loop() {
                eprintln!("{program}: REPL error: {e}");
                let _ = io::stderr().flush();
                return 1;
            }

            0
        }
        ReplMode::Bare => {
            // Bare mode: read stdin until EOF, execute once, exit 0
            match execute_bare_once() {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("{program}: REPL error: {e}");
                    let _ = io::stderr().flush();
                    1
                }
            }
        }
    }
}

fn usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} repl   # Start a Brainfuck REPL (read-eval-print loop)

Options:
  --help,   -h        Show this help
  --bare              Force non-interactive bare mode
  --editor            Force interactive editor mode (errors if stdin is not a TTY)

Description:
  Starts a REPL where you can enter Brainfuck code and execute it live.

Meta commands (line starts with ":")
  :exit            Exit immediately (code 0)
  :help            Show this help
  :reset           Clear current buffer (history is preserved)
  :dump            Print buffer (content → stdout; framing → stderr)
    -n             Include line numbers (stdout)
    --stderr       Send everything to stderr

Notes:
    - While editing, non-Brainfuck characters are ignored; only valid instructions are executed.
    - Ctrl+D executes the current buffer on *nix/macOS.
    - Ctrl+Z and Enter will execute the current buffer on Windows.
    - Ctrl+C exits the REPL immediately.
    - The REPL will print a newline after each execution for readability.
    - Each execution starts with a fresh memory and pointer.
    - The REPL will exit after a single execution if the environment variable `BF_REPL_ONCE` is set to `1`.
    - Mode selection:
        * Flags: --bare|--editor override environment and auto-detection.
        * Env: BF_REPL_MODE=bare|editor overrides auto-detection (flags, when preset, will override env).
        * Auto-detect: if stdin is a TTY, starts in interactive editor mode; otherwise, bare mode.
        * Prompts/banners suppressed if stderr is not a TTY.

"#,
        program
    );
    let _ = io::stderr().flush();
    std::process::exit(code);
}
