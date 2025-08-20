Phase 0: Scope and decisions (10–20 minutes)

- Confirm behavior:
    - Input model: Users type any number of lines; EOF (Ctrl-D on Unix/macOS, Ctrl-Z then Enter on Windows) submits the
      entire buffer for execution.
    - No persistence: Tape and pointer reset for each execution.
    - Ctrl-C behavior: Immediate exit with status 0, regardless of whether a program is running or user is at the
      prompt.
- Output policy:
    - Program output goes to stdout exactly as produced by the interpreter.
    - Prompts and error messages can go to stderr or stdout—choose one and be consistent. Prefer stdout for prompt,
      stderr for errors.
- Error policy:
    - Parsing/runtime errors are printed concisely, then the REPL continues.
- Acceptance: A single paragraph documenting the above is written into a DESIGN.md or a section in README.

Phase 1: CLI entry point and control flow (30–45 minutes)

- Task: Add a binary entry that launches the REPL when invoked without args.
- Loop design:
    - Render a simple prompt (e.g., “bf> ”).
    - Create an empty buffer for the current session.
    - Read from stdin until EOF for this session, then execute.
    - After execution (or error), re-display the prompt and repeat.
- Edge decision: If user presses EOF immediately on an empty buffer, do nothing and re-prompt (explicitly document
  this).
- Acceptance: Manual test shows prompt appears, EOF on empty input re-prompts, no crash.

Phase 2: Interpreter invocation and result handling (30–60 minutes)

- Task: Integrate the existing interpreter (BrainfuckReader) to execute the collected buffer.
- Execution:
    - On non-empty buffer at EOF, pass it to the interpreter.
    - Capture produced output and print to stdout.
- Error handling:
    - Display a concise error message for parse/runtime errors.
    - Do not exit on interpreter errors; just re-prompt.
- Acceptance: Running a known-good Brainfuck snippet yields expected output; an unmatched bracket produces an error
  message and the REPL continues.

Phase 3: Signal handling for Ctrl-C (45–60 minutes)

- Task: Install a SIGINT handler that exits the process immediately and cleanly with exit code 0.
- Considerations:
    - Ensure handler is set before entering the loop.
    - If Ctrl-C occurs mid-execution, still exit immediately (MVP behavior).
    - Avoid leaving the terminal in a messy state; flush stdout/stderr on exit.
- Acceptance: Pressing Ctrl-C at the prompt or during a long-running program terminates the process with exit code 0.

Phase 4: Stream flushing and I/O consistency (15–30 minutes)

- Task: Ensure explicit flushing after:
    - Printing prompts.
    - Printing program output.
    - Printing error messages.
- Rationale: Prevent interleaving and buffering oddities on different platforms.
- Acceptance: Prompt appears promptly, outputs are visible without extra keystrokes, and no garbled ordering is
  observed.

Phase 5: Platform nuances and documentation touch-up (20–30 minutes)

- Task: Document EOF key combos:
    - Unix/macOS: Ctrl-D
    - Windows: Ctrl-Z then Enter
- Task: Note non-persistent state behavior and immediate Ctrl-C exit.
- Acceptance: README or help text updated with the above; manual runs on at least one Unix and one Windows shell confirm
  behavior.

Phase 6: Error message consistency (20–30 minutes)

- Task: Standardize short, clear messages for:
    - Parse errors (e.g., unmatched bracket with position, if available).
    - Runtime errors (e.g., pointer bounds, if applicable).
    - I/O errors (rare; present actionable info and continue or exit only if unrecoverable).
- Acceptance: Entering a malformed program yields a consistent, friendly message; subsequent runs still work.

Phase 7: Automated tests using assert_cmd and predicates (60–120 minutes)

- Task: Add integration tests that spawn the binary and interact via stdin/stdout.
- Test cases:
    - Valid program: send a simple BF script via stdin, followed by EOF; assert expected stdout content and that process
      stays alive or returns to prompt. If keeping the process interactive is hard to assert, it’s acceptable to end
      after one cycle in tests by closing stdin and checking output sequence.
    - Invalid program: send unmatched ‘]’ and EOF; assert error message is printed and process is still usable for
      another run (or exits cleanly if stdin is closed).
    - Empty submission: send just EOF; assert a re-prompt occurs or that the process remains idle and prompts again (
      test presence of the prompt string).
    - Ctrl-C behavior: Typically hard to simulate directly; document as manually tested. Optional: use a harness that
      sends SIGINT to the process running an infinite loop and assert exit code 0.
- Acceptance: Tests are green on CI for primary target OS(es).

Phase 8: QA checklist and manual verification (15–30 minutes)

- Script:
    - Start REPL, type “+++.” then EOF; verify expected character printed and prompt returns.
    - Type another independent program; verify no state persisted.
    - Type a stray “]” then EOF; verify concise error and continued usability.
    - Press EOF on empty buffer; verify consistent behavior.
    - Start a loop (e.g., something that doesn’t terminate) and press Ctrl-C; verify immediate exit with code 0.
- Acceptance: All checks pass without glitches.

Phase 9: Final polish (optional, 20–40 minutes)

- Prompt branding: Keep it short and unintrusive (“bf> ”). Avoid extra whitespace or formatting.
- Consistent newline behavior: Ensure a newline after program output if needed for prompt clarity; decide and be
  consistent.
- Minimal logging: Avoid verbose logs in the REPL; keep it quiet unless errors occur.

Out of scope for MVP (for future iterations)

- Persistent tape state between runs.
- Meta-commands (e.g., :help, :reset, :dump).
- Step limits, debugging, breakpoints.
- Fancy line editing/history. MVP relies on the shell’s basic input; EOF ends a submission.

Success criteria

- Functional: Users can type Brainfuck code, press EOF, see output, and keep using the REPL without restarting.
- Robustness: Errors are reported without crashing. Ctrl-C exits immediately and cleanly.
- Clarity: Users understand how to submit (EOF) and how to quit (Ctrl-C), with platform differences documented.
- Testable: Integration tests verify core behaviors on CI.

Notes and risks

- Windows EOF handling can trip users up; mitigate via clear docs and optional startup message if on Windows.
- Ctrl-C during heavy I/O should still exit cleanly—verify no panics or partial writes.
- If your interpreter can block indefinitely, consider adding a future step-limit or timeout in a next iteration (not in
  MVP).

Deliverables checklist (one-liner each)

- REPL loop with prompt and EOF-based submission.
- Interpreter invocation with output forwarding and error reporting.
- SIGINT handler for immediate exit with code 0.
- Stream flushes in the right places.
- Updated README/docs with usage notes.
- Integration tests for valid/invalid/empty submission; documented manual test for Ctrl-C.
