use bf::BrainfuckWriter;
use clap::Args;
use std::fs;
use std::io::{self, Read, Write};

#[derive(Args, Debug)]
#[command(disable_help_flag = true)]
pub struct WriteArgs {
    /// Treat input as raw bytes (no UTF-8 required)
    #[arg(long = "bytes")]
    pub bytes: bool,

    /// Read input from file at PATH (otherwise reads from TEXT or STDIN)
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,

    /// Positional text (UTF-8). If omitted, reads from STDIN.
    #[arg(value_name = "TEXT", trailing_var_arg = true)]
    pub text: Vec<String>,

    /// Show this help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    pub help: bool,
}

pub fn run(program: &str, args: WriteArgs) -> i32 {
    if args.help {
        usage_and_exit(program, 0);
    }

    let WriteArgs {
        bytes,
        file,
        text,
        ..
    } = args;

    if file.is_some() && !text.is_empty() {
        eprintln!("{program}: cannot use positional TEXT together with --file");
        usage_and_exit(program, 2);
    }

    let input_bytes: Vec<u8> = match file {
        Some(path) => {
            if bytes {
                match fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("{program}: failed to read file: {e}");
                        let _ = io::stderr().flush();
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
                        let _ = io::stderr().flush();
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
                    let _ = io::stderr().flush();
                    return 1;
                }
                buf
            } else {
                let mut s = String::new();
                if let Err(e) = io::stdin().read_to_string(&mut s) {
                    eprintln!(
                        "{program}: failed reading UTF-8 from stdin (use --bytes for binary): {e}"
                    );
                    let _ = io::stderr().flush();
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
            let _ = io::stdout().flush();
            0
        }
        Err(err) => {
            eprintln!("{program}: error generating Brainfuck: {:?}", err);
            let _ = io::stderr().flush();
            1
        }
    }
}

fn usage_and_exit(program: &str, code: i32) -> ! {
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
    let _ = io::stderr().flush();
    std::process::exit(code);
}
