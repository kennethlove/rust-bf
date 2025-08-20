use rust_bf::BrainfuckWriter;
use std::env;
use std::fs;
use std::io::{self, Read};

fn print_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(r#"Usage:
  {0} [--bytes] [TEXT...]           # Read UTF-8 TEXT args (preferred) or from STDIN if no TEXT is given
  {0} [--bytes] --file <PATH>       # Read from file instead of STDIN

Options:
  --file,  -f <PATH>  Read input from file at PATH (otherwise reads from TEXT or STDIN)
  --bytes             Treat input as raw bytes (no UTF-8 required)
  --help,   -h        Show this help

Description:
  Generates Brainfuck code that, when executed, will output the provided input bytes.

Input modes:
  - Default (string-like): expects UTF-8 text from positional TEXT, STDIN, or file and uses its bytes.
  - --bytes (byte-like): reads raw bytes from STDIN or file; positional TEXT is still accepted as UTF-8 and used as bytes.

Examples:
  # From positional args (recommended when using Cargo, note the "--" separator):
  cargo run --bin write -- "Hello world"

  # From STDIN (UTF-8 text)
  echo -n 'Hello' | {0}

  # From a file
  {0} --file ./message.txt

Notes:
  - Output is Brainfuck code printed to stdout followed by a newline.
  - To run the generated code with this project, use the bf_runner read CLI.
"#, program);
    std::process::exit(code);
}

fn main() {
    let mut args = env::args().skip(1);
    let program = env::args().next().unwrap_or_else(|| "write".to_string());

    let mut show_help = false;
    let mut use_bytes = false;
    let mut file_path: Option<String> = None;
    let mut text_parts: Vec<String> = Vec::new();

    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => {
                show_help = true;
                break;
            }
            "--bytes" => use_bytes = true,
            "--file" | "-f" => {
                let Some(p) = args.next() else {
                    eprintln!("{program}: --file requires a path");
                    print_usage_and_exit(&program, 2);
                };
                file_path = Some(p);
            }
            other => {
                // Collect positional TEXT to be used as input
                text_parts.push(other.to_string());
            }
        }
    }

    if show_help {
        print_usage_and_exit(&program, 0);
    }

    // Disallow mixing --file with positional text
    if file_path.is_some() && !text_parts.is_empty() {
        eprintln!("{program}: cannot use positional TEXT together with --file");
        print_usage_and_exit(&program, 2);
    }

    // Acquire input bytes according to mode
    let input_bytes: Vec<u8> = match file_path {
        Some(path) => {
            if use_bytes {
                match fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("{program}: failed to read file: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                // String-like: ensure UTF-8
                match fs::read_to_string(&path) {
                    Ok(s) => s.into_bytes(),
                    Err(e) => {
                        eprintln!(
                            "{program}: failed to read file as UTF-8 (use --bytes for binary): {e}"
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
        None => {
            if !text_parts.is_empty() {
                // Use positional text
                text_parts.join(" ").into_bytes()
            } else {
                // Read from STDIN
                if use_bytes {
                    let mut buf = Vec::new();
                    if let Err(e) = io::stdin().lock().read_to_end(&mut buf) {
                        eprintln!("{program}: failed reading stdin: {e}");
                        std::process::exit(1);
                    }
                    buf
                } else {
                    let mut s = String::new();
                    if let Err(e) = io::stdin().read_to_string(&mut s) {
                        eprintln!(
                            "{program}: failed reading UTF-8 from stdin (use --bytes for binary): {e}"
                        );
                        std::process::exit(1);
                    }
                    s.into_bytes()
                }
            }
        }
    };

    let writer = BrainfuckWriter::new(&input_bytes);
    match writer.generate() {
        Ok(code) => {
            println!("{}", code);
        }
        Err(err) => {
            eprintln!("{program}: error generating Brainfuck: {:?}", err);
            std::process::exit(1);
        }
    }
}
