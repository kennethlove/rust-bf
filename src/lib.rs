//! A tiny Brainfuck interpreter library.
//!
//! This crate provides a minimal Brainfuck interpreter that operates on a
//! memory tape (default 30,000 cells) with a single data pointer.
//!
//! Features and behaviors:
//! - Memory tape initialized to 0.
//! - Strict pointer bounds: moving left from cell 0 or right past the end
//!   returns an error.
//! - Input `,` reads a single byte from stdin; on EOF the current cell is set to 0.
//! - Output `.` prints the byte at the current cell as a character (no newline).
//! - Properly handles nested loops `[]`; unmatched brackets are reported as errors.
//! - Any non-Brainfuck character causes an error.
//!
//! Quick start:
//!
//! ```no_run
//! use rust_bf::Brainfuck;
//!
//! // Classic "Hello World!" in Brainfuck
//! let code = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";
//! let mut bf = Brainfuck::new(code.to_string());
//! bf.run().expect("program should run");
//! println!(); // ensure a trailing newline for readability
//! ```

/// Errors that can occur while interpreting Brainfuck code.
#[derive(Debug)]
pub enum BrainfuckErrors {
    /// The data pointer attempted to move left of cell 0 or beyond the last cell.
    PointerOutOfBounds,
    /// Encountered a character outside the Brainfuck instruction set `><+-.,[]`.
    InvalidCharacter,
    /// Loops were not balanced; a matching `[` or `]` was not found.
    UnmatchedBrackets,
    /// An underlying I/O error occurred when reading from stdin.
    IoError(std::io::Error),
}

/// A simple Brainfuck interpreter.
///
/// The interpreter maintains:
/// - the program codes as a `String`,
/// - a customizable capacity memory tape initialized to zeros (30,000 cells by default),
/// - a data pointer indexing into that tape.
pub struct Brainfuck {
    code: String,
    memory: Vec<u8>,
    pointer: usize,
}

impl Brainfuck {
    /// Create a new interpreter from Brainfuck `code`.
    ///
    /// The memory tape is initialized to 30,000 zeroed cells.
    pub fn new(code: String) -> Self {
        Self {
            code,
            memory: vec![0; 30000],
            pointer: 0,
        }
    }

    /// Create a new interpreter from Brainfuck `code` but with a custom memory size.
    pub fn new_with_memory(code: String, memory_size: usize) -> Self {
        Self {
            code,
            memory: vec![0; memory_size],
            pointer: 0,
        }
    }

    /// Execute the Brainfuck program until completion.
    ///
    /// Returns `Ok(())` on success or a [`BrainfuckErrors`] on failure.
    pub fn run(&mut self) -> Result<(), BrainfuckErrors> {
        let mut code_ptr = 0;
        let chars: Vec<char> = self.code.chars().collect();
        let code_len = chars.len();

        // Precompute matching bracket positions for O(1) jumps and early validation.
        // jump_map[i] holds the matching index for '[' or ']' at index i.
        // For non-bracket positions, it is None.
        let mut jump_map: Vec<Option<usize>> = vec![None; code_len];
        {
            let mut stack: Vec<usize> = Vec::new();
            for (i, &c) in chars.iter().enumerate() {
                if c == '[' {
                    stack.push(i);
                } else if c == ']' {
                    let Some(open_index) = stack.pop() else {
                        return Err(BrainfuckErrors::UnmatchedBrackets);
                    };
                    jump_map[open_index] = Some(i);
                    jump_map[i] = Some(open_index);
                }
            }

            if !stack.is_empty() {
                return Err(BrainfuckErrors::UnmatchedBrackets);
            }
        }

        while code_ptr < code_len {
            match chars[code_ptr] {
                '>' => {
                    if self.pointer >= self.memory.len() - 1 {
                        return Err(BrainfuckErrors::PointerOutOfBounds);
                    }
                    self.pointer += 1;
                }
                '<' => {
                    if self.pointer == 0 {
                        return Err(BrainfuckErrors::PointerOutOfBounds);
                    }
                    self.pointer -= 1;
                }
                '+' => {
                    self.memory[self.pointer] = self.memory[self.pointer].wrapping_add(1);
                }
                '-' => {
                    self.memory[self.pointer] = self.memory[self.pointer].wrapping_sub(1);
                }
                '.' => {
                    // Print the current cell as a character without adding a newline
                    print!("{}", self.memory[self.pointer] as char);
                }
                ',' => {
                    // Read exactly one byte from stdin into the current cell.
                    // On EOF, set the current cell to 0.
                    use std::io::Read;
                    let mut buf = [0u8; 1];
                    match std::io::stdin().read(&mut buf) {
                        Ok(0) => {
                            // EOF: common BF behavior is to set cell to 0
                            self.memory[self.pointer] = 0;
                        }
                        Ok(_) => {
                            self.memory[self.pointer] = buf[0];
                        }
                        Err(e) => {
                            return Err(BrainfuckErrors::IoError(e));
                        }
                    }
                }
                '[' => {
                    // If the current cell is zero, jump forward to the command after
                    // the matching ']' (supports nested loops via a counter).
                    if self.memory[self.pointer] == 0 {
                        let j = jump_map[code_ptr].expect("validated bracket");
                        code_ptr = j;
                    }
                }
                ']' => {
                    // If the current cell is non-zero, jump back to the matching '['.
                    if self.memory[self.pointer] != 0 {
                        let j = jump_map[code_ptr].expect("validated bracket");
                        code_ptr = j;
                    }
                }
                _ => {
                    return Err(BrainfuckErrors::InvalidCharacter);
                }
            }
            // Move to the next instruction
            code_ptr += 1;
        }
        
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_character_returns_error() {
        let mut bf = Brainfuck::new_with_memory("+a+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckErrors::InvalidCharacter)));
    }

    #[test]
    fn unmatched_open_bracket_returns_error() {
        // The starting cell is zero, so encountering '[' with no matching ']' should error.
        let mut bf = Brainfuck::new_with_memory("[+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckErrors::UnmatchedBrackets)));
    }

    #[test]
    fn left_pointer_out_of_bounds_errors() {
        let mut bf = Brainfuck::new_with_memory("<".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckErrors::PointerOutOfBounds)));
    }

    #[test]
    fn right_pointer_out_of_bounds_errors() {
        // Move right beyond the last cell. With 3 cells (0..=2), the 3rd '>' attempts to move beyond index 2.
        let memory_size = 3;
        let code = ">".repeat(memory_size);
        let mut bf = Brainfuck::new_with_memory(code, memory_size);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckErrors::PointerOutOfBounds)));
    }

    #[test]
    fn empty_loop_on_zero_cell_is_ok() {
        let mut bf = Brainfuck::new_with_memory("[]".to_string(), 10);
        let result = bf.run();
        assert!(result.is_ok());
    }

    #[test]
    fn simple_program_without_io_runs_ok() {
        // Increment a few times and use a loop to zero the cell.
        // This exercises '+', '-', '[', ']' without relying on I/O or stdout.
        let mut bf = Brainfuck::new_with_memory("+++[-]".to_string(), 10);
        let result = bf.run();
        assert!(result.is_ok());
    }
    
    #[test]
    fn wrapping_subtraction() {
        let mut bf = Brainfuck::new_with_memory("-".to_string(), 1);
        let result = bf.run();
        assert!(result.is_ok());
        assert_eq!(bf.memory[0], 255);
    }
    
    #[test]
    fn wrapping_addition() {
        let code = "+".repeat(256); // 256 increments should wrap around
        let mut bf = Brainfuck::new_with_memory(code, 1);
        let result = bf.run();
        assert!(result.is_ok());
        assert_eq!(bf.memory[0], 0);
    }
}
