//! The video BIOS: the services of `INT 10h`.
//!
//! One test per spec rule, S1 to S9. The BIOS keeps no state of its own:
//! everything it knows lives in the BIOS data area at `0040:0000`, where the
//! guest can read it, and every service is a function of the machine's
//! memory and its registers.
//!
//! The tests read the machine's own memory and the machine's own data area,
//! never the BIOS's internals: what the guest can see is what is asserted.

use emo8067_core::Cpu;
use emo8067_core::StopReason;
use emo8067_core::flags;
use emo8067_core::regs::{Reg8, Reg16, Seg};

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;

/// The number of columns on screen.
const COLUMNS: u16 = 0x004A;
/// The current video page's start in video memory.
const PAGE_START: u16 = 0x004E;
/// The eight cursors, one pair a page.
const CURSORS: u16 = 0x0050;
/// The current video page number.
const PAGE: u16 = 0x0062;
/// The cursor's shape: the start line and the bottom line.
const SHAPE: u16 = 0x0080;
/// The rows on screen, minus one.
const ROWS: u16 = 0x0084;

/// The segment the text pages live in.
const VIDEO: u16 = 0xB800;

/// The segment the test programs run in.
const CODE: u16 = 0x1000;

/// The attribute the emulator's own text carries: light grey on black.
const TEXT: u8 = 0x07;

/// A machine with a program in it, ready to run.
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

/// Run the program to its `HLT`.
fn run(c: &mut Cpu) {
    for _ in 0..1000 {
        if c.halted {
            return;
        }
        c.step().expect("the program must not fault");
    }
    panic!("the program did not halt");
}

/// A machine that calls `INT 10h` once, with the registers the test sets
/// before the call.
fn bios(setup: impl FnOnce(&mut Cpu)) -> Cpu {
    let mut c = machine(&[0xCD, 0x10, 0xF4]); // int 10h; hlt
    setup(&mut c);
    run(&mut c);
    c
}

/// A machine that calls `INT 10h` twice: the first call with the registers
/// the first closure sets, the second with the registers the second one
/// sets.
///
/// It is one machine throughout, so the first call's state is the second
/// call's state. Each call is two steps: the interrupt, and the service
/// that comes out of it.
fn bios_twice(first: impl FnOnce(&mut Cpu), second: impl FnOnce(&mut Cpu)) -> Cpu {
    let mut c = machine(&[0xCD, 0x10, 0xCD, 0x10, 0xF4]); // int 10h; int 10h; hlt
    first(&mut c);
    c.step().expect("the first call runs");
    c.step().expect("the first service runs");
    second(&mut c);
    c.step().expect("the second call runs");
    c.step().expect("the second service runs");
    run(&mut c);
    c
}

/// The character at a cell of a page.
fn character(c: &Cpu, page: u16, index: u16) -> u8 {
    c.mem.read_u8(VIDEO, page * 0x1000 + index * 2)
}

/// The attribute at a cell of a page.
fn attribute(c: &Cpu, page: u16, index: u16) -> u8 {
    c.mem.read_u8(VIDEO, page * 0x1000 + index * 2 + 1)
}

/// The cursor's row and column on a page, the way the data area keeps them.
fn cursor(c: &Cpu, page: u8) -> (u8, u8) {
    let word = c.mem.read_u16(DATA, CURSORS + 2 * u16::from(page));
    ((word >> 8) as u8, (word & 0xFF) as u8)
}

/// S1 — The `INT 10h` is the 8086's `INT`.
///
/// It pushes FLAGS, CS and IP, it clears TF and IF, and it loads `CS:IP`
/// from the interrupt vector table, exactly as it does for every other
/// vector. The BIOS does not answer the call in place and does not change
/// what the instruction does.
#[test]
fn s1_the_int_pushes_the_frame_and_vectors() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.flags.set(flags::TF, true);
    c.regs.flags.set(flags::IF, true);
    let flags = c.regs.flags.0;
    c.step().expect("the interrupt runs");

    // The stack is six bytes lower: the frame is three words.
    assert_eq!(c.regs.word(Reg16::Sp), 0x0100 - 6, "the frame was pushed");
    // And the frame holds what the machine pushed: FLAGS, then CS, then IP.
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FE),
        flags | flags::UNDEFINED,
        "the flags word is on the top of the stack"
    );
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FC),
        CODE,
        "and the machine's own code segment under it"
    );
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FA),
        0x0002,
        "and the instruction after the call under that"
    );
    // The machine is at the vector table's entry.
    assert_eq!(
        c.regs.segment(Seg::Cs),
        c.mem.read_u16(0x0000, 0x0042),
        "the machine is at the vector's segment"
    );
    assert_eq!(
        c.regs.ip,
        c.mem.read_u16(0x0000, 0x0040),
        "and at the vector's offset"
    );
    // TF and IF are cleared, the way the chip clears them.
    assert!(!c.regs.flags.get(flags::TF), "TF is cleared");
    assert!(!c.regs.flags.get(flags::IF), "IF is cleared");
}

/// A machine whose vector table points somewhere else runs whatever is
/// there, the way the bare chip does.
#[test]
fn a_machine_whose_vector_table_points_elsewhere_runs_what_is_there() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // The guest's own handler: the vector table points at it.
    c.mem.write_u16(0x0000, 0x0040, 0x0100);
    c.mem.write_u16(0x0000, 0x0042, CODE);
    c.mem.write_u8(CODE, 0x0100, 0xB8); // mov ax, 1234h
    c.mem.write_u8(CODE, 0x0101, 0x34);
    c.mem.write_u8(CODE, 0x0102, 0x12);

    c.step().expect("the interrupt runs");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        CODE,
        "the machine is at the vector's entry"
    );
    assert_eq!(c.regs.ip, 0x0100, "and at its offset");

    c.step().expect("the handler runs");
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "and the machine ran what was there: the BIOS's ROM is not reached"
    );
}

/// S1 — Every other vector is untouched, and the program carries on past
/// the call the BIOS answers.
#[test]
fn s1_every_other_vector_is_untouched_and_the_program_carries_on() {
    // mov ax, 0E41h ('A'); int 10h; mov bx, 1234h; hlt
    let mut c = machine(&[
        0xB8, 0x41, 0x0E, // mov ax, 0E41h
        0xCD, 0x10, // int 10h
        0xBB, 0x34, 0x12, // mov bx, 1234h
        0xF4, // hlt
    ]);
    run(&mut c);

    assert_eq!(
        c.regs.word(Reg16::Bx),
        0x1234,
        "the program carried on past the call"
    );
    assert_eq!(
        c.stop_reason,
        StopReason::Halted,
        "and the run ended at the HLT"
    );
    assert_eq!(
        character(&c, 0, 0),
        b'A',
        "the service wrote the character to the page"
    );

    // Another vector is untouched: it is taken, and the machine vectors
    // through the table like it always did.
    let mut c = machine(&[0xCD, 0x21]); // int 21h
    c.step().expect("the step runs");
    assert_eq!(
        c.stop_reason,
        StopReason::Interrupt(0x21),
        "the interrupt was taken"
    );
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0x0000,
        "and the machine vectored to the table"
    );
    assert_eq!(c.regs.ip, 0x0000, "at the entry the table holds");
}

/// S11 — The service returns the way a handler returns.
///
/// It pops the frame the `INT` pushed and carries on at the instruction
/// after the call, so the guest's stack and the guest's flags are where
/// they were and the machine's depth of calls goes down as it came up.
#[test]
fn s11_the_service_returns_the_way_a_handler_returns() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.flags.set(flags::IF, true);
    let sp = c.regs.word(Reg16::Sp);

    c.step().expect("the interrupt runs");
    // The machine is at the BIOS's entry: the service has not run yet.
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0xF400,
        "the machine is at the BIOS's entry"
    );
    assert_eq!(c.regs.ip, 0x0190, "at the entry the vector names");
    assert_eq!(c.call_depth, 1, "and the interrupt went in as a call");

    c.step().expect("the service runs");
    assert_eq!(c.regs.word(Reg16::Sp), sp, "the stack is where it was");
    assert!(
        c.regs.flags.get(flags::IF),
        "and the flags are where they were"
    );
    assert_eq!(
        c.regs.ip, 0x0002,
        "and the machine carries on after the call"
    );
    assert_eq!(c.regs.segment(Seg::Cs), CODE, "in the guest's own segment");
    assert_eq!(
        c.call_depth, 0,
        "and the depth of calls went down as it came up"
    );
}

/// S2 — The thirteen services, with the reference's own register contracts.
#[test]
fn s2_the_services_have_the_reference_own_register_contracts() {
    // AH = 00h — set video mode. AL = the mode.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x00);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        40,
        "mode 00h is forty columns"
    );

    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x03);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        80,
        "mode 03h is eighty columns"
    );

    // AH = 01h — set the text-mode cursor shape. CH = start line, CL =
    // bottom line.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x01);
        c.regs.set_byte(Reg8::Ch, 0x06);
        c.regs.set_byte(Reg8::Cl, 0x07);
    });
    assert_eq!(c.mem.read_u8(DATA, SHAPE), 0x06, "the cursor's start line");
    assert_eq!(c.mem.read_u8(DATA, SHAPE + 1), 0x07, "and its bottom line");

    // AH = 02h — set the cursor position. DH = row, DL = column, BH = page.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x02);
        c.regs.set_byte(Reg8::Dh, 10);
        c.regs.set_byte(Reg8::Dl, 20);
        c.regs.set_byte(Reg8::Bh, 3);
    });
    assert_eq!(
        cursor(&c, 3),
        (10, 20),
        "the cursor of the page BH names, and no other"
    );

    // AH = 03h — get the cursor position and size. BH = page. Returns DH =
    // row, DL = column, CH = start line, CL = bottom line.
    let c = bios(|c| {
        c.mem.write_u16(DATA, CURSORS + 4, 0x0203);
        c.mem.write_u8(DATA, SHAPE, 0x06);
        c.mem.write_u8(DATA, SHAPE + 1, 0x07);
        c.regs.set_byte(Reg8::Ah, 0x03);
        c.regs.set_byte(Reg8::Bh, 2);
    });
    assert_eq!(c.regs.byte(Reg8::Dh), 2, "the row comes back in DH");
    assert_eq!(c.regs.byte(Reg8::Dl), 3, "the column in DL");
    assert_eq!(c.regs.byte(Reg8::Ch), 0x06, "the start line in CH");
    assert_eq!(c.regs.byte(Reg8::Cl), 0x07, "and the bottom line in CL");

    // AH = 05h — select the active video page. AL = the page.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x05);
        c.regs.set_byte(Reg8::Al, 3);
    });
    assert_eq!(
        c.mem.read_u8(DATA, PAGE),
        3,
        "the current video page is the one AL names"
    );
    assert_eq!(
        c.mem.read_u16(DATA, PAGE_START),
        0x3000,
        "and the page's start in video memory is where it is"
    );

    // AH = 06h — scroll up window. AL = lines, BH = the attribute of the
    // blank lines, CH/CL = the upper left corner, DH/DL = the lower right.
    let c = bios(|c| {
        for row in 0..3u16 {
            c.mem.write_u8(VIDEO, row * 160, b'A' + row as u8);
        }
        c.regs.set_byte(Reg8::Ah, 0x06);
        c.regs.set_byte(Reg8::Al, 1);
        c.regs.set_byte(Reg8::Bh, 0x1E);
        c.regs.set_byte(Reg8::Ch, 0);
        c.regs.set_byte(Reg8::Cl, 0);
        c.regs.set_byte(Reg8::Dh, 2);
        c.regs.set_byte(Reg8::Dl, 79);
    });
    assert_eq!(character(&c, 0, 0), b'B', "the window scrolled up");
    assert_eq!(
        attribute(&c, 0, 160),
        0x1E,
        "and the blank line carries the attribute in BH"
    );

    // AH = 07h — scroll down window: the same window, the other way.
    let c = bios(|c| {
        for row in 0..3u16 {
            c.mem.write_u8(VIDEO, row * 160, b'A' + row as u8);
        }
        c.regs.set_byte(Reg8::Ah, 0x07);
        c.regs.set_byte(Reg8::Al, 1);
        c.regs.set_byte(Reg8::Bh, 0x1E);
        c.regs.set_byte(Reg8::Ch, 0);
        c.regs.set_byte(Reg8::Cl, 0);
        c.regs.set_byte(Reg8::Dh, 2);
        c.regs.set_byte(Reg8::Dl, 79);
    });
    assert_eq!(
        character(&c, 0, 160),
        b'B',
        "the window scrolled down: the second row is where the first one was"
    );
    assert_eq!(
        attribute(&c, 0, 0),
        0x1E,
        "and the blank line carries the attribute in BH"
    );

    // AL = 00h clears the entire window.
    let c = bios(|c| {
        for row in 0..3u16 {
            c.mem.write_u8(VIDEO, row * 160, b'A' + row as u8);
        }
        c.regs.set_byte(Reg8::Ah, 0x06);
        c.regs.set_byte(Reg8::Al, 0);
        c.regs.set_byte(Reg8::Bh, 0x1E);
        c.regs.set_byte(Reg8::Ch, 0);
        c.regs.set_byte(Reg8::Cl, 0);
        c.regs.set_byte(Reg8::Dh, 2);
        c.regs.set_byte(Reg8::Dl, 79);
    });
    assert_eq!(
        character(&c, 0, 0),
        b' ',
        "zero lines clears the entire window"
    );
    assert_eq!(
        attribute(&c, 0, 0),
        0x1E,
        "and the window is filled with the attribute in BH"
    );

    // AH = 08h — read the character and the attribute at the cursor. BH =
    // page. Returns AH = attribute, AL = character.
    let c = bios(|c| {
        c.mem.write_u8(VIDEO, 0x0000, b'Z');
        c.mem.write_u8(VIDEO, 0x0001, 0x1E);
        c.regs.set_byte(Reg8::Ah, 0x08);
        c.regs.set_byte(Reg8::Bh, 0);
    });
    assert_eq!(
        c.regs.byte(Reg8::Al),
        b'Z',
        "the character comes back in AL"
    );
    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "and the attribute in AH");

    // AH = 09h — write the character and the attribute at the cursor. AL =
    // character, BH = page, BL = attribute, CX = times.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x09);
        c.regs.set_byte(Reg8::Al, b'Q');
        c.regs.set_byte(Reg8::Bh, 0);
        c.regs.set_byte(Reg8::Bl, 0x1E);
        c.regs.set_word(Reg16::Cx, 3);
    });
    assert_eq!(character(&c, 0, 0), b'Q', "the character was written");
    assert_eq!(attribute(&c, 0, 0), 0x1E, "with the attribute in BL");
    assert_eq!(character(&c, 0, 2), b'Q', "CX times");
    assert_eq!(cursor(&c, 0), (0, 0), "and the cursor stayed where it was");

    // AH = 0Ah — write the character only. AL = character, BH = page, CX =
    // times.
    let c = bios(|c| {
        c.mem.write_u8(VIDEO, 0x0001, 0x1E);
        c.regs.set_byte(Reg8::Ah, 0x0A);
        c.regs.set_byte(Reg8::Al, b'R');
        c.regs.set_byte(Reg8::Bh, 0);
        c.regs.set_word(Reg16::Cx, 2);
    });
    assert_eq!(character(&c, 0, 0), b'R', "the character was written");
    assert_eq!(
        attribute(&c, 0, 0),
        0x1E,
        "and the attribute the cell had was kept"
    );
    assert_eq!(character(&c, 0, 1), b'R', "CX times");

    // AH = 0Eh — teletype output. AL = the character. The printing is
    // always done to the current active page.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x0E);
        c.regs.set_byte(Reg8::Al, b'T');
    });
    assert_eq!(
        character(&c, 0, 0),
        b'T',
        "the character was written to the active page"
    );
    assert_eq!(
        attribute(&c, 0, 0),
        TEXT,
        "with the emulator's own text attribute"
    );
    assert_eq!(cursor(&c, 0), (0, 1), "and the cursor advanced");

    // AH = 13h — write string. AL = the write mode, BH = page, BL = the
    // attribute, CX = the characters, DL/DH = the column and the row,
    // ES:BP = the string.
    let c = bios(|c| {
        c.mem.write_u8(CODE, 0x0200, b'h');
        c.mem.write_u8(CODE, 0x0201, b'i');
        c.regs.set_byte(Reg8::Ah, 0x13);
        c.regs.set_byte(Reg8::Al, 0x00);
        c.regs.set_byte(Reg8::Bh, 0);
        c.regs.set_byte(Reg8::Bl, 0x1E);
        c.regs.set_word(Reg16::Cx, 2);
        c.regs.set_byte(Reg8::Dl, 5);
        c.regs.set_byte(Reg8::Dh, 1);
        c.regs.set_word(Reg16::Bp, 0x0200);
    });
    assert_eq!(character(&c, 0, 85), b'h', "the string was written");
    assert_eq!(character(&c, 0, 86), b'i', "all of it");
    assert_eq!(attribute(&c, 0, 85), 0x1E, "with the attribute in BL");
    assert_eq!(
        cursor(&c, 0),
        (0, 0),
        "and the cursor did not move: the write mode said so"
    );

    // AX = 1003h — toggle intensity/blinking. BL = the write mode, BH = 0.
    let mut c = machine(&[
        0xB8, 0x03, 0x10, // mov ax, 1003h
        0xBB, 0x00, 0x00, // mov bx, 0
        0xCD, 0x10, // int 10h
        0xBB, 0x34, 0x12, // mov bx, 1234h
        0xF4, // hlt
    ]);
    run(&mut c);
    assert_eq!(
        c.regs.word(Reg16::Bx),
        0x1234,
        "the call was answered and the program carried on"
    );
}

/// S3 — The state is the BIOS data area, at the offsets the reference's own
/// memory map gives, and the BIOS keeps no state of its own.
#[test]
fn s3_the_state_lives_in_the_bios_data_area() {
    let c = bios_twice(
        |c| {
            // Select the active page.
            c.regs.set_byte(Reg8::Ah, 0x05);
            c.regs.set_byte(Reg8::Al, 2);
        },
        |c| {
            // And put the cursor of that page somewhere.
            c.regs.set_byte(Reg8::Ah, 0x02);
            c.regs.set_byte(Reg8::Bh, 2);
            c.regs.set_byte(Reg8::Dh, 7);
            c.regs.set_byte(Reg8::Dl, 9);
        },
    );

    assert_eq!(
        c.mem.read_u16(DATA, PAGE_START),
        0x2000,
        "the current video page's start is at 0040:004E"
    );
    assert_eq!(
        c.mem.read_u8(DATA, PAGE),
        2,
        "the current video page number is at 0040:0062"
    );
    assert_eq!(
        cursor(&c, 2),
        (7, 9),
        "the cursors are at 0040:0050, one pair a page"
    );
    assert_eq!(
        cursor(&c, 0),
        (0, 0),
        "and the pages the guest did not touch keep their own cursor"
    );

    // The BIOS keeps no state of its own: two machines do not share one.
    let other = Cpu::new();
    assert_eq!(
        other.mem.read_u8(DATA, PAGE),
        0,
        "a second machine knows nothing of the first one's page"
    );
    assert_eq!(
        other.mem.read_u16(DATA, COLUMNS),
        80,
        "and holds the geometry of its own"
    );
}

/// S4 — The machine starts in the emulator's own default video mode: eighty
/// columns by twenty-five rows, every page's cursor at the origin, the
/// current page zero and its start at zero.
#[test]
fn s4_the_machine_starts_in_the_default_video_mode() {
    let c = Cpu::new();

    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        80,
        "eighty columns on screen"
    );
    assert_eq!(
        c.mem.read_u8(DATA, ROWS),
        24,
        "and twenty-five rows: the map holds the rows minus one"
    );
    assert_eq!(
        c.mem.read_u8(DATA, PAGE),
        0,
        "the current page is the first"
    );
    assert_eq!(
        c.mem.read_u16(DATA, PAGE_START),
        0,
        "and its start in video memory is zero"
    );
    for page in 0..8u8 {
        assert_eq!(
            cursor(&c, page),
            (0, 0),
            "the cursor of page {page} is at the origin"
        );
    }
}

/// S5 — The mode sets the geometry; a mode this slice does not have leaves
/// the geometry as it was.
#[test]
fn s5_the_mode_sets_the_geometry() {
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x00);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        40,
        "mode 00h is forty columns"
    );
    assert_eq!(c.mem.read_u8(DATA, ROWS), 24, "and twenty-five rows");

    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x03);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        80,
        "mode 03h is eighty columns"
    );
    assert_eq!(c.mem.read_u8(DATA, ROWS), 24, "and twenty-five rows");

    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x00);
        c.regs.set_byte(Reg8::Al, 0x13);
    });
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        80,
        "a mode this slice does not have leaves the geometry as it was"
    );
}

/// S6 — The page is laid out the way the geometry says: a row is the
/// geometry's columns, a cell is two bytes, a page is four kilobytes, and
/// the page a service writes is the one its `BH` names.
#[test]
fn s6_the_page_is_laid_out_the_way_the_geometry_says() {
    let c = bios_twice(
        |c| {
            // The cursor of page 1, at the row 1 and the column 2.
            c.regs.set_byte(Reg8::Ah, 0x02);
            c.regs.set_byte(Reg8::Bh, 1);
            c.regs.set_byte(Reg8::Dh, 1);
            c.regs.set_byte(Reg8::Dl, 2);
        },
        |c| {
            // And a character there.
            c.regs.set_byte(Reg8::Ah, 0x09);
            c.regs.set_byte(Reg8::Al, b'X');
            c.regs.set_byte(Reg8::Bh, 1);
            c.regs.set_byte(Reg8::Bl, 0x1E);
            c.regs.set_word(Reg16::Cx, 1);
        },
    );

    // A row is eighty columns, a cell is two bytes: the cell at the row 1
    // and the column 2 is the 82nd cell of the page.
    let index = 80 + 2;
    assert_eq!(
        character(&c, 1, index),
        b'X',
        "the character is on the page BH names"
    );
    assert_eq!(
        attribute(&c, 1, index),
        0x1E,
        "and the attribute is the second byte of the cell"
    );
    assert_eq!(
        c.mem.read_u8(VIDEO, 0x1000 + index * 2),
        b'X',
        "the pages are four kilobytes apart"
    );
    assert_eq!(
        character(&c, 0, index),
        0,
        "and the page BH did not name is untouched"
    );
}

/// S7 — The services write and read the page.
#[test]
fn s7_the_services_write_and_read_the_page() {
    // 09h and 0Ah write at the cursor and leave the cursor where it was.
    let c = bios_twice(
        |c| {
            c.regs.set_byte(Reg8::Ah, 0x09);
            c.regs.set_byte(Reg8::Al, b'K');
            c.regs.set_byte(Reg8::Bl, 0x1E);
            c.regs.set_word(Reg16::Cx, 2);
        },
        |c| {
            // 08h reads the character and the attribute from there.
            c.regs.set_byte(Reg8::Ah, 0x08);
            c.regs.set_byte(Reg8::Bh, 0);
        },
    );
    assert_eq!(character(&c, 0, 0), b'K', "09h wrote at the cursor");
    assert_eq!(character(&c, 0, 1), b'K', "CX times");
    assert_eq!(cursor(&c, 0), (0, 0), "and left the cursor where it was");
    assert_eq!(c.regs.byte(Reg8::Al), b'K', "08h read the character");
    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "and the attribute");

    // 0Eh writes at the cursor and advances it.
    let c = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x0E);
        c.regs.set_byte(Reg8::Al, b'L');
    });
    assert_eq!(character(&c, 0, 0), b'L', "0Eh wrote at the cursor");
    assert_eq!(cursor(&c, 0), (0, 1), "and advanced it");

    // 0Eh scrolls the page when the cursor reaches the bottom.
    let c = bios_twice(
        |c| {
            // The cursor of the last row, at the last column.
            c.regs.set_byte(Reg8::Ah, 0x02);
            c.regs.set_byte(Reg8::Dh, 24);
            c.regs.set_byte(Reg8::Dl, 79);
        },
        |c| {
            c.regs.set_byte(Reg8::Ah, 0x0E);
            c.regs.set_byte(Reg8::Al, b'M');
        },
    );
    assert_eq!(
        character(&c, 0, 23 * 80 + 79),
        b'M',
        "the character was written at the cursor, and the scroll took it up"
    );
    assert_eq!(
        cursor(&c, 0),
        (24, 0),
        "the cursor is at the last row, at the first column"
    );

    let c = bios_twice(
        |c| {
            // A page with two rows of its own, and the cursor at the end of
            // the last one.
            c.mem.write_u8(VIDEO, 0x0000, b'1');
            c.mem.write_u8(VIDEO, 0x00A0, b'2');
            c.regs.set_byte(Reg8::Ah, 0x02);
            c.regs.set_byte(Reg8::Dh, 24);
            c.regs.set_byte(Reg8::Dl, 79);
        },
        |c| {
            c.regs.set_byte(Reg8::Ah, 0x0E);
            c.regs.set_byte(Reg8::Al, b'3');
        },
    );
    assert_eq!(
        character(&c, 0, 0),
        b'2',
        "the page scrolled up: the second row is the first one now"
    );
    assert_eq!(
        character(&c, 0, 24 * 80),
        b' ',
        "and the row the scroll brought in is blank"
    );

    // 13h writes the string where it is told to, and moves the cursor only
    // when the write mode says so.
    let c = bios(|c| {
        c.mem.write_u8(CODE, 0x0200, b'h');
        c.mem.write_u8(CODE, 0x0201, b'i');
        c.regs.set_byte(Reg8::Ah, 0x13);
        c.regs.set_byte(Reg8::Al, 0x01);
        c.regs.set_byte(Reg8::Bl, 0x1E);
        c.regs.set_word(Reg16::Cx, 2);
        c.regs.set_byte(Reg8::Dl, 5);
        c.regs.set_byte(Reg8::Dh, 1);
        c.regs.set_word(Reg16::Bp, 0x0200);
    });
    assert_eq!(character(&c, 0, 85), b'h', "13h wrote the string");
    assert_eq!(character(&c, 0, 86), b'i', "all of it");
    assert_eq!(
        cursor(&c, 0),
        (1, 7),
        "and the cursor moved: the write mode said so"
    );

    // 06h and 07h scroll the window they are given, and fill the space
    // with the attribute in BH.
    let c = bios(|c| {
        for row in 0..3u16 {
            c.mem.write_u8(VIDEO, row * 160, b'A' + row as u8);
        }
        c.regs.set_byte(Reg8::Ah, 0x06);
        c.regs.set_byte(Reg8::Al, 2);
        c.regs.set_byte(Reg8::Bh, 0x1E);
        c.regs.set_byte(Reg8::Ch, 0);
        c.regs.set_byte(Reg8::Cl, 0);
        c.regs.set_byte(Reg8::Dh, 2);
        c.regs.set_byte(Reg8::Dl, 79);
    });
    assert_eq!(character(&c, 0, 0), b'C', "the window scrolled up by two");
    assert_eq!(
        attribute(&c, 0, 160),
        0x1E,
        "and the space the scroll left carries the attribute in BH"
    );
}

/// A machine with no geometry has no page to write on, and the service does
/// nothing rather than faulting.
#[test]
fn a_machine_with_no_geometry_has_no_page_to_write_on() {
    let mut cpu = Cpu::default();
    cpu.mem
        .load_at(0x10000, &[0xCD, 0x10, 0xF4]) // int 10h; hlt
        .expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, CODE);
    cpu.regs.set_byte(Reg8::Ah, 0x0E);
    cpu.regs.set_byte(Reg8::Al, b'X');
    cpu.step().expect("the call runs");
    assert_eq!(
        cpu.mem.read_u8(VIDEO, 0x0000),
        0,
        "nothing was written: the machine has no page"
    );
}
