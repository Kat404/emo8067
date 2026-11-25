//! Independent verification probes for the TUI's half of S8.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory of this run's own.
struct Directory {
    path: PathBuf,
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn directory(name: &str) -> Directory {
    let path = std::env::temp_dir().join(format!(
        "emo8067-verify-tui-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&path).expect("the directory");
    Directory { path }
}

/// The app, with a program of the probe's own words.
fn app_of(name: &str, source: &str) -> App {
    let dir = directory(name);
    let path = dir.path.join("program.asm");
    std::fs::write(&path, source).expect("the program");
    App::new(Some(path.to_string_lossy().to_string())).expect("the program loads")
}

/// The buffer the app draws.
fn buffer(app: &App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("the terminal");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the frame draws");
    terminal.backend().buffer().clone()
}

/// S8 -- the TUI follows the machine's active video page: the panel shows
/// the page the guest is drawing on.
#[test]
fn s8_the_tui_follows_the_machines_active_page() {
    let mut app = app_of(
        "page",
        "org 100h\nmov al, 2\nmov ah, 5\nint 10h\nmov ah, 9\nmov al, 'Z'\n\
         mov bh, 2\nmov cx, 1\nint 10h\nhlt\n",
    );
    // Ten steps: an `INT 10h` is the chip's `INT`, so it pushes the frame
    // and vectors, and the service is entered on the step after that.
    for _ in 0..10 {
        app.step();
    }
    assert_eq!(
        app.panels().screen.page,
        2,
        "the panel shows the page the machine has active"
    );
    assert_eq!(
        app.panels().screen.cells[0].character,
        'Z',
        "and the character the guest wrote on that page is on it"
    );

    // The machine moves to another page, and the panel follows it.
    let mut app = app_of(
        "page2",
        "org 100h\nmov al, 5\nmov ah, 5\nint 10h\nmov ah, 9\nmov al, 'Q'\n\
         mov bh, 5\nmov cx, 1\nint 10h\nhlt\n",
    );
    for _ in 0..10 {
        app.step();
    }
    assert_eq!(
        app.panels().screen.page,
        5,
        "and the panel follows the machine to the page it moved to"
    );
}

/// S8 -- the cursor is drawn where the BIOS says it is, and nowhere else.
#[test]
fn s8_the_cursor_is_drawn_where_the_bios_says_it_is() {
    let mut app = app_of(
        "cursor",
        "org 100h\nmov ah, 2\nmov dh, 3\nmov dl, 7\nint 10h\nhlt\n",
    );
    for _ in 0..5 {
        app.step();
    }
    // The takeover holds the screen.
    app.key(ratatui::crossterm::event::KeyEvent::new(
        ratatui::crossterm::event::KeyCode::Char('x'),
        ratatui::crossterm::event::KeyModifiers::NONE,
    ));

    let buffer = buffer(&app, 80, 24);
    let cursor = buffer.cell((7, 3)).expect("the cell is on the screen");
    assert!(
        cursor
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "the cursor is underlined where the BIOS says it is"
    );
    let other = buffer.cell((6, 3)).expect("the cell is on the screen");
    assert!(
        !other
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "and nowhere else"
    );
}
