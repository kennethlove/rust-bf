use bf::BrainfuckWriter;

fn main() {
    // Classic "Hello World!" in Brainfuck
    let input = "Hello World!".as_bytes();
    
    let bf = BrainfuckWriter::new(input);
    
    let output = bf.generate().unwrap();
    
    println!("{}", output);
    println!(); // ensure a trailing newline for readability
}
