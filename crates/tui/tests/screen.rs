//! The guest's screen: the takeover, and the page under it.
//!
//! The screen is the passive viewer of the text page: the guest's own
//! screen takes the terminal over, and the debugger's keys keep working
//! while it does.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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
