use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

fn cargo_bin() -> Command { Command::cargo_bin("bf").unwrap() }

fn small_valid_bf() -> &'static str { "+++." }

#[test]
fn test_auto_bare_on_piped_stdin_executes_once() {
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .write_stdin(small_valid_bf())
        .assert()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_repl_respects_bf_repl_once_env() {
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .env("BF_REPL_ONCE", "1")
        .write_stdin(small_valid_bf())
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_forced_editor_on_non_tty_errors() {
    // Piped stdin (non-tty) + --editor should error out with non-zero and helpful message.
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .arg("repl")
        .arg("--editor")
        .write_stdin(small_valid_bf())
        .assert()
        .failure()
        .stderr(predicate::str::contains("stdin is not a TTY"));
}
