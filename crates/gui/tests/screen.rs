//! The guest's screen: the framebuffer as real pixels, and the text page
//! as the character grid.
//!
//! The screen is drawn from the panel model and from nothing else, so the
//! tests read the model and the shapes the drawing leaves behind — the
//! headless way T3 established.

use emo8067_core::Cpu;
use emo8067_debug::panels::{Panels, Ui};
use emo8067_gui::app::App;
use emo8067_gui::ui::{
    Screen, screen, screen_texture, screen_window, screen_window_id, text_cells, window,
};

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

/// The framebuffer's width, in pixels.
const WIDTH: usize = 320;

/// The framebuffer's height, in pixels.
const HEIGHT: usize = 200;

/// The shapes a headless run drew.
///
/// The run needs no display: egui lays the widgets out and hands back the
/// shapes it would have painted. The run is done twice, because the first
/// frame loads the fonts and the window measures itself, and the frame
/// claims a second of time: a window fades in, and a frame with no time in
/// it would paint nothing at all.
fn headless(mut add: impl FnMut(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
    headless_frames(2, |ui, _| add(ui))
        .pop()
        .expect("the run has a last frame")
}

/// The shapes of every frame of a headless run.
///
/// The frames share one context, so the state that lives between them — the
/// guest screen's window flag — is seen as it is.
fn headless_frames(
    count: usize,
    mut add: impl FnMut(&mut egui::Ui, usize),
) -> Vec<Vec<egui::epaint::ClippedShape>> {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        predicted_dt: 1.0,
        ..egui::RawInput::default()
    };
    let mut frames = Vec::new();
    for frame in 0..count {
        let output = ctx.run_ui(input.clone(), |ui| add(ui, frame));
        frames.push(output.shapes.clone());
        // The textures of the run are nobody's: the test reads the shapes
        // and paints nothing.
        output.drop_without_applying_deltas();
    }
    frames
}

/// The guest's grid, as the biggest galley on the screen.
///
/// The grid is one galley with a row a page row, and the debugger's own
/// panels draw one row each: the grid is the galley with the most rows.
/// What is in it is checked where it matters — the character the guest
/// wrote and the colour it is drawn with.
fn guest_grid(shapes: &[egui::epaint::ClippedShape]) -> Option<std::sync::Arc<egui::text::Galley>> {
    shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some(text.galley.clone()),
            _ => None,
        })
        .max_by_key(|galley| (galley.rows.len(), galley.job.sections.len()))
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
    cpu.mem.write_u8(GRAPHICS, (20 * WIDTH + 10) as u16, 0x04);

    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let Screen::Graphics {
        width,
        height,
        pixels,
    } = screen(view)
    else {
        panic!("the graphical mode shows the framebuffer");
    };

    assert_eq!(
        (width, height),
        (WIDTH, HEIGHT),
        "the framebuffer is 320 by 200"
    );
    assert_eq!(
        pixels[0],
        egui::Color32::from_rgb(0x55, 0xff, 0x55),
        "the first pixel is the DAC's own light green"
    );
    assert_eq!(
        pixels[20 * WIDTH + 10],
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

/// The attribute byte's own bits are the cell's colours.
///
/// The foreground is the low three bits and the background the high four:
/// the attribute `9Ah` holds a foreground of 2 and a background of 9, and
/// the two boundaries are what the rule decides.
#[test]
fn the_attribute_byte_s_own_bits_are_the_cell_s_colours() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    cpu.mem.write_u8(PAGE, 0x0001, 0x9A);

    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let cells = text_cells(view);

    assert_eq!(
        cells[0].foreground,
        egui::Color32::from_rgb(0x00, 0xaa, 0x00),
        "the foreground is the attribute's low three bits"
    );
    assert_eq!(
        cells[0].background,
        egui::Color32::from_rgb(0x55, 0x55, 0xff),
        "and the background is the high four"
    );
}

/// The screen is drawn: the framebuffer as the image it is, and the text
/// page as the guest's own character grid.
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
        egui::vec2(WIDTH as f32, HEIGHT as f32),
        "the framebuffer is drawn one guest pixel to a point"
    );
    assert_eq!(
        image.1,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        "and the whole of it is shown"
    );

    // The text mode: the guest's own page, drawn as the guest's own cells.
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    cpu.mem.write_u8(PAGE, 0x0001, 0x16);
    cpu.mem.write_u16(DATA, CURSORS, (3 << 8) | 7);
    let view = &Panels::of(&cpu, &Ui::default()).screen;
    let shapes = headless(|ui| screen_window(ui.ctx(), &screen(view)));

    let grid = guest_grid(&shapes).expect("the character grid is drawn as the page's own rows");
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
    // The grid is the guest's page and not one of the debugger's panels:
    // the character the guest wrote is in it, with the guest's colour.
    let text = &grid.job.text;
    let character = grid
        .job
        .sections
        .iter()
        .find(|section| text.get(section.byte_range.start.0..section.byte_range.end.0) == Some("A"))
        .expect("the character the guest wrote is drawn");
    assert_eq!(
        character.format.color,
        egui::Color32::from_rgb(0xaa, 0x55, 0x00),
        "and it is drawn with the guest's own foreground"
    );
}

/// The screen's window is open until it is closed, and a closed window
/// draws nothing.
///
/// The flag is the window's own state and it lives in egui's memory, so it
/// is closed here the way the window's close button closes it.
#[test]
fn the_guest_s_screen_window_can_be_closed() {
    let app = App::new(None).expect("the machine is empty");

    let frames = headless_frames(2, |ui, frame| {
        if frame == 1 {
            let id = screen_window_id();
            ui.ctx()
                .memory_mut(|memory| memory.data.insert_temp(id, false));
        }
        window(ui, &app);
    });

    let drawn = |shapes: &[egui::epaint::ClippedShape]| {
        guest_grid(shapes).is_some_and(|galley| galley.rows.len() == ROWS)
    };
    assert!(
        drawn(&frames[0]),
        "the screen is drawn while the window is open"
    );
    assert!(
        !drawn(&frames[1]),
        "and nothing of it is drawn once the window is closed"
    );
}

/// The texture is kept while the picture is, and a changed picture is a new
/// texture.
///
/// The framebuffer is uploaded once and kept: the picture is compared to
/// know whether the texture still is it.
#[test]
fn the_framebuffer_s_texture_is_kept_while_the_picture_is() {
    let ctx = egui::Context::default();
    let picture = vec![egui::Color32::from_rgb(0x11, 0x22, 0x33); 4];

    let first = screen_texture(&ctx, 2, 2, &picture);
    let again = screen_texture(&ctx, 2, 2, &picture);
    assert_eq!(
        first.id(),
        again.id(),
        "the same picture is the same texture"
    );

    let changed = vec![egui::Color32::from_rgb(0x44, 0x55, 0x66); 4];
    let other = screen_texture(&ctx, 2, 2, &changed);
    assert_ne!(first.id(), other.id(), "and a changed picture is a new one");
}
