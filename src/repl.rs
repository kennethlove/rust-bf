use bf::BrainfuckReader;
use std::env;
use std::io::{self, Write};
use crate::cli_util::print_reader_error;

// Public entry point for the REPL from main.rs
pub fn run(program: &str, help: bool) -> i32 {
    if help {
        usage_and_exit(program, 0);
    }

    // Install SIGINT (ctrl+c) handler to flush and exit(0) immediately
    if let Err(e) = ctrlc::set_handler(|| {
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(0);
    }) {
        eprintln!("{program}: failed to set ctrl+c handler: {e}");
        let _ = std::io::stderr().flush();
        return 1;
    }

    println!("Brainfuck REPL");
    println!("Ctrl+d/Ctrl+z Enter (Windows) executes the current buffer. Press ctrl+c to exit");

    if let Err(e) = repl_loop() {
        eprintln!("{program}: REPL error: {e}");
        let _ = io::stderr().flush();
        return 1;
    }
    0
}

fn usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} repl   # Start a Brainfuck REPL (read-eval-print loop)

Options:
  --help,   -h        Show this help

Description:
  Starts a REPL where you can enter Brainfuck code and execute it live.

Notes:
    - Ctrl+d executes the current buffer on *nix/macOS.
    - Ctrl+z and Enter will execute the current buffer on Windows.
    - Ctrl+c exits the REPL immediately.
    - The REPL will print a newline after each execution for readability.
    - Each execution starts with a fresh memory and pointer.
    - The REPL will exit after a single execution if the environment variable `BF_REPL_ONCE` is set to `1`.
"#,
        program
    );
    let _ = io::stderr().flush();
    std::process::exit(code);
}

fn repl_loop() -> io::Result<()> {
    loop {
        let mut stdin = io::stdin().lock();

        // Prompt
        print!("bf> ");
        io::stdout().flush()?;

        let submission = read_submission(&mut stdin);
        if submission.is_none() {
            // EOF or empty input: end the session cleanly to avoid hanging when stdin is closed
            println!();
            io::stdout().flush()?;
            return Ok(());
        }
        let submission = submission.unwrap();

        let trimmed = submission.trim();
        if trimmed.is_empty() {
            continue;
        }

        let filtered = bf_only(&trimmed);

        if filtered.is_empty() {
            continue;
        }

        // Execute the Brainfuck code buffer
        execute_bf_buffer(filtered);

        // Test hook: if BF_REPL_ONCE is set, exit after single execution to allow integration testing
        if env::var("BF_REPL_ONCE").ok().as_deref() == Some("1") {
            return Ok(());
        }
    }
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
        print_reader_error(None, &buffer, &err);
        let _ = io::stderr().flush();
    }
    println!();
    let _ = io::stdout().flush(); // Ensure output is flushed
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
