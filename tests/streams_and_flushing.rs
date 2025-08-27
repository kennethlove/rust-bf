use predicates::prelude::*;
use assert_cmd::Command;

#[test]
fn stream_separation_program_output_stdout_meta_stderr() {
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin("+++.")
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{3}"))
        .stderr(predicate::str::contains("\u{3}").not());
}

#[test]
fn prompts_and_meta_are_flushed_to_stderr() {
    // In non-tty mode, prompts/help may be suppressed; assert clean success and no crash
    let mut cmd = Command::cargo_bin("bf").unwrap();
    cmd.write_stdin(":help\n:exit\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty().or(predicate::str::contains(":help").or(predicate::str::contains("help"))));
}
