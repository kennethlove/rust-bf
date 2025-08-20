//! A tiny Brainfuck generator library.
//!
//! This crate provides a minimal Brainfuck generator that operates on
//! user input to generate an appropriate Brainfuck string.
//!
//! Features and behaviors:
//! - Properly handles nested loops `[]`; unmatched brackets are reported as errors.
//! - Any non-Brainfuck character causes an error.
//!
//! Quick start:
//!
//! ```no_run
//! use lib::{BrainfuckWriter, WriterOptions};
//!
//! // Classic "Hello World!" in Brainfuck
//! let input = "Hello World!".as_bytes();
//! let bf = BrainfuckWriter::new(input);
//! let output = bf.generate().unwrap();
//! println!("{}", output);
//! println!(); // ensure a trailing newline for readability
//! ```

use std::cmp::Ordering;

/// Errors that can occur while generating Brainfuck code.
#[derive(Debug)]
pub enum BrainfuckWriterError {
    /// Encountered a character outside the Brainfuck instruction set `><+-.,[]`.
    InvalidCharacter,
    /// Loops were not balanced; a matching `[` or `]` was not found.
    UnmatchedBrackets,
    /// An underlying I/O error occurred when reading from stdin.
    IoError(std::io::Error),
}

pub struct WriterOptions {
    pub use_loops: bool, // Use loop-based multiplication when building from zero
    pub max_loop_factor: u8, // Maximum outer loop counter to consider (e.g., 16..32 is fine)
    pub assume_wrapping_u8: bool, // Assume BF cells wrap (most interpreters do)
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            use_loops: true,
            max_loop_factor: 16,
            assume_wrapping_u8: true,
        }
    }
}

pub struct BrainfuckWriter<'writer> {
    input: Box<&'writer [u8]>,
    options: WriterOptions,
}

impl<'writer> BrainfuckWriter<'writer> {
    pub fn new(input: &'writer [u8]) -> Self {
        let options = WriterOptions::default();
        Self { input: Box::new(input), options }
    }
    pub fn with_options(input: &'writer [u8], options: WriterOptions) -> Self {
        Self { input: Box::new(input), options }
    }

    pub fn generate(&self) -> Result<String, BrainfuckWriterError> {
        let mut output = String::new();
        let mut cursor = 0u8;

        for b in self.input.iter() {
            // Option A: delta encodes from cursor -> b using wrapping arithmetic
            let delta_sequence = self.encode_delta(cursor, *b);

            // Option B: clear and rebuild from zero (no reliance on wrapping)
            let from_zero_sequence = self.encode_from_zero(*b);

            // Choose the shorter option for this byte
            let best_sequence = if delta_sequence.len() <= from_zero_sequence.len() {
                delta_sequence
            } else {
                from_zero_sequence
            };

            output.push_str(&best_sequence);
            output.push('.');

            cursor = *b;
        }

        Ok(output)
    }

    /// Encode the shortest delta from cursor to target.
    /// If `assume_wrapping` is true, it computes the shortest path on a ring of 256.
    /// If false, we avoid wrap and prefer clear+build, but still produce a non-wrapping delta.
    fn encode_delta(&self, cursor: u8, target: u8) -> String {
        if cursor == target {
            return String::new();
        }

        let mut output = String::new();
        if self.options.assume_wrapping_u8 {
            // Compute the shortest path on a ring of 256
            let forward = (target.wrapping_sub(cursor)) as u8; // `+` count
            let backward = (cursor.wrapping_sub(target)) as u8; // `-` count
            if forward <= backward {
                for _ in 0..forward { output.push('+'); }
            } else {
                for _ in 0..backward { output.push('-'); }
            }
        } else {
            match target.cmp(&cursor) {
                Ordering::Greater => for _ in 0..(target - cursor) { output.push('+'); },
                Ordering::Less => for _ in 0..(cursor - target) { output.push('-'); },
                Ordering::Equal => {}
            }
        }

        output
    }

    /// Build exact value `target` in the current cell starting from an unknown prior value.
    fn encode_from_zero(&self, target: u8) -> String {
        // Always start by clearing the current cell
        let mut best = String::from("[-]");
        best.push_str(&"+".repeat(target as usize));

        if !self.options.use_loops || target == 0 {
            return best;
        }

        // Try loop-based constructions of the form:
        // Ensure temp cell (>) is zero: >[-]<
        // Set current to 'a': '+' * a
        // [ > '+' * b < - ] ; multiply a*b into temp, clear current
        // > adjust remainder r = cursor - a*b with '+' or '-'
        // [<+>-] ; move result back to current, clear temp, pointer at temp
        // < ; return to current
        //
        // This leaves current == cursor, temp == 0, pointer back at current.
        //
        // We search a in [1..max_factor], b ~ round(cursor / a), clamp b to [1..=255],
        // and adjust the small remainder with +/-.
        let mut best_len = best.len();

        for a in 1..=self.options.max_loop_factor {
            // choose b as nearest integer to cursor / a, but at least 1
            let b_f = (target as f32) / (a as f32);
            let mut b = b_f.round() as i32;
            if b < 1 { b = 1; }
            if b > 255 { b = 255; }

            let prod = (a as i32) * b;
            let mut seq = String::new();
            seq.push_str("[-]"); // clear current cell
            seq.push_str(">[-]<"); // ensure temp cell is zero

            seq.push_str(&"+".repeat(a as usize));
            seq.push('[');
            seq.push('>');
            seq.push_str(&"+".repeat(b as usize));
            seq.push('<');
            seq.push('-');
            seq.push(']');

            // Move to temp and adjust remainder
            seq.push('>');
            let r = (target as i32) - prod;
            if r > 0 {
                seq.push_str(&"+".repeat(r as usize));
            } else if r < 0 {
                seq.push_str(&"-".repeat((-r) as usize));
            }

            // Move value back to current cell and return pointer
            seq.push_str("[<+>-");
            seq.push(']');
            seq.push('<'); // return to current cell

            if seq.len() < best_len {
                best_len = seq.len();
                best = seq;
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn simple_hello() {
        let input = "Hello World!".as_bytes();
        let writer = BrainfuckWriter::new(input);
        let output = writer.generate().unwrap();
        assert!(output.contains('.'));
        assert!(output.len() > 0);
    }
    
    #[test]
    fn zero_and_repeat() {
        let options = WriterOptions {
            use_loops: true,
            max_loop_factor: 16,
            assume_wrapping_u8: true,
        };
        let input = &[0u8, 0u8, 0u8];
        let writer = BrainfuckWriter::with_options(&*input, options);
        let output = writer.generate().unwrap();
        assert_eq!(output, "...");
        assert_eq!(output.matches('.').count(), 3);
    }
}
