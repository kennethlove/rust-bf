use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

fn cargo_bin() -> Command { Command::cargo_bin("bf").unwrap() }

#[test]
fn test_invalid_character_error() {
    cargo_bin()
    .timeout(Duration::from_secs(2)).arg("read").arg("+a+")
    .assert()
    .failure()
    .stderr(predicate::str::contains("invalid").or(predicate::str::contains("error")))
    .stdout(predicate::str::contains("Execution aborted").not());
}

#[test]
fn test_unmatched_brackets_error() {
    cargo_bin()
    .timeout(Duration::from_secs(2)).arg("read").arg("[")
    .assert()
    .failure()
    .stderr(predicate::str::contains("bracket").or(predicate::str::contains("mismatch").or(predicate::str::contains("error"))));
}
