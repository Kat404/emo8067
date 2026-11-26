//! The verifier's own probes for the graphical mode of the video BIOS.
//!
//! Written against the published sources, not against the implementation:
//! the framebuffer is Dr Bill Verts' mode `13h` one, and the mode byte is
//! the IBM's own `0040:0049`.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg8, Reg16, Seg};

const DATA: u16 = 0x0040;
const MODE: u16 = 0x0049;
const COLUMNS: u16 = 0x004A;
const PAGE_START: u16 = 0x004E;
const PAGE: u16 = 0x0062;
const CRTC_PORT: u16 = 0x0063;
const ROWS: u16 = 0x0084;
const VIDEO: u16 = 0xB800;
const GRAPHICS: u16 = 0xA000;
const WIDTH: u16 = 320;
const HEIGHT: u16 = 200;
const CODE: u16 = 0x1000;

fn machine(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).expect("the program loads");
    c.regs.set_segment(Seg::Cs, CODE);
    c.regs.set_segment(Seg::Ds, CODE);
    c.regs.set_segment(Seg::Es, CODE);
    c.regs.set_segment(Seg::Ss, CODE);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

fn run(c: &mut Cpu) {
    for _ in 0..2000 {
        if c.halted {
            return;
        }
        c.step().expect("the program must not fault");
    }
    panic!("the program did not halt");
}

/// A machine that calls `INT 10h` the given number of times.
fn calls(count: usize, setup: impl Fn(usize, &mut Cpu)) -> Cpu {
    let mut code = Vec::new();
    for _ in 0..count {
        code.push(0xCD); // int 10h
        code.push(0x10);
    }
    code.push(0xF4); // hlt
    let mut c = machine(&code);
    for index in 0..count {
        setup(index, &mut c);
        c.step().expect("the call runs");
        c.step().expect("the service runs");
    }
    run(&mut c);
    c
}

/// A machine that calls `INT 10h` once.
fn once(setup: impl FnOnce(&mut Cpu)) -> Cpu {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    setup(&mut c);
    c.step().expect("the call runs");
    c.step().expect("the service runs");
    run(&mut c);
    c
}

/// S22 — the mode `13h` is a mode the machine has.
#[test]
fn s22_the_graphical_mode_is_a_mode_the_machine_has() {
    let c = once(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x13);
    });
    assert_eq!(c.mem.read_u16(DATA, COLUMNS), 40, "forty columns");
    assert_eq!(c.mem.read_u8(DATA, ROWS), 24, "twenty-five rows, minus one");
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x13, "the mode names itself");
    assert_eq!(c.mem.read_u8(DATA, PAGE), 0, "the mode has one page");
    assert_eq!(c.mem.read_u16(DATA, PAGE_START), 0, "and its start is zero");
}

/// S22 — every other mode leaves the machine as it was.
#[test]
fn s22_a_mode_the_machine_does_not_have_leaves_the_machine_as_it_was() {
    let c = calls(2, |index, c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs
            .set_byte(Reg8::Al, if index == 0 { 0x13 } else { 0x07 });
    });
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x13, "the mode is still 13h");
    assert_eq!(c.mem.read_u16(DATA, COLUMNS), 40, "and still forty columns");
}

/// S22 — the machine's own text modes still set their own geometry.
#[test]
fn s22_the_text_modes_still_set_their_own_geometry() {
    let c = once(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x00);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        40,
        "mode 00h is forty columns"
    );
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x00, "and the mode says so");

    let c = once(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x03);
    });
    assert_eq!(c.mem.read_u16(DATA, COLUMNS), 80, "mode 03h is eighty");
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x03, "and the mode says so");
}

/// S23 — the framebuffer is the published mode `13h` one.
///
/// "the screen is an array of 64000 contiguous bytes, arranged as 200
/// rows of 320 bytes, with one byte per pixel, located in memory at
/// absolute address `$A000:$0000`".
#[test]
fn s23_the_framebuffer_is_at_a000_with_320_byte_rows() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x11);
            c.regs.set_word(Reg16::Cx, 0);
            c.regs.set_word(Reg16::Dx, 0);
        }
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 0),
        0x11,
        "the first pixel is at A000:0000"
    );
    assert_eq!(
        c.mem.read_u8(VIDEO, 0),
        0x00,
        "and the text page is not the graphics page"
    );

    for (column, row, colour) in [
        (0u16, 0u16, 0x01u8),
        (319, 0, 0x02),
        (0, HEIGHT - 1, 0x03),
        (319, HEIGHT - 1, 0x04),
        (160, 100, 0x05),
    ] {
        let c = calls(2, |index, c| {
            if index == 0 {
                c.regs.set_byte(Reg8::Ah, 0x00);
                c.regs.set_byte(Reg8::Al, 0x13);
            } else {
                c.regs.set_byte(Reg8::Ah, 0x0C);
                c.regs.set_byte(Reg8::Al, colour);
                c.regs.set_word(Reg16::Cx, column);
                c.regs.set_word(Reg16::Dx, row);
            }
        });
        assert_eq!(
            c.mem.read_u8(GRAPHICS, row * WIDTH + column),
            colour,
            "the pixel at column {column} and row {row} is at row * 320 + column"
        );
    }
}

/// S26 — the framebuffer's bounds are the published ones: a coordinate
/// outside it writes nothing and reads zero, and the address never wraps
/// into the page.
#[test]
fn s26_a_pixel_outside_the_framebuffer_writes_nothing_and_reads_zero() {
    for (column, row) in [
        (320u16, 0u16),
        (0, HEIGHT),
        (320, HEIGHT),
        (321, 201),
        (65535, 65535),
        (65535, 0),
        (0, 65535),
    ] {
        let c = calls(3, |index, c| match index {
            0 => {
                c.regs.set_byte(Reg8::Ah, 0x00);
                c.regs.set_byte(Reg8::Al, 0x13);
            }
            1 => {
                c.regs.set_byte(Reg8::Ah, 0x0C);
                c.regs.set_byte(Reg8::Al, 0x2A);
                c.regs.set_word(Reg16::Cx, column);
                c.regs.set_word(Reg16::Dx, row);
            }
            _ => {
                c.regs.set_byte(Reg8::Ah, 0x0D);
                c.regs.set_byte(Reg8::Al, 0x77);
                c.regs.set_word(Reg16::Cx, column);
                c.regs.set_word(Reg16::Dx, row);
            }
        });
        for offset in 0..=u16::MAX {
            assert_eq!(
                c.mem.read_u8(GRAPHICS, offset),
                0x00,
                "nothing was written at A000:{offset:04X} (column {column}, row {row})"
            );
        }
        assert_eq!(
            c.regs.byte(Reg8::Al),
            0x00,
            "and the read outside the framebuffer is zero"
        );
    }
}

/// S24 — `AH = 0Ch` writes a pixel: `AL` the colour, `CX` the column,
/// `DX` the row, and **no page register**.
#[test]
fn s24_the_write_uses_al_cx_and_dx_and_invents_no_page_register() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
            // Every register the reference does not name is set to
            // something: none of them may move the pixel. (`CX` is the
            // column and `DX` is the row, so their two bytes are part of
            // the contract and are not touched here.)
            c.regs.set_byte(Reg8::Bh, 7);
            c.regs.set_byte(Reg8::Bl, 0xFF);
            c.regs.set_word(Reg16::Bx, 0xFFFF);
            c.regs.set_word(Reg16::Si, 0xFFFF);
            c.regs.set_word(Reg16::Di, 0xFFFF);
            c.regs.set_word(Reg16::Bp, 0xFFFF);
        }
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
        0x2A,
        "the colour is at the pixel the column and the row name"
    );
}

/// S25 — `AH = 0Dh` reads a pixel.
#[test]
fn s25_the_read_returns_the_colour_of_the_pixel() {
    let c = calls(3, |index, c| match index {
        0 => {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        }
        1 => {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
        _ => {
            c.regs.set_byte(Reg8::Ah, 0x0D);
            c.regs.set_byte(Reg8::Al, 0x00);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
    });
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x2A,
        "the read returns what was written"
    );
    assert_eq!(c.regs.word(Reg16::Cx), 10, "and the column is untouched");
    assert_eq!(c.regs.word(Reg16::Dx), 20, "and the row is untouched");
}

/// S25 — a pixel that was never written reads zero.
#[test]
fn s25_a_pixel_that_was_never_written_reads_zero() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0D);
            c.regs.set_byte(Reg8::Al, 0x77);
            c.regs.set_word(Reg16::Cx, 100);
            c.regs.set_word(Reg16::Dx, 50);
        }
    });
    assert_eq!(c.regs.byte(Reg8::Al), 0x00, "an unwritten pixel is zero");
}

/// S26 — the two functions answer only in the graphics mode, and the
/// mode's transition is a transition.
#[test]
fn s26_the_pixel_functions_follow_the_modes_transition() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0D);
            c.regs.set_byte(Reg8::Al, 0x77);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
        0x00,
        "a text mode writes no pixel"
    );
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x77,
        "and the read does not answer at all"
    );

    let c = calls(3, |index, c| match index {
        0 => {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        }
        1 => {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
        _ => {
            c.regs.set_byte(Reg8::Ah, 0x0D);
            c.regs.set_byte(Reg8::Al, 0x00);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
        0x2A,
        "in the graphics mode the write lands"
    );
    assert_eq!(c.regs.byte(Reg8::Al), 0x2A, "and the read answers");

    let c = calls(4, |index, c| match index {
        0 => {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x13);
        }
        1 => {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
        2 => {
            c.regs.set_byte(Reg8::Ah, 0x00);
            c.regs.set_byte(Reg8::Al, 0x03);
        }
        _ => {
            c.regs.set_byte(Reg8::Ah, 0x0D);
            c.regs.set_byte(Reg8::Al, 0x00);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
    });
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x00,
        "a guest that leaves the graphical mode stops getting pixel answers"
    );
}

/// S27 — the mode is one byte at `0040:0049`, and `0040:0063` is not it.
#[test]
fn s27_the_mode_is_one_byte_at_0040_0049_and_not_at_0040_0063() {
    let c = once(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x13);
    });
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x13, "the mode is at 0040:0049");
    assert_eq!(
        c.mem.read_u8(DATA, MODE + 1),
        40,
        "the byte after the mode is the columns' low byte"
    );

    let mut c = Cpu::new();
    c.mem.write_u8(DATA, CRTC_PORT, 0x13);
    assert_eq!(
        c.mem.read_u8(DATA, MODE),
        0x03,
        "writing 13h at 0040:0063 does not make the machine graphical"
    );
    let c = calls(1, |_, c| {
        c.regs.set_byte(Reg8::Ah, 0x0C);
        c.regs.set_byte(Reg8::Al, 0x2A);
        c.regs.set_word(Reg16::Cx, 10);
        c.regs.set_word(Reg16::Dx, 20);
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
        0x00,
        "and the pixel functions still do not answer"
    );
}

/// The mode byte is the guest's own: the BIOS keeps no private copy of
/// it, and a guest that scribbles there never makes the emulator panic.
#[test]
fn the_guest_may_scribble_the_mode_byte() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.mem.write_u8(DATA, MODE, 0x13);
            // A service that is not `AH = 00h`, so the byte survives.
            c.regs.set_byte(Reg8::Ah, 0x0E);
            c.regs.set_byte(Reg8::Al, b'x');
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0C);
            c.regs.set_byte(Reg8::Al, 0x2A);
            c.regs.set_word(Reg16::Cx, 10);
            c.regs.set_word(Reg16::Dx, 20);
        }
    });
    assert_eq!(
        c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
        0x2A,
        "the BIOS has no private copy of the mode: it reads the guest's byte"
    );

    for rubbish in [0x00u8, 0x01, 0x07, 0xFF] {
        let c = calls(2, |index, c| {
            if index == 0 {
                c.mem.write_u8(DATA, MODE, rubbish);
                c.regs.set_byte(Reg8::Ah, 0x0E);
                c.regs.set_byte(Reg8::Al, b'x');
            } else {
                c.regs.set_byte(Reg8::Ah, 0x0C);
                c.regs.set_byte(Reg8::Al, 0x2A);
                c.regs.set_word(Reg16::Cx, 10);
                c.regs.set_word(Reg16::Dx, 20);
            }
        });
        assert_eq!(
            c.mem.read_u8(GRAPHICS, 20 * WIDTH + 10),
            0x00,
            "a mode of {rubbish:#04X} is a text mode"
        );
    }
}

/// The machine starts in the mode its geometry names.
#[test]
fn the_machine_starts_in_the_mode_its_geometry_names() {
    let c = Cpu::new();
    assert_eq!(c.mem.read_u8(DATA, MODE), 0x03, "the machine starts in 03h");
    assert_eq!(c.mem.read_u16(DATA, COLUMNS), 80, "eighty columns");
    assert_eq!(c.mem.read_u8(DATA, ROWS), 24, "twenty-five rows");
}

/// PRESERVE — the thirteen text services are the way they were.
#[test]
fn the_text_services_are_the_way_they_were() {
    let c = calls(2, |index, c| {
        if index == 0 {
            c.regs.set_byte(Reg8::Ah, 0x0E);
            c.regs.set_byte(Reg8::Al, b'A');
        } else {
            c.regs.set_byte(Reg8::Ah, 0x0E);
            c.regs.set_byte(Reg8::Al, b'B');
        }
    });
    assert_eq!(
        c.mem.read_u8(VIDEO, 0),
        b'A',
        "the first character is written"
    );
    assert_eq!(c.mem.read_u8(VIDEO, 2), b'B', "and the second one after it");
    assert_eq!(
        c.mem.read_u16(DATA, 0x0050),
        2,
        "and the cursor advanced twice"
    );

    let c = calls(1, |_, c| {
        c.mem.write_u8(VIDEO, 0, b'Z');
        c.mem.write_u8(VIDEO, 1, 0x1E);
        c.regs.set_byte(Reg8::Ah, 0x08);
        c.regs.set_byte(Reg8::Bh, 0);
    });
    assert_eq!(c.regs.byte(Reg8::Al), b'Z', "the character is read back");
    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "and the attribute with it");
}
