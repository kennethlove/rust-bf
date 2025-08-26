use clap::Args;
use std::{fs, thread};
use std::io::{self, Write};
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crate::{BrainfuckReader, BrainfuckReaderError};
use crate::cli_util::print_reader_error;
use crate::reader::StepControl;

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
pub struct ReadArgs {
    /// Print a step-by-step table of operations instead of executing
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Read Brainfuck code from PATH instead of positional "<code>"
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,

    /// Concatenated Brainfuck code parts
    #[arg(value_name = "code", trailing_var_arg = true)]
    pub code: Vec<String>,

    /// Wall-clock timeout in milliseconds (fallback BF_TIMEOUT_MS; default 2_000)
    #[arg(long = "timeout", value_name = "MS")]
    pub timeout_ms: Option<u64>,

    /// Maximum interpreter steps before abort (fallback BF_MAX_STEPS; default unlimited)
    #[arg(long = "max-steps", value_name = "N")]
    pub max_steps: Option<u64>,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    pub help: bool,
}

pub fn run(program: &str, args: ReadArgs) -> i32 {
    if args.help {
        usage_and_exit(program, 0);
    }

    let ReadArgs {
        debug,
        file,
        code,
        timeout_ms,
        max_steps,
        ..
    } = args;

    if file.is_none() && code.is_empty() {
        usage_and_exit(program, 2);
    }

    if file.is_some() && !code.is_empty() {
        eprintln!("{program}: cannot use positional code together with --file");
        usage_and_exit(program, 2);
    }

    let code_str = if let Some(path) = file {
        match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{program}: failed to read code file as UTF-8: {e}");
                let _ = io::stderr().flush();
                return 1;
            }
        }
    } else {
        code.join("")
    };

    // Resolve limits: flags -> env -> defaults
    let timeout_ms = timeout_ms
        .or_else(|| std::env::var("BF_TIMEOUT_MS").ok().and_then(|s| s.parse::<u64>().ok()))
        .unwrap_or(2_000);
    let max_steps = max_steps
        .or_else(|| std::env::var("BF_MAX_STEPS").ok().and_then(|s| s.parse::<u64>().ok()));

    // Execute on a worker thread with cooperative cancellation
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Result<(), BrainfuckReaderError>>();
    let program_owned = code_str.clone();
    let cancel_clone = cancel.clone();

    thread::spawn(move || {
        let max_steps: usize = max_steps.unwrap_or(usize::MAX as u64) as usize;
        let mut bf = BrainfuckReader::new(program_owned);
        let ctrl = StepControl::new(Some(max_steps), cancel_clone);
        let res = if debug {
            bf.run_debug_with_control(ctrl)
        } else {
            bf.run_with_control(ctrl)
        };
        let _ = tx.send(res);
    });

    let timeout = Duration::from_millis(timeout_ms);
    let exit_code = match rx.recv_timeout(timeout) {
        Ok(Ok(())) => 0,
        Ok(Err(BrainfuckReaderError::StepLimitExceeded { limit })) => {
            eprintln!("Execution aborted: step limit exceeded ({limit})");
            let _ = io::stderr().flush();
            1
        }
        Ok(Err(BrainfuckReaderError::Canceled)) => {
            eprintln!("Execution aborted: wall-clock timeout exceeded ({timeout_ms} ms)");
            let _ = io::stderr().flush();
            1
        }
        Ok(Err(other)) => {
            print_reader_error(Some(program), &code_str, &other);
            let _ = io::stderr().flush();
            1
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel.store(true, Ordering::Relaxed);
            eprintln!("Execution aborted: wall-clock timeout exceeded ({timeout_ms} ms)");
            let _ = io::stderr().flush();
            1
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => 1,
    };

    println!();
    let _ = io::stdout().flush();
    exit_code
}

fn usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} read [--debug|-d] "<code>"
  {0} read [--debug|-d] --file <PATH>

Options:
  --file,  -f <PATH>  Read Brainfuck code from PATH instead of positional "<code>"
  --debug, -d   Print a step-by-step table of operations instead of executing
  --help,  -h   Show this help

Notes:
- Input (`,`) reads a single byte from stdin; on EOF the current cell is set to 0.
- Any characters outside of Brainfuck's ><+-.,[] will result in an error.

Examples:
- Load Brainfuck code from a file:
    {0} read --file ./program.bf
- Read bytes from a file as stdin (`,` will consume file input):
    {0} read ",[.,]" < input.txt
"#,
        program
    );
    let _ = io::stderr().flush();
    std::process::exit(code);
}

