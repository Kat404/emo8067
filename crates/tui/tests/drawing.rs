//! The drawing, and what it shows.
//!
//! The TUI is a renderer of the panel model, so the drawing is a pure
//! function of the machine and the UI's state, and a test can read it back
//! off a buffer without a terminal.

use std::path::PathBuf;

use emo8067_core::regs::Reg16;
use emo8067_tui::app::{App, Focus};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A program to open, written where the test can keep it.
fn program(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-tui-{name}"));
    std::fs::create_dir_all(&dir).expect("the directory");
    let path = dir.join("program.asm");
    std::fs::write(&path, "org 100h\nmov ax, 1234h\nhlt\n").expect("the program");
    path
}

/// The app, with that program open.
fn app(name: &str) -> App {
    App::new(Some(program(name).to_string_lossy().to_string())).expect("the program loads")
}

/// The screen the app draws, as text.
fn screen(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
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

#[test]
fn the_disassembly_shows_the_program_and_the_cursor() {
    let app = app("drawing");
    let text = screen(&app, 80, 20);
    assert!(text.contains("disassembly"), "the panel is named: {text}");
    assert!(text.contains("program.asm"), "the program is named: {text}");
    assert!(text.contains("MOV"), "the listing is the program's: {text}");
    assert!(
        text.contains("q quits"),
        "the status line says how to leave: {text}"
    );
}

#[test]
fn a_step_moves_the_machine_and_the_listing() {
    let mut app = app("step");
    let before = app.cpu.regs.ip;
    app.key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
    assert_ne!(app.cpu.regs.ip, before, "the machine moved");
    assert_eq!(
        app.executed.len(),
        1,
        "and the address it was at is remembered"
    );
}

#[test]
fn a_breakpoint_is_set_at_the_cursor_and_taken_away_again() {
    let mut app = app("breakpoint");
    let key = KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE);
    app.key(key);
    assert_eq!(app.breakpoints.breakpoints().len(), 1);
    app.key(key);
    assert_eq!(app.breakpoints.breakpoints().len(), 0);
}

#[test]
fn the_registers_panel_shows_the_machine() {
    let mut app = app("registers");
    press(&mut app, &[KeyCode::F(8)]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("registers"), "the panel is named: {text}");
    assert!(
        text.contains("AX 1234"),
        "the value is the machine's: {text}"
    );
    assert!(
        text.contains("SP FFFE"),
        "and so is the stack pointer: {text}"
    );
}

#[test]
fn the_flags_panel_shows_the_flags() {
    let app = app("flags");
    let text = screen(&app, 80, 20);
    assert!(text.contains("flags"), "the panel is named: {text}");
    assert!(text.contains("CF 0"), "a clear flag reads as zero: {text}");
    assert!(text.contains("IF 1"), "interrupts start enabled: {text}");
}

#[test]
fn a_register_can_be_edited_in_place() {
    let mut app = app("edit");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Char('e')]);
    type_text(&mut app, "beefh");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.cpu.regs.word(Reg16::Ax),
        0xBEEF,
        "the value was written"
    );
    assert!(app.edit.is_none(), "and the edit is over");
}

#[test]
fn an_edit_that_cannot_be_read_leaves_the_register_alone() {
    let mut app = app("refuse");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Char('e')]);
    type_text(&mut app, "zz");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0, "the register is as it was");
    let status = app.status.clone().unwrap_or_default();
    assert!(
        status.contains("bad expression"),
        "the refusal is its own: {status}"
    );
}

#[test]
fn a_value_that_does_not_fit_in_a_register_is_refused() {
    let mut app = app("range");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Char('e')]);
    type_text(&mut app, "12345h");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0, "the register is as it was");
    let status = app.status.clone().unwrap_or_default();
    assert!(status.contains("out of range"), "and it says why: {status}");
}

#[test]
fn the_focus_moves_between_the_panes() {
    let mut app = app("focus");
    assert_eq!(app.focus, Focus::Disassembly);
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Registers);
    press(&mut app, &[KeyCode::Char('1')]);
    assert_eq!(app.focus, Focus::Disassembly);
}

#[test]
fn the_arrows_still_move_the_disassembly_cursor() {
    let mut app = app("arrows");
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(app.cursor, 1, "the cursor moved down the listing");
    press(&mut app, &[KeyCode::Up]);
    assert_eq!(app.cursor, 0, "and back up again");
}

#[test]
fn the_arrows_move_the_cursor_of_the_pane_that_has_it() {
    let mut app = app("register-arrows");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Down]);
    assert_eq!(app.register_cursor, 2, "one row down the panel");
    assert_eq!(app.cursor, 0, "and the listing did not move");
}
