mod cli_util;
pub mod commands;
mod reader;
pub mod repl;
pub mod ide;
pub mod config;
mod writer;

/// Keep only Brainfuck instruction characters.
pub fn bf_only(s: &str) -> String {
    s.chars()
        .filter(|c| matches!(c, '>' | '<' | '+' | '-' | '.' | ',' | '[' | ']'))
        .collect()
}

pub use reader::{BrainfuckReader, BrainfuckReaderError, UnmatchedBracketKind};
pub use writer::{BrainfuckWriter, WriterOptions};
pub use repl::ModeFlagOverride;
