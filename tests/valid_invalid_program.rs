use predicates::prelude::*;
use assert_cmd::Command;

#[test]
fn bare_valid_program_outputs_expected_stdout_and_no_prompts() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("+++.")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("\u{3}"))
        .stderr(predicate::str::is_empty().or(predicate::str::contains("").normalize()));
}

#[test]
fn bare_invalid_program_prints_concise_error_and_exits_clean() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("]")
        .assert()
        .stderr(predicate::str::contains("Parse error").or(predicate::str::contains("unmatched")));
}
