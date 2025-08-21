use bf::{BrainfuckReader, BrainfuckWriter};
use clap::{Args, Parser, Subcommand};
use std::env;
use std::fs;
use std::io::{self, BufRead, Read, Write};

fn print_top_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} read  [--debug|-d] "<code>"     # Run Brainfuck code (args are concatenated)
  {0} read  [--debug|-d] --file <PATH> # Run Brainfuck code loaded from file
  {0} write [--bytes] [TEXT...]        # Generate Brainfuck to print TEXT/STDIN/file
  {0} write [--bytes] --file <PATH>

Run "{0} <subcommand> --help" for more info.
"#,
        program
    );
    std::process::exit(code);
}

fn read_usage_and_exit(program: &str, code: i32) -> ! {
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
    std::process::exit(code);
}

fn write_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} write [--bytes] [TEXT...]           # Read UTF-8 TEXT args (preferred) or from STDIN if no TEXT is given
  {0} write [--bytes] --file <PATH>       # Read from file instead of STDIN

Options:
  --file,  -f <PATH>  Read input from file at PATH (otherwise reads from TEXT or STDIN)
  --bytes             Treat input as raw bytes (no UTF-8 required)
  --help,   -h        Show this help

Description:
  Generates Brainfuck code that, when executed, will output the provided input bytes.

Input modes:
  - Default (string-like): expects UTF-8 text from positional TEXT, STDIN, or file and uses its bytes.
  - --bytes (byte-like): reads raw bytes from STDIN or file; positional TEXT is still accepted as UTF-8 and used as bytes.

Notes:
  - Output is Brainfuck code printed to stdout followed by a newline.
"#,
        program
    );
    std::process::exit(code);
}

#[derive(Parser, Debug)]
#[command(name = "bf", disable_help_flag = true, disable_help_subcommand = true)]
struct Cli {
    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Read(ReadArgs),
    Write(WriteArgs),
    Repl(ReplArgs),
}

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
struct ReadArgs {
    /// Print a step-by-step table of operations instead of executing
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// Read Brainfuck code from PATH instead of positional "<code>"
    #[arg(short = 'f', long = "file")]
    file: Option<String>,

    /// Concatenated Brainfuck code parts
    #[arg(value_name = "code", trailing_var_arg = true)]
    code: Vec<String>,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,
}

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
struct WriteArgs {
    /// Treat input as raw bytes (no UTF-8 required)
    #[arg(long = "bytes")]
    bytes: bool,

    /// Read input from file at PATH (otherwise reads from TEXT or STDIN)
    #[arg(short = 'f', long = "file")]
    file: Option<String>,

    /// Positional text (UTF-8). If omitted, reads from STDIN.
    #[arg(value_name = "TEXT", trailing_var_arg = true)]
    text: Vec<String>,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,
}

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
struct ReplArgs {
    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,
}

fn run_read_with_args(program: &str, args: ReadArgs) -> i32 {
    if args.help {
        read_usage_and_exit(program, 0);
    }

    let ReadArgs {
        debug,
        file,
        code,
        ..
    } = args;

    if file.is_none() && code.is_empty() {
        read_usage_and_exit(program, 2);
    }

    if file.is_some() && !code.is_empty() {
        eprintln!("{program}: cannot use positional code together with --file");
        read_usage_and_exit(program, 2);
    }

    let code_str = if let Some(path) = file {
        match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{program}: failed to read code file as UTF-8: {e}");
                return 1;
            }
        }
    } else {
        code.join("")
    };

    let mut bf = BrainfuckReader::new(code_str);
    let result = if debug { bf.run_debug() } else { bf.run() };

    if let Err(err) = result {
        eprintln!("{program}: Brainfuck interpreter error: {:?}", err);
        return 1;
    }

    // For readability, ensure output ends with a newline
    println!();
    0
}

fn run_write_with_args(program: &str, args: WriteArgs) -> i32 {
    if args.help {
        write_usage_and_exit(program, 0);
    }

    let WriteArgs {
        bytes,
        file,
        text,
        ..
    } = args;

    if file.is_some() && !text.is_empty() {
        eprintln!("{program}: cannot use positional TEXT together with --file");
        write_usage_and_exit(program, 2);
    }

    let input_bytes: Vec<u8> = match file {
        Some(path) => {
            if bytes {
                match fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("{program}: failed to read file: {e}");
                        return 1;
                    }
                }
            } else {
                match fs::read_to_string(&path) {
                    Ok(s) => s.into_bytes(),
                    Err(e) => {
                        eprintln!(
                            "{program}: failed to read file as UTF-8 (use --bytes for binary): {e}"
                        );
                        return 1;
                    }
                }
            }
        }
        None => {
            if !text.is_empty() {
                text.join(" ").into_bytes()
            } else if bytes {
                let mut buf = Vec::new();
                if let Err(e) = io::stdin().lock().read_to_end(&mut buf) {
                    eprintln!("{program}: failed reading stdin: {e}");
                    return 1;
                }
                buf
            } else {
                let mut s = String::new();
                if let Err(e) = io::stdin().read_to_string(&mut s) {
                    eprintln!(
                        "{program}: failed reading UTF-8 from stdin (use --bytes for binary): {e}"
                    );
                    return 1;
                }
                s.into_bytes()
            }
        }
    };

    let writer = BrainfuckWriter::new(&input_bytes);
    match writer.generate() {
        Ok(code) => {
            println!("{}", code);
            0
        }
        Err(err) => {
            eprintln!("{program}: error generating Brainfuck: {:?}", err);
            1
        }
    }
}

/// Executes a single Brainfuck program contained in `buffer`.
/// - Program output goes to stdout.
/// - Errors are printed concisely to stderr.
/// - A newline is always written to stdout after execution (success or error)
///   so that the prompt begins at column 0 on the next iteration.
fn execute_bf_buffer(buffer: String) {
    // Create a reader and run the program
    let mut bf = BrainfuckReader::new(buffer.to_string());
    if let Err(err) = bf.run() {
        eprintln!("Error: {:?}", err);
    }
    println!();
    let _ = io::stdout().flush(); // Ensure output is flushed
}

fn run_repl_with_args(program: &str, args: ReplArgs) -> i32 {
    if args.help {
        read_usage_and_exit(program, 0);
    }

    println!("Brainfuck REPL");
    println!("Ctrl+d executes the current buffer. Press ctrl+c to exit");

    repl_loop().unwrap();
    0
}

fn repl_loop() -> io::Result<()> {
    loop {
        let mut stdin = io::stdin().lock();

        // Read a line of Brainfuck code from stdin
        print!("bf> ");
        io::stdout().flush()?;

        let submission = read_submission(&mut stdin);
        if submission.is_none() {
            // EOF or empty input
            println!();
            io::stdout().flush()?;
            continue;
        }
        let submission = submission.unwrap();

        let trimmed = submission.trim();
        if trimmed.is_empty() {
            continue;
        }

        let filtered: String = trimmed
            .chars()
            .filter(|c| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']'))
            .collect();

        if filtered.is_empty() {
            continue;
        }

        // Execute the Brainfuck code in the line
        execute_bf_buffer(filtered);

        // Test hook: if BF_REPL_ONCE is set, exit after single execution to allow integration testing
        if std::env::var("BF_REPL_ONCE").ok().as_deref() == Some("1") {
            return Ok(());
        }
    }
}

fn read_submission<R: io::BufRead>(stdin: &mut R) -> Option<String> {
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

fn main() {
    // We still pull the program name for help rendering consistency
    let program = env::args().next().unwrap_or_else(|| String::from("bf"));

    let cli = Cli::parse();

    if cli.help || cli.command.is_none() {
        print_top_usage_and_exit(&program, if cli.help { 0 } else { 2 });
    }

    let code = match cli.command.unwrap() {
        Command::Read(args) => run_read_with_args(&program, args),
        Command::Write(args) =>  run_write_with_args(&program, args),
        Command::Repl(args) => run_repl_with_args(&program, args),
    };

    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_submission_reads_until_eof_multiple_lines() {
        let input = b"+++\n>+.\n";
        let mut cursor = Cursor::new(&input[..]);
        let got = super::read_submission(&mut cursor);
        assert_eq!(got.as_deref(), Some("+++\n>+.\n"));
    }

    #[test]
    fn read_submission_empty_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let got = super::read_submission(&mut cursor);
        assert!(got.is_none());
    }
}
