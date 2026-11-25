//! The verifier's own probes for the BIOS keyboard unit, S12-S21, and for
//! the video BIOS the unit shares its code with, S1-S11.
//!
//! These are written from the specs, not from the implementer's tests: the
//! oracle is the IBM PC's published set 1 scan code table and the machine's
//! own memory, never the BIOS's internals.

use emo8067_core::Cpu;
use emo8067_core::bios;
use emo8067_core::flags;
use emo8067_core::regs::{Reg8, Reg16, Seg};

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;
/// The offset of the keyboard buffer's head.
const HEAD: u16 = 0x001A;
/// The offset of the keyboard buffer's tail.
const TAIL: u16 = 0x001C;
/// The offset of the keyboard buffer's first entry.
const BUFFER: u16 = 0x001E;
/// The offset of the keyboard buffer's size, in bytes.
const SIZE: u16 = 0x0030;
/// The offset the ring's first entry is at, and the one past its last.
const START: u16 = 0x001E;
const END: u16 = 0x0030;
/// The segment the test programs run in.
const CODE: u16 = 0x1000;
/// The segment the text pages live in.
const VIDEO: u16 = 0xB800;
/// The number of columns on screen.
const COLUMNS: u16 = 0x004A;

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

/// The keystroke at the head of the buffer: the ASCII character and the
/// scan code.
fn at_head(c: &Cpu) -> (u8, u8) {
    let head = c.mem.read_u16(DATA, HEAD);
    (c.mem.read_u8(DATA, head), c.mem.read_u8(DATA, head + 1))
}

/// S12 - the machine installs the keyboard's vector, and the ROM's own byte
/// at the entry is the `IRET` that comes out of the service.
#[test]
fn s12_the_vector_is_installed_and_the_roms_byte_is_the_iret() {
    let c = Cpu::new();
    assert_eq!(
        c.mem.read_u16(0x0000, 0x0058),
        0x01C0,
        "the vector's offset is the one the reference's map gives"
    );
    assert_eq!(
        c.mem.read_u16(0x0000, 0x005A),
        0xF400,
        "and its segment is the BIOS's own"
    );
    assert_eq!(
        c.mem.read_u8(0xF400, 0x01C0),
        0xCF,
        "the ROM's own byte at the entry is the IRET"
    );
}

/// S12 - the `INT 16h` is the 8086's `INT`: it pushes FLAGS, CS and IP, it
/// clears TF and IF, and it loads `CS:IP` from the interrupt vector table.
#[test]
fn s12_the_int_pushes_the_frame_and_clears_tf_and_if() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.regs.flags.set(flags::TF, true);
    c.regs.flags.set(flags::IF, true);
    let before = c.regs.flags.0;
    c.step().expect("the interrupt runs");

    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0100 - 6,
        "the stack is six bytes lower: three words were pushed"
    );
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FE),
        before | flags::UNDEFINED,
        "the flags word is on the top of the stack"
    );
    assert_eq!(c.mem.read_u16(CODE, 0x00FC), CODE, "then the code segment");
    assert_eq!(
        c.mem.read_u16(CODE, 0x00FA),
        0x0002,
        "then the return address"
    );
    assert!(!c.regs.flags.get(flags::TF), "TF is cleared");
    assert!(!c.regs.flags.get(flags::IF), "IF is cleared");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0xF400,
        "the machine is at the BIOS"
    );
    assert_eq!(c.regs.ip, 0x01C0, "at the keyboard's own entry");
}

/// S12 - a machine whose vector table points somewhere else runs whatever
/// is there, the way the bare chip does.
#[test]
fn s12_a_vector_that_points_elsewhere_runs_what_is_there() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    // The guest's own handler: mov ax, 1234h; iret.
    c.mem
        .load_at(0x20000, &[0xB8, 0x34, 0x12, 0xCF])
        .expect("the handler loads");
    c.mem.write_u16(0x0000, 0x0058, 0x0000);
    c.mem.write_u16(0x0000, 0x005A, 0x2000);
    c.push_key(0x1E, b'A');
    run(&mut c);

    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "the guest's own handler ran: the native service did not answer"
    );
    assert_eq!(
        c.mem.read_u16(DATA, HEAD),
        START,
        "and the keyboard buffer was left alone"
    );
    assert_eq!(
        c.regs.ip, 0x0003,
        "and the machine carried on past the call"
    );
}

/// S13 - `AH = 00h` returns the pair and takes the keystroke out.
#[test]
fn s13_the_get_returns_the_pair_and_removes_the_keystroke() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.push_key(0x1E, b'A');
    c.regs.set_byte(Reg8::Ah, 0x00);
    run(&mut c);

    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "AH = the BIOS scan code");
    assert_eq!(c.regs.byte(Reg8::Al), b'A', "AL = the ASCII character");
    assert_eq!(
        c.mem.read_u16(DATA, HEAD),
        START + 2,
        "and the keystroke is removed from the buffer"
    );
    assert_eq!(
        c.regs.ip, 0x0003,
        "and the machine carried on past the call"
    );
}

/// S13 - `AH = 01h` returns the pair without removing the keystroke.
#[test]
fn s13_the_check_returns_the_pair_and_keeps_the_keystroke() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.push_key(0x1E, b'A');
    c.regs.set_byte(Reg8::Ah, 0x01);
    run(&mut c);

    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "AH = the BIOS scan code");
    assert_eq!(c.regs.byte(Reg8::Al), b'A', "AL = the ASCII character");
    assert!(
        !c.regs.flags.get(flags::ZF),
        "ZF = 0: a keystroke is available"
    );
    assert_eq!(
        c.mem.read_u16(DATA, HEAD),
        START,
        "and the keystroke is NOT removed from the buffer"
    );
}

/// S17 - `AH = 01h` with an empty buffer sets ZF = 1 and returns.
#[test]
fn s17_the_check_on_an_empty_buffer_sets_zf_and_returns() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.regs.set_byte(Reg8::Ah, 0x01);
    c.step().expect("the interrupt runs");
    c.step().expect("the service runs");
    assert!(
        c.regs.flags.get(flags::ZF),
        "ZF = 1: no keystroke is available"
    );
    assert_eq!(
        c.regs.ip, 0x0002,
        "and the machine carried on past the call: it did not wait"
    );
}

/// S13 - no other `AH` is answered: the reference documents no other
/// `INT 16h` function.
#[test]
fn s13_no_other_function_is_answered() {
    for function in [0x02u8, 0x05, 0x10, 0xFF] {
        let mut c = machine(&[0xCD, 0x16, 0xF4]);
        c.push_key(0x1E, b'A');
        c.regs.set_byte(Reg8::Ah, function);
        run(&mut c);
        assert_eq!(
            c.mem.read_u16(DATA, HEAD),
            START,
            "AH = {function:#04x} takes nothing out of the buffer"
        );
        assert_eq!(
            c.regs.ip, 0x0003,
            "AH = {function:#04x} carries on past the call"
        );
    }
}

/// S14 - the buffer is the BIOS data area, at the IBM PC's conventional
/// offsets.
#[test]
fn s14_the_buffer_is_the_bios_data_area() {
    let c = Cpu::new();
    assert_eq!(
        c.mem.read_u16(DATA, HEAD),
        START,
        "the head is at 0040:001A and it points at the ring's first entry"
    );
    assert_eq!(
        c.mem.read_u16(DATA, TAIL),
        START,
        "the tail is at 0040:001C and it points at it too: the buffer is empty"
    );
    assert_eq!(
        c.mem.read_u16(DATA, SIZE),
        0x12,
        "the size is at 0040:0030 and it is eighteen bytes: nine entries"
    );

    // The body is two bytes an entry: the ASCII character and the scan
    // code, and the head and the tail are the ABSOLUTE offsets of the
    // entries to read and to write.
    let mut c = Cpu::new();
    c.push_key(0x1E, b'A');
    assert_eq!(c.mem.read_u8(DATA, BUFFER), b'A', "the character is first");
    assert_eq!(c.mem.read_u8(DATA, BUFFER + 1), 0x1E, "then the scan code");
    assert_eq!(
        c.mem.read_u16(DATA, TAIL),
        START + 2,
        "and the tail moved two bytes on"
    );
    assert_eq!(
        c.mem.read_u16(DATA, SIZE),
        0x12,
        "and the size word is untouched: it is outside the body"
    );
}

/// S14 - one slot stays free: the ring holds nine slots and the ninth
/// keystroke is dropped.
#[test]
fn s14_one_slot_stays_free_and_the_ninth_is_dropped() {
    let mut c = Cpu::new();
    for _ in 0..8 {
        c.push_key(0x1E, b'A');
    }
    assert_eq!(c.keys_waiting(), 8, "the ring holds eight keystrokes");
    assert_eq!(
        c.mem.read_u16(DATA, TAIL),
        START + 16,
        "and the tail is at the eighth slot"
    );
    c.push_key(0x1F, b'B');
    assert_eq!(c.keys_waiting(), 8, "the ninth keystroke is dropped");
    assert_eq!(
        c.mem.read_u8(DATA, START + 16),
        0,
        "and the free slot holds nothing"
    );
}

/// S14 - the data area is guest-writable memory, and the head and the tail
/// are validated before they are trusted: a guest that scribbles there sees
/// an empty buffer, never a panic.
#[test]
fn s14_a_guest_that_scribbles_the_ends_sees_an_empty_buffer() {
    for scribble in [0xFFFFu16, 0x0000, 0x0001, 0x001F, 0x0030, 0x0100] {
        let mut c = machine(&[0xCD, 0x16, 0xF4]);
        c.mem.write_u16(DATA, HEAD, scribble);
        c.mem.write_u16(DATA, TAIL, scribble);
        c.regs.set_byte(Reg8::Ah, 0x01);
        run(&mut c);
        assert!(
            c.regs.flags.get(flags::ZF),
            "the ends at {scribble:#06x} are not offsets of the ring: the buffer is empty"
        );
        assert_eq!(c.regs.ip, 0x0003, "and the machine carried on");
    }
    // A valid head with an invalid tail is an empty buffer too.
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.mem.write_u16(DATA, HEAD, START);
    c.mem.write_u16(DATA, TAIL, 0xFFFF);
    c.regs.set_byte(Reg8::Ah, 0x01);
    run(&mut c);
    assert!(
        c.regs.flags.get(flags::ZF),
        "an invalid tail empties the buffer"
    );
    // And the door does not panic on a scribbled buffer either.
    let mut c = Cpu::new();
    c.mem.write_u16(DATA, HEAD, 0xFFFF);
    c.mem.write_u16(DATA, TAIL, 0xFFFF);
    c.push_key(0x1E, b'A');
    assert_eq!(c.keys_waiting(), 0, "and the keystroke has nowhere to go");
}

/// S14 - the size word is the guest's own too: a size that would take the
/// ring's end past the segment empties the buffer, and the emulator lives.
#[test]
fn s14_a_guest_that_scribbles_the_size_word_sees_an_empty_buffer() {
    for scribble in [0x0000u16, 0x0001, 0x0012, 0x0014, 0xFFE2, 0xFFFE, 0xFFFF] {
        let mut c = machine(&[0xCD, 0x16, 0xF4]);
        c.mem.write_u16(DATA, SIZE, scribble);
        c.regs.set_byte(Reg8::Ah, 0x01);
        run(&mut c);
        assert!(
            c.regs.flags.get(flags::ZF),
            "the size {scribble:#06x} is not a ring's size: the buffer is empty"
        );
        assert_eq!(c.regs.ip, 0x0003, "and the machine carried on");
    }
}

/// S14 - the size word is at `0040:0030` and the buffer's body is
/// `0040:001E`-`:002F`. The size word is OUTSIDE the body, so the ring can
/// never clobber it, and the whole layout lives in `0040:001A`-`:0031`.
#[test]
fn s14_the_size_word_is_outside_the_body_and_the_ring_never_clobbers_it() {
    let mut c = Cpu::new();
    assert_eq!(
        c.mem.read_u16(DATA, SIZE),
        0x12,
        "the size is eighteen bytes"
    );
    for index in 0..8 {
        c.push_key(0x1E, b'A');
        assert_eq!(
            c.mem.read_u16(DATA, SIZE),
            0x12,
            "the size word is intact after {} keystrokes",
            index + 1
        );
    }
    assert_eq!(
        c.mem.read_u16(DATA, END),
        0x12,
        "and the size word is where the ring ends: the ring wrote nothing there"
    );
    assert_eq!(c.mem.read_u8(DATA, END + 1), 0, "nor past it");
}

/// S15 - the scan code is the IBM PC's set 1, character by character.
#[test]
fn s15_the_scan_code_of_every_character_is_the_ibm_set_1() {
    // The published set 1 table: the digits, the letters and the
    // punctuation the BIOS module maps.
    let published: [(u8, u8); 48] = [
        (b'1', 0x02),
        (b'2', 0x03),
        (b'3', 0x04),
        (b'4', 0x05),
        (b'5', 0x06),
        (b'6', 0x07),
        (b'7', 0x08),
        (b'8', 0x09),
        (b'9', 0x0A),
        (b'0', 0x0B),
        (b'-', 0x0C),
        (b'=', 0x0D),
        (b'q', 0x10),
        (b'w', 0x11),
        (b'e', 0x12),
        (b'r', 0x13),
        (b't', 0x14),
        (b'y', 0x15),
        (b'u', 0x16),
        (b'i', 0x17),
        (b'o', 0x18),
        (b'p', 0x19),
        (b'[', 0x1A),
        (b']', 0x1B),
        (b'a', 0x1E),
        (b's', 0x1F),
        (b'd', 0x20),
        (b'f', 0x21),
        (b'g', 0x22),
        (b'h', 0x23),
        (b'j', 0x24),
        (b'k', 0x25),
        (b'l', 0x26),
        (b';', 0x27),
        (b'\'', 0x28),
        (b'`', 0x29),
        (b'\\', 0x2B),
        (b'z', 0x2C),
        (b'x', 0x2D),
        (b'c', 0x2E),
        (b'v', 0x2F),
        (b'b', 0x30),
        (b'n', 0x31),
        (b'm', 0x32),
        (b',', 0x33),
        (b'.', 0x34),
        (b'/', 0x35),
        (b' ', 0x39),
    ];
    for (character, code) in published {
        assert_eq!(
            bios::scan_code(character),
            code,
            "the set 1 code of {:?} is {code:#04x}",
            character as char
        );
        // The upper case letter has the same code as the lower case one.
        if character.is_ascii_alphabetic() {
            let upper = character.to_ascii_uppercase();
            assert_eq!(
                bios::scan_code(upper),
                code,
                "the set 1 code of {:?} is {code:#04x}",
                upper as char
            );
        }
    }
    // A character the shift made has the code of the KEY it is on: the
    // shift itself lives in the `AH = 02h` service this unit does not have.
    let shifted: [(u8, u8); 21] = [
        (b'!', 0x02),
        (b'@', 0x03),
        (b'#', 0x04),
        (b'$', 0x05),
        (b'%', 0x06),
        (b'^', 0x07),
        (b'&', 0x08),
        (b'*', 0x09),
        (b'(', 0x0A),
        (b')', 0x0B),
        (b'_', 0x0C),
        (b'+', 0x0D),
        (b'{', 0x1A),
        (b'}', 0x1B),
        (b':', 0x27),
        (b'"', 0x28),
        (b'~', 0x29),
        (b'|', 0x2B),
        (b'<', 0x33),
        (b'>', 0x34),
        (b'?', 0x35),
    ];
    for (character, code) in shifted {
        assert_eq!(
            bios::scan_code(character),
            code,
            "the set 1 code of {:?} is the key {code:#04x}",
            character as char
        );
    }
    // And the control characters the table names.
    assert_eq!(bios::scan_code(0x1B), 0x01, "escape is 01h");
    assert_eq!(bios::scan_code(0x08), 0x0E, "backspace is 0Eh");
    assert_eq!(bios::scan_code(b'\t'), 0x0F, "tab is 0Fh");
    assert_eq!(bios::scan_code(b'\r'), 0x1C, "enter is 1Ch");
    assert_eq!(bios::scan_code(b'\n'), 0x1C, "and a line feed is enter too");
}

/// S15 - the scan codes the BIOS module names, against the published set 1.
#[test]
fn s15_the_named_scan_codes_are_the_ibm_set_1() {
    assert_eq!(bios::SCAN_ESCAPE, 0x01, "escape is 01h");
    assert_eq!(bios::SCAN_BACKSPACE, 0x0E, "backspace is 0Eh");
    assert_eq!(bios::SCAN_TAB, 0x0F, "tab is 0Fh");
    assert_eq!(bios::SCAN_ENTER, 0x1C, "enter is 1Ch");
    assert_eq!(bios::SCAN_SPACE, 0x39, "the space bar is 39h");
    assert_eq!(bios::SCAN_HOME, 0x47, "home is 47h");
    assert_eq!(bios::SCAN_UP, 0x48, "up is 48h");
    assert_eq!(bios::SCAN_PAGE_UP, 0x49, "page up is 49h");
    assert_eq!(bios::SCAN_LEFT, 0x4B, "left is 4Bh");
    assert_eq!(bios::SCAN_RIGHT, 0x4D, "right is 4Dh");
    assert_eq!(bios::SCAN_END, 0x4F, "end is 4Fh");
    assert_eq!(bios::SCAN_DOWN, 0x50, "down is 50h");
    assert_eq!(bios::SCAN_PAGE_DOWN, 0x51, "page down is 51h");
    assert_eq!(bios::SCAN_INSERT, 0x52, "insert is 52h");
    assert_eq!(bios::SCAN_DELETE, 0x53, "delete is 53h");
    assert_eq!(bios::SCAN_FUNCTION_FIRST, 0x3B, "F1 is 3Bh");
    // The corpus names the four arrow codes itself.
    assert_eq!(bios::SCAN_LEFT, 0x4B, "the corpus's left is 4Bh");
    assert_eq!(bios::SCAN_RIGHT, 0x4D, "the corpus's right is 4Dh");
    assert_eq!(bios::SCAN_UP, 0x48, "the corpus's up is 48h");
    assert_eq!(bios::SCAN_DOWN, 0x50, "the corpus's down is 50h");
}

/// S16 - `AH = 00h` waits when the buffer is empty, one step at a time: the
/// machine stays at the entry and comes back to it on the next step.
#[test]
fn s16_the_get_waits_one_step_at_a_time() {
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.regs.set_byte(Reg8::Ah, 0x00);
    c.step().expect("the interrupt runs");
    assert_eq!(c.regs.ip, 0x01C0, "the machine is at the keyboard's entry");

    for step in 0..5 {
        c.step().expect("the service runs");
        assert_eq!(
            c.regs.ip, 0x01C0,
            "the wait holds the machine at the entry (step {step})"
        );
        assert_eq!(c.regs.segment(Seg::Cs), 0xF400, "in the ROM");
        assert_eq!(
            c.regs.word(Reg16::Sp),
            0x0100 - 6,
            "and the frame the INT pushed is still on the stack"
        );
    }

    // A keystroke arrives: the service answers it and the machine returns.
    c.push_key(0x1E, b'A');
    c.step().expect("the service runs");
    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "AH = the scan code");
    assert_eq!(c.regs.byte(Reg8::Al), b'A', "AL = the ASCII character");
    assert_eq!(
        c.regs.ip, 0x0002,
        "and the machine carries on after the call"
    );
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0100,
        "and the stack is where it was"
    );
}

/// S19 - the core's one door: the pair of the scan code and the ASCII
/// character, and how many keystrokes are waiting.
#[test]
fn s19_the_door_is_the_pair_and_the_count() {
    let mut c = Cpu::new();
    assert_eq!(c.keys_waiting(), 0, "the buffer is empty to begin with");
    c.push_key(0x1E, b'A');
    assert_eq!(c.keys_waiting(), 1, "and the keystroke is waiting");
    assert_eq!(at_head(&c), (b'A', 0x1E), "the character and the scan code");
    assert_eq!(
        c.mem.read_u16(DATA, TAIL),
        START + 2,
        "and the tail moved past it"
    );

    // And the BIOS reads the buffer from there: the guest gets the
    // keystroke the door put in.
    let mut c = machine(&[0xCD, 0x16, 0xF4]);
    c.push_key(0x1E, b'A');
    c.regs.set_byte(Reg8::Ah, 0x00);
    run(&mut c);
    assert_eq!(c.regs.byte(Reg8::Ah), 0x1E, "AH = the scan code");
    assert_eq!(c.regs.byte(Reg8::Al), b'A', "AL = the ASCII character");
}

/// S1-S11 - the video BIOS still runs at its own entry, and the keyboard's
/// entry did not move it: the interrupt path is shared code.
#[test]
fn the_video_service_still_runs_at_its_own_entry() {
    let mut c = machine(&[0xCD, 0x10, 0xF4]);
    c.regs.set_byte(Reg8::Ah, 0x0E);
    c.regs.set_byte(Reg8::Al, b'T');
    c.step().expect("the interrupt runs");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0xF400,
        "the machine is at the BIOS"
    );
    assert_eq!(c.regs.ip, 0x0190, "at the video's own entry");
    c.step().expect("the service runs");
    assert_eq!(
        c.mem.read_u8(VIDEO, 0x0000),
        b'T',
        "and the video's service wrote the character"
    );
    assert_eq!(
        c.mem.read_u16(DATA, COLUMNS),
        80,
        "and the data area still holds the geometry"
    );
    assert_eq!(
        c.regs.ip, 0x0002,
        "and the machine carried on past the call"
    );
}

/// S12-S21 - the keyboard's entry is a second entry and the video's vector
/// is untouched: a machine that calls `INT 10h` still reaches the video.
#[test]
fn the_two_entries_do_not_collide() {
    let mut c = machine(&[0xCD, 0x10, 0xCD, 0x16, 0xF4]);
    c.regs.set_byte(Reg8::Ah, 0x0E);
    c.regs.set_byte(Reg8::Al, b'T');
    // The video's call.
    c.step().expect("the interrupt runs");
    c.step().expect("the video's service runs");
    assert_eq!(
        c.mem.read_u8(VIDEO, 0x0000),
        b'T',
        "the video's service ran and wrote the character"
    );
    // And then the keyboard's call, with its own registers.
    c.push_key(0x1E, b'A');
    c.regs.set_byte(Reg8::Ah, 0x00);
    c.step().expect("the interrupt runs");
    assert_eq!(c.regs.ip, 0x01C0, "the machine is at the keyboard's entry");
    c.step().expect("the keyboard's service runs");
    assert_eq!(
        c.regs.byte(Reg8::Ah),
        0x1E,
        "the keyboard's service ran too"
    );
    assert_eq!(c.regs.byte(Reg8::Al), b'A', "and it answered with the pair");
    assert_eq!(
        c.regs.ip, 0x0004,
        "and the machine carried on past the call"
    );
}

/// S13, S17 - the check answers with the zero flag whatever the stack
/// pointer is: the frame's flags word is at `SP + 4` and the segment
/// wraps, so the answer must not overflow.
#[test]
fn s17_the_check_answers_with_any_stack_pointer() {
    for sp in [
        0x0000u16, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0xFFF0, 0xFFFE,
    ] {
        let mut c = machine(&[0xCD, 0x16, 0xF4]);
        c.regs.set_word(Reg16::Sp, sp);
        c.regs.set_byte(Reg8::Ah, 0x01);
        c.step().expect("the interrupt runs");
        c.step().expect("the service runs");
        assert!(
            c.regs.flags.get(flags::ZF),
            "ZF = 1 with SS:SP = {sp:#06x}: the empty buffer is an answer"
        );
        assert_eq!(
            c.regs.ip, 0x0002,
            "and the machine carried on with SP = {sp:#06x}"
        );
    }
}
