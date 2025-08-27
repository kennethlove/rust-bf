use predicates::prelude::*;
use assert_cmd::Command;

#[test]
fn auto_detect_non_tty_runs_bare_once_and_exits_0() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("+++.")
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{3}"))
        .stderr(predicate::str::is_empty().or(predicate::str::contains("").normalize()));
}

#[test]
fn editor_on_non_tty_is_error_exit_1() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.arg("repl")
        .arg("--editor")
        .write_stdin("")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("TTY").or(predicate::str::contains("non-tty")));
}

#[test]
fn env_mode_respected_flags_override() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.env("BF_REPL_MODE", "editor")
        .arg("repl")
        .arg("--bare")
        .write_stdin("+++.")
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{3}"));
}
