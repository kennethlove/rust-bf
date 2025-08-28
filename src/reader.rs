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

use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Errors that can occur while interpreting Brainfuck code.
#[derive(Debug, thiserror::Error)]
pub enum BrainfuckReaderError {
    /// The data pointer attempted to move left of cell 0 or beyond the last cell.
    #[error("Pointer out of bounds at instruction {ip} (ptr={ptr}, op='{op}')")]
    PointerOutOfBounds { ip: usize, ptr: usize, op: char },
    
    /// Encountered a character outside the Brainfuck instruction set `><+-.,[]`.
    #[error("Invalid character: '{ch}' at instruction {ip}")]
    InvalidCharacter { ch: char, ip: usize },
    
    /// Loops were not balanced; a matching `[` or `]` was not found.
    #[error("Unmatched bracket {kind} at instruction {ip}")]
    UnmatchedBrackets{ ip: usize, kind: UnmatchedBracketKind },
    
    /// An underlying I/O error occurred when reading from stdin.
    #[error("I/O error at instruction {ip}: {source}")]
    IoError { ip: usize, #[source] source: std::io::Error },

    /// Execution aborted due to step limit.
    #[error("Execution aborted: step limit exceeded ({limit})")]
    StepLimitExceeded { limit: usize },

    /// Execution aborted due to cooperative cancellation (e.g., timeout)
    #[error("Execution aborted: cancelled")]
    Canceled,
}

/// Which side of the loop was unmatched.
#[derive(Debug, Clone, Copy)]
pub enum UnmatchedBracketKind {
    Open,
    Close,
}

impl fmt::Display for UnmatchedBracketKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnmatchedBracketKind::Open => write!(f, "'['"),
            UnmatchedBracketKind::Close => write!(f, "']'"),
        }
    }
}

/// Controls for cooperative cancellation and step limiting.
#[derive(Clone)]
pub struct StepControl {
    pub max_steps: Option<usize>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl StepControl {
    pub fn new(max_steps: Option<usize>, cancel_flag: Arc<AtomicBool>) -> Self {
        Self { max_steps, cancel_flag }
    }
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
    // Optional hooks:
    output_sink: Option<Box<dyn Fn(&[u8]) + Send + Sync>>,
    input_provider: Option<Box<dyn Fn() -> Option<u8> + Send + Sync>>,
    // (window_size, observer (ptr, base, window_slice))
    tape_observer: Option<(usize, Box<dyn Fn(usize, usize, &[u8]) + Send + Sync>)>,
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
            output_sink: None,
            input_provider: None,
            tape_observer: None,
        }
    }

    /// Create a new interpreter from Brainfuck `code` but with a custom memory size.
    pub fn new_with_memory(code: String, memory_size: usize) -> Self {
        Self {
            code,
            memory: vec![0; memory_size],
            pointer: 0,
            output_sink: None,
            input_provider: None,
            tape_observer: None,
        }
    }

    /// Provide an output sink. When set, '.' sends bytes to this sink instead of stdout.
    /// The sink receives a slice of bytes; for Brainfuck, it will be a single-byte slice per '.'.
    pub fn set_output_sink<F>(&mut self, sink: F)
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        self.output_sink = Some(Box::new(sink));
    }

    /// Provide an input provider. When set, ',' reads from this provider instead of stdin.
    /// Returning None indicates EOF (cell is set to 0).
    pub fn set_input_provider<F>(&mut self, provider: F)
    where
        F: Fn() -> Option<u8> + Send + Sync + 'static,
    {
        self.input_provider = Some(Box::new(provider));
    }

    /// Provide a tape observer and desired window size.
    pub fn set_tape_observer<F>(&mut self, window_size: usize, observer: F)
    where
        // ptr: absolute data pointer
        // base: start index of the window slice (page-aligned)
        // window: slice view of memory[base..base+window_size]
        F: Fn(usize, usize, &[u8]) + Send + Sync + 'static,
    {
        self.tape_observer = Some((window_size.max(1), Box::new(observer)));
    }

    /// Internal executor shared by run and run_debug.
    fn execute(&mut self, debug: bool, step_control: Option<&StepControl>) -> Result<(), BrainfuckReaderError> {
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
                        return Err(BrainfuckReaderError::UnmatchedBrackets {
                            ip: i,
                            kind: UnmatchedBracketKind::Close,
                        });
                    };
                    jump_map[open_index] = Some(i);
                    jump_map[i] = Some(open_index);
                }
            }

            if let Some(unmatched_open) = stack.last().copied() {
                return Err(BrainfuckReaderError::UnmatchedBrackets {
                    ip: unmatched_open,
                    kind: UnmatchedBracketKind::Open,
                })
            }
        }

        let mut step: usize = 0;
        if debug {
            println!("STEP | IP  | PTR | CELL | INSTR | ACTION");
            println!("-----+-----+-----+------+-------+------------------------------------------------");
        }

        while code_ptr < code_len {
            // Cooperative cancellation check
            if let Some(ctrl) = step_control {
                if ctrl.cancel_flag.load(Ordering::Relaxed) {
                    return Err(BrainfuckReaderError::Canceled);
                }
            }

            // Step counting
            if let Some(ctrl) = step_control {
                if let Some(max) = ctrl.max_steps {
                    if step >= max {
                        return Err(BrainfuckReaderError::StepLimitExceeded { limit: max });
                    }
                }
            }

            let instr = chars[code_ptr];
            let (ptr_before, cell_before) = (self.pointer, self.memory[self.pointer]);
            let mut action: Option<String> = if debug { Some(String::new()) } else { None };

            match instr {
                '>' => {
                    if self.pointer >= self.memory.len() - 1 {
                        return Err(BrainfuckReaderError::PointerOutOfBounds {
                            ip: code_ptr,
                            ptr: self.pointer,
                            op: instr,
                        });
                    }
                    self.pointer += 1;
                    if let Some(a) = action.as_mut() { *a = format!("Moved pointer head to index {}", self.pointer); }
                }
                '<' => {
                    if self.pointer == 0 {
                        return Err(BrainfuckReaderError::PointerOutOfBounds {
                        ip: code_ptr,
                            ptr: self.pointer,
                            op: instr,
                        });
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
                        // Use output sink when provided; fallback to stdout.
                        if let Some(sink) = self.output_sink.as_ref() {
                            let b = [self.memory[self.pointer]];
                            (sink)(&b);
                        } else {
                            print!("{}", self.memory[self.pointer] as char);
                        }
                    }
                }
                ',' => {
                    if debug {
                        self.memory[self.pointer] = 0; // simulate EOF
                        if let Some(a) = action.as_mut() { *a = "Read byte from stdin -> simulated EOF (set cell to 0)".to_string(); }
                    } else {
                        // Prefer input provider when set; fall back to stdin.
                        if let Some(provider) = self.input_provider.as_ref() {
                            match (provider)() {
                                Some(b) => { self.memory[self.pointer] = b; }
                                None => { self.memory[self.pointer] = 0; } // EOF
                            }
                            if let Some(a) = action.as_mut() { *a = format!("Read byte from input provider -> {}", self.memory[self.pointer]); }
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
                                    return Err(BrainfuckReaderError::IoError { ip: code_ptr, source: e });
                                }
                            }
                            if let Some(a) = action.as_mut() { *a = format!("Read byte from stdin -> {}", self.memory[self.pointer]); }
                        }
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
                    return Err(BrainfuckReaderError::InvalidCharacter { ch: instr, ip: code_ptr });
                }
            }

            // Notify tape observer (if any) after applying the instruction's effect.
            if let Some((win_size, observer)) = self.tape_observer.as_ref() {
                let base = self.pointer.saturating_sub(self.pointer % *win_size);
                let end = (base + *win_size).min(self.memory.len());
                (observer)(self.pointer, base, &self.memory[base..end]);
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

            // Advance step counter
            step += 1;
            // Move to the next instruction
            code_ptr += 1;
        }

        Ok(())
    }

    /// Execute the Brainfuck program until completion.
    ///
    /// Returns `Ok(())` on success or a [`BrainfuckReaderError`] on failure.
    pub fn run(&mut self) -> Result<(), BrainfuckReaderError> {
        self.execute(false, None)
    }

    /// Debug-run the Brainfuck program, printing a step-by-step table of operations
    /// instead of producing I/O side effects. The interpreter state (pointer, memory)
    /// advances exactly as it would during a real run, but:
    /// - '.' does not print the character; we log the action instead
    /// - ',' does not read from stdin; we simulate EOF and set the cell to 0 and log
    pub fn run_debug(&mut self) -> Result<(), BrainfuckReaderError> { self.execute(true, None) }

    /// Execute with cooperative cancellation and optional step limit.
    pub fn run_with_control(&mut self, step_control: StepControl) -> Result<(), BrainfuckReaderError> {
        self.execute(false, Some(&step_control))
    }

    /// Debug-run with cooperative cancellation and optional step limit.
    pub fn run_debug_with_control(&mut self, step_control: StepControl) -> Result<(), BrainfuckReaderError> {
        self.execute(true, Some(&step_control))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_character_returns_error() {
        let mut bf = BrainfuckReader::new_with_memory("+a+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::InvalidCharacter { ch: 'a', .. })));
    }

    #[test]
    fn unmatched_open_bracket_returns_error() {
        // The starting cell is zero, so encountering '[' with no matching ']' should error.
        let mut bf = BrainfuckReader::new_with_memory("[+".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::UnmatchedBrackets { kind: UnmatchedBracketKind::Open, .. })));
    }

    #[test]
    fn left_pointer_out_of_bounds_errors() {
        let mut bf = BrainfuckReader::new_with_memory("<".to_string(), 10);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::PointerOutOfBounds { op: '<', .. })));
    }

    #[test]
    fn right_pointer_out_of_bounds_errors() {
        // Move right beyond the last cell. With 3 cells (0..=2), the 3rd '>' attempts to move beyond index 2.
        let memory_size = 3;
        let code = ">".repeat(memory_size);
        let mut bf = BrainfuckReader::new_with_memory(code, memory_size);
        let result = bf.run();
        assert!(matches!(result, Err(BrainfuckReaderError::PointerOutOfBounds { op: '>', .. })));
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
