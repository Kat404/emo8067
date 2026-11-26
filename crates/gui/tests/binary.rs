//! The binary's own contract: what it takes, and what it says when it
//! refuses.
//!
//! The binary is run, not called: the exit code and the words on the
//! standard error are what a person sees, and neither needs a display.

use std::process::Command;

/// The binary, run with the arguments given.
fn run(arguments: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_emo8067-gui"))
        .args(arguments)
        .output()
        .expect("the binary runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn the_binary_refuses_a_program_it_cannot_open() {
    let (code, stderr) = run(&["/nonexistent/program.asm"]);

    assert_eq!(code, Some(2), "the refusal is the exit code 2");
    assert!(
        stderr.contains("file not found"),
        "the refusal says why: {stderr}"
    );
}

#[test]
fn the_binary_refuses_a_second_program() {
    let (code, stderr) = run(&["one.asm", "two.asm"]);

    assert_eq!(code, Some(2), "the refusal is the exit code 2");
    assert!(
        stderr.contains("usage:"),
        "the refusal says what the binary takes: {stderr}"
    );
}

#[test]
fn the_binary_says_what_it_takes() {
    let output = Command::new(env!("CARGO_BIN_EXE_emo8067-gui"))
        .arg("--help")
        .output()
        .expect("the binary runs");

    assert_eq!(output.status.code(), Some(0), "the help is not a refusal");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("usage:"),
        "the help says what the binary takes: {stdout}"
    );
}
