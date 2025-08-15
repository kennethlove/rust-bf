use rust_bf::Brainfuck;

fn main() {
    // Classic Brainfuck "Hello World!" program
    let code = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";

    let mut bf = Brainfuck::new(code.to_string());

    if let Err(err) = bf.run() {
        eprintln!("Brainfuck interpreter error: {:?}", err);
        std::process::exit(1);
    }

    // Print a newline after the Brainfuck program output for readability
    println!();
}
