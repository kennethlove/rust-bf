use rust_bf::{BrainfuckReader, BrainfuckWriter};
use std::env;
use std::fs;
use std::io::{self, Read};

fn print_top_usage_and_exit(program: &str, code: i32) -> ! {
    eprintln!(
        r#"Usage:
  {0} read  [--debug|-d] "<code>"     # Run Brainfuck code (args are concatenated)
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
        "Usage:\n  {0} read [--debug|-d] \"<code>\"\n\nOptions:\n  --debug, -d   Print a step-by-step table of operations instead of executing\n  --help,  -h   Show this help\n\nNotes:\n- Input (`,`) reads a single byte from stdin; on EOF the current cell is set to 0.\n- Any characters outside of Brainfuck's ><+-.,[] will result in an error.\n",
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

fn run_read<I>(program: &str, args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    let mut debug = false;
    let mut code_parts: Vec<String> = Vec::new();

    let mut saw_any = false;
    for a in args {
        saw_any = true;
        match a.as_str() {
            "--debug" | "-d" => debug = true,
            "--help" | "-h" => return { read_usage_and_exit(program, 0) },
            _ => code_parts.push(a),
        }
    }

    if !saw_any || code_parts.is_empty() {
        read_usage_and_exit(program, 2);
    }

    let code = code_parts.join("");
    let mut bf = BrainfuckReader::new(code);
    let result = if debug { bf.run_debug() } else { bf.run() };

    if let Err(err) = result {
        eprintln!("{program}: Brainfuck interpreter error: {:?}", err);
        return 1;
    }

    // For readability, ensure output ends with a newline
    println!();
    0
}

fn run_write<I>(program: &str, mut args: I) -> i32
where
    I: Iterator<Item = String>,
{
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
            "--debug" | "-d" => {
                eprintln!("{program}: 'write' does not support --debug/-d");
                return { write_usage_and_exit(program, 2) };
            }
            "--bytes" => use_bytes = true,
            "--file" | "-f" => {
                let Some(p) = args.next() else {
                    eprintln!("{program}: --file requires a path");
                    return { write_usage_and_exit(program, 2) };
                };
                file_path = Some(p);
            }
            other => {
                // If it looks like a flag, but we didn't recognize it, error out
                if other.starts_with('-') {
                    eprintln!("{program}: unknown flag '{other}' for 'write'");
                    return { write_usage_and_exit(program, 2) };
                }
                text_parts.push(other.to_string());
            }
        }
    }

    if show_help {
        return { write_usage_and_exit(program, 0) };
    }

    if file_path.is_some() && !text_parts.is_empty() {
        eprintln!("{program}: cannot use positional TEXT together with --file");
        return { write_usage_and_exit(program, 2) };
    }

    // Acquire input bytes according to mode
    let input_bytes: Vec<u8> = match file_path {
        Some(path) => {
            if use_bytes {
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
            if !text_parts.is_empty() {
                text_parts.join(" ").into_bytes()
            } else if use_bytes {
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

fn main() {
    let mut args = env::args();
    let program = args
        .next()
        .unwrap_or_else(|| String::from("bf"));

    let Some(subcmd) = args.next() else {
        print_top_usage_and_exit(&program, 2);
    };

    let code = match subcmd.as_str() {
        "read" => run_read(&program, args.map(|s| s)),
        "write" => run_write(&program, args),
        "--help" | "-h" => { print_top_usage_and_exit(&program, 0) },
        other => {
            eprintln!("{program}: unknown subcommand '{other}'");
            print_top_usage_and_exit(&program, 2);
        }
    };

    std::process::exit(code);
}
