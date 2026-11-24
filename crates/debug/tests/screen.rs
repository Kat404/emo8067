//! The guest's screen: the text page the machine wrote to.
//!
//! The panel is a pure function of the machine and of the UI's state, and
//! the page is the machine's own memory: what the guest wrote to it is
//! what it shows.

use emo8067_core::Cpu;
use emo8067_debug::panels::{Panels, Ui};

/// The segment the text page lives in.
const PAGE: u16 = 0xB800;

/// The cells the page holds.
const CELLS: usize = 80 * 25;

#[test]
fn the_screen_panel_is_read_out_of_the_machines_own_memory() {
    let mut cpu = Cpu::new();
    // The reference's own program writes "Hello, World!" at B800:0002.
    cpu.mem.write_u8(PAGE, 0x0002, b'H');
    cpu.mem.write_u8(PAGE, 0x0003, 0x1E);

    let panels = Panels::of(&cpu, &Ui::default());
    let screen = &panels.screen;

    assert_eq!(
        screen.cells[1].character, 'H',
        "the character the guest wrote is on the screen"
    );
    assert_eq!(
        screen.cells[1].attribute, 0x1E,
        "and so is the attribute byte beside it"
    );
    assert_eq!(
        screen.cells[0].character, ' ',
        "the cell before it is empty"
    );
}

#[test]
fn the_page_is_read_through_the_segment_the_ui_says() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'H');
    cpu.mem.write_u8(0x1000, 0x0000, b'X');
    let ui = Ui {
        screen_segment: 0x1000,
        ..Ui::default()
    };

    let screen = &Panels::of(&cpu, &ui).screen;

    assert_eq!(
        screen.cells[0].character, 'X',
        "the page is read through the segment the UI says"
    );
    assert_eq!(
        screen.segment, 0x1000,
        "and the panel says which one it read"
    );
}

#[test]
fn the_page_is_eighty_columns_by_twenty_five_rows() {
    let screen = &Panels::of(&Cpu::new(), &Ui::default()).screen;

    assert_eq!(screen.columns, 80, "the page is eighty columns wide");
    assert_eq!(screen.rows, 25, "and twenty-five rows tall");
    assert_eq!(
        screen.cells.len(),
        CELLS,
        "two bytes a cell, the character and the attribute"
    );
    assert_eq!(screen.segment, PAGE, "the page is read from B800:0000");
    assert_eq!(screen.page, 0, "the first page of the text memory");
}

#[test]
fn a_character_that_is_not_printable_is_a_space() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, 0x07);
    cpu.mem.write_u8(PAGE, 0x0002, b'A');

    let screen = &Panels::of(&cpu, &Ui::default()).screen;

    assert_eq!(
        screen.cells[0].character, ' ',
        "a control glyph is a space and not a glyph"
    );
    assert_eq!(
        screen.cells[1].character, 'A',
        "and a printable character is itself"
    );
}

#[test]
fn the_page_the_ui_says_is_the_page_that_is_read() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'0');
    cpu.mem.write_u8(PAGE, 0x1000, b'1');

    let first = &Panels::of(&cpu, &Ui::default()).screen;
    let second = &Panels::of(
        &cpu,
        &Ui {
            screen_page: 1,
            ..Ui::default()
        },
    )
    .screen;

    assert_eq!(first.cells[0].character, '0', "the first page is read");
    assert_eq!(second.cells[0].character, '1', "and the second one too");
}

#[test]
fn a_page_past_the_last_one_reads_the_last_page() {
    let mut cpu = Cpu::new();
    // The eighth page is the last one the hardware has.
    cpu.mem.write_u8(PAGE, 0x7000, b'Z');
    let ui = Ui {
        screen_page: 200,
        ..Ui::default()
    };

    let screen = &Panels::of(&cpu, &ui).screen;

    assert_eq!(
        screen.page, 7,
        "the page is bounded to the eight the hardware has"
    );
    assert_eq!(
        screen.cells[0].character, 'Z',
        "and the last page is what is read: nothing ran off the bus"
    );
}

/// The attribute byte says which colours the cell is drawn with.
///
/// The foreground is the low three bits and the background the high
/// nibble, and the background's own brightness is the eighth background
/// and not a shade of the seventh.
#[test]
fn the_attribute_byte_says_the_colours_the_cell_is_drawn_with() {
    let mut cpu = Cpu::new();
    // A red character on a blue background.
    cpu.mem.write_u8(PAGE, 0x0001, 0x14);
    // A bright black background: the high bit and nothing else.
    cpu.mem.write_u8(PAGE, 0x0003, 0x80);
    // A blinking character.
    cpu.mem.write_u8(PAGE, 0x0005, 0x08);

    let screen = &Panels::of(&cpu, &Ui::default()).screen;

    let red = screen.cells[0];
    assert_eq!(red.foreground(), 4, "the foreground is the low three bits");
    assert_eq!(red.background(), 1, "and the background the high nibble");
    assert!(!red.blink(), "and the blink bit is not set");

    let bright = screen.cells[1];
    assert_eq!(bright.background(), 8, "the bright bit is the eighth");
    assert_eq!(bright.foreground(), 0, "and the foreground is still black");

    assert!(screen.cells[2].blink(), "the blink bit is the third one");
}
