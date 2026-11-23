//! The console: the commands it takes, and what it says.
//!
//! The reference has a debug console of its own, and it documents the
//! commands and not what they print: "All output values are in hex. All
//! input parameters must be in hex. The h suffix is not required."
//!
//! **Every string this file pins is this project's own**, and the registers
//! and the flags are printed as the panels draw them, so the project grows
//! one format and not two.

use std::path::PathBuf;

use emo8067_core::devices::TrafficLights;
use emo8067_core::flags::CF;
use emo8067_core::regs::Reg16;
use emo8067_tui::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A program to open, written where the test can keep it.
fn program(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-tui-console-{name}"));
    std::fs::create_dir_all(&dir).expect("the directory");
    let path = dir.join("program.asm");
    std::fs::write(&path, "org 100h\nmov ax, 1234h\nhlt\n").expect("the program");
    path
}

/// The app, with that program open.
fn app(name: &str) -> App {
    App::new(Some(program(name).to_string_lossy().to_string())).expect("the program loads")
}

/// A program of the test's own words, written where the test can keep it.
fn program_of(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-tui-console-{name}"));
    std::fs::create_dir_all(&dir).expect("the directory");
    let path = dir.join("program.asm");
    std::fs::write(&path, source).expect("the program");
    path
}

/// The app, with that program of the test's own words open.
fn app_of(name: &str, source: &str) -> App {
    App::new(Some(program_of(name, source).to_string_lossy().to_string()))
        .expect("the program loads")
}

/// The keys, one at a time.
fn press(app: &mut App, keys: &[KeyCode]) {
    for key in keys {
        app.key(KeyEvent::new(*key, KeyModifiers::NONE));
    }
}

/// The keys of a text, one at a time.
fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, &[KeyCode::Char(character)]);
    }
}

/// Run a command in the console, and give back the lines it added to the
/// log.
///
/// The log says what was run and then what it said, so the first line is
/// the echo of the command itself.
fn run(app: &mut App, command: &str) -> Vec<String> {
    if !app.console_open {
        press(app, &[KeyCode::Char(':')]);
    }
    let before = app.console.output.len();
    type_text(app, command);
    press(app, &[KeyCode::Enter]);
    app.console.output[before..].to_vec()
}

/// What a command said, without the echo of the command itself.
fn said(app: &mut App, command: &str) -> Vec<String> {
    let lines = run(app, command);
    lines.into_iter().skip(1).collect()
}

#[test]
fn the_registers_command_shows_the_registers_and_the_instruction() {
    let mut app = app("registers");
    let text = said(&mut app, "r").join("\n");
    assert!(
        text.contains("AX 0000"),
        "the registers are the panel's own: {text}"
    );
    assert!(
        text.contains("IP 0100"),
        "and so is the instruction pointer: {text}"
    );
    assert!(
        text.contains("0000:0100"),
        "the instruction is at CS:IP: {text}"
    );
    assert!(
        text.contains("MOV AX"),
        "and it is the instruction itself: {text}"
    );
}

#[test]
fn a_register_is_read_and_written_through_the_console() {
    let mut app = app("register");
    assert_eq!(
        said(&mut app, "r AX"),
        vec!["AX 0000".to_string()],
        "the register is read"
    );
    assert_eq!(
        said(&mut app, "r AX BEEF"),
        vec!["AX BEEF".to_string()],
        "and the new value is said back"
    );
    assert_eq!(
        app.cpu.regs.word(Reg16::Ax),
        0xBEEF,
        "the machine holds the value"
    );
}

#[test]
fn the_flags_command_shows_the_flags_and_writes_one() {
    let mut app = app("flags");
    let lines = said(&mut app, "rf");
    assert!(
        lines.contains(&"CF 0".to_string()),
        "the flags are the panel's own: {lines:?}"
    );
    assert!(
        lines.contains(&"IF 1".to_string()),
        "and so is the interrupt flag: {lines:?}"
    );
    assert_eq!(
        said(&mut app, "rf CF 1"),
        vec!["CF 1".to_string()],
        "the new value is said back"
    );
    assert!(app.cpu.regs.flags.get(CF), "the flags word holds the flag");
}

#[test]
fn the_step_command_steps_the_machine() {
    let mut app = app("step");
    let before = app.cpu.regs.ip;
    said(&mut app, "t");
    assert_eq!(app.steps, 1, "the counter moved");
    assert_ne!(app.cpu.regs.ip, before, "and the machine moved");
}

#[test]
fn the_dump_command_shows_the_memory_and_the_next_block() {
    let mut app = app("dump");
    let lines = said(&mut app, "d 0100");
    assert!(
        lines[0].starts_with("0000:0100 "),
        "the row is at the address: {lines:?}"
    );
    assert!(
        lines[0].contains("B8 34 12 F4"),
        "and it holds the program's own bytes: {lines:?}"
    );
    let lines = said(&mut app, "d");
    assert!(
        lines[0].starts_with("0000:0180 "),
        "a bare dump shows the next block: {lines:?}"
    );
}

#[test]
fn the_port_commands_read_and_write_a_port() {
    let mut app = app("port");
    assert_eq!(
        said(&mut app, "i 0123"),
        vec!["port 0123 = FF".to_string()],
        "a port nobody owns reads as all ones"
    );
    // The traffic lights are on port 4, and they are a word.
    app.cpu.bus.borrow_mut().add(Box::new(TrafficLights::new()));
    assert_eq!(
        said(&mut app, "ow 0004 00FF"),
        vec!["wrote 00FF to port 0004".to_string()],
        "the word is written to the port"
    );
    assert_eq!(
        said(&mut app, "iw 0004"),
        vec!["port 0004 = 00FF".to_string()],
        "and the port reads it back"
    );
}

#[test]
fn the_compare_command_shows_only_the_bytes_that_differ() {
    let mut app = app("compare");
    // Two ranges of eight bytes that are the same, and then one byte of
    // the second that is not.
    for index in 0..8u16 {
        app.cpu.mem.write_u8(0x0000, 0x0300 + index, 0x10);
        app.cpu.mem.write_u8(0x0000, 0x0400 + index, 0x10);
    }
    app.cpu.mem.write_u8(0x0000, 0x0403, 0xBB);
    let lines = said(&mut app, "c 0300 0400");
    assert_eq!(lines.len(), 1, "one address differs: {lines:?}");
    assert!(
        lines[0].contains("0000:0303"),
        "the first address is named: {lines:?}"
    );
    assert!(lines[0].contains("10"), "and its byte: {lines:?}");
    assert!(
        lines[0].contains("0000:0403"),
        "the second address is named too: {lines:?}"
    );
    assert!(lines[0].contains("BB"), "and its byte: {lines:?}");
    assert!(
        said(&mut app, "c 0300 0300").is_empty(),
        "equal bytes are not printed out"
    );
}

#[test]
fn a_command_the_console_does_not_know_is_refused_and_changes_nothing() {
    let mut app = app("unknown");
    press(&mut app, &[KeyCode::F(9)]);
    let steps = app.steps;
    let executed = app.executed.len();
    let breakpoints = app.breakpoints.breakpoints().len();
    let lines = said(&mut app, "x");
    assert!(
        lines[0].contains("the console does not know the command: x"),
        "the refusal is the console's own: {lines:?}"
    );
    assert_eq!(app.steps, steps, "the counter did not move");
    assert_eq!(
        app.executed.len(),
        executed,
        "and neither did the addresses"
    );
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        breakpoints,
        "and the breakpoints are as they were"
    );
}

#[test]
fn a_parameter_that_is_not_hexadecimal_is_refused_and_changes_nothing() {
    let mut app = app("hex");
    let steps = app.steps;
    let lines = said(&mut app, "d 12ZZ");
    assert!(
        lines[0].contains("not a hexadecimal number: 12ZZ"),
        "the refusal is the console's own: {lines:?}"
    );
    assert_eq!(app.steps, steps, "the counter did not move");
}

#[test]
fn a_value_that_does_not_fit_is_refused_and_changes_nothing() {
    let mut app = app("range");
    let lines = said(&mut app, "r AX 12345");
    assert!(
        lines[0].contains("value out of range"),
        "the refusal is the project's own: {lines:?}"
    );
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0, "the register is as it was");
    assert_eq!(app.steps, 0, "and the counter did not move");
}

#[test]
fn the_commands_the_console_cannot_do_yet_are_refused_honestly() {
    let mut app = app("not-implemented");
    let lines = said(&mut app, "p");
    assert!(
        lines[0].contains("not implemented: p"),
        "the step over is not implemented: {lines:?}"
    );
    assert!(
        lines[0].contains("no call tracking"),
        "and it says why: {lines:?}"
    );
    let lines = said(&mut app, "d bx+5");
    assert!(
        lines[0].contains("not implemented: effective addresses"),
        "the effective addresses are not implemented: {lines:?}"
    );
    let lines = said(&mut app, "d lion L 10");
    assert!(
        lines[0].contains("not implemented: symbols"),
        "the symbols are not implemented: {lines:?}"
    );
    assert!(
        lines[0].contains("no symbol table"),
        "and it says why: {lines:?}"
    );
    assert_eq!(app.steps, 0, "and nothing moved");
}

#[test]
fn the_console_says_the_command_it_ran() {
    let mut app = app("echo");
    let lines = run(&mut app, "r AX");
    assert_eq!(lines[0], "> r AX", "the log says what was run: {lines:?}");
}

#[test]
fn the_log_keeps_what_the_commands_said() {
    let mut app = app("log");
    run(&mut app, "r AX");
    run(&mut app, "r AX");
    assert_eq!(
        app.console.output,
        vec![
            "> r AX".to_string(),
            "AX 0000".to_string(),
            "> r AX".to_string(),
            "AX 0000".to_string(),
        ],
        "the output is added under the last one"
    );
}

#[test]
fn the_dump_command_reads_a_memory_reference() {
    let mut app = app("reference");
    // The colon of the reference is a character of the command: the `:` is
    // only the key that closes the console when the line is empty.
    let lines = said(&mut app, "d 0000:0100");
    assert!(
        lines[0].starts_with("0000:0100 "),
        "the row is at the address: {lines:?}"
    );
}

#[test]
fn the_dump_command_reads_the_machines_own_segment() {
    let mut app = app_of("segment", "org 100h\n#ds=0200#\nhlt\n");
    let lines = said(&mut app, "d DS:0100");
    assert!(
        lines[0].starts_with("0200:0100 "),
        "the segment is the machine's own: {lines:?}"
    );
}
