// Verifies that --debug prints a step-by-step table instead of executing.
use predicates::prelude::*;

#[test]
fn debug_flag_prints_table() {
    let mut cmd = assert_cmd::Command::cargo_bin("bf_runner")
        .expect("failed to locate bf_runner binary");

    cmd.args(["--debug", ">"]) // single instruction: move pointer right
        .assert()
        .success()
        .stdout(predicates::str::contains("STEP | IP")
            .and(predicates::str::contains("Moved pointer head to index 1"))
        );
}
