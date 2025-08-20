// Roundtrip test: bf write generates BF code that, when fed to bf read, emits the same text.

#[test]
fn write_then_read_roundtrip_plus_plus_plus_dot() {
    // First, generate code for the string
    let mut cmd_gen = assert_cmd::Command::cargo_bin("bf").expect("bf binary");
    let assert = cmd_gen.args(["write", "+++."]).assert().success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");

    // The write CLI adds a trailing newline; strip it for reuse as code
    let generated_code = output.trim_end().to_string();
    assert!(!generated_code.is_empty(), "writer should produce code");

    // Now run the generated BF code and assert it prints the original string
    let mut run = assert_cmd::Command::cargo_bin("bf").expect("bf binary");
    run.args(["read", &generated_code])
        .assert()
        .success()
        .stdout("+++.\n");
}
