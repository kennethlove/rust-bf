use rust_bf::BrainfuckReader;

fn main() {
    // Classic Brainfuck "Hello World!" program
    let code = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";

    let mut bf = BrainfuckReader::new(code.to_string());

    if let Err(err) = bf.run() {
        eprintln!("Brainfuck interpreter error: {:?}", err);
        std::process::exit(1);
    }

    // Print a newline after the Brainfuck program output for readability
    println!();

    // Tip: to inspect program execution without performing I/O, you can use:
    // let mut bf = Brainfuck::new(">+.<".to_string());
    // let _ = bf.run_debug(); // prints a step-by-step table
}
