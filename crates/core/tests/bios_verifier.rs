//! Independent verification probes for the video BIOS.
//!
//! Written from the spec (`odd/tasks/video-bios.md`, S1-S11) and from the
//! reference's own contract, not from the implementer's tests.

use emo8067_core::Cpu;
use emo8067_core::StopReason;
use emo8067_core::flags;
use emo8067_core::regs::{Reg8, Reg16, Seg};

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;
/// The eight cursors, one pair a page.
const CURSORS: u16 = 0x0050;

/// The segment the text pages live in.
const VIDEO: u16 = 0xB800;
/// The segment the test programs run in.
const CODE: u16 = 0x1000;
/// Another segment, for a guest's own handler.
const HANDLER: u16 = 0x2000;

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

/// The guest writes the columns word itself: a guest program stores a
/// column count at 0040:004A and calls `INT 10h`.
///
/// The program is
/// ```text
///   xor  ax, ax                 ; 31 C0
///   mov  ds, ax                 ; 8E D8
///   mov  word [044Ah], value    ; C7 06 4A 04 lo hi
///   mov  ax, 0958h              ; B8 58 09   (AH = 09h, AL = 'X')
///   mov  cx, 1                  ; B9 01 00
///   int  10h                    ; CD 10
///   hlt                         ; F4
/// ```
/// `DS = 0` and the offset `044Ah` is physical `044Ah`, which is
/// `0040:004A`.
fn guest_that_sets_the_columns(value: u16) -> Vec<u8> {
    vec![
        0x31,
        0xC0, // xor ax, ax
        0x8E,
        0xD8, // mov ds, ax
        0xC7,
        0x06,
        0x4A,
        0x04, // mov word [044Ah], value
        value.to_le_bytes()[0],
        value.to_le_bytes()[1],
        0xB8,
        0x58,
        0x09, // mov ax, 0958h
        0xB9,
        0x01,
        0x00, // mov cx, 1
        0xCD,
        0x10, // int 10h
        0xF4, // hlt
    ]
}

// ---------------------------------------------------------------------------
// S1 -- the `INT` is the 8086's `INT`.
// ---------------------------------------------------------------------------

/// S1 -- The `INT` pushes FLAGS, CS and IP, clears TF and IF, and loads
/// `CS:IP` from the interrupt vector table.
///
/// The probe puts its *own* entry in the table, so the landing place is the
/// table's and not the BIOS's.

#[test]
fn s1_the_int_pushes_the_frame_clears_tf_and_if_and_vectors_through_the_table() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // The table's own entry for `10h`: 2000:0300.
    c.mem.write_u16(0x0000, 0x0040, 0x0300);
    c.mem.write_u16(0x0000, 0x0042, HANDLER);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.flags.set(flags::TF, true);
    c.regs.flags.set(flags::IF, true);
    let before = c.regs.flags.0;

    c.step().expect("the interrupt runs");

    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x00FA,
        "the stack is six bytes lower"
    );
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FE),
        before | flags::UNDEFINED,
        "FLAGS is on the top of the stack, with the undefined bits read as one"
    );
    assert_eq!(c.mem.read_u16(CODE, 0x00FC), CODE, "then CS");
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FA),
        0x0002,
        "then the instruction after the call"
    );
    assert_eq!(
        c.regs.segment(Seg::Cs),
        HANDLER,
        "the machine is at the table"
    );
    assert_eq!(c.regs.ip, 0x0300, "at the offset the table holds");
    assert!(!c.regs.flags.get(flags::TF), "TF is cleared");
    assert!(!c.regs.flags.get(flags::IF), "IF is cleared");
    assert_eq!(
        c.stop_reason,
        StopReason::Interrupt(0x10),
        "and the interrupt is reported as taken"
    );
}

/// S1 -- The BIOS does not answer the call in place: a machine whose vector
/// table points somewhere else runs whatever is there, and the BIOS's
/// service is not reached at all.

#[test]
fn s1_a_machine_whose_vector_table_points_elsewhere_runs_what_is_there() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // The guest's own handler, and the guest's own entry in the table.
    c.mem.write_u16(0x0000, 0x0040, 0x0100);
    c.mem.write_u16(0x0000, 0x0042, HANDLER);
    // mov ax, 1234h
    c.mem.write_u8(HANDLER, 0x0100, 0xB8);
    c.mem.write_u8(HANDLER, 0x0101, 0x34);
    c.mem.write_u8(HANDLER, 0x0102, 0x12);
    // The guest asks the BIOS for a character, so that a service that ran in
    // place would leave a mark.
    c.regs.set_byte(Reg8::Ah, 0x0E);
    c.regs.set_byte(Reg8::Al, b'X');

    c.step().expect("the interrupt runs");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        HANDLER,
        "the machine is at the table"
    );

    c.step().expect("the guest's handler runs");
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "the machine ran what was there"
    );
    assert_eq!(
        character(&c, 0, 0),
        0,
        "and the BIOS's service did not run: the answer was not in place"
    );
}

/// S1 -- Every other vector is untouched, and a vector the BIOS has no
/// service for is taken and returned like any other.

#[test]
fn s1_every_other_vector_is_untouched() {
    // INT 21h: the table holds nothing for it, and the machine vectors
    // through the table the way it always did.
    let mut c = machine(&[0xCD, 0x21]);
    c.step().expect("the interrupt runs");
    assert_eq!(c.stop_reason, StopReason::Interrupt(0x21), "it was taken");
    assert_eq!(c.regs.segment(Seg::Cs), 0x0000, "and it vectored");
    assert_eq!(c.regs.ip, 0x0000, "to the entry the table holds");

    // INT 10h with a function the BIOS has no service for: the call is
    // answered, nothing is changed, and the program carries on.
    let mut c = machine(&[
        0xB8, 0x0C, 0x00, // mov ax, 000Ch
        0xCD, 0x10, // int 10h
        0xBB, 0x34, 0x12, // mov bx, 1234h
        0xF4, // hlt
    ]);
    run(&mut c);
    assert_eq!(c.regs.word(Reg16::Bx), 0x1234, "the program carried on");
    assert_eq!(character(&c, 0, 0), 0, "and nothing was written");
}

// ---------------------------------------------------------------------------
// S2 -- the thirteen services, with the reference's own contracts.
// ---------------------------------------------------------------------------

/// S2 -- `AH = 13h`, write mode bit 1: the string carries attributes, and
/// the attributes are not counted in `CX`.

#[test]
fn s2_write_string_with_attributes_writes_both_bytes_of_every_cell() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // ES:BP holds "a", 1Eh, "b", 4Fh.
    c.mem.write_u8(CODE, 0x0200, b'a');
    c.mem.write_u8(CODE, 0x0201, 0x1E);
    c.mem.write_u8(CODE, 0x0202, b'b');
    c.mem.write_u8(CODE, 0x0203, 0x4F);
    c.regs.set_byte(Reg8::Ah, 0x13);
    c.regs.set_byte(Reg8::Al, 0x02); // bit 1: the string has attributes
    c.regs.set_byte(Reg8::Bh, 0);
    c.regs.set_byte(Reg8::Bl, 0x07); // ignored: the string has its own
    c.regs.set_word(Reg16::Cx, 2); // two characters, not four bytes
    c.regs.set_byte(Reg8::Dl, 10);
    c.regs.set_byte(Reg8::Dh, 3);
    c.regs.set_word(Reg16::Bp, 0x0200);
    run(&mut c);

    let index = 3 * 80 + 10;
    assert_eq!(
        character(&c, 0, index),
        b'a',
        "the first character is there"
    );
    assert_eq!(
        attribute(&c, 0, index),
        0x1E,
        "with the attribute that came with it"
    );
    assert_eq!(
        character(&c, 0, index + 1),
        b'b',
        "and the second character is there"
    );
    assert_eq!(
        attribute(&c, 0, index + 1),
        0x4F,
        "with its own attribute, not the first one's"
    );
    assert_eq!(
        character(&c, 0, index + 2),
        0,
        "and the string stops after the characters CX counts"
    );
    assert_eq!(
        cursor(&c, 0),
        (0, 0),
        "and the cursor did not move: bit 0 of the write mode is clear"
    );
}

/// S7 -- `AH = 06h` scrolls the window up and moves every cell of it: a
/// scroll that repeats one cell cannot pass for a scroll that moves them
/// all.

#[test]
fn s7_scroll_up_moves_every_cell_of_the_window() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // Three rows of eighty cells, each cell with a character of its own.
    for row in 0..3u16 {
        for column in 0..80u16 {
            c.mem.write_u8(
                VIDEO,
                (row * 80 + column) * 2,
                b'A' + row as u8 + column as u8,
            );
            c.mem.write_u8(VIDEO, (row * 80 + column) * 2 + 1, 0x1E);
        }
    }
    c.regs.set_byte(Reg8::Ah, 0x06);
    c.regs.set_byte(Reg8::Al, 1); // one line
    c.regs.set_byte(Reg8::Bh, 0x1E);
    c.regs.set_byte(Reg8::Ch, 0);
    c.regs.set_byte(Reg8::Cl, 0);
    c.regs.set_byte(Reg8::Dh, 2);
    c.regs.set_byte(Reg8::Dl, 79);
    run(&mut c);

    // One line up: the second row is the first row now, the third row is the
    // second, and the last row is the new one.
    for column in 0..80u16 {
        assert_eq!(
            character(&c, 0, column),
            b'B' + column as u8,
            "the row moved whole: column {column}"
        );
        assert_eq!(
            character(&c, 0, 80 + column),
            b'C' + column as u8,
            "and the row below it: column {column}"
        );
    }
    // The row the scroll brought in is blank, with the attribute in BH.
    for column in 0..80u16 {
        assert_eq!(character(&c, 0, 160 + column), b' ', "the new row is blank");
        assert_eq!(
            attribute(&c, 0, 160 + column),
            0x1E,
            "with the attribute in BH"
        );
    }
}

/// S7 -- `AH = 07h` scrolls the window down, and `AL = 00h` clears it.

#[test]
fn s7_scroll_down_moves_every_cell_of_the_window() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    for row in 0..3u16 {
        for column in 0..80u16 {
            c.mem.write_u8(
                VIDEO,
                (row * 80 + column) * 2,
                b'A' + row as u8 + column as u8,
            );
        }
    }
    c.regs.set_byte(Reg8::Ah, 0x07);
    c.regs.set_byte(Reg8::Al, 1);
    c.regs.set_byte(Reg8::Bh, 0x1E);
    c.regs.set_byte(Reg8::Ch, 0);
    c.regs.set_byte(Reg8::Cl, 0);
    c.regs.set_byte(Reg8::Dh, 2);
    c.regs.set_byte(Reg8::Dl, 79);
    run(&mut c);

    for column in 0..80u16 {
        assert_eq!(
            character(&c, 0, 80 + column),
            b'A' + column as u8,
            "the first row is the second row now: column {column}"
        );
    }
    assert_eq!(character(&c, 0, 0), b' ', "and the top row is blank");

    // AL = 00h clears the whole window.
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.mem.write_u8(VIDEO, 0x0000, b'Z');
    c.regs.set_byte(Reg8::Ah, 0x06);
    c.regs.set_byte(Reg8::Al, 0);
    c.regs.set_byte(Reg8::Bh, 0x1E);
    c.regs.set_byte(Reg8::Ch, 0);
    c.regs.set_byte(Reg8::Cl, 0);
    c.regs.set_byte(Reg8::Dh, 2);
    c.regs.set_byte(Reg8::Dl, 79);
    run(&mut c);
    assert_eq!(character(&c, 0, 0), b' ', "zero lines clears the window");
    assert_eq!(attribute(&c, 0, 0), 0x1E, "with the attribute in BH");
}

/// S11 -- The service returns the way a handler returns: it pops the frame
/// the `INT` pushed and carries on at the instruction after the call.

#[test]
fn s11_the_service_returns_the_way_a_handler_returns() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.flags.set(flags::IF, true);
    let sp = c.regs.word(Reg16::Sp);
    let flags_before = c.regs.flags.0;

    c.step().expect("the interrupt runs");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0xF400,
        "the machine is at the entry"
    );
    assert_eq!(c.regs.ip, 0x0190, "at the offset the vector names");
    assert_eq!(c.call_depth, 1, "and the interrupt went in as a call");

    c.step().expect("the service runs");
    assert_eq!(c.regs.word(Reg16::Sp), sp, "the stack is where it was");
    assert_eq!(
        c.regs.flags.0, flags_before,
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

/// Finding 1 -- the guest's column count overflows the `u16` arithmetic the
/// BIOS does with it, and the emulator process panics.

#[test]
fn finding_the_guests_column_count_must_not_panic_the_emulator() {
    let code = guest_that_sets_the_columns(0xFFFF);
    let mut c = machine(&code);
    run(&mut c);
    assert_eq!(character(&c, 0, 0), b'X', "the service answered the call");
}

/// Finding 2 -- a column count of exactly 256 overflows the teletype's own
/// increment.

#[test]
fn finding_two_hundred_and_fifty_six_columns_must_not_panic_the_teletype() {
    // The guest stores 0100h in the columns word, puts the cursor on the
    // last column, and asks for a character.
    let code = [
        0x31, 0xC0, // xor ax, ax
        0x8E, 0xD8, // mov ds, ax
        0xC7, 0x06, 0x4A, 0x04, 0x00, 0x01, // mov word [044Ah], 0100h
        0xB4, 0x02, // mov ah, 02h
        0xB6, 0x00, // mov dh, 0
        0xB2, 0xFF, // mov dl, 0FFh
        0xCD, 0x10, // int 10h: set the cursor
        0xB4, 0x0E, // mov ah, 0Eh
        0xB0, 0x58, // mov al, 'X'
        0xCD, 0x10, // int 10h: the character
        0xF4, // hlt
    ];
    let mut c = machine(&code);
    run(&mut c);
    assert_eq!(character(&c, 0, 255), b'X', "the character was written");
}

/// Finding 2, second form: the rows word, at 256 rows, overflows the same
/// increment.

#[test]
fn finding_two_hundred_and_fifty_six_rows_must_not_panic_the_teletype() {
    // The guest stores 0FFh in the rows word at 0040:0084 -- physical
    // 0484h -- and puts the cursor on the last row.
    let code = [
        0x31, 0xC0, // xor ax, ax
        0x8E, 0xD8, // mov ds, ax
        0xC7, 0x06, 0x84, 0x04, 0xFF, 0x00, // mov word [0484h], 00FFh
        0xB4, 0x02, // mov ah, 02h
        0xB6, 0xFF, // mov dh, 0FFh
        0xB2, 0x4F, // mov dl, 4Fh
        0xCD, 0x10, // int 10h: set the cursor
        0xB4, 0x0E, // mov ah, 0Eh
        0xB0, 0x58, // mov al, 'X'
        0xCD, 0x10, // int 10h: the character
        0xF4, // hlt
    ];
    let mut c = machine(&code);
    run(&mut c);
}

/// A machine whose geometry is zero has no page to write on, and the
/// service says so instead of faulting.

#[test]
fn a_machine_with_no_geometry_has_no_page_to_write_on_and_does_not_fault() {
    let mut c = machine(&[
        0x31, 0xC0, // xor ax, ax
        0x8E, 0xD8, // mov ds, ax
        0xC7, 0x06, 0x4A, 0x04, 0x00, 0x00, // mov word [044Ah], 0000h
        0xB4, 0x0E, // mov ah, 0Eh
        0xB0, 0x58, // mov al, 'X'
        0xCD, 0x10, // int 10h
        0xF4, // hlt
    ]);
    run(&mut c);
    assert_eq!(
        character(&c, 0, 0),
        0,
        "nothing was written: the machine has no page"
    );
    assert_eq!(c.stop_reason, StopReason::Halted, "and the run ended");
}

/// The writer's own "a machine with no geometry has no page to write on"
/// case takes one step: the interrupt. The service is never reached, so the
/// case holds for any machine.
///
/// This probe shows what that one step leaves: the machine is at the vector
/// table's entry, which is not the BIOS's.

#[test]
fn the_no_geometry_case_never_reaches_the_service() {
    // The machine the writer's case uses: the derived `Default`, which does
    // not install the BIOS.
    let mut c = Cpu::default();
    assert_eq!(
        c.mem.read_u16(0x0000, 0x0040),
        0,
        "the derived Default installs no vector: there is no BIOS in it"
    );
    c.mem
        .load_at(0x10000, &[0xCD, 0x10, 0xF4])
        .expect("the program loads");
    c.regs.set_segment(Seg::Cs, CODE);
    c.regs.set_byte(Reg8::Ah, 0x0E);
    c.regs.set_byte(Reg8::Al, b'X');
    c.step().expect("the call runs");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0x0000,
        "the machine is at the empty table: the service was never reached"
    );
    assert_eq!(c.regs.ip, 0x0000, "at 0000:0000");
}

/// S3 -- the BIOS keeps no state of its own: a service is a function of the
/// machine's memory and its registers, and of nothing else.
///
/// The cursor is put in the data area by hand -- the guest can write there --
/// and the service reads it back from there.

#[test]
fn s3_a_service_is_a_function_of_the_memory_and_the_registers() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    // The guest writes the cursor of page 3 into the data area itself.
    c.mem.write_u16(DATA, CURSORS + 6, (11 << 8) | 17);
    c.regs.set_byte(Reg8::Ah, 0x03);
    c.regs.set_byte(Reg8::Bh, 3);
    run(&mut c);
    assert_eq!(c.regs.byte(Reg8::Dh), 11, "the row comes from the memory");
    assert_eq!(c.regs.byte(Reg8::Dl), 17, "and the column with it");

    // And the same machine, with the memory changed, answers differently:
    // there is no copy of the cursor anywhere else.
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.mem.write_u16(DATA, CURSORS + 6, (2 << 8) | 3);
    c.regs.set_byte(Reg8::Ah, 0x03);
    c.regs.set_byte(Reg8::Bh, 3);
    run(&mut c);
    assert_eq!(c.regs.byte(Reg8::Dh), 2, "the row follows the memory");
    assert_eq!(c.regs.byte(Reg8::Dl), 3, "and the column with it");
}
