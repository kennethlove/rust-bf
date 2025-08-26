use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::time::Duration;

fn cargo_bin() -> Command {
    Command::cargo_bin("bf").unwrap()
}

fn infinite_bf() -> &'static str { 
    "+[]" // increments to 1, then [] does nothing forever (infinite loop)
}

fn read_to_tempfile(content: &str) -> tempfile::NamedTempFile {
    let mut tf = tempfile::NamedTempFile::new().expect("tempfile");
    write!(tf, "{}", content).unwrap();
    tf
}

#[test]
fn test_repl_bare_timeout_infinite() {
    // Pipe infinite program to stdin so auto-bare triggers
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .env_remove("BF_MAX_STEPS")
        .env("BF_TIMEOUT_MS", "100")
        .write_stdin(infinite_bf())
        .assert()
        .stderr(predicate::str::contains("Execution aborted").and(predicate::str::contains("timeout")))
        .stdout(predicate::str::contains("Execution aborted").not());
}

#[test]
fn test_repl_bare_step_limit_infinite() {
    cargo_bin()
        .timeout(Duration::from_secs(2))
        .env("BF_MAX_STEPS", "50")
        .env_remove("BF_TIMEOUT_MS")
        .write_stdin(infinite_bf())
        .assert()
        .stderr(predicate::str::contains("step limit exceeded (50)"))
        .stdout(predicate::str::contains("Execution aborted").not());
}

#[test]
fn test_read_timeout_infinite_flag() {
    let tf = read_to_tempfile(infinite_bf());
    cargo_bin()
        .arg("read").arg("--timeout").arg("100").arg("--file").arg(tf.path())
        .timeout(Duration::from_secs(2))
        .assert()
        .failure()
        .stderr(predicate::str::contains("timeout"))
        .stdout(predicate::str::contains("Execution aborted").not());
}

#[test]
fn test_read_step_limit_infinite_flag() {
    let tf = read_to_tempfile(infinite_bf());
    cargo_bin()
        .arg("read").arg("--max-steps").arg("50").arg("--file").arg(tf.path())
        .timeout(Duration::from_secs(2))
        .assert()
        .failure()
        .stderr(predicate::str::contains("step limit exceeded (50)"))
        .stdout(predicate::str::contains("Execution aborted").not());
}
