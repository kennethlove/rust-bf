use lib::BrainfuckReader;

fn main() {
    // Example: demonstrate debug mode (no real I/O; prints a step-by-step table)
    // Program: move pointer right, increment, then move left and output (suppressed in debug)
    let code = ">+.<";

    let mut bf = BrainfuckReader::new(code.to_string());

    if let Err(err) = bf.run_debug() {
        eprintln!("Brainfuck interpreter error: {:?}", err);
        std::process::exit(1);
    }

    // Note: run_debug prints its own table; no trailing newline is necessary.
}
