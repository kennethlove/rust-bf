use predicates::prelude::*;
use assert_cmd::Command;

#[test]
fn bare_empty_input_exits_clean_and_quiet() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty().or(predicate::str::contains("").normalize()));
}
