use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

fn cargo_bin() -> Command { Command::cargo_bin("bf").unwrap() }

fn small_valid_bf() -> &'static str { "+++." }
fn infinite_bf() -> &'static str { "+[]" }

#[test]
fn test_stdout_only_for_program_output() {
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .write_stdin(small_valid_bf())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::contains("Execution aborted").not());
}

#[test]
fn test_stderr_only_for_abort_messages() {
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .env("BF_TIMEOUT_MS", "100")
        .write_stdin(infinite_bf())
        .assert()
        .stderr(predicate::str::contains("Execution aborted"))
        .stdout(predicate::str::contains("Execution aborted").not());
}
