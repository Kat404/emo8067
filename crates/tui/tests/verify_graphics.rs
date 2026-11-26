//! The verifier's own probes for the guest's graphics page.
//!
//! The palette oracle is the published one: the *Encyclopedia of Graphics
//! File Formats*'s "IBM VGA palette", chapter 2
//! (`https://www.fileformat.info/mirror/egff/ch02_02.htm`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_tui::app::App;
use emo8067_tui::ui::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

/// The counter that tells the directories of one run apart.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory of this run's own, and the guard that takes it away again.
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
        "emo8067-verify-graphics-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&path).expect("the directory");
    Directory { path }
}

fn app_of(name: &str, source: &str) -> App {
    let dir = directory(name);
    let path = dir.path.join("program.asm");
    std::fs::write(&path, source).expect("the program");
    App::new(Some(path.to_string_lossy().to_string())).expect("the program loads")
}

fn press(app: &mut App, keys: &[KeyCode]) {
    for key in keys {
        app.key(KeyEvent::new(*key, KeyModifiers::NONE));
    }
}

fn buffer(app: &App, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    terminal.backend().buffer().clone()
}

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;
/// The offset of the machine's current video mode.
const MODE: u16 = 0x0049;
/// The segment the text page lives in.
const PAGE: u16 = 0xB800;
/// The segment the graphics page lives in.
const GRAPHICS: u16 = 0xA000;
/// The pixels a row of the graphics page has.
const WIDTH: u16 = 320;
/// The rows the graphics page has.
const HEIGHT: u16 = 200;

/// The palette the *Encyclopedia of Graphics File Formats* publishes as
/// the "IBM VGA palette": the sixteen EGA text colours, then sixteen
/// evenly spaced greys, then the 216 hue-saturation-intensity colours,
/// then the eight unused blacks.
const PUBLISHED: [(u8, u8, u8); 256] = [
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0xaa),
    (0x00, 0xaa, 0x00),
    (0x00, 0xaa, 0xaa),
    (0xaa, 0x00, 0x00),
    (0xaa, 0x00, 0xaa),
    (0xaa, 0x55, 0x00),
    (0xaa, 0xaa, 0xaa),
    (0x55, 0x55, 0x55),
    (0x55, 0x55, 0xff),
    (0x55, 0xff, 0x55),
    (0x55, 0xff, 0xff),
    (0xff, 0x55, 0x55),
    (0xff, 0x55, 0xff),
    (0xff, 0xff, 0x55),
    (0xff, 0xff, 0xff),
    (0x00, 0x00, 0x00),
    (0x14, 0x14, 0x14),
    (0x20, 0x20, 0x20),
    (0x2c, 0x2c, 0x2c),
    (0x38, 0x38, 0x38),
    (0x45, 0x45, 0x45),
    (0x51, 0x51, 0x51),
    (0x61, 0x61, 0x61),
    (0x71, 0x71, 0x71),
    (0x82, 0x82, 0x82),
    (0x92, 0x92, 0x92),
    (0xa2, 0xa2, 0xa2),
    (0xb6, 0xb6, 0xb6),
    (0xcb, 0xcb, 0xcb),
    (0xe3, 0xe3, 0xe3),
    (0xff, 0xff, 0xff),
    (0x00, 0x00, 0xff),
    (0x41, 0x00, 0xff),
    (0x7d, 0x00, 0xff),
    (0xbe, 0x00, 0xff),
    (0xff, 0x00, 0xff),
    (0xff, 0x00, 0xbe),
    (0xff, 0x00, 0x7d),
    (0xff, 0x00, 0x41),
    (0xff, 0x00, 0x00),
    (0xff, 0x41, 0x00),
    (0xff, 0x7d, 0x00),
    (0xff, 0xbe, 0x00),
    (0xff, 0xff, 0x00),
    (0xbe, 0xff, 0x00),
    (0x7d, 0xff, 0x00),
    (0x41, 0xff, 0x00),
    (0x00, 0xff, 0x00),
    (0x00, 0xff, 0x41),
    (0x00, 0xff, 0x7d),
    (0x00, 0xff, 0xbe),
    (0x00, 0xff, 0xff),
    (0x00, 0xbe, 0xff),
    (0x00, 0x7d, 0xff),
    (0x00, 0x41, 0xff),
    (0x7d, 0x7d, 0xff),
    (0x9e, 0x7d, 0xff),
    (0xbe, 0x7d, 0xff),
    (0xdf, 0x7d, 0xff),
    (0xff, 0x7d, 0xff),
    (0xff, 0x7d, 0xdf),
    (0xff, 0x7d, 0xbe),
    (0xff, 0x7d, 0x9e),
    (0xff, 0x7d, 0x7d),
    (0xff, 0x9e, 0x7d),
    (0xff, 0xbe, 0x7d),
    (0xff, 0xdf, 0x7d),
    (0xff, 0xff, 0x7d),
    (0xdf, 0xff, 0x7d),
    (0xbe, 0xff, 0x7d),
    (0x9e, 0xff, 0x7d),
    (0x7d, 0xff, 0x7d),
    (0x7d, 0xff, 0x9e),
    (0x7d, 0xff, 0xbe),
    (0x7d, 0xff, 0xdf),
    (0x7d, 0xff, 0xff),
    (0x7d, 0xdf, 0xff),
    (0x7d, 0xbe, 0xff),
    (0x7d, 0x9e, 0xff),
    (0xb6, 0xb6, 0xff),
    (0xc7, 0xb6, 0xff),
    (0xdb, 0xb6, 0xff),
    (0xeb, 0xb6, 0xff),
    (0xff, 0xb6, 0xff),
    (0xff, 0xb6, 0xeb),
    (0xff, 0xb6, 0xdb),
    (0xff, 0xb6, 0xc7),
    (0xff, 0xb6, 0xb6),
    (0xff, 0xc7, 0xb6),
    (0xff, 0xdb, 0xb6),
    (0xff, 0xeb, 0xb6),
    (0xff, 0xff, 0xb6),
    (0xeb, 0xff, 0xb6),
    (0xdb, 0xff, 0xb6),
    (0xc7, 0xff, 0xb6),
    (0xb6, 0xdf, 0xb6),
    (0xb6, 0xff, 0xc7),
    (0xb6, 0xff, 0xdb),
    (0xb6, 0xff, 0xeb),
    (0xb6, 0xff, 0xff),
    (0xb6, 0xeb, 0xff),
    (0xb6, 0xdb, 0xff),
    (0xb6, 0xc7, 0xff),
    (0x00, 0x00, 0x71),
    (0x1c, 0x00, 0x71),
    (0x38, 0x00, 0x71),
    (0x55, 0x00, 0x71),
    (0x71, 0x00, 0x71),
    (0x71, 0x00, 0x55),
    (0x71, 0x00, 0x38),
    (0x71, 0x00, 0x1c),
    (0x71, 0x00, 0x00),
    (0x71, 0x1c, 0x00),
    (0x71, 0x38, 0x00),
    (0x71, 0x55, 0x00),
    (0x71, 0x71, 0x00),
    (0x55, 0x71, 0x00),
    (0x38, 0x71, 0x00),
    (0x1c, 0x71, 0x00),
    (0x00, 0x71, 0x00),
    (0x00, 0x71, 0x1c),
    (0x00, 0x71, 0x38),
    (0x00, 0x71, 0x55),
    (0x00, 0x71, 0x71),
    (0x00, 0x55, 0x71),
    (0x00, 0x38, 0x71),
    (0x00, 0x1c, 0x71),
    (0x38, 0x38, 0x71),
    (0x45, 0x38, 0x71),
    (0x55, 0x38, 0x71),
    (0x61, 0x38, 0x71),
    (0x71, 0x38, 0x71),
    (0x71, 0x38, 0x61),
    (0x71, 0x38, 0x55),
    (0x71, 0x38, 0x45),
    (0x71, 0x38, 0x38),
    (0x71, 0x45, 0x38),
    (0x71, 0x55, 0x38),
    (0x71, 0x61, 0x38),
    (0x71, 0x71, 0x38),
    (0x61, 0x71, 0x38),
    (0x55, 0x71, 0x38),
    (0x45, 0x71, 0x38),
    (0x38, 0x71, 0x38),
    (0x38, 0x71, 0x45),
    (0x38, 0x71, 0x55),
    (0x38, 0x71, 0x61),
    (0x38, 0x71, 0x71),
    (0x38, 0x61, 0x71),
    (0x38, 0x55, 0x71),
    (0x38, 0x45, 0x71),
    (0x51, 0x51, 0x71),
    (0x59, 0x51, 0x71),
    (0x61, 0x51, 0x71),
    (0x69, 0x51, 0x71),
    (0x71, 0x51, 0x71),
    (0x71, 0x51, 0x69),
    (0x71, 0x51, 0x61),
    (0x71, 0x51, 0x59),
    (0x71, 0x51, 0x51),
    (0x71, 0x59, 0x51),
    (0x71, 0x61, 0x51),
    (0x71, 0x69, 0x51),
    (0x71, 0x71, 0x51),
    (0x69, 0x71, 0x51),
    (0x61, 0x71, 0x51),
    (0x59, 0x71, 0x51),
    (0x51, 0x71, 0x51),
    (0x51, 0x71, 0x59),
    (0x51, 0x71, 0x61),
    (0x51, 0x71, 0x69),
    (0x51, 0x71, 0x71),
    (0x51, 0x69, 0x71),
    (0x51, 0x61, 0x71),
    (0x51, 0x59, 0x71),
    (0x00, 0x00, 0x41),
    (0x10, 0x00, 0x41),
    (0x20, 0x00, 0x41),
    (0x30, 0x00, 0x41),
    (0x41, 0x00, 0x41),
    (0x41, 0x00, 0x30),
    (0x41, 0x00, 0x20),
    (0x41, 0x00, 0x10),
    (0x41, 0x00, 0x00),
    (0x41, 0x10, 0x00),
    (0x41, 0x20, 0x00),
    (0x41, 0x30, 0x00),
    (0x41, 0x41, 0x00),
    (0x30, 0x41, 0x00),
    (0x20, 0x41, 0x00),
    (0x10, 0x41, 0x00),
    (0x00, 0x41, 0x00),
    (0x00, 0x41, 0x10),
    (0x00, 0x41, 0x20),
    (0x00, 0x41, 0x30),
    (0x00, 0x41, 0x41),
    (0x00, 0x30, 0x41),
    (0x00, 0x20, 0x41),
    (0x00, 0x10, 0x41),
    (0x20, 0x20, 0x41),
    (0x28, 0x20, 0x41),
    (0x30, 0x20, 0x41),
    (0x38, 0x20, 0x41),
    (0x41, 0x20, 0x41),
    (0x41, 0x20, 0x38),
    (0x41, 0x20, 0x30),
    (0x41, 0x20, 0x28),
    (0x41, 0x20, 0x20),
    (0x41, 0x28, 0x20),
    (0x41, 0x30, 0x20),
    (0x41, 0x38, 0x20),
    (0x41, 0x41, 0x20),
    (0x38, 0x41, 0x20),
    (0x30, 0x41, 0x20),
    (0x28, 0x41, 0x20),
    (0x20, 0x41, 0x20),
    (0x20, 0x41, 0x28),
    (0x20, 0x41, 0x30),
    (0x20, 0x41, 0x38),
    (0x20, 0x41, 0x41),
    (0x20, 0x38, 0x41),
    (0x20, 0x30, 0x41),
    (0x20, 0x28, 0x41),
    (0x2c, 0x2c, 0x41),
    (0x30, 0x2c, 0x41),
    (0x34, 0x2c, 0x41),
    (0x3c, 0x2c, 0x41),
    (0x41, 0x2c, 0x41),
    (0x41, 0x2c, 0x3c),
    (0x41, 0x2c, 0x34),
    (0x41, 0x2c, 0x30),
    (0x41, 0x2c, 0x2c),
    (0x41, 0x30, 0x2c),
    (0x41, 0x34, 0x2c),
    (0x41, 0x3c, 0x2c),
    (0x41, 0x41, 0x2c),
    (0x3c, 0x41, 0x2c),
    (0x34, 0x41, 0x2c),
    (0x30, 0x41, 0x2c),
    (0x2c, 0x41, 0x2c),
    (0x2c, 0x41, 0x30),
    (0x2c, 0x41, 0x34),
    (0x2c, 0x41, 0x3c),
    (0x2c, 0x41, 0x41),
    (0x2c, 0x3c, 0x41),
    (0x2c, 0x34, 0x41),
    (0x2c, 0x30, 0x41),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x00),
];

/// S28 — the palette is the published one, every entry of it.
#[test]
fn s28_the_palette_is_the_published_table_every_entry() {
    for (index, (red, green, blue)) in PUBLISHED.iter().enumerate() {
        let Color::Rgb(r, g, b) = theme::VGA_DAC[index] else {
            panic!("the DAC's {index} is not an RGB colour");
        };
        assert_eq!(
            (r, g, b),
            (*red, *green, *blue),
            "the DAC's {index} is the published one"
        );
    }
}

/// S28 — the first sixteen are the EGA text colours, and the project's
/// own sixteen are that table's first sixteen.
#[test]
fn s28_the_first_sixteen_are_the_ega_text_colours() {
    for index in 0..16 {
        let Color::Rgb(r, g, b) = theme::VGA_DAC[index] else {
            panic!("the DAC's {index} is not an RGB colour");
        };
        for channel in [r, g, b] {
            assert!(
                matches!(channel, 0x00 | 0x55 | 0xaa | 0xff),
                "the EGA colours step 00, 55, aa and ff: {index} is {channel:#04x}"
            );
        }
    }
    for index in 0..16 {
        assert_eq!(
            theme::VGA_DAC[index],
            theme::VGA[index],
            "the DAC's {index} is the project's own {index}"
        );
    }
}

/// S28 — the Windows 3.x cube and the step-10 greys are not the VGA's.
///
/// The cube's steps are `00`, `33`, `66`, `99` and `cc`; the VGA's own
/// steps are `00`, `41`, `7d`, `be` and `ff`.
#[test]
fn s28_the_windows_cube_and_the_step_ten_greys_are_nowhere_in_it() {
    // None of the cube's own steps is a channel of any colour in the DAC.
    for cube_step in [0x33u8, 0x66, 0x99, 0xcc] {
        let found = (0..256).any(|index| {
            let Color::Rgb(r, g, b) = theme::VGA_DAC[index] else {
                return false;
            };
            r == cube_step || g == cube_step || b == cube_step
        });
        assert!(
            !found,
            "the cube's step {cube_step:#04x} is nowhere in the DAC"
        );
    }
    // And the cube's very first entry after black, (00, 00, 33), is not
    // in it either.
    let found = (0..256).any(|index| {
        let Color::Rgb(r, g, b) = theme::VGA_DAC[index] else {
            return false;
        };
        (r, g, b) == (0x00, 0x00, 0x33)
    });
    assert!(!found, "the cube's own (00, 00, 33) is nowhere in the DAC");

    // The sixteen greys are the DAC's own ladder, and not the step 10.
    let ladder = [
        0x00u8, 0x14, 0x20, 0x2c, 0x38, 0x45, 0x51, 0x61, 0x71, 0x82, 0x92, 0xa2, 0xb6, 0xcb, 0xe3,
        0xff,
    ];
    for (index, grey) in ladder.iter().enumerate() {
        let Color::Rgb(r, g, b) = theme::VGA_DAC[16 + index] else {
            panic!("the DAC's {} is not an RGB colour", 16 + index);
        };
        assert_eq!(
            (r, g, b),
            (*grey, *grey, *grey),
            "the grey {} is the DAC's own",
            16 + index
        );
        if index > 0 {
            assert_ne!(
                (r, g, b),
                (10 * index as u8, 10 * index as u8, 10 * index as u8),
                "and it is not the step 10"
            );
        }
    }

    // The eight unused blacks are at the end.
    for index in 248..256 {
        let Color::Rgb(r, g, b) = theme::VGA_DAC[index] else {
            panic!("the DAC's {index} is not an RGB colour");
        };
        assert_eq!((r, g, b), (0, 0, 0), "the last eight entries are black");
    }
}

/// S28 — the panel reads the framebuffer out of the machine's own
/// memory, and the machine's own cell grid.
#[test]
fn s28_the_panel_reads_the_framebuffer_of_the_machines_own_memory() {
    let mut app = app_of(
        "graphics-panel",
        "org 100h\nmov ah, 0\nmov al, 13h\nint 10h\nhlt\n",
    );
    // The machine sets the mode through the BIOS: the mode byte and the
    // geometry are the machine's own.
    for _ in 0..4 {
        app.step();
    }
    assert_eq!(
        app.cpu.mem.read_u8(DATA, MODE),
        0x13,
        "the machine is in the graphical mode"
    );
    app.cpu.mem.write_u8(GRAPHICS, 20 * WIDTH + 10, 0x2A);
    app.cpu.mem.write_u8(GRAPHICS, 0, 0x04);

    let ui = emo8067_debug::panels::Ui::default();
    let panels = emo8067_debug::panels::Panels::of(&app.cpu, &ui);
    let screen = &panels.screen;
    let graphics = screen
        .graphics
        .as_ref()
        .expect("the panel shows the graphics page");

    assert_eq!(graphics.width, 320, "the framebuffer is 320 pixels wide");
    assert_eq!(graphics.height, 200, "and two hundred rows tall");
    assert_eq!(
        graphics.pixels[20 * usize::from(WIDTH) + 10],
        0x2A,
        "the pixel is at the offset row * 320 + column"
    );
    assert_eq!(
        graphics.pixels[0], 0x04,
        "and the first pixel is the first byte"
    );
    assert_eq!(screen.columns, 40, "the cell grid is forty columns");
    assert_eq!(screen.rows, 25, "and twenty-five rows");
    assert_eq!(screen.cursor, None, "the graphics page has no text cursor");
}

/// S28 — the graphics page is drawn scaled: the block's size is the
/// framebuffer's size over the area's size, and the cell's colour is the
/// average of the block's colours.
#[test]
fn s28_the_graphics_page_is_drawn_scaled_with_the_average_colour() {
    let mut app = app_of("graphics-scaled", "org 100h\nhlt\n");
    app.cpu.mem.write_u8(DATA, MODE, 0x13);

    // The whole framebuffer is black but its first pixel, which is red
    // (the DAC's index 4 is (aa, 00, 00)).
    for index in 0..(WIDTH as usize * HEIGHT as usize) {
        app.cpu.mem.write_u8(GRAPHICS, index as u16, 0x00);
    }
    app.cpu.mem.write_u8(GRAPHICS, 0, 0x04);

    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);

    // The takeover takes every row but the status line: the area is
    // eighty columns by twenty-three rows, and the block is the
    // framebuffer's size over the area's size: 320 / 80 = 4 wide and
    // 200 / 23 = 8 tall, so thirty-two pixels a cell.
    let cell = buffer.cell((0, 0)).expect("the cell is drawn");
    assert_eq!(
        cell.bg,
        Color::Rgb(170 / 32, 0, 0),
        "the cell is the block of thirty-two pixels, and its colour is their average"
    );
    let next = buffer.cell((1, 0)).expect("the cell is drawn");
    assert_eq!(next.bg, Color::Rgb(0, 0, 0), "and the next block is black");
    let below = buffer.cell((0, 1)).expect("the cell is drawn");
    assert_eq!(below.bg, Color::Rgb(0, 0, 0), "and the block below it too");
    assert_eq!(cell.symbol(), " ", "and the cell is blank");
    assert_eq!(cell.fg, cell.bg, "and the foreground is the colour too");
}

/// S28 — the whole framebuffer's colour reaches the cells.
#[test]
fn s28_the_whole_framebuffer_reaches_the_cells() {
    let mut app = app_of("graphics-uniform", "org 100h\nhlt\n");
    app.cpu.mem.write_u8(DATA, MODE, 0x13);
    // The DAC's index 2 is (00, aa, 00): green.
    for index in 0..(WIDTH as usize * HEIGHT as usize) {
        app.cpu.mem.write_u8(GRAPHICS, index as u16, 0x02);
    }

    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);

    for y in 0..23 {
        for x in 0..80 {
            let cell = buffer.cell((x, y)).expect("the cell is drawn");
            assert_eq!(
                cell.bg,
                Color::Rgb(0, 170, 0),
                "the cell at ({x}, {y}) is the colour of the framebuffer"
            );
        }
    }
}

/// The text page is still drawn cell by cell.
#[test]
fn the_text_page_is_still_drawn_cell_by_cell() {
    let mut app = app_of("text-page", "org 100h\nhlt\n");
    app.cpu.mem.write_u8(PAGE, 0x0000, b'A');
    app.cpu.mem.write_u8(PAGE, 0x0001, 0x14);

    press(&mut app, &[KeyCode::Char('x')]);
    let buffer = buffer(&app, 80, 24);
    let cell = buffer.cell((0, 0)).expect("the cell is drawn");

    assert_eq!(cell.symbol(), "A", "the character is drawn");
    assert_eq!(
        cell.bg,
        theme::VGA[1],
        "and the background is the attribute's own blue"
    );
}
