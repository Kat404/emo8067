//! The verifier's own probes for the TUI's half of the keyboard unit:
//! S18 (the host injects the keys) and S21 (the help moves with it).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_tui::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The counter that tells the directories of one run apart.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;

/// A directory of this run's own, and the guard that takes it away again.
struct Directory {
    path: PathBuf,
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A directory of this run's own.
fn directory(name: &str) -> Directory {
    let path = std::env::temp_dir().join(format!(
        "emo8067-tui-verify-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&path).expect("the directory");
    Directory { path }
}

/// The app, with that program of the test's own words open.
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

/// The ASCII character and the scan code of the keystroke at the head of
/// the guest's keyboard buffer.
///
/// The head holds the absolute offset of the entry to read.
fn at_head(app: &App) -> (u8, u8) {
    let head = app.cpu.mem.read_u16(DATA, 0x001A);
    (
        app.cpu.mem.read_u8(DATA, head),
        app.cpu.mem.read_u8(DATA, head + 1),
    )
}

/// The ASCII character and the scan code of the keystroke at the tail of
/// the guest's keyboard buffer: the one that went in last.
///
/// The tail holds the absolute offset of the entry to write, so the entry
/// that went in last is the one before it.
fn at_tail(app: &App) -> (u8, u8) {
    let tail = app.cpu.mem.read_u16(DATA, 0x001C);
    (
        app.cpu.mem.read_u8(DATA, tail - 2),
        app.cpu.mem.read_u8(DATA, tail - 1),
    )
}

/// S18 - a key the TUI does not bind is the guest's, and it carries the
/// IBM's set 1 scan code.
#[test]
fn s18_an_unbound_key_is_the_guests() {
    let mut app = app_of("unbound", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('a')]);
    assert_eq!(app.cpu.keys_waiting(), 1, "the key reached the guest");
    assert_eq!(
        at_head(&app),
        (b'a', 0x1E),
        "and it carries the set 1 scan code of 'a'"
    );
}

/// S18 - a key the TUI binds to a debugger command is the debugger's, and
/// it does not reach the guest.
#[test]
fn s18_a_bound_key_is_the_debuggers() {
    let mut app = app_of("bound", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(app.help, "the help is up");
    assert_eq!(
        app.cpu.keys_waiting(),
        0,
        "and the key did not reach the guest"
    );
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(!app.help, "and the help closes again");
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the guest's screen took the terminal over");
    assert_eq!(
        app.cpu.keys_waiting(),
        0,
        "and the bound key did not reach the guest either"
    );
}

/// S18 - in the takeover the keys go to the guest: the arrows are the
/// guest's most wanted key, and the corpus reads 48h, 4Bh, 4Dh and 50h.
#[test]
fn s18_in_the_takeover_the_keys_go_to_the_guest() {
    let mut app = app_of("takeover", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover is open");

    press(&mut app, &[KeyCode::Up]);
    assert_eq!(app.cpu.keys_waiting(), 1, "the arrow reached the guest");
    assert_eq!(
        at_head(&app),
        (0, 0x48),
        "and it carries the set 1 scan code of the up arrow"
    );
    press(&mut app, &[KeyCode::Left]);
    assert_eq!(at_tail(&app), (0, 0x4B), "left is 4Bh");
    press(&mut app, &[KeyCode::Right]);
    assert_eq!(at_tail(&app), (0, 0x4D), "right is 4Dh");
    press(&mut app, &[KeyCode::Down]);
    assert_eq!(at_tail(&app), (0, 0x50), "down is 50h");

    // And a key the TUI does not bind reaches the guest once, and not
    // twice.
    press(&mut app, &[KeyCode::Char('z')]);
    assert_eq!(
        app.cpu.keys_waiting(),
        5,
        "the key reached the guest's keyboard once"
    );
}

/// S18 - the quit keys are the TUI's own, in every mode: they never reach
/// the guest.
#[test]
fn s18_the_quit_keys_never_reach_the_guest() {
    let mut app = app_of("quit", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('q')]);
    assert!(app.quit, "the user asked to quit");
    assert_eq!(
        app.cpu.keys_waiting(),
        0,
        "and the quit key did not reach the guest"
    );

    let mut app = app_of("quit-takeover", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover is open");
    app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit, "the user asked to quit with Ctrl-C");
    assert_eq!(
        app.cpu.keys_waiting(),
        0,
        "and the quit key did not reach the guest either"
    );
}

/// S18 - the takeover keeps the debugger's own keys: the tree's own
/// behaviour, which this unit does not change.
#[test]
fn s18_the_debugger_keys_keep_working_in_the_takeover() {
    let mut app = app_of("keep-working", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.screen_open, "the takeover is open");

    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.steps, 1, "F8 still steps the machine");
    press(&mut app, &[KeyCode::Char('?')]);
    assert!(app.help, "and ? is still the help");
    press(&mut app, &[KeyCode::Char('?')]);

    press(&mut app, &[KeyCode::Char('x')]);
    assert!(!app.screen_open, "and x closes the takeover again");
}

/// S21 - the help says which keys reach the guest.
#[test]
fn s21_the_help_says_the_keys_go_to_the_guest() {
    assert!(
        emo8067_tui::ui::KEYS
            .iter()
            .any(|line| line.contains("the guest's screen") && line.contains("keys")),
        "the help says the keys go to the guest: {:?}",
        emo8067_tui::ui::KEYS
    );
}

/// S18 - a key the set has no code for is not a keystroke: it reaches the
/// guest nowhere, and the guest's buffer stays as it was.
#[test]
fn s18_a_key_the_set_has_no_code_for_is_not_a_keystroke() {
    let mut app = app_of("no-code", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::CapsLock]);
    assert_eq!(
        app.cpu.keys_waiting(),
        0,
        "the guest's buffer is empty: an unnamed key is not a keystroke"
    );
}
