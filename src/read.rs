use clap::Args;
use std::fs;
use std::io::{self, Write};
use crate::BrainfuckReader;
use crate::cli_util::print_reader_error;

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

    // Execute the original code so that error ip matches the original source
    let mut bf = BrainfuckReader::new(code_str.clone());
    let result = if debug { bf.run_debug() } else { bf.run() };

    if let Err(err) = result {
        print_reader_error(Some(program), &code_str, &err);
        let _ = std::io::stderr().flush();
        return 1;
    }

    // For readability, ensure output ends with a newline
    println!();
    let _ = io::stdout().flush();
    0
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

