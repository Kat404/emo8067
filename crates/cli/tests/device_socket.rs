//! The device socket: a unix thing, and the runner says so where it is not.
//!
//! The I/O bus speaks over a unix socket, and a unix socket is a unix
//! thing: the platforms without one refuse the option with a message that
//! says why, and they do not offer it in the usage either.

use std::path::PathBuf;
use std::process::Command;

/// A scratch directory outside the worktree, for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the runner and say what it printed and how it ended.
fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_emo8067"))
        .args(args)
        .output()
        .expect("the runner runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn the_device_socket_is_a_unix_thing_and_the_runner_says_so() {
    let dir = scratch("device-socket");
    // A program that halts at once: the runner has nothing to do but the
    // socket.
    let program = dir.join("program.bin");
    std::fs::write(&program, [0xF4]).expect("the program is written");
    let socket = dir.join("bus.sock");

    let (_stdout, stderr, code) = run(&[
        "run",
        program.to_str().unwrap(),
        "--device-socket",
        socket.to_str().unwrap(),
    ]);

    if cfg!(unix) {
        assert_eq!(code, 0, "the runner ends on its own: {stderr}");
        assert!(
            socket.exists(),
            "and the socket it served the bus on is there"
        );
    } else {
        assert_eq!(code, 2, "the option is refused: {stderr}");
        assert!(
            stderr.contains("unix socket"),
            "and the refusal says why: {stderr}"
        );
        assert!(
            !socket.exists(),
            "and no socket was made where there is none"
        );
    }
}

#[test]
fn the_usage_offers_the_device_socket_where_there_is_a_unix_socket() {
    let (_stdout, _stderr, code) = run(&["--help"]);
    assert_eq!(code, 0, "the usage is printed");
    // The usage is the runner's own contract: the option is in it where it
    // works, and it is out of it where it does not.
    let usage = {
        let (stdout, _stderr, _code) = run(&["--help"]);
        stdout
    };
    assert_eq!(
        usage.contains("--device-socket"),
        cfg!(unix),
        "the usage offers the option where the socket is: {usage}"
    );
}
