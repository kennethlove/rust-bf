use rust_bf::Brainfuck;
use std::env;

fn print_usage_and_exit(program: &str) -> ! {
    eprintln!(
        "Usage:\n  {0} \"<code>\"   # Run Brainfuck code provided as CLI arguments (they will be concatenated)\n\nNotes:\n- Input (`,`) is not supported by this interpreter and will result in an error.\n- Any characters outside of Brainfuck's ><+-.,[] will result in an error.\n",
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

    // Concatenate all args without spaces to form the Brainfuck code
    let code = args.join("");

    let mut bf = Brainfuck::new(code);

    if let Err(err) = bf.run() {
        eprintln!("Brainfuck interpreter error: {:?}", err);
        std::process::exit(1);
    }

    // For readability, ensure output ends with a newline
    println!();
}
