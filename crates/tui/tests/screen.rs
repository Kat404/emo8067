//! The guest's screen: the takeover, and the page under it.
//!
//! The screen is the passive viewer of the text page: the guest's own
//! screen takes the terminal over, and the debugger's keys keep working
//! while it does.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_core::regs::Seg;
use emo8067_tui::app::App;
use emo8067_tui::ui::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Modifier;

/// The counter that tells the directories of one run apart.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory of this run's own, and the guard that takes it away again.
///
/// The name carries the process id and a counter, so two runs never share
/// one, and the directory is made exclusively: another local user cannot
/// have made it before us, and the program inside it cannot be a symlink
/// somebody else left behind.
struct Directory {
    path: PathBuf,
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A directory of this run's own, made exclusively.
fn directory(name: &str) -> Directory {
    let path = std::env::temp_dir().join(format!(
        "emo8067-tui-screen-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&path).expect("the directory");
    Directory { path }
}

/// The app, with that program of the test's own words open.
///
/// The program is read while it is there, and the directory goes when the
/// app is built: nothing reads it again.
fn app_of(name: &str, source: &str) -> App {
    let dir = directory(name);
    let path = dir.path.join("program.asm");
    std::fs::write(&path, source).expect("the program");
    App::new(Some(path.to_string_lossy().to_string())).expect("the program loads")
}

/// The keys, one at a time.
fn press(app: &mut App, keys: &[KeyCode]) {
    for key in keys {
        app.key(KeyEvent::new(*key, KeyModifiers::NONE));
    }
}

/// The screen the app draws, one string per row.
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

/// The cells the app draws.
fn buffer(app: &App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    terminal.backend().buffer().clone()
}

/// A program that does nothing at all.
const IDLE: &str = "org 100h\nhlt\n";

/// One of the reference's own examples, when it is there.
///
/// The examples are local gold: they are not in the repository, and the
/// gate stands aside when they are not.
fn example(name: &str) -> Option<PathBuf> {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../re/extracted/app/examples"
    ))
    .join(name);
    if path.is_file() { Some(path) } else { None }
}

/// The segment the text page lives in.
const PAGE: u16 = 0xB800;

/// The screen takes the terminal over, and the keys keep working.
#[test]
fn the_screen_takes_the_terminal_over_and_the_keys_keep_working() {
    let mut app = app_of("takeover", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'H');
    let before = rows(&app, 80, 24).join("\n");
    assert!(
        before.contains("disassembly"),
        "the listing is there to begin with: {before}"
    );

    press(&mut app, &[KeyCode::Char('x')]);
    let text = rows(&app, 80, 24).join("\n");

    assert!(text.contains('H'), "the guest's screen is on: {text}");
    assert!(
        !text.contains("disassembly"),
        "and the debugger's own panels are not: {text}"
    );
    assert!(!text.contains("registers"), "nor the bottom band: {text}");
    // The machine's state is in the status bar, and the status bar is
    // not under the takeover.
    let status = rows(&app, 80, 24).last().cloned().unwrap_or_default();
    assert!(
        status.contains("IP 0100"),
        "the machine's state is in the status bar: {status}"
    );

    // The debugger's own keys keep working.
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.steps, 1, "F8 still steps the machine");
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(app.help, "and ? is still the help");
    press(&mut app, &[KeyCode::Char('?')]);

    // The keys that act on the listing do not act: the takeover holds
    // the listing's place with the whole screen, and the listing's cursor
    // is not on the screen while it does.
    press(&mut app, &[KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        0,
        "and F9 sets no breakpoint at a cursor nobody can see"
    );
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(app.cursor, 0, "and the arrows move no cursor either");

    // And `x` again closes it.
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(!app.screen_open, "the screen is closed again");
    let text = rows(&app, 80, 24).join("\n");
    assert!(
        text.contains("disassembly"),
        "and the debugger's own screen comes back: {text}"
    );

    // The listing's own keys work again.
    press(&mut app, &[KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        1,
        "and F9 sets a breakpoint at the cursor once more"
    );
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(app.cursor, 1, "and the arrows move the cursor again");
}

/// Only one window holds the screen at a time.
///
/// The takeover and the three windows that hold the listing's place are
/// four windows over the same screen, and the last one to be opened is
/// the one that is there.
#[test]
fn the_takeover_and_the_windows_hold_the_screen_one_at_a_time() {
    let mut app = app_of("exclusive", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover is open");

    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack takes the screen");
    assert!(!app.screen_open, "and the takeover gives way to it");

    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover takes it back");
    assert!(!app.stack_open, "and the stack gives way to it");

    press(&mut app, &[KeyCode::Esc]);
    assert!(!app.screen_open, "Esc closes the takeover");

    press(&mut app, &[KeyCode::Char('x')]);
    press(&mut app, &[KeyCode::Char('1')]);
    assert!(!app.screen_open, "and the listing takes it back too");
    assert!(
        rows(&app, 80, 24).join("\n").contains("disassembly"),
        "the listing is on the screen again"
    );
}

/// The cell is drawn with the colours its attribute byte carries.
#[test]
fn the_attribute_is_drawn_with_the_colours_the_byte_carries() {
    let mut app = app_of("attribute", IDLE);
    // A red character on a blue background.
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x14);
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 80, 24);
    let cell = buffer.cell((0, 0)).expect("the cell is drawn");

    assert_eq!(cell.symbol(), "A", "the character is drawn");
    assert_eq!(
        cell.fg,
        theme::VGA[4],
        "the foreground is the attribute's own red"
    );
    assert_eq!(
        cell.bg,
        theme::VGA[1],
        "and the background is the attribute's own blue"
    );
}

/// The background's own brightness is the eighth background.
#[test]
fn the_backgrounds_bright_bit_is_the_eighth_background() {
    let mut app = app_of("bright", IDLE);
    // The attribute's high bit: the background's own brightness.
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x80);
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 80, 24);
    let cell = buffer.cell((0, 0)).expect("the cell is drawn");

    assert_eq!(
        cell.bg,
        theme::VGA[8],
        "the bright bit is the eighth background"
    );
    assert_ne!(cell.bg, theme::VGA[0], "and not the black one");
    assert_ne!(
        cell.bg,
        theme::VGA[7],
        "and not a shade of the seventh either"
    );
}

/// A cell that blinks carries the blink, and not a colour of its own.
///
/// The attribute's blink bit is a state of the cell, and a state is
/// never colour alone.
#[test]
fn a_cell_that_blinks_carries_the_blink() {
    let mut app = app_of("blink", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    // The blink bit of the attribute: the third bit.
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x08);
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 80, 24);
    let cell = buffer.cell((0, 0)).expect("the cell is drawn");

    assert!(
        cell.modifier.contains(Modifier::SLOW_BLINK),
        "the cell blinks: {:?}",
        cell.modifier
    );
}

/// The help names the key, and it loses no other one.
#[test]
fn the_help_names_the_screen_key_and_keeps_every_key_whole() {
    let mut app = app_of("help", IDLE);
    press(&mut app, &[KeyCode::Char('?')]);

    let text = rows(&app, 80, 24).join("\n");
    assert!(
        text.contains("x    the guest's screen"),
        "the help names the key: {text}"
    );
    for key in emo8067_tui::ui::KEYS {
        assert!(text.contains(key), "the help loses {key:?}: {text}");
    }
}

/// The page is drawn from its top-left corner.
#[test]
fn the_page_is_drawn_from_its_top_left_corner() {
    let mut app = app_of("corner", IDLE);
    // The first cell of the page, and the eightieth column of its row.
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x009E, b'Z');
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 80, 24);

    assert_eq!(
        buffer.cell((0, 0)).map(|cell| cell.symbol()),
        Some("A"),
        "the page starts at the top-left corner"
    );
    assert_eq!(
        buffer.cell((79, 0)).map(|cell| cell.symbol()),
        Some("Z"),
        "and the eightieth column is at the eightieth column"
    );
}

/// What does not fit is cut.
#[test]
fn what_does_not_fit_is_cut() {
    let mut app = app_of("cut", IDLE);
    // The first cell, the eightieth column, and the thirteenth row.
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x009E, b'Z');
    app.cpu.mem.write_u8(PAGE, 0x0780, b'B');
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 40, 12);

    assert_eq!(
        buffer.cell((0, 0)).map(|cell| cell.symbol()),
        Some("A"),
        "the page starts at the top-left corner"
    );
    assert_ne!(
        buffer.cell((39, 0)).map(|cell| cell.symbol()),
        Some("Z"),
        "the eightieth column does not fit"
    );
    assert_ne!(
        buffer.cell((0, 11)).map(|cell| cell.symbol()),
        Some("B"),
        "and the thirteenth row does not fit either"
    );
}

/// The cells past the page's width are the ground, and not the row below.
///
/// The page is eighty columns wide, and a wider terminal has room for
/// the ground beside it.
#[test]
fn the_cells_past_the_page_are_the_ground_and_not_the_row_below() {
    let mut app = app_of("wide", IDLE);
    // The first cell of the page, and the first cell of its second row.
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x00A0, b'B');
    press(&mut app, &[KeyCode::Char('x')]);

    let buffer = buffer(&app, 100, 24);

    assert_eq!(
        buffer.cell((0, 0)).map(|cell| cell.symbol()),
        Some("A"),
        "the first cell is the first cell"
    );
    assert_eq!(
        buffer.cell((0, 1)).map(|cell| cell.symbol()),
        Some("B"),
        "and the second row is the second row"
    );
    assert_ne!(
        buffer.cell((80, 0)).map(|cell| cell.symbol()),
        Some("B"),
        "and the cells past the eightieth column are not the row below"
    );
    assert_eq!(
        buffer.cell((80, 0)).map(|cell| cell.symbol()),
        Some(" "),
        "they are the ground"
    );
}

// The probes below were written from the specs and not from the code, so
// they are a second opinion on the same question.

/// The takeover takes every row but the status line.
#[test]
fn the_takeover_takes_every_row_but_the_status_line() {
    let mut app = app_of("takeover-rows", IDLE);
    for row in 0..25u16 {
        app.cpu.mem.write_u8(PAGE, row * 160, b'Q');
    }
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);
    for row in 0..23u16 {
        assert_eq!(
            buffer.cell((0, row)).map(|c| c.symbol()),
            Some("Q"),
            "row {row} is the takeover's"
        );
    }
    let status: String = (0..80)
        .map(|x| buffer.cell((x, 23)).map_or("", |c| c.symbol()))
        .collect();
    assert!(
        status.contains("IP 0100"),
        "the machine's state is in the status bar: {status}"
    );
    assert!(
        !status.contains('Q'),
        "and the status bar is not under the takeover: {status}"
    );
}

/// The debugger's own panels are gone under the takeover.
#[test]
fn the_debuggers_own_panels_are_gone_under_the_takeover() {
    let mut app = app_of("takeover-panels", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    let text = rows(&app, 80, 24).join("\n");
    for title in ["disassembly", "registers", "flags", "source"] {
        assert!(
            !text.contains(title),
            "the {title} panel is under the takeover: {text}"
        );
    }
}

/// The status bar still carries the machine's state.
#[test]
fn the_status_bar_still_carries_the_machines_state() {
    let mut app = app_of("takeover-status", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    let before = rows(&app, 80, 24).last().cloned().unwrap_or_default();
    assert!(before.contains("IP 0100"), "the IP is there: {before}");
    assert!(before.contains("STEP 0"), "and the step count: {before}");
    press(&mut app, &[KeyCode::F(8)]);
    let after = rows(&app, 80, 24).last().cloned().unwrap_or_default();
    assert!(
        after.contains("IP 0101"),
        "and it follows the machine: {after}"
    );
    assert!(after.contains("STEP 1"), "and the step count: {after}");
}

/// The debugger's own keys keep working while the takeover is open.
#[test]
fn the_debuggers_own_keys_keep_working_while_the_takeover_is_open() {
    let mut app = app_of("takeover-keys", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.steps, 1, "F8 still steps the machine");
    assert_eq!(app.cpu.regs.ip, 0x0101, "and the machine moved");
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(app.steps, 0, "F6 still steps the machine back");
    assert_eq!(app.cpu.regs.ip, 0x0100, "and the machine is back");
    press(&mut app, &[KeyCode::F(5)]);
    assert!(app.cpu.halted, "F5 still runs the machine to the end");
}

/// The keys that act on the listing act on nothing.
#[test]
fn the_keys_that_act_on_the_listing_act_on_nothing() {
    let mut app = app_of("takeover-listing", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    press(&mut app, &[KeyCode::F(9)]);
    assert_eq!(
        app.breakpoints.breakpoints().len(),
        0,
        "F9 sets no breakpoint at a cursor nobody can see"
    );
    press(&mut app, &[KeyCode::Down]);
    press(&mut app, &[KeyCode::Right]);
    assert_eq!(app.cursor, 0, "and the arrows move no cursor either");
    assert_eq!(app.ui.memory_at, 0, "and no memory cursor either");
}

/// Only one window holds the screen at a time.
#[test]
fn only_one_window_holds_the_screen_at_a_time() {
    let mut app = app_of("takeover-windows", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover is open");
    press(&mut app, &[KeyCode::Char('b')]);
    assert!(app.conditions_open, "the conditions take the screen");
    assert!(!app.screen_open, "and the takeover gives way");
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover takes it back");
    assert!(!app.conditions_open, "and the conditions give way");
    press(&mut app, &[KeyCode::Char('5')]);
    assert!(app.stack_open, "the stack takes the screen");
    assert!(!app.screen_open, "and the takeover gives way");
    press(&mut app, &[KeyCode::Char('x')]);
    press(&mut app, &[KeyCode::Esc]);
    assert!(!app.screen_open, "Esc closes the takeover");
}

/// The TUI holds no copy of the page.
#[test]
fn the_tui_holds_no_copy_of_the_page() {
    let mut app = app_of("takeover-copy", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    press(&mut app, &[KeyCode::Char('x')]);
    let first = buffer(&app, 80, 24);
    assert_eq!(
        first.cell((0, 0)).map(|c| c.symbol()),
        Some("A"),
        "the guest's byte is on the screen"
    );
    app.cpu.mem.write_u8(PAGE, 0x0000, b'B');
    let second = buffer(&app, 80, 24);
    assert_eq!(
        second.cell((0, 0)).map(|c| c.symbol()),
        Some("B"),
        "the screen is read again: the TUI holds no copy of the page"
    );
}

/// Every attribute is drawn with its own colours.
#[test]
fn every_attribute_is_drawn_with_its_own_colours() {
    for attribute in 0u8..=255 {
        let mut app = app_of("attribute-colours", IDLE);
        app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
        app.cpu.mem.write_u8(PAGE, 0x0001, attribute);
        press(&mut app, &[KeyCode::Char('x')]);
        let buffer = buffer(&app, 80, 24);
        let cell = buffer.cell((0, 0)).expect("the cell is drawn");
        assert_eq!(
            cell.fg,
            theme::VGA[usize::from(attribute & 0x07)],
            "the foreground of {attribute:#04x}"
        );
        assert_eq!(
            cell.bg,
            theme::VGA[usize::from((attribute >> 4) & 0x0F)],
            "the background of {attribute:#04x}"
        );
        assert_eq!(
            cell.modifier.contains(Modifier::SLOW_BLINK),
            attribute & 0x08 != 0,
            "the blink of {attribute:#04x}"
        );
    }
}

/// The sixteen colours are sixteen colours.
#[test]
fn the_sixteen_colours_are_sixteen_colours() {
    let mut seen = Vec::new();
    for colour in theme::VGA {
        assert!(
            !seen.contains(&colour),
            "the palette holds a colour twice: {colour:?}"
        );
        seen.push(colour);
    }
    assert_ne!(
        theme::VGA[8],
        theme::VGA[7],
        "the eighth background is not a shade of the seventh"
    );
    assert_ne!(
        theme::VGA[8],
        theme::VGA[0],
        "and it is not the black one either"
    );
}

/// A blinking cell is not colour alone.
#[test]
fn a_blinking_cell_is_not_colour_alone() {
    let mut app = app_of("attribute-blink", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x00);
    press(&mut app, &[KeyCode::Char('x')]);
    let still = buffer(&app, 80, 24);
    let still_cell = still.cell((0, 0)).expect("the cell is drawn");
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x08);
    let blinking = buffer(&app, 80, 24);
    let blinking_cell = blinking.cell((0, 0)).expect("the cell is drawn");
    assert_eq!(
        still_cell.fg, blinking_cell.fg,
        "the two cells have the same foreground"
    );
    assert_eq!(still_cell.bg, blinking_cell.bg, "and the same background");
    assert!(
        !still_cell.modifier.contains(Modifier::SLOW_BLINK),
        "and the still one does not blink"
    );
    assert!(
        blinking_cell.modifier.contains(Modifier::SLOW_BLINK),
        "while the other one does: the state is not colour alone"
    );
}

/// The help names the key and loses none of the others.
#[test]
fn the_help_names_the_key_and_loses_none_of_the_others() {
    let mut app = app_of("help-keys", IDLE);
    press(&mut app, &[KeyCode::Char('?')]);
    let text = rows(&app, 80, 24).join("\n");
    assert!(
        text.contains("x    the guest's screen"),
        "the help names the key: {text}"
    );
    for key in emo8067_tui::ui::KEYS {
        assert!(text.contains(key), "the help loses {key:?}: {text}");
    }
}

/// Every bound key is named in the help.
#[test]
fn every_bound_key_is_named_in_the_help() {
    for key in emo8067_tui::app::BOUND_KEYS {
        assert!(
            emo8067_tui::ui::KEYS
                .iter()
                .any(|line| line.split_whitespace().any(|word| word == key)),
            "the handler binds {key} and the help does not name it"
        );
    }
}

/// The published lists are published.
#[test]
fn the_published_lists_are_published() {
    assert_eq!(
        emo8067_tui::ui::KEYS.len(),
        25,
        "the help's list holds twenty-five keys"
    );
    assert_eq!(
        emo8067_tui::app::BOUND_KEYS.len(),
        28,
        "the bound list holds twenty-eight keys"
    );
    assert!(
        emo8067_tui::app::BOUND_KEYS.contains(&"x"),
        "and `x` is one of the bound keys"
    );
}

/// What does not fit is cut, and the stride is the page's own.
#[test]
fn what_does_not_fit_is_cut_and_the_stride_is_the_pages_own() {
    let mut app = app_of("cut-stride", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x0050, b'B');
    app.cpu.mem.write_u8(PAGE, 0x00A0, b'C');
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 40, 12);
    assert_eq!(
        buffer.cell((0, 0)).map(|c| c.symbol()),
        Some("A"),
        "the page starts at the top-left corner"
    );
    assert_eq!(
        buffer.cell((0, 1)).map(|c| c.symbol()),
        Some("C"),
        "the second row is the page's second row and not a wrap of the first"
    );
    assert_ne!(
        buffer.cell((0, 1)).map(|c| c.symbol()),
        Some("B"),
        "and the cells past the terminal's width are not the row below"
    );
}

/// The cells past the eightieth column are the ground.
#[test]
fn the_cells_past_the_eightieth_column_are_the_ground() {
    let mut app = app_of("cut-ground", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x00A0, b'B');
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 100, 24);
    assert_eq!(
        buffer.cell((0, 1)).map(|c| c.symbol()),
        Some("B"),
        "the second row of the page is the second row of the screen"
    );
    assert_ne!(
        buffer.cell((80, 0)).map(|c| c.symbol()),
        Some("B"),
        "and the cells past the eightieth column are not the row below"
    );
    assert_eq!(
        buffer.cell((80, 0)).map(|c| c.symbol()),
        Some(" "),
        "they are the ground"
    );
}

/// The page is drawn from its top-left corner, and its eightieth column is
/// at the eightieth column.
#[test]
fn the_page_is_drawn_from_its_top_left_corner_and_its_eightieth_column_is_at_the_eightieth_column()
{
    let mut app = app_of("cut-corner", IDLE);
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x009E, b'Z');
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);
    assert_eq!(
        buffer.cell((0, 0)).map(|c| c.symbol()),
        Some("A"),
        "the page starts at the top-left corner"
    );
    assert_eq!(
        buffer.cell((79, 0)).map(|c| c.symbol()),
        Some("Z"),
        "and the eightieth column is at the eightieth column"
    );
}

/// The takeover takes every row but the status line, at every size.
#[test]
fn the_takeover_takes_every_row_but_the_status_line_at_every_size() {
    for (width, height) in [(80u16, 24u16), (100, 30), (140, 40), (40, 12)] {
        let mut app = app_of("geometry", IDLE);
        for row in 0..25u16 {
            app.cpu.mem.write_u8(PAGE, row * 160, b'Q');
        }
        press(&mut app, &[KeyCode::Char('x')]);
        let buffer = buffer(&app, width, height);
        let takeover_rows = height - 1;
        for row in 0..takeover_rows.min(25) {
            assert_eq!(
                buffer.cell((0, row)).map(|c| c.symbol()),
                Some("Q"),
                "at {width}x{height} the row {row} is the takeover's"
            );
        }
        for row in 25..takeover_rows {
            assert_eq!(
                buffer.cell((0, row)).map(|c| c.symbol()),
                Some(" "),
                "at {width}x{height} the row {row} is past the page and is the ground"
            );
        }
        let status: String = (0..width)
            .map(|x| buffer.cell((x, height - 1)).map_or("", |c| c.symbol()))
            .collect();
        assert!(
            status.contains("IP 0100"),
            "at {width}x{height} the status bar carries the machine's state: {status}"
        );
    }
}

/// The page ends at its twenty-fifth row.
#[test]
fn the_page_ends_at_its_twenty_fifth_row() {
    let mut app = app_of("rows", IDLE);
    app.cpu.mem.write_u8(PAGE, 24 * 160, b'Q');
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 40);
    assert_eq!(
        buffer.cell((0, 24)).map(|c| c.symbol()),
        Some("Q"),
        "the twenty-fifth row of the page is the twenty-fifth row of the screen"
    );
    for row in 25..39u16 {
        assert_eq!(
            buffer.cell((0, row)).map(|c| c.symbol()),
            Some(" "),
            "the row {row} is past the page and is the ground"
        );
    }
}

/// The takeover does not hide the machine's state.
#[test]
fn the_takeover_does_not_hide_the_machines_state() {
    let mut app = app_of("state", IDLE);
    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);
    let status: String = (0..80)
        .map(|x| buffer.cell((x, 23)).map_or("", |c| c.symbol()))
        .collect();
    assert!(status.contains("IP 0100"), "the IP is there: {status}");
    assert!(
        status.contains("STEP 0"),
        "the step count is there: {status}"
    );
    assert!(
        status.contains("BREAK 0"),
        "the breakpoints are there: {status}"
    );
    assert!(
        status.contains("WATCH 0"),
        "the watchpoints are there: {status}"
    );
}

/// An interrupt without a BIOS vectors to zero.
#[test]
fn an_int_without_a_bios_vectors_to_zero() {
    let Some(path) = example("1_sample.asm") else {
        // The examples are local gold, not part of the repository.
        return;
    };
    let mut app = match App::new(Some(path.to_string_lossy().to_string())) {
        Ok(app) => app,
        Err(error) => panic!("the sample loads: {error}"),
    };
    let vector_offset = app.cpu.mem.read_u16(0x0000, 0x0040);
    let vector_segment = app.cpu.mem.read_u16(0x0000, 0x0042);
    println!("the vector of INT 10h is {vector_segment:04X}:{vector_offset:04X}");
    app.key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
    app.key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
    assert_eq!(
        app.cpu.regs.segment(Seg::Cs),
        0x0000,
        "there is no BIOS: the vector is empty"
    );
    assert_eq!(
        app.cpu.regs.ip, 0x0000,
        "and the machine lands at 0000:0000"
    );
}
