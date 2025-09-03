use clap::{Parser, Subcommand};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

fn print_top_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} read  [--debug|-d] "<code>"      # Run Brainfuck code (args are concatenated)
  {0} read  [--debug|-d] --file <PATH> # Run Brainfuck code loaded from file
  {0} write [--bytes] [TEXT...]        # Generate Brainfuck to print TEXT/STDIN/file
  {0} write [--bytes] --file <PATH>    # Generate Brainfuck to print file contents
  {0} repl                             # Start a Brainfuck REPL (read-eval-print loop)
  {0} ide                              # Start a terminal-based Brainfuck IDE

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
    Read(bf::commands::read::ReadArgs),
    Write(bf::commands::write::WriteArgs),
    Repl(bf::commands::repl::ReplArgs),
    Ide(bf::commands::ide::IdeArgs),
}

fn main() {
    // We still pull the program name for help rendering consistency
    let program = env::args().next().unwrap_or_else(|| String::from("bf"));

    let cli = Cli::parse();
    
    if cli.help {
        print_top_usage_and_exit(&program, 0);
    }

    let code = match cli.command {
        Some(Command::Read(args)) => bf::commands::read::run(&program, args),
        Some(Command::Write(args)) => bf::commands::write::run(&program, args),
        Some(Command::Repl(args)) => {
            let program = "repl";
            let mode_flag = if args.bare {
                bf::repl::ModeFlagOverride::Bare
            } else if args.editor {
                bf::repl::ModeFlagOverride::Editor
            } else {
                bf::repl::ModeFlagOverride::None
            };
            
            let code = bf::commands::repl::run(&program, false, mode_flag);
            std::process::exit(code);
        },
        Some(Command::Ide(args)) => {
            let filename = if let Some(filename) = &args.filename {
                if args.help {
                    eprintln!("Error: --help cannot be used with --file");
                    std::process::exit(1);
                }
                Some(PathBuf::from(filename))
            } else {
                None
            };

            bf::commands::ide::run(&program, args.help, filename, args.vi_mode)
        }
        None => {
            // Default to REPL when no subcommand is provided
            bf::commands::repl::run(&program, false, bf::repl::ModeFlagOverride::None)
        }
    };

    std::process::exit(code);
}
