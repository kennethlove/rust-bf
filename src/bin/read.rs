use rust_bf::BrainfuckReader;
use std::env;

fn print_usage_and_exit(program: &str) -> ! {
    eprintln!(
        "Usage:\n  {0} [--debug|-d] \"<code>\"   # Run Brainfuck code (args are concatenated)\n\nOptions:\n  --debug, -d   Print a step-by-step table of operations instead of executing\n  --help,  -h   Show this help\n\nNotes:\n- Input (`,`) reads a single byte from stdin; on EOF the current cell is set to 0.\n- Any characters outside of Brainfuck's ><+-.,[] will result in an error.\n",
        program
    );
    std::process::exit(2);
}

fn main() {
    // Collect all CLI args after the program name
    let args: Vec<String> = env::args().skip(1).collect();
    let program = env::args().next().unwrap_or_else(|| "bf_runner".to_string());

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_usage_and_exit(&program);
    }

    // Parse flags and build code by concatenating non-flag args
    let mut debug = false;
    let mut code_parts: Vec<String> = Vec::new();
    for a in args {
        match a.as_str() {
            "--debug" | "-d" => debug = true,
            "--help" | "-h" => {
                print_usage_and_exit(&program);
            }
            _ => code_parts.push(a),
        }
    }

    if code_parts.is_empty() {
        print_usage_and_exit(&program);
    }

    let code = code_parts.join("");

    let mut bf = BrainfuckReader::new(code);

    let result = if debug { bf.run_debug() } else { bf.run() };

    if let Err(err) = result {
        eprintln!("Brainfuck interpreter error: {:?}", err);
        std::process::exit(1);
    }

    // For readability, ensure output ends with a newline
    println!();
}
