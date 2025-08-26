# Version 0.3.0 — Advanced REPL Plan

Scope: Add richer line editing and history, multi-modal navigation (Edit vs History-Browse), meta commands, and
non-blocking execution guarantees while preserving the simple “submit-on-EOF” workflow and stdout/stderr separation.

## Phase 0: UX decisions and semantics (15–30 minutes)

- Input model and submission
    - Users edit a multi-line buffer; EOF submits the entire buffer for execution.
    - Enter inserts a newline; no implicit submission on Enter.
- Streams policy
    - Program output: stdout, exactly as produced by the interpreter.
    - REPL/meta output (prompts, help, banners, errors): stderr for framing and messages. See :dump below for details.
- Meta commands (line starts with “:”)
    - :exit — Exit immediately with code 0 (same as Ctrl-C policy).
    - :help — Print advanced usage (key bindings, meta commands, EOF per OS, timeout policy).
    - :reset — Clear the current editing buffer (does not touch history).
    - :dump — Print the current editing buffer for inspection.
        - Default: raw buffer lines to stdout; framing markers to stderr.
        - Options: “-n” to include line numbers in stdout; “--stderr” to force the entire dump to stderr.
- Non-blocking execution
    - Configure a step limit and a wall-clock timeout. If either is exceeded, abort execution, print a concise message,
      and return to the prompt.
    - Defaults are sensible and configurable via CLI flags and environment variables.

Acceptance: A concise docs section explains edit/history modes, meta commands, stream policy, and timeout/step-limit
defaults.

## Phase 1: Line editor integration (60–120 minutes)

- Integrate a line editor supporting:
    - Multiline editing and in-buffer Left/Right/Up/Down navigation.
    - History with program-level entries (one entry per submitted buffer).
    - Custom keybindings and redraw APIs to print meta output and restore the buffer/cursor.
- Behavior
    - Prompt remains short, e.g., “bf> ”.
    - History is session-scoped; file persistence can be deferred.
- Acceptance
    - Manual: Left/Right move the cursor; Up/Down move across lines within the current buffer; EOF submits.

## 1.25: Interactive vs Non-interactive (Bare) mode policy (15–30 minutes)

Goal: Provide predictable behavior for humans (interactive editor) and tooling/pipes (bare), with simple overrides.

- Default mode selection (auto-detect):
    - If stdin is a TTY: start the interactive editor REPL.
    - If stdin is not a TTY (piped/redirected): run in bare mode — read until EOF, execute once, then exit 0.
    - Prompt/meta output:
        - Continue to use stderr for prompts/meta/errors.
        - If stderr is not a TTY, suppress prompts/banners to keep pipeline output clean.
- Flags:
    - --bare (alias: --non-interactive): force bare mode even if stdin is a TTY.
    - --editor: force interactive mode. If stdin is not a TTY, print a concise error on stderr and exit with code 1.
- Environment override (optional):
    - BF_REPL_MODE=bare|editor. CLI flags take precedence over the environment.
- Behavior details:
    - Bare mode:
        - No line editor or history; single submission read until EOF; execute once; exit 0.
        - No syntax highlighting is needed.
        - Stream policy unchanged: program output to stdout; meta/errors to stderr.
    - Interactive mode:
        - Multiline editing with Enter inserting newline; EOF (e.g., Ctrl-D) submits.
        - Session-scoped in-memory history; no file persistence.
        - Meta commands available.
        - Syntax highlighting
    - Ctrl-C behavior unchanged: exits immediately and cleanly with code 0.
- Acceptance:
    - Piping input into the REPL executes once and exits 0; outputs respect stream policy; prompts are suppressed when stderr is not a TTY.
    - --bare forces bare behavior on a TTY; --editor errors out on non-TTY stdin with a clear message and exit code 1.
    - BF_REPL_MODE works when set; flags override it.

## Phase 1.5: Syntax highlighting (60–90 minutes)
- Integrate a syntax highlighting library (e.g., syntect).
- Define a simple theme for Brainfuck syntax (e.g., commands in one color, non-command characters in another).
- Acceptance
    - Manual: The current buffer displays with syntax highlighting; editing and navigation remain functional.

## Phase 2: Multi-modal navigation: Edit vs History-Browse (90–150 minutes)

Goal: Make Up/Down navigate history only when explicitly intended, otherwise navigate within the buffer.

- Modes
    - Edit mode (default): Up/Down move the cursor across lines inside the current buffer. History is not engaged.
    - History-Browse mode: Up/Down navigate submission history (full-program entries) as previews.
- Entering History-Browse
    - Press Up when the cursor is at line 0, column 0 (the “0,0 gate”) to enter History-Browse and preview the most
      recent submission.
    - Optional convenience bindings: Alt+Up/Alt+Down (or Ctrl+Up/Ctrl+Down) always enter History-Browse regardless of
      cursor location.
    - If the buffer is empty and you press Up, jump straight to the most recent history entry (common shell behavior).
- While browsing history
    - Up: move to older submissions. Down: move toward newer submissions.
    - The “current buffer” acts as a virtual newest entry; pressing Down from the most recent history entry returns to
      it.
    - Enter accepts the previewed history entry and switches back to Edit mode with that content loaded.
    - Esc cancels browsing and restores the original buffer/cursor position.
    - Any edit keystroke can implicitly accept the previewed content and return to Edit mode (optional UX choice).
- Indicators
    - Show a lightweight indicator while browsing, e.g., prompt suffix “[history i/N]” or a transient status line on
      stderr.
- State management
    - Keep: mode, saved_current_buffer (for cancel), history_index (0 = most recent).
    - Redraw the buffer when previews change; restore on cancel/accept.
- Acceptance
    - Manual: Multi-line buffers keep Up/Down for in-buffer movement; Up at (0,0) opens history; Enter accepts; Esc
      cancels and restores edits.

## Phase 3: Meta commands parsing and behavior (45–75 minutes)

- Recognition
    - If a line starts with “:”, interpret it as a meta command immediately on Enter (do not add it to the program
      buffer or history).
- Commands
    - :exit — Exit with code 0 immediately.
    - :help — Print:
        - Key bindings: Left/Right, Up/Down in Edit mode; history controls; EOF per OS.
        - Modes: entering/leaving History-Browse (0,0 gate, Esc, Enter).
        - Meta commands and examples.
        - Timeout/step-limit defaults and configuration.
    - :reset — Clear the current buffer and reposition cursor to start; history remains intact.
    - :dump — Print buffer content without modifying it:
        - Default: raw lines to stdout; framing markers (e.g., “— dump (N lines) —” and “— end dump —”) to stderr.
        - Flags: -n (line numbers on stdout), --stderr (everything to stderr).
- Redraw discipline
    - Before printing meta output, temporarily yield the editor view; after printing, re-render prompt and restore
      buffer/cursor.
- Acceptance
    - Manual: Meta commands do not alter the buffer (except :reset), do not enter history, and redraw cleanly.

## Phase 4: Execution isolation and non-blocking guarantees (90–150 minutes)

- Design
    - Run the interpreter on a worker thread.
    - Cooperative cancellation via:
        - Step counting inside the interpreter; exceed -> return a specific “step limit exceeded” error.
        - Time-based deadline from the REPL; on timeout, signal cancellation and join with a deadline.
- Configuration
    - CLI flags: --timeout <ms> and --max-steps <n> with sensible defaults.
    - Env vars: BF_TIMEOUT_MS and BF_MAX_STEPS as fallbacks.
- User messages
    - On step limit: “Execution aborted: step limit exceeded (N).”
    - On timeout: “Execution aborted: wall-clock timeout (T ms).”
- Acceptance
    - Manual: Infinite/non-terminating programs are aborted promptly; REPL stays responsive and ready for the next
      input.

## Phase 5: Signal handling and shutdown (30–45 minutes)

- Maintain behavior: Ctrl-C exits immediately and cleanly with code 0.
- Ensure the worker thread does not block shutdown; flush streams on exit.
- Acceptance
    - Manual: Pressing Ctrl-C at prompt or during execution exits with code 0; no terminal mess.

## Phase 6: I/O and flushing polish (15–30 minutes)

- Explicitly flush after:
    - Printing prompts.
    - Printing interpreter output.
    - Printing meta/help/error messages.
- Acceptance
    - Manual: Outputs appear promptly; ordering remains consistent.

## Phase 7: Tests and reliability (90–180 minutes)

- Integration tests (assert_cmd + predicates)
    - Valid program: submit via stdin, check stdout matches; process continues (or exits cleanly when stdin closes).
    - Invalid program: print concise error and remain usable.
    - Empty submission: re-prompt behavior is consistent and non-crashing.
    - Non-blocking: a non-terminating program triggers timeout/step-limit message; process does not hang on CI.
    - Meta commands: :help, :exit, :reset, :dump behaviors (assert stream separation and exit code for :exit).
- Mode logic tests
    - Unit-test the Edit vs History-Browse state machine: 0,0 gate, Esc restore, Enter accept, index bounds, and
      “virtual current buffer” behavior.
    - Automated terminal key simulation for multi-line cursor motion is brittle; cover the state machine in unit tests
      and rely on a manual checklist for interactive nuances.
- Acceptance
    - CI green across target platforms.

## Phase 8: Documentation and :help text (30–45 minutes)

- Document:
    - Submission model (EOF per OS) and immediate Ctrl-C exit policy.
    - Streams: stdout for program output; stderr for prompts/meta/errors; :dump options.
    - Modes and navigation:
        - Edit mode: Up/Down move within buffer.
        - History-Browse: Up/Down navigate submissions; Up at (0,0) to enter; Enter accepts; Esc cancels; optional
          Alt/Ctrl+Up/Down shortcut.
    - Meta commands with examples: :exit, :help, :reset, :dump [-n|--stderr].
    - Timeouts and step limits with defaults and configuration knobs.

## Phase 9: Manual QA checklist (20–30 minutes)

- Start REPL, type “+++.” then EOF; verify expected output and prompt returns.
- After an error, press Up at (0,0) to recall the last submission; browse older ones; Enter accepts; Esc cancels and
  restores edits.
- Confirm Up/Down within a multi-line buffer do not trigger history unless at (0,0).
- Use Left/Right to navigate within lines; insert/delete works.
- :dump shows buffer; :dump -n adds line numbers; :dump --stderr forces stderr-only; buffer remains intact.
- :reset clears buffer without affecting history.
- Run a non-terminating program; verify timeout/step-limit message and prompt returns quickly.
- Press Ctrl-C during execution; process exits with code 0.

## Clarifications

- :reset — Clears only the current in-memory editing buffer. History is unchanged.
- :dump — Prints the current buffer for inspection. By default, raw content goes to stdout and framing to stderr; flags
  modify this. It never alters the buffer or history.
- History semantics — One history entry per submitted buffer. The “current buffer” is treated as a virtual entry when
  browsing, so you can return to it via Down or Esc.
- Stream separation — Interpreter output uses stdout exclusively; REPL/meta use stderr for prompts/help/errors,
  maintaining clear separation for users and tests.

## Deliverables checklist

- Multiline-capable line editor integrated; correct flush behavior.
- Multi-modal navigation implemented (Edit and History-Browse) with 0,0 gate and accept/cancel semantics.
- Session-scoped submission history; Up at empty buffer recalls the last submission.
- Meta commands: :exit, :help, :reset, :dump (with -n and --stderr).
- Execution isolation with step limit and wall-clock timeout; clear abort messages.
- Ctrl-C still exits immediately with code 0.
- Tests covering meta commands, non-blocking behavior, and mode state machine; manual QA plan for interactive keys.
- Updated docs and :help text covering navigation, meta commands, and non-blocking policies.
