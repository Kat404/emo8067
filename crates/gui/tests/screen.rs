//! The guest's screen: the framebuffer as real pixels, and the text page
//! as the character grid.
//!
//! The screen is drawn from the panel model and from nothing else, so the
//! tests read the model and the shapes the drawing leaves behind — the
//! headless way T3 established.

use emo8067_core::Cpu;
use emo8067_debug::panels::{Panels, Ui};
use emo8067_gui::app::App;
use emo8067_gui::ui::{Screen, screen, screen_window, window};

/// The segment the text page lives in.
const PAGE: u16 = 0xB800;

/// The segment the graphics page lives in.
const GRAPHICS: u16 = 0xA000;

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;

/// The offset of the machine's video mode.
const MODE: u16 = 0x0049;

/// The offset of the cursors of the pages in the data area.
const CURSORS: u16 = 0x0050;

/// The columns the text page has.
const COLUMNS: usize = 80;

/// The rows the text page has.
const ROWS: usize = 25;

/// The shapes a headless run drew.
///
/// The run needs no display: egui lays the widgets out and hands back the
/// shapes it would have painted. The run is done twice, because the first
/// frame loads the fonts and the window measures itself, and the frame
/// claims a second of time: a window fades in, and a frame with no time in
/// it would paint nothing at all.
fn headless(mut add: impl FnMut(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        predicted_dt: 1.0,
        ..egui::RawInput::default()
    };
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let output = ctx.run_ui(input.clone(), &mut add);
        shapes = output.shapes.clone();
        // The textures of the run are nobody's: the test reads the shapes
        // and paints nothing.
        output.drop_without_applying_deltas();
    }
    shapes
}

/// The graphical mode's framebuffer is the DAC's own colours, pixel by
/// pixel.
///
/// The guest's bytes are colour indices and the DAC names them: the
/// picture is the VGA's own and not the cube Windows 3.x and xterm use.
#[test]
fn the_framebuffer_s_bytes_become_the_dac_s_own_colours() {
    let mut cpu = Cpu::new();
    // The graphical mode: the panel shows the framebuffer then.
    cpu.mem.write_u8(DATA, MODE, 0x13);
    // Two pixels the guest drew: at the origin, and at the row 20 and the
    // column 10.
    cpu.mem.write_u8(GRAPHICS, 0, 0x0A);
    cpu.mem.write_u8(GRAPHICS, 20 * 320 + 10, 0x04);

    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let Screen::Graphics {
        width,
        height,
        pixels,
    } = screen(view)
    else {
        panic!("the graphical mode shows the framebuffer");
    };

    assert_eq!((width, height), (320, 200), "the framebuffer is 320 by 200");
    assert_eq!(
        pixels[0],
        egui::Color32::from_rgb(0x55, 0xff, 0x55),
        "the first pixel is the DAC's own light green"
    );
    assert_eq!(
        pixels[20 * 320 + 10],
        egui::Color32::from_rgb(0xaa, 0x00, 0x00),
        "and the pixel at that offset is the DAC's own red"
    );
}

/// The text page's cells are the rows the panel draws.
///
/// The character is the guest's own, the colours are the DAC's, and the
/// cursor is where the BIOS says it is — the TUI's own mapping, so the two
/// front ends draw the guest's page the same way.
#[test]
fn the_text_page_s_cells_become_the_rows_the_panel_draws() {
    let mut cpu = Cpu::new();
    // The character and the attribute of the first cell: 'A', brown on blue.
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    cpu.mem.write_u8(PAGE, 0x0001, 0x16);
    // The cursor is where the BIOS says it is: the row 3 and the column 7.
    // The word holds the row in its high byte and the column in its low
    // one, the way the machine's memory holds every other word.
    cpu.mem.write_u16(DATA, CURSORS, (3 << 8) | 7);

    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let Screen::Text {
        columns,
        rows,
        cells,
    } = screen(view)
    else {
        panic!("a text mode shows the character grid");
    };

    assert_eq!(
        (columns, rows),
        (COLUMNS, ROWS),
        "the page is eighty columns by twenty-five rows"
    );
    assert_eq!(cells.len(), columns * rows, "and it holds that many cells");
    assert_eq!(cells[0].character, 'A', "the character is the guest's own");
    assert_eq!(
        cells[0].foreground,
        egui::Color32::from_rgb(0xaa, 0x55, 0x00),
        "the foreground is the DAC's own brown"
    );
    assert_eq!(
        cells[0].background,
        egui::Color32::from_rgb(0x00, 0x00, 0xaa),
        "and the background is the DAC's own blue"
    );
    assert!(
        cells[3 * COLUMNS + 7].cursor,
        "the cursor is on the cell the BIOS names"
    );
    assert!(!cells[0].cursor, "and on no other");
}

/// The screen is drawn: the framebuffer as the image it is, and the text
/// page as the character grid it is.
///
/// A drawing that draws nothing must fail here: the shapes are what the
/// window would have painted.
#[test]
fn the_guest_s_screen_is_drawn() {
    // The graphical mode: the framebuffer is one image, and it is the
    // framebuffer's own size — a guest pixel to a point.
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(DATA, MODE, 0x13);
    cpu.mem.write_u8(GRAPHICS, 0, 0x0A);
    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let shapes = headless(|ui| screen_window(ui.ctx(), &screen(view)));

    let image = shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(shape) => shape.brush.as_ref().map(|brush| (shape.rect, brush.uv)),
            _ => None,
        })
        .expect("the framebuffer is drawn as a textured rectangle");
    assert_eq!(
        image.0.size(),
        egui::vec2(320.0, 200.0),
        "the framebuffer is drawn one guest pixel to a point"
    );
    assert_eq!(
        image.1,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        "and the whole of it is shown"
    );

    // The text mode: the debugger's own window draws the page as the
    // character grid, twenty-five rows of the page's own cells.
    let app = App::new(None).expect("the machine is empty");
    let shapes = headless(|ui| window(ui, &app));

    let grid = shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => (text.galley.rows.len() > 1).then_some(&text.galley),
            _ => None,
        })
        .expect("the character grid is drawn as the page's own rows");
    assert_eq!(
        grid.rows.len(),
        ROWS,
        "the grid is the page's twenty-five rows"
    );
    assert!(
        grid.job
            .sections
            .iter()
            .any(|section| section.format.underline != egui::Stroke::NONE),
        "and the cursor's cell is underlined"
    );
}
