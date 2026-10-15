//! Whole programs, run end to end.
//!
//! Every other test in this repository builds a machine by hand, sets a couple
//! of registers, executes one instruction and looks at the result. That is the
//! right way to test an instruction and the wrong way to test a processor: it
//! never checks that instruction *n* leaves the machine in a state where
//! instruction *n+1* decodes and runs correctly, which is where addressing bugs,
//! instruction-length bugs and flag-reach bugs actually live.
//!
//! These tests run a complete program from its first byte to `HLT`. The bytes
//! are hand-assembled, because there is no assembler yet — and that is the point
//! of keeping them here rather than in an assembler fixture directory: when the
//! assembler arrives (T13), each program carries the assembly it stands for, so
//! "does the assembler produce these bytes" becomes a testable claim rather than
//! an opinion.
//!
//! The expected final state is computed in each test from the algorithm, never
//! read back from the emulator. That is what makes these tests able to fail —
//! and on their first execution they did: three of these eight programs were
//! hand-assembled wrongly and the emulator was right in all three cases.
//!
//! Segments: code and data live in `DS = CS = 0x1000`, and `ES = 0x2000` is a
//! *different* segment so that the two string instructions which must use `ES`
//! prove it rather than appearing to work because everything shares a base.

use emo8067_core::Cpu;
use emo8067_core::flags::{CF, DF, OF, ZF};
use emo8067_core::regs::{Reg8, Reg16, Seg};

const CODE: u16 = 0x1000;
const DATA: u16 = 0x1000;
const EXTRA: u16 = 0x2000;

fn machine(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, CODE);
    c.regs.set_segment(Seg::Ds, DATA);
    c.regs.set_segment(Seg::Es, EXTRA);
    c.regs.set_segment(Seg::Ss, CODE);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

/// Run to `HLT`. A program that never halts is a failure, not a timeout.
fn run(c: &mut Cpu, budget: usize) {
    for step in 0..budget {
        if c.halted {
            assert!(step > 0, "the program halted before executing anything");
            return;
        }
        c.step().expect("the program must not fault");
    }
    panic!("the program did not halt within {budget} steps");
}

fn at(c: &mut Cpu, segment: u16, offset: u16, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        c.mem.write_u8(segment, offset + i as u16, *b);
    }
}

/// Sum eight bytes with an indexed loop.
///
/// ```asm
///     mov  si, 0100h
///     mov  cx, 8
///     xor  ax, ax
/// L1: add  al, [si]
///     inc  si
///     dec  cx
///     jnz  L1
///     hlt
/// ```
#[test]
fn sums_a_byte_array_with_an_indexed_loop() {
    let mut c = machine(&[
        0xBD, 0x00, 0x01, // mov si, 0100h
        0xB9, 0x08, 0x00, // mov cx, 8
        0x31, 0xC0, // xor ax, ax
        0x02, 0x04, // L1: add al, [si]   (/0 is ADD r/m,r; /2 is ADD r,r/m)
        0x45, // inc si   (40+r follows the REG field, so SI is 101b = 45h)
        0x49, // dec cx
        0x75, 0xFA, // jnz L1  (next is +14, target is +8)
        0xF4, // hlt
    ]);
    let values: [u8; 8] = [10, 20, 30, 40, 50, 60, 5, 6];
    at(&mut c, DATA, 0x0100, &values);
    let sum: u16 = values.iter().map(|b| u16::from(*b)).sum();

    run(&mut c, 1_000);
    assert_eq!(c.regs.word(Reg16::Ax), sum, "eight adds into AL");
    assert_eq!(c.regs.word(Reg16::Si), 0x0108, "SI walked the array");
    assert_eq!(c.regs.word(Reg16::Cx), 0, "and CX ran out");
}

/// Copy sixteen bytes with `REP MOVS`, from `DS` into `ES`.
///
/// ```asm
///     mov  si, 0100h
///     mov  di, 0100h
///     mov  cx, 10h
///     cld
///     rep  movsb
///     hlt
/// ```
#[test]
fn copies_a_block_from_ds_into_es_with_rep_movs() {
    let mut c = machine(&[
        0xBD, 0x00, 0x01, // mov si, 0100h   (BD is MOV SI)
        0xBE, 0x00, 0x01, // mov di, 0100h   (BE is MOV DI)
        0xB9, 0x10, 0x00, // mov cx, 10h
        0xFC, // cld
        0xF3, 0xA4, // rep movsb
        0xF4, // hlt
    ]);
    let block: Vec<u8> = (0u8..16).map(|i| i.wrapping_mul(17)).collect();
    at(&mut c, DATA, 0x0100, &block);
    at(&mut c, EXTRA, 0x0100, &[0u8; 16]);

    run(&mut c, 1_000);
    assert_eq!(c.regs.word(Reg16::Cx), 0, "the whole count was spent");
    assert_eq!(c.regs.word(Reg16::Si), 0x0110);
    assert_eq!(c.regs.word(Reg16::Di), 0x0110);
    let copied: Vec<u8> = (0..16u16)
        .map(|i| c.mem.read_u8(EXTRA, 0x0100 + i))
        .collect();
    assert_eq!(copied, block, "the block landed in ES:0100, not DS:0100");
}

/// Multiply two bytes from memory; the sixteen-bit product is left in `AX`.
///
/// ```asm
///     mov  al, [0100h]
///     mov  bl, [0101h]
///     mul  bl
///     hlt
/// ```
#[test]
fn multiplies_two_bytes_and_leaves_the_product_in_ax() {
    let mut c = machine(&[
        0xA0, 0x00, 0x01, // mov al, [0100h]
        0x8A, 0x1E, 0x01, 0x01, // mov bl, [0101h]  (rm=110 is the direct form)
        0xF6, 0xE3, // mul bl
        0xF4, // hlt
    ]);
    let (a, b) = (12u8, 13u8);
    at(&mut c, DATA, 0x0100, &[a, b]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), u16::from(a) * u16::from(b));
    assert_eq!(c.regs.byte(Reg8::Ah), 0, "and AH is the high half");
}

/// Divide a thirty-two-bit dividend by a sixteen-bit divisor, the wide path.
///
/// `DX` is the *high* half, which is the whole point of the instruction.
///
/// ```asm
///     mov  dx, [0100h]
///     mov  ax, [0102h]
///     mov  cx, [0104h]
///     div  cx
///     hlt
/// ```
#[test]
fn divides_a_wide_dividend_by_a_word_divisor() {
    let mut c = machine(&[
        0x8B, 0x16, 0x00, 0x01, // mov dx, [0100h]
        0x8B, 0x06, 0x02, 0x01, // mov ax, [0102h]  (disp16 is little-endian)
        0x8B, 0x0E, 0x04, 0x01, // mov cx, [0104h]
        0xF7, 0xF1, // div cx
        0xF4, // hlt
    ]);
    let (dividend, divisor) = (65_536u32, 100u16);
    at(
        &mut c,
        DATA,
        0x0100,
        &((dividend >> 16) as u16).to_le_bytes(),
    );
    at(&mut c, DATA, 0x0102, &(dividend as u16).to_le_bytes());
    at(&mut c, DATA, 0x0104, &divisor.to_le_bytes());

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        (dividend / u32::from(divisor)) as u16
    );
    assert_eq!(
        c.regs.word(Reg16::Dx),
        (dividend % u32::from(divisor)) as u16
    );
}

/// Find a byte with `REPNE SCASB`, which ends the repeat on the match, and
/// which reads `ES:DI` whatever `DS` says.
///
/// ```asm
///     mov  di, 0100h
///     mov  cx, 10h
///     mov  al, 5Ah
///     repne scasb
///     hlt
/// ```
#[test]
fn scans_es_for_a_byte_and_stops_on_the_match() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // mov di, 0100h   (BE is MOV DI)
        0xB9, 0x10, 0x00, // mov cx, 10h
        0xB0, 0x5A, // mov al, 5Ah
        0xF2, 0xAE, // repne scasb
        0xF4, // hlt
    ]);
    let mut haystack = [0u8; 16];
    haystack[4] = 0x5A;
    // A decoy at the same offset in DS: SCAS must not look there.
    at(&mut c, EXTRA, 0x0100, &haystack);
    at(&mut c, DATA, 0x0100, &[0x5Au8; 16]);

    run(&mut c, 100);
    assert!(
        c.regs.flags.get(ZF),
        "the match sets ZF, which ends the repeat"
    );
    assert_eq!(c.regs.word(Reg16::Cx), 11, "eleven comparisons left");
    assert_eq!(
        c.regs.word(Reg16::Di),
        0x0105,
        "DI sits just past the match"
    );
}

/// A subroutine called and returned, so the stack is used for its real purpose.
///
/// ```asm
///     mov  ax, 0005h
///     call times3
///     hlt
/// times3:
///     add  ax, ax
///     add  ax, ax
///     ret
/// ```
#[test]
fn calls_a_subroutine_and_returns_to_the_instruction_after_it() {
    let mut c = machine(&[
        0xB8, 0x05, 0x00, // mov ax, 0005h
        0xE8, 0x01, 0x00, // call times3
        0xF4, // hlt
        0x03, 0xC0, // times3: add ax, ax
        0x03, 0xC0, // add ax, ax
        0xC3, // ret
    ]);
    let stack = c.regs.word(Reg16::Sp);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 20, "5 doubled twice");
    assert_eq!(c.regs.word(Reg16::Sp), stack, "and the stack is balanced");
}

/// `XLAT`, which reads through `DS:[BX+AL]` with a *signed* index.
///
/// ```asm
///     mov  bx, 0100h
///     mov  al, 02h
///     xlat
///     hlt
/// ```
#[test]
fn translates_through_a_table_with_xlat() {
    let mut c = machine(&[
        0xBF, 0x00, 0x01, // mov bx, 0100h   (BF is MOV BX; BB is MOV SP)
        0xB0, 0x02, // mov al, 02h
        0xD7, // xlat
        0xF4, // hlt
    ]);
    let table = *b"ABCDEFGH";
    at(&mut c, DATA, 0x0100, &table);

    run(&mut c, 100);
    assert_eq!(
        c.regs.byte(Reg8::Al),
        table[2],
        "AL takes the byte at BX+AL"
    );
}

/// Read through a segment override, with a decoy at the same offset in `DS`.
///
/// ```asm
///     mov  ax, CS:[0200h]
///     hlt
/// ```
#[test]
fn reads_through_a_segment_override_to_the_code_segment() {
    let mut c = machine(&[
        0x2E, 0xA1, 0x00, 0x02, // mov ax, CS:[0200h]
        0xF4, // hlt
    ]);
    at(&mut c, DATA, 0x0200, &0x1111u16.to_le_bytes());
    c.mem.write_u16(CODE, 0x0200, 0x2222);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x2222,
        "the override read CS:0200, not the decoy at DS:0200"
    );
}

// ---------------------------------------------------------------------------
// Second batch. The first eight missed everything that needs more than one
// memory reference to get right: direction flag, interrupts, far control
// transfer, a segment register being written rather than read, and LOOP's
// different counting rule.
// ---------------------------------------------------------------------------

/// `REP MOVSB` with `DF` set copies backwards, and the two pointers still walk
/// in lockstep so the block lands mirrored.
///
/// ```asm
///     mov  si, 0110h
///     mov  di, 0210h
///     mov  cx, 10h
///     std
///     rep  movsb
///     cld
///     hlt
/// ```
#[test]
fn copies_a_block_backwards_when_df_is_set() {
    let mut c = machine(&[
        0xBD, 0x10, 0x01, // mov si, 0110h
        0xBE, 0x10, 0x02, // mov di, 0210h
        0xB9, 0x10, 0x00, // mov cx, 10h
        0xFD, // std
        0xF3, 0xA4, // rep movsb
        0xFC, // cld
        0xF4, // hlt
    ]);
    let block: Vec<u8> = (0u8..16).map(|i| 0xC0 | i).collect();
    at(&mut c, DATA, 0x0100, &block);
    at(&mut c, EXTRA, 0x0200, &[0u8; 16]);

    run(&mut c, 1_000);
    assert_eq!(c.regs.word(Reg16::Cx), 0);
    assert_eq!(
        c.regs.word(Reg16::Si),
        0x0100,
        "SI walked down past the block"
    );
    assert_eq!(c.regs.word(Reg16::Di), 0x0200);
    assert!(
        !c.regs.flags.get(DF),
        "the program ends with cld, so the flag is clear again"
    );
}

/// `LOOP` decrements *before* it tests, which is the opposite of `REP` and the
/// reason a loop with `CX = 0` runs 65,536 times rather than none.
///
/// `CX = 1` is the case that tells the two rules apart. Decrement-first runs
/// the body once, because `1` becomes `0` and the test fails. Test-first runs it
/// twice, because `1` is still non-zero when the test happens. A count larger
/// than one cannot tell them apart at all, which is what an earlier version of
/// this program did and why it proved nothing.
///
/// ```asm
///     mov  cx, 1
///     mov  ax, 0
/// L:  add  ax, 3
///     loop L
///     hlt
/// ```
#[test]
fn counts_with_loop_which_decrements_before_it_tests() {
    let mut c = machine(&[
        0xB9, 0x01, 0x00, // mov cx, 1
        0xB8, 0x00, 0x00, // mov ax, 0
        0x83, 0xC0, 0x03, // L: add ax, 3
        0xE2, 0xFB, // loop L  (next is +11, target is +6)
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        3,
        "one iteration: LOOP decrements first, so 1 becomes 0 before the test"
    );
    assert_eq!(c.regs.word(Reg16::Cx), 0, "and CX is spent");
}

/// `INT n` pushes a frame, the handler runs and `IRET` unwinds it, and
/// execution resumes at the instruction after the `INT`.
///
/// ```asm
///     mov  ax, 1111h
///     int  21h
///     hlt
/// ```
#[test]
fn an_interrupt_reaches_its_handler_and_returns() {
    let mut c = machine(&[
        0xB8, 0x11, 0x11, // mov ax, 1111h
        0xCD, 0x21, // int 21h
        0xF4, // hlt, reached after IRET
    ]);
    // Type 21h points at CODE:0050, where the handler sets AX and returns.
    c.mem.write_u16(0x0000, 0x21 * 4, 0x0050);
    c.mem.write_u16(0x0000, 0x21 * 4 + 2, CODE);
    assert!(
        c.mem
            .load_at(0x10000 + 0x0050, &[0xB8, 0x22, 0x22, 0xCF])
            .is_ok()
    );
    let stack = c.regs.word(Reg16::Sp);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x2222,
        "the handler ran and its result survived the IRET"
    );
    assert_eq!(c.regs.word(Reg16::Sp), stack, "the frame came back off");
    assert_eq!(c.regs.segment(Seg::Cs), CODE, "and CS was restored");
}

/// A far `CALL` saves `CS` as well as `IP`, so the handler can be in a different
/// segment and `RETF` puts both back.
///
/// ```asm
///     call far 0008h
///     hlt
/// 0008: mov ax, 1234h
///       retf
/// ```
#[test]
fn a_far_call_saves_and_restores_the_code_segment() {
    let mut c = machine(&[
        0x9A, 0x08, 0x00, 0x00, 0x10, // call far 1000h:0008h
        0xF4, // 0005: hlt
        0x90, 0x90, // 0006, 0007: padding
        0xB8, 0x34, 0x12, // 0008: mov ax, 1234h
        0xCB, // retf
    ]);
    let stack = c.regs.word(Reg16::Sp);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 0x1234, "the far procedure ran");
    assert_eq!(c.regs.word(Reg16::Sp), stack, "RETF unwound the frame");
    assert_eq!(c.regs.segment(Seg::Cs), CODE, "and CS came back");
}

/// `XLAT` treats `AL` as a **signed** index, so `FFh` means minus one rather than
/// plus 255.
///
/// ```asm
///     mov  bx, 0100h
///     mov  al, FFh
///     xlat
///     hlt
/// ```
#[test]
fn xlat_signs_its_index() {
    let mut c = machine(&[
        0xBF, 0x00, 0x01, // mov bx, 0100h
        0xB0, 0xFF, // mov al, FFh
        0xD7, // xlat
        0xF4, // hlt
    ]);
    let table = *b"ABCDEFGH";
    at(&mut c, DATA, 0x0100, &table);
    c.mem.write_u8(DATA, 0x00FF, 0xFE); // the byte just below the table

    run(&mut c, 100);
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0xFE,
        "AL = -1 indexes one byte below BX, not 255 bytes above it"
    );
}

/// Write a segment register and then index through it, so the base is not the
/// one the machine happened to boot with.
///
/// ```asm
///     mov  ax, 1200h
///     mov  ds, ax
///     mov  bx, 0080h
///     mov  al, [bx]
///     hlt
/// ```
#[test]
fn writes_a_segment_register_and_then_indexes_through_it() {
    let mut c = machine(&[
        0xB8, 0x00, 0x12, // mov ax, 1200h
        0x8E, 0xD8, // mov ds, ax
        0xBF, 0x80, 0x00, // mov bx, 0080h   (BF is MOV BX; BB is MOV SP)
        0x8A, 0x07, // mov al, [bx]
        0xF4, // hlt
    ]);
    c.mem.write_u8(0x1200, 0x0080, 0x5A);

    run(&mut c, 100);
    assert_eq!(c.regs.segment(Seg::Ds), 0x1200, "DS took the value");
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x5A,
        "and the operand came through the new base"
    );
}

/// `CBW` then `CWD` sign-extend through `AX` into `DX:AX`.
///
/// ```asm
///     mov  al, 80h
///     mov  ah, 0
///     cbw
///     cwd
///     hlt
/// ```
#[test]
fn extends_a_negative_byte_across_ax_and_dx() {
    let mut c = machine(&[
        0xB0, 0x80, // mov al, 80h
        0xB4, 0x00, // mov ah, 0
        0x98, // cbw
        0x99, // cwd
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 0xFF80, "CBW sign-extended into AH");
    assert_eq!(
        c.regs.word(Reg16::Dx),
        0xFFFF,
        "CWD copied the sign into DX"
    );
}

/// A byte multiply whose product does not fit leaves a non-zero high half, and
/// says so in `CF` and `OF`.
///
/// ```asm
///     mov  al, 10h
///     mov  bl, 10h
///     mul  bl
///     hlt
/// ```
#[test]
fn an_eight_bit_multiply_reports_overflow_in_its_high_half() {
    let mut c = machine(&[
        0xB0, 0x10, // mov al, 10h
        0xB3, 0x10, // mov bl, 10h
        0xF6, 0xE3, // mul bl
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 0x0100, "10h * 10h = 100h");
    assert!(
        c.regs.flags.get(CF) && c.regs.flags.get(OF),
        "and it overflowed"
    );
}

/// A shift loop that builds a mask, which is the shape almost every real
/// program uses `SHL` for.
///
/// ```asm
///     mov  ax, 0001h
///     mov  cx, 000Ch
/// L:  shl  ax, 1
///     loop L
///     hlt
/// ```
#[test]
fn shifts_a_bit_into_a_mask_with_a_loop() {
    let mut c = machine(&[
        0xB8, 0x01, 0x00, // mov ax, 0001h
        0xB9, 0x0C, 0x00, // mov cx, 000Ch
        0xD1, 0xE0, // L: shl ax, 1
        0xE2, 0xFC, // loop L  (next is +10, target is +6)
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 1 << 12, "twelve left shifts");
}

/// XOR every byte of a block into `AL`: a checksum, and the shape a
/// self-checking program is built from.
///
/// ```asm
///     mov  si, 0100h
///     mov  cx, 10h
///     xor  ax, ax
/// L:  xor  al, [si]
///     inc  si
///     dec  cx
///     jnz  L
///     hlt
/// ```
#[test]
fn checksums_a_block_with_xor() {
    let mut c = machine(&[
        0xBD, 0x00, 0x01, // mov si, 0100h
        0xB9, 0x10, 0x00, // mov cx, 10h
        0x31, 0xC0, // xor ax, ax
        0x32, 0x04, // L: xor al, [si]
        0x45, // inc si
        0x49, // dec cx
        0x75, 0xFA, // jnz L
        0xF4, // hlt
    ]);
    let block: Vec<u8> = (1u8..=16).collect();
    at(&mut c, DATA, 0x0100, &block);
    let checksum = block.iter().fold(0u8, |acc, b| acc ^ b);

    run(&mut c, 1_000);
    assert_eq!(c.regs.byte(Reg8::Al), checksum, "sixteen XORs");
    assert_eq!(c.regs.byte(Reg8::Ah), 0, "AH was zeroed and never touched");
}
