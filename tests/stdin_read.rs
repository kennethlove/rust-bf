// This test exercises the ',' (input) instruction by providing a byte on stdin
// to the bf CLI executing the program ",." (read one byte, then echo it).
#[test]
fn reads_from_stdin_and_echoes_byte() {
    let mut cmd = assert_cmd::Command::cargo_bin("bf")
        .expect("failed to locate bf binary");

    cmd.arg("read")
        .arg(",.")
        .write_stdin("Z")
        .assert()
        .success()
        .stdout("Z\n");
}
