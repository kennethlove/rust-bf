use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cargo_bin() -> Command { Command::cargo_bin("bf").unwrap() }

fn small_valid_bf() -> &'static str { "+++." }

fn read_to_tempfile(content: &str) -> tempfile::NamedTempFile {
    let mut tf = tempfile::NamedTempFile::new().expect("tempfile");
    write!(tf, "{}", content).unwrap();
    tf
}

#[test]
fn test_read_positional_code_success() {
    cargo_bin()
        .arg("read").arg(small_valid_bf())
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_read_file_success() {
    let tf = read_to_tempfile(small_valid_bf());
    cargo_bin()
        .arg("read").arg("--file").arg(tf.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty());
}

#[test]
fn test_read_debug_prints_table() {
    // We don't know exact content, only that debug mode should succeed and stdout should contain some headers/rows.
    cargo_bin()
        .arg("read").arg("--debug").arg("+.+")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not())
        // In debug mode, implementation prints table and still adds a newline at end; stderr should be empty when OK
        .stderr(predicate::str::is_empty());
}
