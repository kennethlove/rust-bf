# Design Document for `bf` Project

## Input

- Users type Brainfuck code directly into the REPL.
- Any number of lines is supported.
- EOF (Ctrl-D on Unix/macOS, Ctrl-Z then Enter on Windows) submits the entire buffer for execution.
- Ctrl-C behavior: Immediate exit with status 0, regardless of whether a program is running or user is at the prompt.

## Output

- Program output is printed to stdout exactly as produced by the interpreter.
- Prompts go to stdout, and error messages go to stderr.

## Errors

- Errors are printed concisely to stderr.
- The REPL continues after an error, allowing users to correct and re-run their code.

## Memory and Execution

- No persistence: Tape and pointer reset for each execution.
