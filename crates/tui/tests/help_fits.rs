//! The help fits the screen it is drawn on.
//!
//! The keys are the screen's own list, and the overlay takes the room
//! the layout it draws needs: two columns when the width holds them and
//! one column when it does not.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The counter that tells the directories of one run apart.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory of this run's own.
fn directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "emo8067-help-fits-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).expect("the directory");
    path
}

/// The app, with that program of the test's own words open.
fn app_of(name: &str, source: &str) -> App {
    let dir = directory(name);
    let path = dir.join("program.asm");
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
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    let buffer = terminal.backend().buffer().clone();
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

/// At the width the design speaks of, every key is on the screen.
///
/// The one-column layout needs more rows than the two-column one, and a
/// short screen holds neither every key nor every row of them: the
/// promise holds at the width the design promises it at.
#[test]
fn every_key_is_on_the_screen_at_eighty_columns_and_above() {
    for height in [24u16, 40] {
        for width in [78u16, 79, 80, 100, 140] {
            let mut app = app_of("help-fit", "org 100h\nhlt\n");
            press(&mut app, &[KeyCode::Char('?')]);
            let text = rows(&app, width, height).join("\n");
            for key in emo8067_tui::ui::KEYS {
                assert!(
                    text.contains(key),
                    "at {width}x{height} the help loses {key:?}"
                );
            }
        }
    }
}

/// At a narrow width the keys that are shown are whole.
///
/// A key that is cut is a key that is lost, and the height of the screen
/// — and not the layout — decides how many keys are on it.
#[test]
fn a_narrow_width_shows_whole_keys() {
    for height in [24u16, 40] {
        for width in [40u16, 50, 60, 70, 77] {
            let mut app = app_of("help-narrow", "org 100h\nhlt\n");
            press(&mut app, &[KeyCode::Char('?')]);
            for row in rows(&app, width, height) {
                for key in emo8067_tui::ui::KEYS {
                    // A key that is cut in half is drawn as its own
                    // beginning: the row is the prefix of the key and
                    // nothing else.
                    let cut = key.chars().count() > 3
                        && (1..key.chars().count())
                            .any(|n| row == key.chars().take(n).collect::<String>());
                    assert!(
                        !cut,
                        "at {width}x{height} a key is cut in half: {key:?} in {row:?}"
                    );
                }
            }
        }
    }
}

/// The narrow screen shows as many keys as it can hold.
#[test]
fn a_narrow_screen_shows_as_many_keys_as_it_can_hold() {
    let mut app = app_of("help-short", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('?')]);
    let text = rows(&app, 40, 24).join("\n");
    let visible = emo8067_tui::ui::KEYS
        .iter()
        .filter(|key| text.contains(**key))
        .count();
    assert!(
        visible >= 22,
        "the screen holds twenty-two of the keys: {visible} of {}",
        emo8067_tui::ui::KEYS.len()
    );
}

/// The overlay does not cover the title.
///
/// The program's name is on the screen once and only there.
#[test]
fn the_overlay_does_not_cover_the_title() {
    let mut app = app_of("help-title", "org 100h\nhlt\n");
    press(&mut app, &[KeyCode::Char('?')]);
    let rows = rows(&app, 40, 24);
    assert!(
        rows[0].contains("program.asm"),
        "the title is on the screen: {:?}",
        rows[0]
    );
}
