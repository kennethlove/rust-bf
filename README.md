# rust-bf

A tiny Brainfuck interpreter written in Rust, exposed as both a library and a simple CLI (bf).

- Memory tape defaults to 30,000 cells initialized to 0
- Strict pointer bounds (moving left of 0 or beyond the last cell is an error)
- Input `,` reads a single byte from stdin (EOF sets current cell to 0)
- Output `.` prints the byte as a character (no newline); the CLI appends a trailing newline for readability
- Proper handling of nested loops `[]`; unmatched brackets are an error
- Any non-Brainfuck character results in an error
- Arithmetic wraps at 8 bits (`u8`) for `+` and `-`
- Debug mode (`--debug` or `-d`) prints a step-by-step execution table instead of performing I/O

## Install / Build

You need Rust and Cargo installed.

- Build: `cargo build`
- Run tests: `cargo test`
- Run example: `cargo run --example usage`

## CLI usage (read)

The CLI concatenates all args into the Brainfuck program and runs it. It prints a trailing newline after execution.

Examples:

- Hello World
  - `cargo run --bin bf -- read "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.
  ------.--------.>+.>."`

- Echo a single byte from stdin (",.")
  - `printf 'Z' | cargo run --bin bf -- read ",."`
  - Output: `Z` followed by a newline from the CLI

- Debug mode (prints a table instead of executing I/O)
  - `cargo run --bin bf -- read --debug ">+.<"`
  - Useful for understanding control flow; `,` behaves as EOF (cell set to 0) and `.` output is suppressed

Notes:
- Non-Brainfuck characters cause an error.
- Unmatched `[` or `]` cause an error.
- Moving the pointer out of bounds causes an error.

## CLI usage (write)

Generate Brainfuck code that prints the provided input.

Examples:
- From positional args (recommended with Cargo; note the `--` separator):
  - `cargo run --bin bf -- write "Hello world"`
- From STDIN (UTF-8 text):
  - `echo -n 'Hello' | cargo run --bin bf -- write`
- From a file:
  - `cargo run --bin bf -- write --file ./message.txt`
- Raw bytes from a file:
  - `cargo run --bin bf -- write --bytes --file ./image.bin`

The output is Brainfuck code printed to stdout (a trailing newline is added for readability).

## CLI usage (REPL)

Interactive REPL for Brainfuck code execution.

- Start the REPL:
  - `cargo run --bin bf -- repl`
- Type Brainfuck code directly into the REPL.
- Invalid instructions are ignored.
- Tape and pointer are reset for each execution. No state is maintained.
- Press Ctrl-D (Unix/macOS) or Ctrl-Z and then Enter (Windows) to signal EOF and execute the code.
- Alt-Up/Down and Ctrl-Up/Down navigate command history.
- The REPL will print the output of the Brainfuck program.
- Press Ctrl-C to exit the REPL immediately with exit code 0.

### REPL Features

- Multi-line buffer editing
- Non-blocking execution
  - Configurable with flags
  - Default timeout: XXXX seconds, default max steps: YYYY
- Command history (up/down arrows on a blank buffer)
- Meta-commands (start with `:`):
    - `:help` - show help
    - `:exit` - exit the REPL
    - `:reset` - clear the current buffer
    - `:dump` - print the current buffer
        - add `-n` to print line numbers
        - add `-stderr` to send everything to stderr

## Library usage

Add this crate to your workspace or use it via a path dependency. Then:

```rust,no_run
use rust_bf::Brainfuck;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Classic Hello World
    let code = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";
    let mut bf = Brainfuck::new(code.to_string());
    bf.run()?;
    println!(); // optional: newline for readability
    Ok(())
}
```

Debug run (no real I/O; prints a table):

```rust,no_run
use rust_bf::Brainfuck;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let code = ">+.<"; // simple program
    let mut bf = Brainfuck::new(code.to_string());
    bf.run_debug()?; // prints a step-by-step table
    Ok(())
}
```

### Custom memory size

```rust,ignore
use rust_bf::Brainfuck;
let mut bf = Brainfuck::new_with_memory(
    "+>+<[->+<]".to_string(),
    1024, // custom tape size
);
let _ = bf.run();
```

## Behavior details

- Input `,`: reads exactly one byte from stdin. On EOF, sets current cell to `0`.
- Output `.`: prints the current cell as a `char` (no newline).
- Pointer `>` / `<`: moving beyond the tape bounds returns `PointerOutOfBounds`.
- Brackets: a pre-pass validates matching pairs; unmatched pairs produce `UnmatchedBrackets`.
- Invalid chars: any char not in `><+-.,[]` produces `InvalidCharacter`.
- I/O errors: wrapped as `IoError(std::io::Error)`.

## Testing

- Unit tests live in `src/lib.rs`.
- Integration tests:
  - `tests/stdin_read.rs` verifies stdin handling for the CLI
  - `tests/debug_flag.rs` verifies the `--debug` table output
- Run all tests with: `cargo test`

## Examples

- `examples/usage.rs` shows a minimal library usage example.
- `examples/debug.rs` shows how to run a program in debug mode (prints a step-by-step table).

Run:
- `cargo run --example usage`
- `cargo run --example debug`

## License

Apache 2.0
