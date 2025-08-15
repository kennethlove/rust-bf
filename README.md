# rust-bf

A tiny Brainfuck interpreter written in Rust, exposed as both a library and a simple CLI (bf_runner).

- Memory tape defaults to 30,000 cells initialized to 0
- Strict pointer bounds (moving left of 0 or beyond the last cell is an error)
- Input `,` reads a single byte from stdin (EOF sets current cell to 0)
- Output `.` prints the byte as a character (no newline); the CLI appends a trailing newline for readability
- Proper handling of nested loops `[]`; unmatched brackets are an error
- Any non-Brainfuck character results in an error
- Arithmetic wraps at 8 bits (`u8`) for `+` and `-`

## Install / Build

You need Rust and Cargo installed.

- Build: `cargo build`
- Run tests: `cargo test`
- Run example: `cargo run --example usage`

## CLI usage (bf_runner)

The CLI concatenates all args into the Brainfuck program and runs it. It prints a trailing newline after execution.

Examples:

- Hello World
  - `cargo run --bin bf_runner -- "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>."`

- Echo a single byte from stdin (",.")
  - `printf 'Z' | cargo run --bin bf_runner -- ",."`
  - Output: `Z` followed by a newline from the CLI

Notes:
- Non-Brainfuck characters cause an error.
- Unmatched `[` or `]` cause an error.
- Moving the pointer out of bounds causes an error.

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
- An integration test verifies stdin handling for the CLI: `tests/stdin_read.rs`.
- Run all tests with: `cargo test`

## Examples

- `examples/usage.rs` shows a minimal library usage example.

## License

Apache 2.0
