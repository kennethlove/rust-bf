use assert_cmd::prelude::*;
use std::process::{Command, Stdio};

#[cfg(unix)]
#[test]
fn sigint_at_prompt_exits_0() {
    // Placeholder: sending SIGINT requires libc; keep test scaffolding ready.
    let mut child = Command::cargo_bin("bf")
        .unwrap()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // TODO: libc::kill(child.id() as i32, libc::SIGINT);
    // std::thread::sleep(std::time::Duration::from_millis(100));

    let status = child.wait().unwrap();
    assert!(status.success() || status.code() == Some(0));
}

#[cfg(unix)]
#[test]
fn sigint_during_execution_exits_0() {
    // TODO: write a long-running program and send SIGINT; expect exit 0
}
