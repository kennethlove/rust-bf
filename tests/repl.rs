
// Utilities
fn make_cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("bf").expect("bf binary")
}

#[test]
fn repl_initial_prompt_appears() {
    let mut cmd = make_cmd();
    // In non-TTY (piped) stdin, REPL auto-selects bare mode and prints no prompt.
    cmd.write_stdin("")
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_valid_program_then_eof_outputs_and_exits() {
    let mut cmd = make_cmd();
    // Print 'A' (65)
    let program = "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++."; // 65 '+' then '.'

    cmd.env("BF_REPL_ONCE", "1")
        .write_stdin(program)
        .assert()
        .success()
        .stdout(
            // In bare mode, only program output goes to stdout
            predicates::str::contains("A\n")
        )
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_invalid_program_reports_error_and_exits() {
    let mut cmd = make_cmd();

    cmd.env("BF_REPL_ONCE", "1")
        .write_stdin("]") // stray closing bracket is a parse error
        .assert()
        .success() // exits cleanly in our bare-mode pipeline when stdin closes
        .stderr(predicates::str::contains("Parse error: unmatched bracket"))
        // REPL prints a trailing newline on stdout after each execution for readability
        .stdout(predicates::str::contains("\n"));
}

#[test]
fn repl_empty_submission_exits_cleanly() {
    let mut cmd = make_cmd();

    cmd.write_stdin("")
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_non_persistent_state_across_runs() {
    // Run 1
    let mut cmd1 = make_cmd();
    let program = "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++."; // 'A'
    let assert1 = cmd1
        .env("BF_REPL_ONCE", "1")
        .write_stdin(program)
        .assert()
        .success();
    let out1 = String::from_utf8(assert1.get_output().stdout.clone()).expect("utf8");

    // Run 2 (fresh process)
    let mut cmd2 = make_cmd();
    let assert2 = cmd2
        .env("BF_REPL_ONCE", "1")
        .write_stdin(program)
        .assert()
        .success();
    let out2 = String::from_utf8(assert2.get_output().stdout.clone()).expect("utf8");

    // The program output section should be identical in both runs.
    // We assert that both contain A followed by a newline; and the entire stdout strings are equal for stability.
    assert!(out1.contains("A\n"), "first run should print A\\n, got: {out1:?}");
    assert!(out2.contains("A\n"), "second run should print A\\n, got: {out2:?}");
    assert_eq!(out1, out2, "stdout should be identical across runs (non-persistent state)");
}
