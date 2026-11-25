//! The drawing, and what it shows.
//!
//! The TUI is a renderer of the panel model, so the drawing is a pure
//! function of the machine and the UI's state, and a test can read it back
//! off a buffer without a terminal.

use std::path::PathBuf;

use emo8067_core::flags::{CF, PF};
use emo8067_core::regs::Reg16;
use emo8067_debug::panels::Flag;
use emo8067_debug::{Access, Machine};
use emo8067_tui::app::{App, Focus};
use emo8067_tui::ui::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Modifier;

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

/// A program of the test's own words, written where the test can keep it.
fn program_of(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-tui-{name}"));
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

/// The screen the app draws, one string per row.
///
/// The rows are what the drawing is made of: the strip is a band of the
/// screen, and a test wants to know which row a line of the source is
/// on.
fn rows(app: &App, width: u16, height: u16) -> Vec<String> {
    let buffer = buffer(app, width, height);
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer.cell((x, y)).map_or("", |cell| cell.symbol()))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// The cells the app draws, so a test can read the marks on them.
fn buffer(app: &App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    terminal.backend().buffer().clone()
}

/// The text of the cells the app marks.
///
/// A mark is the way the panels say where the cursor is: the word the
/// stack pointer is at, the entry the conditions cursor is on.
fn marked(app: &App, width: u16, height: u16) -> String {
    buffer(app, width, height)
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(Modifier::REVERSED))
        .map(|cell| cell.symbol())
        .collect()
}

/// Where a text is on the screen: the column and the row.
///
/// The column is counted in characters and not in bytes, so a glyph that
/// is not ASCII is found where it is drawn.
fn place(buffer: &Buffer, text: &str) -> Option<(u16, u16)> {
    let area = buffer.area;
    for y in 0..area.height {
        let row: String = (0..area.width)
            .filter_map(|x| buffer.cell((x, y)))
            .map(|cell| cell.symbol())
            .collect();
        if let Some(index) = row.find(text) {
            let column = row[..index].chars().count();
            return Some((column as u16, y));
        }
    }
    None
}

/// The status line the app draws.
///
/// The status line is the last row of the screen, and it says what is
/// being edited while an edit is open.
fn status(app: &App, width: u16, height: u16) -> String {
    rows(app, width, height)
        .pop()
        .map(|row| row.trim().to_string())
        .unwrap_or_default()
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

#[test]
fn the_focus_cycles_the_four_panes() {
    let mut app = app("focus-four");
    assert_eq!(app.focus, Focus::Disassembly);
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Registers);
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Memory, "the memory pane is the third");
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Flags, "and the flags are the fourth");
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Disassembly, "and the cycle closes");
}

#[test]
fn the_four_keys_take_their_own_panes() {
    let mut app = app("focus-keys");
    press(&mut app, &[KeyCode::Char('1')]);
    assert_eq!(app.focus, Focus::Disassembly, "1 takes the disassembly");
    press(&mut app, &[KeyCode::Char('2')]);
    assert_eq!(app.focus, Focus::Registers, "2 takes the registers");
    press(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.focus, Focus::Memory, "3 takes the memory");
    press(&mut app, &[KeyCode::Char('4')]);
    assert_eq!(app.focus, Focus::Flags, "and 4 takes the flags");
}

#[test]
fn the_memory_panel_shows_the_bytes_around_the_address() {
    let mut app = app("memory");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0100");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0x0100, "the window is at the address");
    let text = screen(&app, 80, 20);
    assert!(text.contains("memory"), "the panel is named: {text}");
    assert!(
        text.contains("0100 B8 34 12 F4"),
        "the bytes are the program's, and the gutter has the address: {text}"
    );
}

#[test]
fn the_memory_panel_shows_the_printable_side_when_there_is_room() {
    let mut app = app_of("memory-ascii", "org 100h\ndb 'Hello'\nhlt\n");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0100");
    press(&mut app, &[KeyCode::Enter]);
    let text = screen(&app, 120, 24);
    assert!(
        text.contains("0100 48 65 6C 6C 6F F4"),
        "the bytes are the program's: {text}"
    );
    assert!(
        text.contains("Hello."),
        "and the printable side is beside them: {text}"
    );
}

#[test]
fn the_memory_cursor_moves_a_byte_and_a_row_at_a_time() {
    let mut app = app("memory-cursor");
    press(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.ui.memory_at, 0, "the cursor is at the start");
    press(&mut app, &[KeyCode::Right]);
    assert_eq!(app.ui.memory_at, 1, "one byte to the right");
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(app.ui.memory_at, 17, "and one row down");
    press(&mut app, &[KeyCode::Up]);
    assert_eq!(app.ui.memory_at, 1, "and back up again");
    press(&mut app, &[KeyCode::Left]);
    assert_eq!(app.ui.memory_at, 0, "and back to the start");
}

#[test]
fn a_page_moves_the_memory_cursor_by_a_window() {
    let mut app = app("memory-page");
    press(&mut app, &[KeyCode::Char('3')]);
    press(&mut app, &[KeyCode::PageDown]);
    assert_eq!(app.ui.memory_at, 0x0100, "a page is a window");
    press(&mut app, &[KeyCode::PageUp]);
    assert_eq!(app.ui.memory_at, 0, "and back again");
}

#[test]
fn a_goto_that_cannot_be_read_leaves_the_address_alone() {
    let mut app = app("memory-refuse");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "zz");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0, "the address is as it was");
    let status = app.status.clone().unwrap_or_default();
    assert!(status.contains("bad syntax"), "and it says why: {status}");
}

#[test]
fn the_goto_prompt_can_be_left_without_moving() {
    let mut app = app("memory-esc");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0100");
    press(&mut app, &[KeyCode::Esc]);
    assert_eq!(app.ui.memory_at, 0, "the address is as it was");
}

#[test]
fn the_stack_pane_opens_where_the_listing_is() {
    let mut app = app("stack");
    let before = screen(&app, 80, 20);
    assert!(
        before.contains("disassembly"),
        "the listing is there to begin with: {before}"
    );
    press(&mut app, &[KeyCode::Char('5')]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("stack"), "the pane is named: {text}");
    assert!(
        !text.contains("disassembly"),
        "the listing is not on the screen: {text}"
    );
    assert!(
        !text.contains("MOV"),
        "and neither is the listing's own text: {text}"
    );
    press(&mut app, &[KeyCode::Char('5')]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "and the listing comes back: {text}"
    );
    press(&mut app, &[KeyCode::Char('5'), KeyCode::Esc]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "and the escape closes it too: {text}"
    );
}

#[test]
fn the_stack_window_moves_with_the_arrows() {
    let mut app = app_of("stack-arrows", "org 100h\n#sp=0200#\nhlt\n");
    press(&mut app, &[KeyCode::Char('5')]);
    let text = screen(&app, 80, 24);
    assert!(
        text.contains("0200  0000"),
        "the window starts at the pointer: {text}"
    );
    press(&mut app, &[KeyCode::Down]);
    let text = screen(&app, 80, 24);
    assert!(
        text.contains("0210  0000"),
        "and it moves down a word: {text}"
    );
    assert!(
        !text.contains("0200  0000"),
        "and the word it left is gone: {text}"
    );
    press(&mut app, &[KeyCode::Up]);
    let text = screen(&app, 80, 24);
    assert!(text.contains("0200  0000"), "and back again: {text}");
}

#[test]
fn a_pushed_word_is_in_the_window() {
    let mut app = app_of(
        "stack-push",
        "org 100h\n#sp=0200#\nmov ax, 1234h\npush ax\nhlt\n",
    );
    press(
        &mut app,
        &[KeyCode::Char('5'), KeyCode::F(8), KeyCode::F(8)],
    );
    let text = screen(&app, 80, 24);
    assert!(
        text.contains("01FE  1234"),
        "the pushed word is in the window: {text}"
    );
}

#[test]
fn the_word_the_pointer_is_at_is_marked() {
    let mut app = app_of(
        "stack-mark",
        "org 100h\n#sp=0200#\nmov ax, 1234h\npush ax\nhlt\n",
    );
    press(
        &mut app,
        &[KeyCode::Char('5'), KeyCode::F(8), KeyCode::F(8)],
    );
    assert_eq!(
        marked(&app, 80, 24),
        "1234",
        "the word the pointer is at is the marked one"
    );
}

#[test]
fn the_arrows_do_not_move_the_listing_while_the_stack_pane_is_open() {
    let mut app = app("stack-listing");
    press(&mut app, &[KeyCode::Char('5'), KeyCode::Down]);
    assert_eq!(app.cursor, 0, "the listing's cursor did not move");
    assert_eq!(app.stack_scroll, 2, "and the stack window did");
}

#[test]
fn a_breakpoint_is_not_set_while_the_stack_pane_holds_the_listings_place() {
    let mut app = app("stack-breakpoint");
    press(&mut app, &[KeyCode::Char('5'), KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        0,
        "the cursor is not on the screen, so there is nowhere to set one"
    );
    press(&mut app, &[KeyCode::Char('5'), KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        1,
        "and with the listing back there is a cursor again"
    );
}

#[test]
fn the_stack_window_starts_at_the_pointer_every_time_it_is_opened() {
    let mut app = app_of("stack-open", "org 100h\n#sp=0200#\nhlt\n");
    press(&mut app, &[KeyCode::Char('5'), KeyCode::Down]);
    assert_eq!(app.stack_scroll, 2, "the window was moved away");
    press(&mut app, &[KeyCode::Char('5'), KeyCode::Char('5')]);
    assert_eq!(app.stack_scroll, 0, "and it starts at the pointer again");
    let text = screen(&app, 80, 24);
    assert!(
        text.contains("0200  0000"),
        "and the window is at the pointer: {text}"
    );
}

#[test]
fn the_keys_that_were_there_before_still_do_what_they_did_with_the_stack_open() {
    let mut app = app("stack-keys");
    press(&mut app, &[KeyCode::Char('5')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.steps, 1, "F8 still steps the machine");
    press(&mut app, &[KeyCode::Char('2')]);
    assert_eq!(app.focus, Focus::Registers, "and 2 takes the registers");
    press(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.focus, Focus::Memory, "and 3 takes the memory");
    press(&mut app, &[KeyCode::Char('1')]);
    assert_eq!(app.focus, Focus::Disassembly, "and 1 takes the listing");
    assert!(!app.stack_open, "and the listing comes back for it");
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(app.help, "and ? is still the help");
    press(&mut app, &[KeyCode::Char('?')]);
    press(&mut app, &[KeyCode::Char('q')]);
    assert!(app.quit, "and q still quits");
}

#[test]
fn the_help_overlay_lists_the_stack() {
    let mut app = app("stack-help");
    press(&mut app, &[KeyCode::Char('?')]);
    let text = screen(&app, 80, 40);
    assert!(
        text.contains("5    the stack"),
        "the help names the key: {text}"
    );
}

/// A program with a few lines of source in it.
///
/// The first line is a directive, and a directive produces no bytes, so
/// the source starts at the second line of the file.
const SOURCE: &str = "org 100h\n\
                      mov ax, 1234h\n\
                      mov bx, 5678h\n\
                      add ax, bx\n\
                      sub cx, 1\n\
                      xor dx, dx\n\
                      hlt\n";

/// The marker the strip puts on the line the machine is stopped at.
///
/// It is the listing's own glyph: `▸` always means "here".
const MARK: char = '▸';

#[test]
fn the_source_strip_shows_the_programs_own_lines() {
    let app = app_of("source-lines", SOURCE);
    let rows = rows(&app, 80, 24);
    let text = rows.join("\n");
    assert!(
        text.contains("mov ax, 1234h"),
        "the source is the program's own: {text}"
    );
    assert!(
        text.contains("mov bx, 5678h"),
        "and the line after it: {text}"
    );
    assert!(
        text.contains("xor dx, dx"),
        "and the last of the five: {text}"
    );
    assert!(
        !text.contains("hlt"),
        "the strip shows five lines and no more: {text}"
    );
}

#[test]
fn the_source_lines_carry_their_own_numbers() {
    let app = app_of("source-numbers", SOURCE);
    let rows = rows(&app, 80, 24);
    let row = rows
        .iter()
        .find(|row| row.contains("mov bx, 5678h"))
        .expect("the strip shows the line");
    assert!(
        row.contains("3  mov bx, 5678h"),
        "the line carries the number the source gave it: {row:?}"
    );
}

#[test]
fn the_line_the_machine_is_stopped_at_is_marked() {
    let app = app_of("source-mark", "org 100h\nmov ax, 1234h\nhlt\n");
    let rows = rows(&app, 80, 24);
    let strip = rows
        .iter()
        .position(|row| row.contains("source"))
        .expect("the strip has a header of its own");
    let marked: Vec<&String> = rows[strip..]
        .iter()
        .filter(|row| row.contains(MARK))
        .collect();
    assert_eq!(marked.len(), 1, "one line is marked: {rows:?}");
    assert!(
        marked[0].contains("mov ax, 1234h"),
        "and it is the line the machine is stopped at: {marked:?}"
    );
}

#[test]
fn the_mark_moves_when_the_machine_steps() {
    let mut app = app_of("source-step", "org 100h\nmov ax, 1234h\nhlt\n");
    press(&mut app, &[KeyCode::F(8)]);
    let rows = rows(&app, 80, 24);
    let strip = rows
        .iter()
        .position(|row| row.contains("source"))
        .expect("the strip has a header of its own");
    let marked: Vec<&String> = rows[strip..]
        .iter()
        .filter(|row| row.contains(MARK))
        .collect();
    assert_eq!(marked.len(), 1, "one line is marked: {rows:?}");
    assert!(
        marked[0].contains("hlt"),
        "and the mark moved with the machine: {marked:?}"
    );
}

#[test]
fn the_strip_is_under_the_listing() {
    let app = app_of("source-place", SOURCE);
    let rows = rows(&app, 80, 24);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let strip = rows
        .iter()
        .position(|row| row.contains("mov ax, 1234h"))
        .expect("the strip");
    let panels = rows
        .iter()
        .position(|row| row.contains("registers"))
        .expect("the panels");
    assert!(listing < strip, "the strip is under the listing: {rows:?}");
    assert!(strip < panels, "and over the panels below: {rows:?}");
}

#[test]
fn the_strip_does_not_squeeze_the_listing_out() {
    let app = app_of("source-rows", SOURCE);
    let rows = rows(&app, 80, 20);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let strip = rows
        .iter()
        .position(|row| row.contains("mov ax, 1234h"))
        .expect("the strip");
    // The listing keeps three rows of its own, and its border is one row
    // above them.
    assert!(
        strip - listing >= 4,
        "the listing keeps three rows: {rows:?}"
    );
}

#[test]
fn an_app_with_no_program_has_no_strip() {
    let app = App::new(None).expect("the app");
    let rows = rows(&app, 80, 20);
    let text = rows.join("\n");
    assert!(
        text.contains("disassembly"),
        "the listing is still there: {text}"
    );
    assert!(!text.contains(MARK), "and there is no strip: {text}");
    assert!(
        !text.contains("mov ax"),
        "and no source of any kind: {text}"
    );
}

#[test]
fn the_listing_keeps_the_whole_band_when_there_is_no_source() {
    let app = App::new(None).expect("the app");
    let rows = rows(&app, 80, 20);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let panels = rows
        .iter()
        .position(|row| row.contains("registers"))
        .expect("the panels");
    assert_eq!(
        panels - listing,
        7,
        "the listing has the band to itself: {rows:?}"
    );
}

#[test]
fn the_app_keeps_the_program_it_opened() {
    let app = app_of("source-app", SOURCE);
    let view = app.source_view(3);
    assert_eq!(view.lines.len(), 3, "the strip shows three lines");
    assert_eq!(view.current, Some(0), "and the machine is at the first");
    assert_eq!(view.lines[0].number, 2, "the line's own number");
    assert_eq!(
        view.lines[0].text, "mov ax, 1234h",
        "and the text the source wrote"
    );
}

#[test]
fn an_app_with_no_program_has_no_source() {
    let app = App::new(None).expect("the app");
    let view = app.source_view(3);
    assert!(view.lines.is_empty(), "there is no source to show");
    assert_eq!(view.current, None, "and nowhere to be stopped at");
}

#[test]
fn the_byte_under_the_memory_cursor_is_written() {
    let mut app = app("memory-edit");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('e')]);
    type_text(&mut app, "34h");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.cpu.mem.read_u8(0x0000, 0x0000),
        0x34,
        "the byte is in the machine's memory"
    );
    let panels = app.panels();
    assert_eq!(
        panels.memory.bytes[panels.memory.cursor], 0x34,
        "and the panel reads it back"
    );
    assert!(app.edit.is_none(), "and the edit is over");
}

#[test]
fn a_byte_that_does_not_fit_is_refused() {
    let mut app = app("memory-refuse-range");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('e')]);
    type_text(&mut app, "1FFh");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.cpu.mem.read_u8(0x0000, 0x0000),
        0x00,
        "the memory is as it was"
    );
    let status = app.status.clone().unwrap_or_default();
    assert!(status.contains("out of range"), "and it says why: {status}");
    assert!(app.edit.is_some(), "and the edit is still open");
}

#[test]
fn the_byte_under_the_cursor_is_drawn_as_being_edited() {
    let mut app = app("memory-edit-drawing");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('e')]);
    type_text(&mut app, "5Ah");
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("5Ah"),
        "the byte shows what was typed: {text}"
    );
}

#[test]
fn the_flag_under_the_flags_cursor_is_written() {
    let mut app = app("flag-edit");
    press(&mut app, &[KeyCode::Char('4'), KeyCode::Char('e')]);
    type_text(&mut app, "1");
    press(&mut app, &[KeyCode::Enter]);
    assert!(app.cpu.regs.flags.get(CF), "the carry is in the flags word");
    assert!(
        app.panels().registers.flag(Flag::Cf),
        "and the panel reads the new value"
    );
    assert!(app.edit.is_none(), "and the edit is over");
}

#[test]
fn the_flag_the_cursor_is_on_is_the_one_that_is_written() {
    let mut app = app("flag-cursor");
    press(&mut app, &[KeyCode::Char('4'), KeyCode::Right]);
    press(&mut app, &[KeyCode::Char('e')]);
    type_text(&mut app, "1");
    press(&mut app, &[KeyCode::Enter]);
    assert!(app.cpu.regs.flags.get(PF), "the parity flag is set");
    assert!(!app.cpu.regs.flags.get(CF), "and the carry is as it was");
}

#[test]
fn a_flag_that_is_not_a_boolean_is_refused() {
    let mut app = app("flag-refuse");
    press(&mut app, &[KeyCode::Char('4'), KeyCode::Char('e')]);
    type_text(&mut app, "2");
    press(&mut app, &[KeyCode::Enter]);
    assert!(!app.cpu.regs.flags.get(CF), "the flag is as it was");
    let status = app.status.clone().unwrap_or_default();
    assert!(status.contains("out of range"), "and it says why: {status}");
    assert!(app.edit.is_some(), "and the edit is still open");
}

#[test]
fn the_flag_under_the_cursor_is_drawn_as_being_edited() {
    let mut app = app("flag-edit-drawing");
    press(&mut app, &[KeyCode::Char('4'), KeyCode::Char('e')]);
    type_text(&mut app, "1");
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("CF 1"),
        "the flag shows what was typed: {text}"
    );
}

#[test]
fn the_arrows_move_the_flags_cursor() {
    let mut app = app("flag-arrows");
    press(&mut app, &[KeyCode::Char('4')]);
    assert_eq!(app.flag_cursor, 0, "the cursor is on the first flag");
    press(&mut app, &[KeyCode::Right]);
    assert_eq!(app.flag_cursor, 1, "one flag to the right");
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(app.flag_cursor, 4, "and one row down");
    press(&mut app, &[KeyCode::Up]);
    assert_eq!(app.flag_cursor, 1, "and back up again");
    press(&mut app, &[KeyCode::Left]);
    assert_eq!(app.flag_cursor, 0, "and back to the first");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Down]);
    assert_eq!(
        app.flag_cursor, 0,
        "and it stays put when another pane has the focus"
    );
}

#[test]
fn the_status_line_says_what_is_being_edited() {
    let mut app = app("status-edit");
    press(&mut app, &[KeyCode::Char('2'), KeyCode::Char('e')]);
    type_text(&mut app, "34h");
    assert_eq!(status(&app, 80, 20), "AX = 34h", "the register is named");
    press(&mut app, &[KeyCode::Esc]);
    press(&mut app, &[KeyCode::Char('4'), KeyCode::Char('e')]);
    type_text(&mut app, "1");
    assert_eq!(status(&app, 80, 20), "CF", "the flag is named");
    press(&mut app, &[KeyCode::Esc]);
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('e')]);
    type_text(&mut app, "34h");
    assert_eq!(status(&app, 80, 20), "0000:0000", "the address is named");
}

#[test]
fn the_stack_opens_with_five() {
    let mut app = app("stack-five");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack is open");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(!app.stack_open, "and it closes again");
}

#[test]
fn the_goto_with_a_label_jumps_to_the_label() {
    let mut app = app_of("goto-label", "org 100h\nstart: mov ax, 1234h\nhlt\n");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "start");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0x0100, "the window is at the label");
    assert!(app.goto.is_none(), "and the prompt is closed");
}

#[test]
fn the_goto_with_a_memory_reference_jumps_and_looks_through_that_segment() {
    let mut app = app("goto-reference");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0200:0300");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0x0300, "the window is at the offset");
    assert_eq!(
        app.panels().memory.segment,
        0x0200,
        "and it looks through the segment the goto named"
    );
}

#[test]
fn the_goto_with_a_segment_name_looks_through_the_machines_own_segment() {
    let mut app = app_of("goto-segment", "org 100h\n#ds=0200#\nhlt\n");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "DS:0300");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0x0300, "the window is at the offset");
    assert_eq!(
        app.panels().memory.segment,
        0x0200,
        "and it looks through the machine's own data segment"
    );
}

#[test]
fn the_window_looks_through_ds_until_a_goto_names_another_segment() {
    let mut app = app_of("goto-keeps", "org 100h\n#ds=0200#\nhlt\n");
    assert_eq!(
        app.panels().memory.segment,
        0x0200,
        "the window looks through the data segment"
    );
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0400:0500");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.panels().memory.segment,
        0x0400,
        "the segment the goto named is kept"
    );
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0600");
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(app.ui.memory_at, 0x0600, "a bare offset moves the window");
    assert_eq!(
        app.panels().memory.segment,
        0x0400,
        "and the segment is the one the goto named"
    );
}

#[test]
fn the_conditions_pane_opens_with_b_and_draws_the_breakpoints() {
    let mut app = app("conditions");
    press(&mut app, &[KeyCode::F(9)]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the breakpoint is there");
    app.breakpoints
        .set_condition(entry.segment, entry.offset, Some("AX==1234h"))
        .expect("the condition is written");
    press(&mut app, &[KeyCode::Char('b')]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("conditions"), "the pane is named: {text}");
    assert!(
        !text.contains("disassembly"),
        "the listing is not on the screen: {text}"
    );
    assert!(
        text.contains(&format!("{:04X}:{:04X}", entry.segment, entry.offset)),
        "the breakpoint's segment and offset are drawn: {text}"
    );
    assert!(
        text.contains("AX==1234h"),
        "and the condition it carries: {text}"
    );
}

#[test]
fn a_breakpoint_without_a_condition_says_that_it_carries_none() {
    let mut app = app("conditions-none");
    press(&mut app, &[KeyCode::F(9)]);
    press(&mut app, &[KeyCode::Char('b')]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("no condition"),
        "the pane says that the breakpoint carries none: {text}"
    );
}

#[test]
fn a_watchpoint_is_drawn_as_a_watchpoint_and_says_what_it_watches() {
    let mut app = app("watchpoint");
    app.breakpoints.watch(0x0000, 0x0200, Access::Write);
    press(&mut app, &[KeyCode::Char('b')]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("watchpoint"), "it is a watchpoint: {text}");
    assert!(
        text.contains("0000:0200"),
        "with its segment and offset: {text}"
    );
    assert!(
        text.contains("write"),
        "and it says what it watches: {text}"
    );
}

#[test]
fn a_watchpoint_that_reads_says_that_too() {
    let mut app = app("watchpoint-read");
    app.breakpoints.watch(0x0000, 0x0200, Access::Read);
    press(&mut app, &[KeyCode::Char('b')]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("read"), "it says what it watches: {text}");
}

#[test]
fn the_condition_under_the_cursor_is_edited_and_the_machine_stops_on_it() {
    let mut app = app("condition");
    press(&mut app, &[KeyCode::F(9)]);
    press(&mut app, &[KeyCode::Char('b'), KeyCode::Char('e')]);
    type_text(&mut app, "AX==1234h");
    press(&mut app, &[KeyCode::Enter]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the entry is there");
    assert_eq!(
        entry.condition.as_deref(),
        Some("AX==1234h"),
        "the model has the condition"
    );
    assert!(app.edit.is_none(), "and the prompt is closed");
    assert!(
        !app.breakpoints
            .stops_at(&Machine::new(&app.cpu), entry.segment, entry.offset),
        "the condition does not hold yet"
    );
    press(&mut app, &[KeyCode::F(8)]);
    assert!(
        app.breakpoints
            .stops_at(&Machine::new(&app.cpu), entry.segment, entry.offset),
        "and the machine stops on it once the condition holds"
    );
}

#[test]
fn a_condition_that_does_not_read_is_refused_and_the_old_one_stays() {
    let mut app = app("condition-refuse");
    press(&mut app, &[KeyCode::F(9)]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the entry is there");
    app.breakpoints
        .set_condition(entry.segment, entry.offset, Some("AX==1234h"))
        .expect("the condition is written");
    press(&mut app, &[KeyCode::Char('b'), KeyCode::Char('e')]);
    type_text(&mut app, "zz");
    press(&mut app, &[KeyCode::Enter]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the entry is there");
    assert_eq!(
        entry.condition.as_deref(),
        Some("AX==1234h"),
        "the old condition is still there"
    );
    let status = app.status.clone().unwrap_or_default();
    assert!(
        status.contains("bad expression"),
        "the refusal is the evaluator's own: {status}"
    );
    assert!(app.edit.is_some(), "and the prompt stays open");
}

#[test]
fn a_condition_that_is_typed_empty_clears_it() {
    let mut app = app("condition-clear");
    press(&mut app, &[KeyCode::F(9)]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the entry is there");
    app.breakpoints
        .set_condition(entry.segment, entry.offset, Some("AX==1234h"))
        .expect("the condition is written");
    press(&mut app, &[KeyCode::Char('b'), KeyCode::Char('e')]);
    press(&mut app, &[KeyCode::Enter]);
    let entry = app
        .breakpoints
        .listing()
        .into_iter()
        .next()
        .expect("the entry is there");
    assert_eq!(entry.condition, None, "the condition is gone");
    assert!(app.edit.is_none(), "and the prompt is closed");
}

#[test]
fn the_arrows_move_the_cursor_over_the_entries() {
    let mut app = app("conditions-arrows");
    press(&mut app, &[KeyCode::F(9)]);
    press(&mut app, &[KeyCode::Down, KeyCode::F(9)]);
    assert_eq!(app.breakpoints.breakpoints().len(), 2, "two breakpoints");
    press(&mut app, &[KeyCode::Char('b')]);
    let entries = app.breakpoints.listing();
    let first = format!("{:04X}:{:04X}", entries[0].segment, entries[0].offset);
    let second = format!("{:04X}:{:04X}", entries[1].segment, entries[1].offset);
    let text = screen(&app, 80, 20);
    assert!(text.contains(&first), "the first entry is drawn: {text}");
    assert!(text.contains(&second), "and the second one: {text}");
    press(&mut app, &[KeyCode::Down]);
    let on_the_second = marked(&app, 80, 20);
    assert!(
        on_the_second.contains(&second),
        "the cursor is on the second entry: {on_the_second}"
    );
    press(&mut app, &[KeyCode::Up]);
    let on_the_first = marked(&app, 80, 20);
    assert!(
        on_the_first.contains(&first),
        "and back to the first one: {on_the_first}"
    );
}

#[test]
fn the_conditions_pane_closes_with_b_again_and_with_esc() {
    let mut app = app("conditions-close");
    press(&mut app, &[KeyCode::Char('b')]);
    assert!(
        screen(&app, 80, 20).contains("conditions"),
        "the pane is open"
    );
    press(&mut app, &[KeyCode::Char('b')]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "the listing comes back: {text}"
    );
    assert!(!text.contains("conditions"), "and the pane is gone: {text}");
    press(&mut app, &[KeyCode::Char('b'), KeyCode::Esc]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "and the escape closes it too: {text}"
    );
    assert!(!text.contains("conditions"), "and the pane is gone: {text}");
}

#[test]
fn the_conditions_and_the_stack_do_not_hold_the_listings_place_at_the_same_time() {
    let mut app = app("conditions-windows");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack is open");
    press(&mut app, &[KeyCode::Char('b')]);
    assert!(app.conditions_open, "the conditions are open");
    assert!(!app.stack_open, "and the stack gives way to them");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack takes the place back");
    assert!(!app.conditions_open, "and the conditions give way to it");
}

#[test]
fn a_breakpoint_is_not_set_while_the_conditions_pane_holds_the_listings_place() {
    let mut app = app("conditions-breakpoint");
    press(&mut app, &[KeyCode::Char('b'), KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        0,
        "the cursor is not on the screen, so there is nowhere to set one"
    );
}

#[test]
fn the_keys_that_were_there_before_still_do_what_they_did_with_the_conditions_open() {
    let mut app = app("conditions-keys");
    press(&mut app, &[KeyCode::Char('b')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.steps, 1, "F8 still steps the machine");
    press(&mut app, &[KeyCode::Char('2')]);
    assert_eq!(app.focus, Focus::Registers, "and 2 takes the registers");
    press(&mut app, &[KeyCode::Char('3')]);
    assert_eq!(app.focus, Focus::Memory, "and 3 takes the memory");
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(app.help, "and ? is still the help");
    press(&mut app, &[KeyCode::Char('?')]);
    press(&mut app, &[KeyCode::Char('q')]);
    assert!(app.quit, "and q still quits");
}

#[test]
fn the_help_overlay_lists_the_conditions() {
    let mut app = app("conditions-help");
    press(&mut app, &[KeyCode::Char('?')]);
    let text = screen(&app, 80, 40);
    assert!(
        text.contains("b    the conditions"),
        "the help names the key: {text}"
    );
}

#[test]
fn the_disassembly_draws_the_address_the_bytes_and_the_instruction() {
    let app = app("disassembly-text");
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("0000:0100  B8 34 12"),
        "the address and the bytes are drawn: {text}"
    );
    assert!(
        text.contains("MOV AX, 01234h"),
        "and the instruction is the reference's own: {text}"
    );
}

#[test]
fn the_console_opens_where_the_listing_is() {
    let mut app = app("console-window");
    let before = screen(&app, 80, 20);
    assert!(
        before.contains("disassembly"),
        "the listing is there to begin with: {before}"
    );
    press(&mut app, &[KeyCode::Char(':')]);
    let text = screen(&app, 80, 20);
    assert!(text.contains("console"), "the window is named: {text}");
    assert!(
        !text.contains("disassembly"),
        "the listing is not on the screen: {text}"
    );
    press(&mut app, &[KeyCode::Char(':')]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "and the listing comes back: {text}"
    );
    press(&mut app, &[KeyCode::Char(':'), KeyCode::Esc]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("disassembly"),
        "and the escape closes it too: {text}"
    );
}

#[test]
fn the_console_shows_what_the_commands_said() {
    let mut app = app("console-output");
    press(&mut app, &[KeyCode::Char(':')]);
    type_text(&mut app, "r AX");
    press(&mut app, &[KeyCode::Enter]);
    let text = screen(&app, 80, 20);
    assert!(
        text.contains("AX 0000"),
        "the console says what the command said: {text}"
    );
    // The output is above and the command line is at the bottom.
    let rows = rows(&app, 80, 20);
    let output = rows
        .iter()
        .position(|row| row.contains("AX 0000"))
        .expect("the output is on the screen");
    let command = rows
        .iter()
        .position(|row| row.contains(": "))
        .expect("the command line is on the screen");
    assert!(
        output < command,
        "the command line is under the output: {rows:?}"
    );
}

#[test]
fn the_console_holds_the_listings_place_alone() {
    let mut app = app("console-alone");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack is open");
    press(&mut app, &[KeyCode::Char(':')]);
    assert!(app.console_open, "the console is open");
    assert!(!app.stack_open, "and the stack gives way to it");
}

#[test]
fn a_breakpoint_is_not_set_while_the_console_holds_the_listings_place() {
    let mut app = app("console-breakpoint");
    press(&mut app, &[KeyCode::Char(':'), KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        0,
        "the cursor is not on the screen, so there is nowhere to set one"
    );
}

#[test]
fn the_help_overlay_lists_the_console() {
    let mut app = app("console-help");
    press(&mut app, &[KeyCode::Char('?')]);
    let text = screen(&app, 80, 40);
    assert!(
        text.contains(":    the console"),
        "the help names the key: {text}"
    );
}

#[test]
fn a_watchpoint_carries_no_condition_and_the_model_says_so() {
    let mut app = app("condition-watchpoint");
    app.breakpoints.watch(0x0000, 0x0200, Access::Write);
    press(&mut app, &[KeyCode::Char('b'), KeyCode::Char('e')]);
    type_text(&mut app, "AX==1");
    press(&mut app, &[KeyCode::Enter]);
    let status = app.status.clone().unwrap_or_default();
    assert!(
        status.contains("no breakpoint there"),
        "the model says that there is no breakpoint there: {status}"
    );
    assert!(app.edit.is_some(), "and the prompt stays open");
}

/// The design's own rules, and what the screen says about them.
///
/// The redesign changes the frame and not the vocabulary: the words the
/// tests pin are still there, and the frame around them is the design's
/// own.

#[test]
fn no_panel_is_framed_with_a_box() {
    let app = app("design-boxes");
    let text = screen(&app, 140, 40);
    for corner in ['┌', '┐', '└', '┘'] {
        assert!(!text.contains(corner), "no panel is a card: {text}");
    }
}

#[test]
fn the_sections_are_separated_by_thin_rules() {
    let app = app("design-rules");
    let text = screen(&app, 140, 40);
    assert!(
        text.contains('│'),
        "the bottom band's sections have a rule between them: {text}"
    );
    assert!(
        text.contains('─'),
        "the bands have a rule between them: {text}"
    );
}

#[test]
fn the_focused_sections_header_is_the_accent_and_the_others_are_not() {
    let app = app("design-focus");
    let buffer = buffer(&app, 140, 40);
    let (column, row) = place(&buffer, "disassembly").expect("the listing is named");
    let header = buffer.cell((column, row)).expect("the header is drawn");
    assert_eq!(
        header.fg,
        theme::ACCENT,
        "the section that has the focus is the accent"
    );
    assert!(
        header.modifier.contains(Modifier::BOLD),
        "and its header is bold"
    );
    let (column, row) = place(&buffer, "registers").expect("the registers are named");
    let header = buffer.cell((column, row)).expect("the header is drawn");
    assert_ne!(
        header.fg,
        theme::ACCENT,
        "the sections that do not have the focus are not the accent"
    );
}

#[test]
fn the_memory_cursor_is_marked_even_when_the_memory_does_not_have_the_focus() {
    let mut app = app("design-memory-cursor");
    press(&mut app, &[KeyCode::Char('3'), KeyCode::Char('g')]);
    type_text(&mut app, "0100");
    press(&mut app, &[KeyCode::Enter]);
    // The listing takes the focus back, and the window keeps its cursor.
    press(&mut app, &[KeyCode::Char('1')]);
    assert_eq!(app.focus, Focus::Disassembly);
    let unfocused = buffer(&app, 140, 40);
    let (column, row) = place(&unfocused, "0100 B8 34 12").expect("the row is drawn");
    let byte = unfocused
        .cell((column + 5, row))
        .expect("the byte under the cursor is drawn");
    assert_eq!(byte.symbol(), "B8", "the byte under the cursor is there");
    assert_eq!(
        byte.fg,
        theme::TEXT_MUTED,
        "and it is marked without the focus"
    );
    // With the focus the same byte is the accent.
    press(&mut app, &[KeyCode::Char('3')]);
    let focused = buffer(&app, 140, 40);
    let (column, row) = place(&focused, "0100 B8 34 12").expect("the row is drawn");
    let byte = focused
        .cell((column + 5, row))
        .expect("the byte under the cursor is drawn");
    assert_eq!(
        byte.fg,
        theme::ACCENT,
        "the accent when the section has the focus"
    );
}

#[test]
fn the_instruction_the_machine_is_at_carries_the_marker() {
    let app = app_of("design-marker", SOURCE);
    let buffer = buffer(&app, 140, 40);
    let (_, row) = place(&buffer, "MOV AX, 01234h").expect("the instruction is drawn");
    let marker = buffer.cell((0, row)).expect("the marker is drawn");
    assert_eq!(
        marker.symbol(),
        "▸",
        "the instruction the machine is at carries the marker"
    );
}

#[test]
fn a_breakpoint_carries_its_own_marker() {
    let mut app = app_of("design-breakpoint", SOURCE);
    press(&mut app, &[KeyCode::Down, KeyCode::F(9)]);
    let buffer = buffer(&app, 140, 40);
    let (_, row) = place(&buffer, "MOV BX, 05678h").expect("the instruction is drawn");
    let marker = buffer.cell((1, row)).expect("the marker is drawn");
    assert_eq!(marker.symbol(), "◆", "a breakpoint carries its own marker");
}

#[test]
fn a_watchpoint_carries_its_own_marker() {
    let mut app = app_of("design-watchpoint", SOURCE);
    app.breakpoints.watch(0x0000, 0x0103, Access::Write);
    let buffer = buffer(&app, 140, 40);
    let (_, row) = place(&buffer, "MOV BX, 05678h").expect("the instruction is drawn");
    let marker = buffer.cell((1, row)).expect("the marker is drawn");
    assert_eq!(marker.symbol(), "◇", "a watchpoint carries its own marker");
}

#[test]
fn the_header_shows_the_program_and_the_state() {
    let app = app("design-header");
    let rows = rows(&app, 140, 40);
    let header = &rows[0];
    assert!(header.contains("emo8067"), "the product is named: {header}");
    assert!(
        header.contains("program.asm"),
        "the program is named: {header}"
    );
    assert!(
        header.contains('●'),
        "the state carries its glyph: {header}"
    );
    assert!(header.contains("RUNNING"), "and it is named: {header}");
}

#[test]
fn the_header_says_when_the_machine_is_halted() {
    let mut app = app_of("design-halted", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::F(8)]);
    let rows = rows(&app, 140, 40);
    assert!(
        rows[0].contains('○'),
        "the halted machine has its own glyph: {}",
        rows[0]
    );
}

#[test]
fn the_status_bar_puts_the_hints_on_the_left_and_the_numbers_on_the_right() {
    let app = app("design-status");
    let rows = rows(&app, 140, 40);
    let status = rows.last().expect("the status bar is there");
    let hints = status.find("q quits").expect("the hints are on the bar");
    let numbers = status.find("IP 0100").expect("the numbers are on the bar");
    assert!(hints < numbers, "the hints are on the left: {status}");
    assert!(
        status.trim_end().ends_with("WATCH 0"),
        "the numbers are on the right: {status}"
    );
}

#[test]
fn the_labels_are_dim_and_the_values_are_not() {
    let app = app("design-labels");
    let buffer = buffer(&app, 140, 40);
    let (column, row) = place(&buffer, "AX 0000").expect("the register is drawn");
    let name = buffer.cell((column, row)).expect("the name is drawn");
    assert_eq!(name.fg, theme::TEXT_SECONDARY, "a label is dim");
    let value = buffer.cell((column + 3, row)).expect("the value is drawn");
    assert_eq!(value.fg, theme::TEXT_PRIMARY, "and a value is not");
}

#[test]
fn the_source_strip_has_a_header_of_its_own() {
    let app = app_of("design-strip", SOURCE);
    let rows = rows(&app, 140, 40);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let strip = rows
        .iter()
        .position(|row| row.contains("source"))
        .expect("the strip has a header");
    assert!(listing < strip, "the strip is under the listing: {rows:?}");
    let panels = rows
        .iter()
        .position(|row| row.contains("registers"))
        .expect("the panels");
    assert!(strip < panels, "and it is over the panels below: {rows:?}");
}

/// The polish: the second round of the design.
///
/// The redesign changed the frame and not the vocabulary. The polish
/// changes what the screen says with the space it has: the listing
/// keeps the rows it needs, the strip takes the rest, the console's
/// caret is the terminal's own and the help overlay is a section and
/// not a box.

#[test]
fn the_listing_keeps_more_than_three_instructions_at_eighty_columns() {
    let app = app_of("polish-listing", SOURCE);
    let rows = rows(&app, 80, 24);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let strip = rows
        .iter()
        .position(|row| row.contains("source"))
        .expect("the strip");
    let instructions = strip - listing - 1;
    assert!(
        instructions > 3,
        "the listing is the panel a debugger lives in: {instructions} instructions, {rows:?}"
    );
}

#[test]
fn the_listing_fills_the_band_on_a_tall_screen() {
    let app = app_of("polish-tall", SOURCE);
    let rows = rows(&app, 140, 40);
    let listing = rows
        .iter()
        .position(|row| row.contains("disassembly"))
        .expect("the listing");
    let strip = rows
        .iter()
        .position(|row| row.contains("source"))
        .expect("the strip");
    // Every row between the listing's header and the strip's header
    // holds an instruction: the band says nothing.
    for (offset, row) in rows[listing + 1..strip].iter().enumerate() {
        assert!(
            !row.trim().is_empty(),
            "the band says nothing at row {}: {rows:?}",
            listing + 1 + offset
        );
    }
}

#[test]
fn the_console_caret_is_not_the_listings_glyph() {
    let mut app = app("polish-console");
    press(&mut app, &[KeyCode::Char(':')]);
    let rows = rows(&app, 80, 24);
    let command = rows
        .iter()
        .find(|row| row.contains(": "))
        .expect("the command line is on the screen");
    assert!(
        !command.contains(MARK),
        "the caret is not a second arrow: {command:?}"
    );
    // And it is the terminal's own: the inverted cell.
    let buffer = buffer(&app, 80, 24);
    let (column, row) = place(&buffer, ": ").expect("the command line is drawn");
    let caret = buffer.cell((column + 2, row)).expect("the caret is drawn");
    assert!(
        caret.modifier.contains(Modifier::REVERSED),
        "the caret is the inverted cell"
    );
}

#[test]
fn the_help_overlay_is_a_section_and_not_a_box() {
    let mut app = app("polish-help");
    press(&mut app, &[KeyCode::Char('?')]);
    let buffer = buffer(&app, 80, 24);
    let rows = rows(&app, 80, 24);
    let header = rows
        .iter()
        .position(|row| row.contains("keys"))
        .expect("the help has a header of its own");
    let panels = rows
        .iter()
        .position(|row| row.contains("registers"))
        .expect("the panels");
    for row in &rows[header..panels] {
        for corner in ['┌', '┐', '└', '┘', '│', '─'] {
            assert!(!row.contains(corner), "the help is not a box: {row:?}");
        }
    }
    // The section that has the focus says so with its header, the way
    // every other section does.
    let (column, row) = place(&buffer, "keys").expect("the header is drawn");
    let header = buffer.cell((column, row)).expect("the header is drawn");
    assert_eq!(
        header.fg,
        theme::ACCENT,
        "the help is the section that is on the screen"
    );
    assert!(
        header.modifier.contains(Modifier::BOLD),
        "and its header is bold"
    );
}

#[test]
fn the_memory_window_takes_the_width_its_bytes_need() {
    let app = app("polish-memory");
    let rows = rows(&app, 80, 24);
    let row = rows
        .iter()
        .position(|row| row.contains("0000 00"))
        .expect("the window is drawn");
    let text = &rows[row];
    // The rule that follows the memory window, and the content that
    // comes before it. The rule is counted in characters, so that the
    // column it is at is the column it is drawn in.
    let rule = text
        .chars()
        .enumerate()
        .filter(|(_, character)| *character == '│')
        .map(|(index, _)| index)
        .last()
        .expect("the rule that follows the window");
    let content: String = text.chars().take(rule).collect();
    let content = content.trim_end();
    assert_eq!(
        content.chars().count(),
        rule,
        "the window is as wide as the bytes it shows: {text:?}"
    );
}

/// The keys that come with the core: the watchpoint, the step back and the
/// step over.
#[test]
fn the_watchpoint_key_watches_the_byte_under_the_cursor() {
    let mut app = app("watchpoint");
    app.ui.memory_at = 0x0200;
    press(&mut app, &[KeyCode::Char('w')]);
    let watchpoints = app.breakpoints.watchpoints();
    assert_eq!(watchpoints.len(), 1, "the watchpoint is the model's own");
    assert_eq!(
        watchpoints[0].segment,
        app.memory_segment(),
        "through the segment the window looks through"
    );
    assert_eq!(
        watchpoints[0].offset, 0x0200,
        "on the byte under the cursor"
    );
    assert_eq!(
        watchpoints[0].access,
        Access::Write,
        "and it watches the writes"
    );
    // And the model lists it, the way the conditions pane draws it.
    assert_eq!(
        app.breakpoints.listing().len(),
        1,
        "the listing is the model's own"
    );
    // And `w` again takes it away.
    press(&mut app, &[KeyCode::Char('w')]);
    assert!(
        app.breakpoints.watchpoints().is_empty(),
        "and w again takes it away"
    );
}

#[test]
fn the_step_back_key_rewinds_the_machine() {
    let mut app = app_of("step-back", SOURCE);
    let before = app.cpu.regs.ip;
    press(&mut app, &[KeyCode::F(8)]);
    assert_ne!(app.cpu.regs.ip, before, "the machine moved");
    let after = app.cpu.regs.ip;
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(app.cpu.regs.ip, before, "and F6 puts it back");
    assert_eq!(app.steps, 0, "the counter went down");
    assert!(
        !app.executed.contains(&after),
        "and the address the machine was at is not executed any more"
    );
}

#[test]
fn the_step_back_takes_the_address_the_machine_was_at_off_the_list() {
    // The machine visits the same address twice: the list holds it once,
    // and the step back takes that one off the list.
    let mut app = app_of("step-back-list", "org 100h\njmp lower\nlower: jmp lower\n");
    press(&mut app, &[KeyCode::F(8), KeyCode::F(8)]);
    let at = app.cpu.regs.ip;
    assert!(
        app.executed.contains(&at),
        "the machine has been there: {:?}",
        app.executed
    );
    press(&mut app, &[KeyCode::F(6)]);
    assert!(
        !app.executed.contains(&at),
        "and the step back takes it off the list: {:?}",
        app.executed
    );
}

#[test]
fn a_step_back_with_nothing_to_rewind_is_refused() {
    let mut app = app("step-back-empty");
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.status.as_deref(),
        Some("There are no recorded back steps ...."),
        "the refusal is the reference's own words"
    );
    assert_eq!(app.steps, 0, "and nothing moved");
}

#[test]
fn the_step_over_key_runs_a_call_to_its_end() {
    let mut app = app_of("step-over", CALL);
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0103,
        "the machine is where the call returns"
    );
    assert_eq!(
        app.cpu.regs.word(Reg16::Ax),
        0x1234,
        "and the call ran to its end"
    );
}

#[test]
fn the_help_lists_every_key_at_eighty_columns() {
    let mut app = app("help-keys");
    press(&mut app, &[KeyCode::Char('?')]);
    let text = rows(&app, 80, 24).join("\n");
    // The list is the screen's own: the test reads the production list,
    // so a key that is added and not shown fails the test.
    for entry in emo8067_tui::ui::KEYS {
        assert!(text.contains(entry), "the help lists {entry}: {text}");
    }
}

/// A program that writes to a byte and then runs on.
const STORES: &str = "org 100h\n\
                      mov ax, 1234h\n\
                      mov [0200h], ax\n\
                      mov bx, 5678h\n\
                      hlt\n";

#[test]
fn a_watchpoint_fires_when_the_machine_writes_the_byte() {
    let mut app = app_of("watchpoint-fires", STORES);
    app.ui.memory_at = 0x0200;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0200"),
        "the status names the address that was written"
    );
    assert_eq!(
        app.cpu.regs.ip, 0x0107,
        "and the machine stopped there: it did not run on"
    );
}

#[test]
fn a_program_that_writes_elsewhere_runs_on() {
    let mut app = app_of("watchpoint-miss", STORES);
    // The watchpoint is on a byte the program does not write.
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(app.status, None, "the watchpoint did not fire");
    assert!(app.finished, "and the machine ran on to the halt");
}

/// A program with a call in it, for the step over.
const CALL: &str = "org 100h\n\
                    call f\n\
                    hlt\n\
                    f:\n\
                    mov ax, 1234h\n\
                    ret\n";
