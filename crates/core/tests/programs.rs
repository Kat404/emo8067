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
use emo8067_core::flags::{AF, CF, DF, OF, ZF};
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
        0xBE, 0x00, 0x01, // mov si, 0100h
        0xB9, 0x08, 0x00, // mov cx, 8
        0x31, 0xC0, // xor ax, ax
        0x02, 0x04, // L1: add al, [si]   (/0 is ADD r/m,r; /2 is ADD r,r/m)
        0x46, // inc si   (40+r follows the REG field, so SI is 110b = 46h)
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
        0xBE, 0x00, 0x01, // mov si, 0100h   (BE is MOV SI)
        0xBF, 0x00, 0x01, // mov di, 0100h   (BF is MOV DI)
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
        0xBF, 0x00, 0x01, // mov di, 0100h   (BF is MOV DI)
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
        0xBB, 0x00, 0x01, // mov bx, 0100h   (BB is MOV BX; BC is MOV SP)
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
        0xBE, 0x10, 0x01, // mov si, 0110h
        0xBF, 0x10, 0x02, // mov di, 0210h
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
fn xlat_indexes_with_an_unsigned_al() {
    let mut c = machine(&[
        0xBB, 0x00, 0x01, // mov bx, 0100h
        0xB0, 0xFF, // mov al, FFh
        0xD7, // xlat
        0xF4, // hlt
    ]);
    let table = *b"ABCDEFGH";
    at(&mut c, DATA, 0x0100, &table);
    c.mem.write_u8(DATA, 0x01FF, 0xFE); // 255 bytes above the table

    run(&mut c, 100);
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0xFE,
        "AL = FFh indexes 255 bytes above BX: the index is unsigned"
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
        0xBB, 0x80, 0x00, // mov bx, 0080h   (BB is MOV BX; BC is MOV SP)
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
        0xBE, 0x00, 0x01, // mov si, 0100h
        0xB9, 0x10, 0x00, // mov cx, 10h
        0x31, 0xC0, // xor ax, ax
        0x32, 0x04, // L: xor al, [si]
        0x46, // inc si
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

// ---------------------------------------------------------------------------
// Third batch. What the first two could not reach: a program reading its own
// instruction stream, an interrupt vector as a control-flow tool, code that
// rewrites itself, a stack that has to unwind in order, a search with a
// conditional branch in it, and two segments live at once.
// ---------------------------------------------------------------------------

/// Read a byte out of the program's own instruction stream and branch on it.
///
/// ```asm
///     mov  al, CS:[0009h]
///     cmp  al, 5Ah
///     jz   0008h
///     hlt
///         db 5Ah
/// ```
#[test]
fn reads_a_byte_of_its_own_instruction_stream() {
    let mut c = machine(&[
        0x2E, 0xA0, 0x09, 0x00, // mov al, CS:[0009h]
        0x3C, 0x5A, // cmp al, 5Ah
        0x74, 0x01, // jz 0009h  (next is +8, target is +9)
        0xF4, // 0008: hlt, reached only if the byte was not 5Ah
        0x5A, // 0009: the sentinel the program read
        0xF4, // 000A: the sentinel is a POP DX, so the program needs its own halt
    ]);

    run(&mut c, 100);
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x5A,
        "the byte came from the program's own code"
    );
    assert_eq!(
        c.regs.ip, 11,
        "IP executed the sentinel and then the halt after it"
    );
}

/// `REP` with a zero count executes nothing at all — the rule the microcode
/// settles at ROM `0x112`, seen here end to end.
///
/// ```asm
///     mov  si, 0100h
///     mov  di, 0200h
///     mov  cx, 0
///     rep  movsb
///     hlt
/// ```
#[test]
fn rep_with_a_zero_count_writes_nothing() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // mov si, 0100h
        0xBF, 0x00, 0x02, // mov di, 0200h
        0xB9, 0x00, 0x00, // mov cx, 0
        0xF3, 0xA4, // rep movsb
        0xF4, // hlt
    ]);
    at(&mut c, DATA, 0x0100, &[0x11; 16]);
    at(&mut c, EXTRA, 0x0200, &[0x22; 16]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Si), 0x0100, "SI never moved");
    assert_eq!(c.regs.word(Reg16::Di), 0x0200, "DI never moved");
    assert_eq!(c.regs.word(Reg16::Cx), 0, "CX is still zero");
    assert_eq!(c.mem.read_u8(EXTRA, 0x0200), 0x22, "and nothing was copied");
}

/// Two nested calls, so the stack has to unwind in the right order.
///
/// ```asm
///     mov  ax, 1
///     call outer
///     hlt
/// outer:
///     inc  ax
///     call inner
///     inc  ax
///     ret
/// inner:
///     inc  ax
///     inc  ax
///     ret
/// ```
#[test]
fn nested_calls_unwind_the_stack_in_order() {
    let mut c = machine(&[
        0xB8, 0x01, 0x00, // 0000: mov ax, 1
        0xE8, 0x01, 0x00, // 0003: call outer  (next is +6, outer is +7)
        0xF4, // 0006: hlt
        0x40, // 0007: outer: inc ax
        0xE8, 0x02, 0x00, // 0008: call inner (next is +11, inner is +13)
        0x40, // 0011: inc ax
        0xC3, // 0012: ret
        0x40, // 0013: inner: inc ax
        0x40, // 0014: inc ax
        0xC3, // 0015: ret
    ]);
    let stack = c.regs.word(Reg16::Sp);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        5,
        "1, plus outer, plus inner twice, plus outer again"
    );
    assert_eq!(c.regs.word(Reg16::Sp), stack, "both frames came back off");
}

/// Write the machine code into memory and then jump into it.
///
/// ```asm
///     mov  di, 0200h
///     mov  ax, 34B8h          ; the word B8 34h, which reads as MOV AX,1234h
///     mov  [di], ax
///     mov  di, 0202h
///     mov  ax, F412h          ; the word 12 F4h, which reads as 12h then HLT
///     mov  [di], ax
///     jmp  far 1000h:0200h
/// ```
///
/// The store goes through `DS` and the jump goes through `CS`, so the same
/// physical address is written as data and executed as code.
#[test]
fn writes_its_own_code_and_then_jumps_into_it() {
    let mut c = machine(&[
        0xBF, 0x00, 0x02, // mov di, 0200h
        0xB8, 0xB8, 0x34, // mov ax, 34B8h
        0x89,
        0x05, // mov [di], ax   (mod=00, rm=101 is [DI]; C6 was mod=11, a register move)   (r/m is the destination: DI is 110b)
        0xBF, 0x02, 0x02, // mov di, 0202h
        0xB8, 0x12, 0xF4, // mov ax, F412h
        0x89, 0x05, // mov [di], ax   (mod=00, rm=101 is [DI]; C6 was mod=11, a register move)
        0xEA, 0x00, 0x02, 0x00, 0x10, // jmp far 1000h:0200h
    ]);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "the patched code ran: B8 34 12h is MOV AX,1234h"
    );
    assert_eq!(c.regs.ip, 0x0204, "and the F4h it also wrote stopped it");
}

/// Two segments live at once, with an override in the middle of it.
///
/// ```asm
///     mov  si, 0100h
///     mov  al, [si]         ; DS:0100
///     mov  bl, cs:[si]     ; the same offset in CS
///     hlt
/// ```
#[test]
fn reads_two_segments_at_the_same_offset() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // mov si, 0100h
        0x8A, 0x04, // mov al, [si]
        0x2E, 0x8A, 0x1C, // mov bl, cs:[si]
        0xF4, // hlt
    ]);
    // The two segments must genuinely differ, or the test reads one byte twice.
    c.regs.set_segment(Seg::Ds, 0x3000);
    at(&mut c, 0x3000, 0x0100, &[0xA1]);
    at(&mut c, CODE, 0x0100, &[0xB2]);

    run(&mut c, 100);
    assert_eq!(c.regs.byte(Reg8::Al), 0xA1, "the first read took DS");
    assert_eq!(c.regs.byte(Reg8::Bl), 0xB2, "and the second took CS");
}

/// A signed multiply followed by the divide that undoes it, across `DX:AX`.
///
/// ```asm
///     mov  ax, FFFCh        ; -4
///     mov  bx, 0007h
///     imul bx               ; DX:AX = -28
///     mov  cx, dx
///     idiv bx               ; -28 / 7 = -4, remainder 0
///     hlt
/// ```
#[test]
fn a_signed_divide_undoes_a_signed_multiply() {
    let mut c = machine(&[
        0xB8, 0xFC, 0xFF, // mov ax, FFFCh
        0xBB, 0x07, 0x00, // mov bx, 0007h
        0xF7, 0xEB, // imul bx
        0x8B, 0xCA, // mov cx, dx
        0xF7, 0xFB, // idiv bx
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.word(Reg16::Ax), 0xFFFC, "the quotient is -4 again");
    assert_eq!(c.regs.word(Reg16::Dx), 0, "and the remainder is zero");
    assert_eq!(c.regs.word(Reg16::Cx), 0xFFFF, "the product had DX = FFFFh");
}

/// A linear search: a comparison, a conditional branch, and a loop — the shape
/// of almost every program that does something.
///
/// ```asm
///     mov  si, 0100h
///     mov  cx, 0008h
///     mov  al, 2Ah
/// search:
///     cmp  [si], al
///     je   found
///     inc  si
///     loop search
///     mov  bx, 0FFFFh       ; not found
///     jmp  done
/// found:
///     mov  bx, si
/// done:
///     hlt
/// ```
#[test]
fn searches_an_array_and_reports_where_it_stopped() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // 0000: mov si, 0100h
        0xB9, 0x08, 0x00, // 0003: mov cx, 8
        0xB0, 0x2A, // 0006: mov al, 2Ah
        0x38, 0x04, // 0008: cmp [si], al
        0x74, 0x0E, // 0010: je found   (next is +12, found is +26)
        0x46, // 0012: inc si
        0xE2, 0xF9, // 0013: loop search (next is +15, search is +8)
        0xBB, 0xFF, 0xFF, // 0015: mov bx, FFFFh
        0xEB, 0x09, // 0018: jmp done    (next is +20, done is +29)
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, // 0020: padding
        0x8B, 0xDE, // 0026: found: mov bx, si
        0xF4, // 0029: done: hlt
    ]);
    let mut haystack = [0x11u8; 8];
    haystack[3] = 0x2A;
    at(&mut c, DATA, 0x0100, &haystack);

    run(&mut c, 200);
    assert_eq!(
        c.regs.word(Reg16::Bx),
        0x0103,
        "BX holds the address of the match, which is what a search returns"
    );
}

/// The same search when the needle is absent, so the other branch is exercised.
///
/// ```asm
///     mov  si, 0100h
///     mov  cx, 0008h
///     mov  al, 2Ah
/// search:
///     cmp  [si], al
///     je   found
///     inc  si
///     loop search
///     mov  bx, 0FFFFh
///     jmp  done
/// found:
///     mov  bx, si
/// done:
///     hlt
/// ```
#[test]
fn a_search_that_finds_nothing_returns_the_sentinel() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // 0000: mov si, 0100h
        0xB9, 0x08, 0x00, // 0003: mov cx, 8
        0xB0, 0x2A, // 0006: mov al, 2Ah
        0x38, 0x04, // 0008: cmp [si], al
        0x74, 0x0E, // 0010: je found
        0x46, // 0012: inc si
        0xE2, 0xF9, // 0013: loop search
        0xBB, 0xFF, 0xFF, // 0015: mov bx, FFFFh
        0xEB, 0x09, // 0018: jmp done
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, // 0020: padding
        0x8B, 0xDE, // 0026: found: mov bx, si
        0xF4, // 0029: done: hlt
    ]);
    at(&mut c, DATA, 0x0100, &[0x11; 8]);

    run(&mut c, 200);
    assert_eq!(
        c.regs.word(Reg16::Bx),
        0xFFFF,
        "the sentinel, not an address"
    );
}

/// Push three and pop three in the other order, which is the only proof the
/// stack really is a stack.
///
/// ```asm
///     mov  ax, 1111h
///     mov  bx, 2222h
///     mov  cx, 3333h
///     push ax
///     push bx
///     push cx
///     pop  dx
///     pop  si
///     pop  di
///     hlt
/// ```
#[test]
fn the_stack_gives_back_what_was_pushed_last_first() {
    let mut c = machine(&[
        0xB8, 0x11, 0x11, // mov ax, 1111h
        0xBB, 0x22, 0x22, // mov bx, 2222h
        0xB9, 0x33, 0x33, // mov cx, 3333h
        0x50, // push ax
        0x53, // push bx   (50+r, and BX is 011b)
        0x51, // push cx
        0x5A, // pop dx    (58+r)
        0x5E, // pop si    (SI is 110b)
        0x5F, // pop di    (DI is 111b)
        0xF4, // hlt
    ]);
    let stack = c.regs.word(Reg16::Sp);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Dx),
        0x3333,
        "the last push comes back first"
    );
    assert_eq!(c.regs.word(Reg16::Si), 0x2222);
    assert_eq!(c.regs.word(Reg16::Di), 0x1111, "and the first push last");
    assert_eq!(c.regs.word(Reg16::Sp), stack, "with SP back where it began");
}

/// `REPE CMPSB` over two buffers, stopping at the first byte that differs.
///
/// ```asm
///     mov  si, 0100h
///     mov  di, 0200h
///     mov  cx, 0008h
///     repe cmpsb
///     hlt
/// ```
#[test]
fn repe_cmpsb_stops_at_the_first_mismatch_between_two_buffers() {
    let mut c = machine(&[
        0xBE, 0x00, 0x01, // mov si, 0100h
        0xBF, 0x00, 0x02, // mov di, 0200h
        0xB9, 0x08, 0x00, // mov cx, 8
        0xF3, 0xA6, // repe cmpsb
        0xF4, // hlt
    ]);
    at(&mut c, DATA, 0x0100, &[1, 2, 3, 4, 5, 6, 7, 8]);
    at(&mut c, EXTRA, 0x0200, &[1, 2, 3, 9, 5, 6, 7, 8]);

    run(&mut c, 100);
    assert_eq!(
        c.regs.word(Reg16::Cx),
        4,
        "four comparisons matched before the fifth did not"
    );
    assert_eq!(
        c.regs.word(Reg16::Si),
        0x0104,
        "SI sits on the mismatching byte"
    );
    assert_eq!(c.regs.word(Reg16::Di), 0x0204, "and DI on the same index");
    assert!(!c.regs.flags.get(ZF), "ZF clear is what ended the repeat");
}

/// Two ASCII digits added, and adjusted into a binary digit and a carry.
///
/// ```asm
///     mov  al, 35h        ; '5'
///     mov  bl, 35h        ; '5'
///     add  al, bl         ; 6Ah: the low nibble is not a digit
///     aaa                 ; AL = 00h, AH = 01h
///     hlt
/// ```
///
/// The expected value is stated here rather than read back from the emulator,
/// because that is the only way the test can fail. `35h + 35h` is `6Ah`: the
/// low nibble is `Ah`, which is not a decimal digit, so `AAA` adds six, carries
/// into `AH`, and leaves `AL` with the digit `0` and a carry of one. The
/// manual's rule — the nibble out of range, or `AF` set, means adjust — is
/// what the `ADD` above sets up: `5 + 5` does not carry out of the low nibble,
/// so `AF` is clear and the adjustment is the nibble's own doing.
#[test]
fn adds_two_ascii_digits_into_a_binary_digit_and_a_carry() {
    let mut c = machine(&[
        0xB0, 0x35, // mov al, 35h
        0xB3, 0x35, // mov bl, 35h
        0x00, 0xD8, // add al, bl
        0x37, // aaa
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.byte(Reg8::Al), 0x00, "the digit is zero");
    assert_eq!(c.regs.byte(Reg8::Ah), 0x01, "and the carry is one");
    assert_eq!(c.regs.word(Reg16::Ax), 0x0100);
    assert!(
        c.regs.flags.get(CF),
        "a decimal carry came out of the nibble"
    );
    assert!(c.regs.flags.get(AF), "and AF says the same");
}

/// Two packed BCD operands added, and the sum repaired into packed BCD.
///
/// ```asm
///     mov  al, 28h
///     mov  bl, 37h
///     add  al, bl         ; 5Fh
///     daa                 ; 65h
///     hlt
/// ```
///
/// `28h + 37h` is `5Fh`, whose low nibble is not a digit. `DAA` adds six to
/// the low nibble, which carries nothing out of it, so the result is the
/// packed pair `65h` — `28 + 37 = 65` — with no carry out of the tens. The
/// low correction is what writes `AF`, and the manual's Table 2-21 gives `DAA`
/// an `X` in that column, so `AF` is set here and the tens carry is not.
#[test]
fn repairs_a_packed_bcd_sum_into_packed_bcd() {
    let mut c = machine(&[
        0xB0, 0x28, // mov al, 28h
        0xB3, 0x37, // mov bl, 37h
        0x00, 0xD8, // add al, bl
        0x27, // daa
        0xF4, // hlt
    ]);

    run(&mut c, 100);
    assert_eq!(c.regs.byte(Reg8::Al), 0x65, "28 + 37 is 65 in packed BCD");
    assert!(!c.regs.flags.get(CF), "nothing carried out of the tens");
    assert!(c.regs.flags.get(AF), "the low nibble was corrected");
    assert!(!c.regs.flags.get(ZF));
    assert!(
        !c.regs.flags.get(OF),
        "the sum of two positives cannot overflow"
    );
}
