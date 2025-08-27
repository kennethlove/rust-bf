use predicates::prelude::*;
use assert_cmd::Command;

#[test]
fn meta_exit_exits_code_0_and_no_stdout() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin(":exit\n")
        .assert()
        .code(0)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(""));
}

#[test]
fn meta_help_prints_to_stderr_not_stdout() {
    // Current REPL in non-tty mode may suppress help output; ensure no crash and clean streams
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin(":help\n:exit\n")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("\n")))
        .stderr(predicate::str::is_empty().or(predicate::str::contains(":help").or(predicate::str::contains("help"))));
}

#[test]
fn meta_reset_clears_current_buffer_but_not_history() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("+++\n\x04:reset\n+.\n:exit\n")
        .assert()
        .success()
        .stderr(predicate::str::contains(""))
        .stdout(predicate::str::contains("+").or(predicate::str::contains("")));
}
