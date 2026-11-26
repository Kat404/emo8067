//! The guest's screen: the text page the machine wrote to.
//!
//! The panel is a pure function of the machine and of the UI's state, and
//! the page is the machine's own memory: what the guest wrote to it is
//! what it shows.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg8, Seg};
use emo8067_debug::panels::{Cursor, Panels, Ui};

/// The segment the text page lives in.
const PAGE: u16 = 0xB800;

/// The segment the graphics page lives in.
const GRAPHICS: u16 = 0xA000;

/// The physical address the text page lives at.
const PAGE_PHYSICAL: u32 = 0xB8000;

/// The cells the page holds.
const CELLS: usize = 80 * 25;

/// The segment the test programs run in.
const CODE: u16 = 0x1000;

/// A machine that has called the video BIOS, with the registers the test
/// sets before the call.
///
/// The call is two steps: the interrupt, and the service that comes out of
/// it.
fn bios(setup: impl FnOnce(&mut Cpu)) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x10000, &[0xCD, 0x10, 0xF4]) // int 10h; hlt
        .expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, CODE);
    setup(&mut cpu);
    cpu.step().expect("the call runs");
    cpu.step().expect("the service runs");
    cpu
}

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

// The probes below were written from the specs and not from the code, so
// they are a second opinion on the same question.

/// The panel is a function of the machine's state: it is read again every
/// time, and it holds no copy of the page.
#[test]
fn the_panel_is_read_again_and_holds_no_copy() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    let first = Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        first.cells[0].character, 'A',
        "the guest's byte is on the page"
    );
    cpu.mem.write_u8(PAGE, 0x0000, b'B');
    let second = Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        second.cells[0].character, 'B',
        "the page is read again: it is a function of the machine's state"
    );
    assert_eq!(
        first.cells[0].character, 'A',
        "and the first reading is its own: no copy is shared"
    );
}

/// The panel follows the UI: the segment the UI names is the one that is
/// read, and the panel says which one it read.
#[test]
fn the_panel_follows_the_ui_state() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    cpu.mem.write_u8(0x1000, 0x0000, b'B');
    let default = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(default.cells[0].character, 'A', "the default page");
    let other = &Panels::of(
        &cpu,
        &Ui {
            screen_segment: 0x1000,
            ..Ui::default()
        },
    )
    .screen;
    assert_eq!(
        other.cells[0].character, 'B',
        "the segment the UI says is the one that is read"
    );
    assert_eq!(
        other.segment, 0x1000,
        "and the panel says which one it read"
    );
}

/// The default page is the one at physical B8000.
#[test]
fn the_default_page_is_the_one_at_physical_b8000() {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(PAGE_PHYSICAL, &[b'Q', 0x1E])
        .expect("the page loads");
    cpu.mem
        .load_at(0x0B800, b"Z")
        .expect("the other byte loads");
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        screen.cells[0].character, 'Q',
        "B800:0000 is physical B8000"
    );
    assert_eq!(
        screen.cells[0].attribute, 0x1E,
        "and the attribute is the second byte of the cell"
    );
    assert_ne!(
        screen.cells[0].character, 'Z',
        "and the page is not read at some other place"
    );
    assert_eq!(screen.segment, PAGE, "the default segment is B800");
    assert_eq!(screen.page, 0, "and the default page is the first one");
}

/// The pages of text memory are four kilobytes apart.
#[test]
fn every_page_is_four_kilobytes_apart() {
    let mut cpu = Cpu::new();
    for page in 0..8u32 {
        cpu.mem
            .load_at(PAGE_PHYSICAL + page * 0x1000, &[b'0' + page as u8, 0x1E])
            .expect("the page loads");
    }
    for page in 0..8u8 {
        let ui = Ui {
            screen_page: page,
            ..Ui::default()
        };
        let screen = &Panels::of(&cpu, &ui).screen;
        assert_eq!(
            screen.cells[0].character,
            (b'0' + page) as char,
            "page {page} is four kilobytes after the one before it"
        );
        assert_eq!(
            screen.cells[0].attribute, 0x1E,
            "and its attribute is read with it"
        );
        assert_eq!(screen.page, page, "and the panel says which page it read");
    }
}

/// A page past the last one is the last one, and it stays on the bus.
#[test]
fn a_page_past_the_last_one_is_the_last_one_and_stays_on_the_bus() {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0xBFF9E, &[b'L', 0x9F])
        .expect("the last cell loads");
    for page in [7u8, 8, 9, 200, 255] {
        let ui = Ui {
            screen_page: page,
            ..Ui::default()
        };
        let screen = &Panels::of(&cpu, &ui).screen;
        assert_eq!(screen.page, 7, "the page {page} is bounded to the eighth");
        assert_eq!(screen.cells.len(), CELLS, "the page still holds its cells");
        assert_eq!(
            screen.cells[CELLS - 1].character,
            'L',
            "the last cell of the last page is on the bus"
        );
        assert_eq!(
            screen.cells[CELLS - 1].attribute,
            0x9F,
            "and so is its attribute"
        );
    }
}

/// No segment the UI names reads off the bus.
#[test]
fn no_segment_the_ui_names_reads_off_the_bus() {
    for segment in [0x0000u16, 0x0001, PAGE, 0xBFFF, 0xC000, 0xFFFF] {
        let cpu = Cpu::new();
        let ui = Ui {
            screen_segment: segment,
            screen_page: 255,
            ..Ui::default()
        };
        let screen = &Panels::of(&cpu, &ui).screen;
        assert_eq!(screen.cells.len(), CELLS, "segment {segment:04X}");
        assert_eq!(screen.page, 7, "segment {segment:04X}");
    }
}

/// Every byte that is not printable is a space.
#[test]
fn every_byte_that_is_not_printable_is_a_space() {
    let mut cpu = Cpu::new();
    for index in 0..256u16 {
        cpu.mem.write_u8(PAGE, index * 2, index as u8);
    }
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    for (index, cell) in screen.cells[..256].iter().enumerate() {
        let byte = index as u8;
        let expected = if (0x20..0x7F).contains(&byte) {
            byte as char
        } else {
            ' '
        };
        assert_eq!(
            cell.character, expected,
            "the byte {byte:#04x} is drawn as {expected:?}"
        );
    }
}

/// Two bytes a cell: the character and the attribute.
#[test]
fn two_bytes_a_cell_the_character_and_the_attribute() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    cpu.mem.write_u8(PAGE, 0x0001, 0x07);
    cpu.mem.write_u8(PAGE, 0x0002, b'B');
    cpu.mem.write_u8(PAGE, 0x0003, 0x80);
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(screen.cells[0].character, 'A', "the first cell's character");
    assert_eq!(screen.cells[0].attribute, 0x07, "and its attribute");
    assert_eq!(
        screen.cells[1].character, 'B',
        "the second cell's character"
    );
    assert_eq!(screen.cells[1].attribute, 0x80, "and its attribute");
}

/// The attribute says the colours, for every byte there is.
#[test]
fn the_attribute_says_the_colours_for_every_byte() {
    for attribute in 0u8..=255 {
        let mut cpu = Cpu::new();
        cpu.mem.write_u8(PAGE, 0x0000, b'A');
        cpu.mem.write_u8(PAGE, 0x0001, attribute);
        let screen = &Panels::of(&cpu, &Ui::default()).screen;
        let cell = screen.cells[0];
        assert_eq!(
            cell.foreground(),
            attribute & 0x07,
            "the foreground of {attribute:#04x} is the low three bits"
        );
        assert_eq!(
            cell.background(),
            (attribute >> 4) & 0x0F,
            "the background of {attribute:#04x} is the high nibble"
        );
        assert_eq!(
            cell.blink(),
            attribute & 0x08 != 0,
            "the blink of {attribute:#04x} is the third bit"
        );
    }
}

/// S8 — The screen panel shows the guest's screen as the machine has it:
/// the geometry the machine has, the page the machine has active, and the
/// cursor where the BIOS says it is. The panel holds no copy of either.
#[test]
fn the_panel_shows_the_machines_geometry_the_page_and_the_cursor() {
    // The machine's mode: forty columns by twenty-five rows.
    let cpu = bios(|cpu| {
        cpu.regs.set_byte(Reg8::Ah, 0x00);
        cpu.regs.set_byte(Reg8::Al, 0x00);
    });
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        screen.columns, 40,
        "the panel shows the geometry the machine has"
    );
    assert_eq!(screen.rows, 25, "and the rows the machine has");
    assert_eq!(
        screen.cells.len(),
        40 * 25,
        "and the cells that geometry holds"
    );

    // The cursor is where the BIOS says it is.
    let cpu = bios(|cpu| {
        cpu.regs.set_byte(Reg8::Ah, 0x02);
        cpu.regs.set_byte(Reg8::Dh, 3);
        cpu.regs.set_byte(Reg8::Dl, 7);
    });
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        screen.cursor,
        Some(Cursor { row: 3, column: 7 }),
        "the cursor is where the BIOS says it is"
    );

    // A cursor the guest hid is not on the screen.
    let cpu = bios(|cpu| {
        cpu.regs.set_byte(Reg8::Ah, 0x01);
        // Bit 5 of CH set is the cursor that is not visible.
        cpu.regs.set_byte(Reg8::Ch, 0x20);
    });
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(screen.cursor, None, "a hidden cursor is not drawn");

    // The panel reads the machine again: it holds no copy of the page.
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');
    let first = Panels::of(&cpu, &Ui::default()).screen;
    cpu.mem.write_u8(PAGE, 0x0000, b'B');
    let second = Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        second.cells[0].character, 'B',
        "the page is read again: the panel is a function of the machine"
    );
    assert_eq!(first.cells[0].character, 'A', "and it holds no copy");
}

/// The panel shows the page the panel shows, and the TUI follows the
/// machine's active page with it.
#[test]
fn the_panel_shows_the_page_the_ui_says() {
    let cpu = bios(|cpu| {
        // The machine's active page is the third one.
        cpu.regs.set_byte(Reg8::Ah, 0x05);
        cpu.regs.set_byte(Reg8::Al, 2);
    });
    let screen = &Panels::of(
        &cpu,
        &Ui {
            screen_page: 2,
            ..Ui::default()
        },
    )
    .screen;
    assert_eq!(screen.page, 2, "the page the UI says is the page shown");
}

/// The background's own brightness is the eighth background, and not a
/// shade of the seventh.
#[test]
fn the_bright_background_bit_is_the_eighth_background_and_not_a_shade_of_the_seventh() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0001, 0x80);
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(
        screen.cells[0].background(),
        8,
        "the high bit is the eighth background"
    );
    assert_ne!(
        screen.cells[0].background(),
        7,
        "and not a shade of the seventh"
    );
    assert_eq!(
        screen.cells[0].foreground(),
        0,
        "and the foreground is still the low three bits"
    );
}

/// S28 — The panel shows the graphics page.
///
/// In the graphics mode the panel reads the framebuffer from `A000:0000`,
/// and it reads the machine's own memory: what the guest drew is what the
/// panel shows.
#[test]
fn s28_the_panel_reads_the_framebuffer_in_the_graphics_mode() {
    let mut cpu = bios(|cpu| {
        cpu.regs.set_byte(Reg8::Ah, 0x00);
        cpu.regs.set_byte(Reg8::Al, 0x13);
    });
    // A pixel the guest drew, at the row 20 and the column 10.
    cpu.mem.write_u8(GRAPHICS, 20 * 320 + 10, 0x2A);

    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    let graphics = screen
        .graphics
        .as_ref()
        .expect("the panel shows the graphics page");

    assert_eq!(graphics.width, 320, "the framebuffer is 320 pixels wide");
    assert_eq!(graphics.height, 200, "and two hundred rows tall");
    assert_eq!(
        graphics.pixels[20 * 320 + 10],
        0x2A,
        "the pixel is at the offset row * 320 + column"
    );
    assert_eq!(
        screen.columns, 40,
        "and the machine's cell grid is forty columns"
    );
    assert_eq!(screen.rows, 25, "and twenty-five rows");
    assert_eq!(screen.cursor, None, "the graphics page has no text cursor");
}

/// The text page is still the page the panel shows.
///
/// The graphics page is a second shape of the same thing: the text page
/// is the one the panel shows when the machine is in a text mode.
#[test]
fn the_text_page_is_still_the_page_the_panel_shows() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(PAGE, 0x0000, b'A');

    let screen = &Panels::of(&cpu, &Ui::default()).screen;

    assert_eq!(
        screen.cells[0].character, 'A',
        "the text page is the page the panel shows"
    );
    assert!(
        screen.graphics.is_none(),
        "and there is no graphics page in a text mode"
    );
}
