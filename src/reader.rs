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
//! use bf::BrainfuckReader;
//!
//! // Classic "Hello World!" in Brainfuck
//! let code = "++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.";
//! let mut bf = BrainfuckReader::new(code.to_string());
//! bf.run().expect("program should run");
//! println!(); // ensure a trailing newline for readability
//! ```

/// Errors that can occur while interpreting Brainfuck code.
#[derive(Debug, thiserror::Error)]
pub enum BrainfuckReaderError {
    /// The data pointer attempted to move left of cell 0 or beyond the last cell.
    #[error("Pointer out of bounds")]
    PointerOutOfBounds,
    
    /// Encountered a character outside the Brainfuck instruction set `><+-.,[]`.
    #[error("Invalid character: '{0}'")]
    InvalidCharacter(char),
    
    /// Loops were not balanced; a matching `[` or `]` was not found.
    #[error("Unmatched brackets: a loop was not properly closed")]
    UnmatchedBrackets,
    
    /// An underlying I/O error occurred when reading from stdin.
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
}

/// A simple Brainfuck interpreter.
///
/// The interpreter maintains:
/// - the program codes as a `String`,
/// - a customizable capacity memory tape initialized to zeros (30,000 cells by default),
/// - a data pointer indexing into that tape.
pub struct BrainfuckReader {
    code: String,
    memory: Vec<u8>,
    pointer: usize,
}

impl BrainfuckReader {
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

    /// Internal executor shared by run and run_debug.
    fn execute(&mut self, debug: bool) -> Result<(), BrainfuckReaderError> {
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
                        return Err(BrainfuckReaderError::UnmatchedBrackets);
                    };
                    jump_map[open_index] = Some(i);
                    jump_map[i] = Some(open_index);
                }
            }

            if !stack.is_empty() {
                return Err(BrainfuckReaderError::UnmatchedBrackets);
            }
        }

        let mut step: usize = 0;
        if debug {
            println!("STEP | IP  | PTR | CELL | INSTR | ACTION");
            println!("-----+-----+-----+------+-------+------------------------------------------------");
        }

        while code_ptr < code_len {
            let instr = chars[code_ptr];
            let (ptr_before, cell_before) = (self.pointer, self.memory[self.pointer]);
            let mut action: Option<String> = if debug { Some(String::new()) } else { None };

            match instr {
                '>' => {
                    if self.pointer >= self.memory.len() - 1 {
                        return Err(BrainfuckReaderError::PointerOutOfBounds);
                    }
                    self.pointer += 1;
                    if let Some(a) = action.as_mut() { *a = format!("Moved pointer head to index {}", self.pointer); }
                }
                '<' => {
                    if self.pointer == 0 {
                        return Err(BrainfuckReaderError::PointerOutOfBounds);
                    }
                    self.pointer -= 1;
                    if let Some(a) = action.as_mut() { *a = format!("Moved pointer head to index {}", self.pointer); }
                }
                '+' => {
                    let after = self.memory[self.pointer].wrapping_add(1);
                    self.memory[self.pointer] = after;
                    if let Some(a) = action.as_mut() { *a = format!("Increment cell[{}] from {} to {}", ptr_before, cell_before, after); }
                }
                '-' => {
                    let after = self.memory[self.pointer].wrapping_sub(1);
                    self.memory[self.pointer] = after;
                    if let Some(a) = action.as_mut() { *a = format!("Decrement cell[{}] from {} to {}", ptr_before, cell_before, after); }
                }
                '.' => {
                    if debug {
                        if let Some(a) = action.as_mut() { *a = format!("Output byte '{}' (suppressed in debug)", self.memory[self.pointer] as char); }
                    } else {
                        print!("{}", self.memory[self.pointer] as char);
                    }
                }
                ',' => {
                    if debug {
                        self.memory[self.pointer] = 0; // simulate EOF
                        if let Some(a) = action.as_mut() { *a = "Read byte from stdin -> simulated EOF (set cell to 0)".to_string(); }
                    } else {
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
                                return Err(BrainfuckReaderError::IoError(e));
                            }
                        }
                        if let Some(a) = action.as_mut() { *a = format!("Read byte from stdin -> {}", self.memory[self.pointer]); }
                    }
                }
                '[' => {
                    if self.memory[self.pointer] == 0 {
                        let j = jump_map[code_ptr].expect("validated bracket");
                        if let Some(a) = action.as_mut() { *a = format!("Cell is 0; jump forward to matching ']' at IP {}", j); }
                        code_ptr = j;
                    } else if let Some(a) = action.as_mut() {
                        *a = "Enter loop (cell != 0)".to_string();
                    }
                }
                ']' => {
                    if self.memory[self.pointer] != 0 {
                        let j = jump_map[code_ptr].expect("validated bracket");
                        if let Some(a) = action.as_mut() { *a = format!("Cell != 0; jump back to matching '[' at IP {}", j); }
                        code_ptr = j;
                    } else if let Some(a) = action.as_mut() {
                        *a = "Exit loop (cell is 0)".to_string();
                    }
                }
                _ => {
                    return Err(BrainfuckReaderError::InvalidCharacter(instr));
                }
            }

            if debug {
                println!(
                    "{:<4} | {:<3} | {:<3} | {:<4} |  {}    | {}",
                    step,
                    code_ptr,
                    ptr_before,
                    cell_before,
                    instr,
                    action.unwrap_or_default()
                );
                step += 1;
            }

            // Move to the next instruction
            code_ptr += 1;
        }

        Ok(())
    }

    /// Execute the Brainfuck program until completion.
    ///
    /// Returns `Ok(())` on success or a [`BrainfuckReaderError`] on failure.
    pub fn run(&mut self) -> Result<(), BrainfuckReaderError> {
        self.execute(false)
    }

    /// Debug-run the Brainfuck program, printing a step-by-step table of operations
    /// instead of producing I/O side effects. The interpreter state (pointer, memory)
    /// advances exactly as it would during a real run, but:
    /// - '.' does not print the character; we log the action instead
    /// - ',' does not read from stdin; we simulate EOF and set the cell to 0 and log
    pub fn run_debug(&mut self) -> Result<(), BrainfuckReaderError> {
        self.execute(true)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_character_returns_error() {
        let mut bf = BrainfuckReader::new_with_memory("+a+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::InvalidCharacter('a'))));
    }

    #[test]
    fn unmatched_open_bracket_returns_error() {
        // The starting cell is zero, so encountering '[' with no matching ']' should error.
        let mut bf = BrainfuckReader::new_with_memory("[+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::UnmatchedBrackets)));
    }

    #[test]
    fn left_pointer_out_of_bounds_errors() {
        let mut bf = BrainfuckReader::new_with_memory("<".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::PointerOutOfBounds)));
    }

    #[test]
    fn right_pointer_out_of_bounds_errors() {
        // Move right beyond the last cell. With 3 cells (0..=2), the 3rd '>' attempts to move beyond index 2.
        let memory_size = 3;
        let code = ">".repeat(memory_size);
        let mut bf = BrainfuckReader::new_with_memory(code, memory_size);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::PointerOutOfBounds)));
    }

    #[test]
    fn empty_loop_on_zero_cell_is_ok() {
        let mut bf = BrainfuckReader::new_with_memory("[]".to_string(), 10);
        let result = bf.run();
        assert!(result.is_ok());
    }

    #[test]
    fn simple_program_without_io_runs_ok() {
        // Increment a few times and use a loop to zero the cell.
        // This exercises '+', '-', '[', ']' without relying on I/O or stdout.
        let mut bf = BrainfuckReader::new_with_memory("+++[-]".to_string(), 10);
        let result = bf.run();
        assert!(result.is_ok());
    }

    #[test]
    fn wrapping_subtraction() {
        let mut bf = BrainfuckReader::new_with_memory("-".to_string(), 1);
        let result = bf.run();
        assert!(result.is_ok());
        assert_eq!(bf.memory[0], 255);
    }

    #[test]
    fn wrapping_addition() {
        let code = "+".repeat(256); // 256 increments should wrap around
        let mut bf = BrainfuckReader::new_with_memory(code, 1);
        let result = bf.run();
        assert!(result.is_ok());
        assert_eq!(bf.memory[0], 0);
    }
}
