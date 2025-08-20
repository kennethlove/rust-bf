// Ensure `bf write --debug` (or -d) is an error; write does not support debug mode.
use predicates::prelude::*;

#[test]
fn write_debug_flag_is_an_error() {
    let mut cmd = assert_cmd::Command::cargo_bin("bf").expect("bf binary");

    cmd.args(["write", "--debug"]).assert().failure().stderr(
        // Should mention usage or unknown flag in stderr
        predicates::str::contains("Usage:")
            .and(predicates::str::contains("write"))
            .or(predicates::str::contains("unknown flag"))
            .or(predicates::str::contains("unsupported")),
    );

    // Also check short flag
    let mut cmd2 = assert_cmd::Command::cargo_bin("bf").expect("bf binary");
    cmd2.args(["write", "-d"]).assert().failure();
}
