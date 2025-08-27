# Version 0.4.0 — TUI Plan

Project goals and constraints
- UI: TUI-only (ratatui + crossterm).
- Execution: run/stop only.
- Input on ,: prompt the user; cancel means EOF.
- Semantics: 30,000 cells, 8-bit wrap on +/-; pointer wraps; tape window size = 32.
- Output: raw by default; toggle to escaped rendering.
- Use your existing library (BrainfuckReader, BrainfuckReaderError, ModeFlagOverride, commands/repl/theme as helpful). Do not use BrainfuckWriter.

High-level UX and layout
- Left pane: Editor
    - Multiline text editing, vertical scroll.
    - Minimal Brainfuck syntax highlighting for <>+-.,[].
    - Matching bracket highlight when cursor on [ or ].
- Bottom-left: Output pane
    - Scrollback of program output.
    - Toggle to switch between raw and escaped display.
    - Input prompt line appears here when , is encountered.
- Right pane: Tape
    - Dense 32-cell window around data pointer.
    - Highlights pointer cell; shows absolute pointer index and current value.
- Status bar
    - Filename and dirty indicator.
    - Run state (Running/Stopped).
    - Pointer index and current cell value.
    - Output display mode (Raw/Esc).
    - Last message or error.

Keybindings
- Run: F5 or Ctrl+R
- Stop: Shift+F5 or Ctrl+.
- Open: Ctrl+O
- Save: Ctrl+S
- Switch pane focus: Tab / Shift+Tab
- Output view toggle (Raw/Escaped): Ctrl+E
- Help overlay: F1 / Ctrl+H
- Editor navigation: Arrows, PageUp/PageDown, Home/End
- Tape pane navigation: [ and ] to shift the 32-cell window; Left/Right to move highlight when focused
- Input prompt during ,: Enter submits first byte; Esc cancels (EOF)

Architecture and data flow (no code)
- Threads and channels
    - UI thread: renders TUI, handles key events, manages files, shows input modal, toggles output mode.
    - Runner thread: executes Brainfuck using the library; sends events back to UI.
    - Channels:
        - Runner -> UI: Output(bytes), Tape(ptr, 32-cell window), NeedsInput, Halted(success/error), optional heartbeat.
        - UI -> Runner: Start(source, config), Stop, ProvideInput(Some(byte) or None for EOF).
- Library integration
    - Parse and run with BrainfuckReader; display BrainfuckReaderError messages in status bar on failure.
    - Apply pointer wrap behavior via ModeFlagOverride (or equivalent).
    - Output: capture produced bytes and forward to UI as events.
    - Input: on , request, post NeedsInput to UI; wait for ProvideInput (None = EOF).
    - Stop: listen for Stop signal and exit cleanly.

Execution semantics and tape window
- Pointer wrap-around at edges.
- 32-cell window centered on the pointer when possible; handle wrap-around near boundaries.
- Periodic state snapshots from runner: current pointer index and 32-cell slice.

File I/O behavior
- Open (Ctrl+O): prompt for path, load, mark dirty=false.
- Save (Ctrl+S): write to current path (or prompt for one), mark dirty=false.
- On unsaved changes before destructive actions: confirm with a simple yes/no modal.

Output display modes
- Raw (default): render bytes as-is (terminal control bytes may affect display).
- Escaped: printable ASCII as-is, non-printables as escaped sequences (e.g., \xNN). Toggle with Ctrl+E.

## Phased delivery plan

1. TUI scaffolding
    - Initialize terminal; implement layout for editor, output, tape, and status bar.
    - Focus handling and keybinding dispatch.
    - Minimal syntax highlighting for BF tokens and matching bracket highlight.

2. Runner wiring
    - Define channels/events for UI <-> runner.
    - Start/Stop lifecycle: parse with BrainfuckReader; on error, show message; on success, run on worker thread.
    - Periodically send output and tape snapshots; send completion and error events.

3. Input prompt on ,
    - Display a modal prompt using reedline for single-byte input.
    - Enter sends the first byte; Esc sends EOF (None).
    - Suspend other keybindings while modal is active.

4. Tape pane (32 cells)
    - Render pointer index, 32-cell window with wrap-around.
    - Highlight current cell; show absolute index/value in status.

5. Output pane and toggle
    - Append incoming bytes; implement scrollback behavior.
    - Add Raw/Escaped toggle; re-render using the same buffer.

6. File open/save
    - Simple path prompts; handle errors with status messages.
    - Track file path and dirty state.

7. Polish and help
    - Status messages for run completion, stop, and errors.
    - Help overlay (F1) listing keybindings and behaviors.
    - Graceful terminal teardown on exit and on errors.

Acceptance criteria
- Can edit, open, and save Brainfuck source files.
- Run executes the program; Stop halts promptly.
- Output appears in real time; toggle between Raw and Escaped works.
- On , program pauses; input modal accepts one byte; Esc yields EOF.
- Tape pane shows a correct 32-cell window with pointer highlighting and wraps at edges.
- Status bar reflects run state, pointer index, current cell value, file info, and output mode.
- Unmatched brackets or other parse/runtime errors are shown clearly without crashing the UI.

Testing checklist (manual)
- Hello World: correct output, Raw/Escaped toggling.
- Echo program: prompts on , and echoes typed byte; Esc at prompt leads to EOF behavior.
- Pointer wrap program: verify left of 0 wraps to end and right of end wraps to 0.
- Long-running loop: Stop is responsive and returns to Stopped state.
- File I/O: Open/Save workflows, dirty flag handling, error messages for invalid paths.

Risk management
- Deadlock on input: ensure runner posts NeedsInput before blocking and UI always responds ProvideInput(Some/None).
- UI overwhelm: batch output and tape updates (by time or instruction count).
- Raw output control bytes disrupting terminal: provide Escaped mode in help; default to Raw but document the toggle.

Decisions locked in
- TUI-only MVP using your existing library.
- Pointer wrap-around; 32-cell tape window.
- , prompts for input; Esc at prompt delivers EOF.
- Output Raw by default; toggle to Escaped.
- Use ratatui, crossterm, and reedline.
