//! The drawing, and what it shows.
//!
//! The TUI is a renderer of the panel model, so the drawing is a pure
//! function of the machine and the UI's state, and a test can read it back
//! off a buffer without a terminal.

use std::path::PathBuf;

use emo8067_tui::app::App;
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

#[test]
fn the_disassembly_shows_the_program_and_the_cursor() {
    let app = app("drawing");
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(&app, frame))
        .expect("the drawing");

    let buffer = terminal.backend().buffer().clone();
    let text = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
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
    app.key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
    assert_eq!(app.breakpoints.breakpoints().len(), 0);
}
