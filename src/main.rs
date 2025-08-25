mod repl;
mod read;
mod write;
mod cli_util;
use clap::{Args, Parser, Subcommand};
use std::env;
use std::io::{self, Write};

fn print_top_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} read  [--debug|-d] "<code>"      # Run Brainfuck code (args are concatenated)
  {0} read  [--debug|-d] --file <PATH> # Run Brainfuck code loaded from file
  {0} write [--bytes] [TEXT...]        # Generate Brainfuck to print TEXT/STDIN/file
  {0} write [--bytes] --file <PATH>    # Generate Brainfuck to print file contents
  {0} repl                             # Start a Brainfuck REPL (read-eval-print loop)

Run "{0} <subcommand> --help" for more info.
"#,
        program
    );
    let _ = io::stderr().flush();
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
    Read(read::ReadArgs),
    Write(write::WriteArgs),
    Repl(ReplArgs),
}

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
struct ReplArgs {
    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    help: bool,
}

fn main() {
    // We still pull the program name for help rendering consistency
    let program = env::args().next().unwrap_or_else(|| String::from("bf"));

    let cli = Cli::parse();

    if cli.help {
        print_top_usage_and_exit(&program, 0);
    }

    let code = match cli.command {
        Some(Command::Read(args)) => read::run(&program, args),
        Some(Command::Write(args)) => write::run(&program, args),
        Some(Command::Repl(args)) => repl::run(&program, args.help),
        None => {
            // Default to REPL when no subcommand is provided
            repl::run(&program, false)
        }
    };

    std::process::exit(code);
}
