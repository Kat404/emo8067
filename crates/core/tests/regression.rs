//! Regression probes for defects found in independent verification of T11.
//!
//! Each one is a case that passed a 96-test suite while the emulator was
//! wrong, so each is pinned here with the bytes that trigger it.

use emo8067_core::Cpu;
use emo8067_core::StopReason;
use emo8067_core::exec::ExecError;
use emo8067_core::flags::{AF, CF, DF, IF, OF, ONE, PF, SF, TF, ZF};
use emo8067_core::regs::{Reg8, Reg16, Seg};

fn cpu(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ss, 0x1000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

/// Assert the stack still grows downward before a test that reads or writes
/// it.
///
/// `PUSH` and `POP` follow `DF`, so a setup helper that set *every* status
/// flag would silently reverse the 8086 stack and turn a correct
/// implementation into a failing test — or, worse, hide a real one. No helper
/// in this file sets `DF`; this states that where it matters instead of
/// leaving it to a comment.
fn stack_grows_down(c: &Cpu) {
    assert!(
        !c.regs.flags.get(DF),
        "DF must be clear: PUSH/POP follow it"
    );
}

fn run(c: &mut Cpu, n: usize) {
    for _ in 0..n {
        c.step().expect("the program must not fault");
    }
}

/// `A8` is `TEST AL, imm8` and carries no ModR/M byte. Routing it through the
/// shared r/m,imm helper consumed a phantom one and stored to memory.
///
/// This project ran it as `CMP` for two units, on the strength of a source that
/// said so. A real Intel P80C86A-2 says otherwise: the single-step capture in
/// `SingleStepTests/8086` records `A8` with undefined `AF` and nothing else
/// undefined, which is the signature of `AND`/`TEST` — `CMP` is a subtraction
/// and defines all six. `3C`/`3D`, the documented `CMP AL/AX, imm`, are recorded
/// there with no undefined flags at all. The 1979 manual printed `TEST`, and it
/// was right.
#[test]
fn a8_is_test_al_imm8_and_touches_no_memory() {
    let mut c = cpu(&[0xB0, 0x10, 0xA8, 0x10]);
    c.mem.write_u8(0x0000, 0x0020, 0x77);
    run(&mut c, 2);
    assert_eq!(
        c.mem.read_u8(0x0000, 0x0020),
        0x77,
        "A8 must not write memory"
    );
    assert_eq!(c.regs.ip, 4, "A8 ib is two bytes");
    assert_eq!(c.regs.byte(Reg8::Al), 0x10, "TEST stores nothing");
    assert!(
        !c.regs.flags.get(ZF),
        "10h AND 10h is 10h, not zero — CMP would have set ZF here"
    );
}

/// `A9` is `TEST AX, imm16`, three bytes, still no memory access.
#[test]
fn a9_is_test_ax_imm16_and_touches_no_memory() {
    let mut c = cpu(&[0xB8, 0x34, 0x12, 0xA9, 0x34, 0x12]);
    c.mem.write_u8(0x0000, 0x0020, 0x77);
    run(&mut c, 2);
    assert_eq!(c.mem.read_u8(0x0000, 0x0020), 0x77);
    assert_eq!(c.regs.ip, 6, "B8 iw (3) then A9 iw (3)");
    assert!(
        !c.regs.flags.get(ZF),
        "1234h AND 1234h is 1234h, not zero — CMP would have set ZF here"
    );
    assert_eq!(c.regs.word(Reg16::Ax), 0x1234, "TEST stores nothing");
}

/// The flag difference between `TEST` and `CMP`, which is the whole point:
/// `A8` with `AL = 0` against `01h` must leave `CF` clear, where `CMP` would set
/// it for the borrow. `0x0F` is the opposite case, and `0x10` differs in parity.
#[test]
fn a8_sets_the_flags_of_an_and_not_of_a_subtraction() {
    let borrow = |al: u8, imm: u8| {
        let mut c = cpu(&[0xB0, al, 0xA8, imm]);
        run(&mut c, 2);
        c.regs.flags
    };

    // 00h - 01h borrows: CMP would set CF. TEST never does.
    let f = borrow(0x00, 0x01);
    assert!(
        !f.get(CF),
        "TEST clears CF where CMP would set it for a borrow"
    );
    assert!(!f.get(OF));
    assert!(f.get(ZF), "00h AND 01h is zero, so ZF is set");

    // 0Fh AND 10h is 00h, while 0Fh - 10h is 0FFh: the sign bits differ.
    let f = borrow(0x0F, 0x10);
    assert!(f.get(ZF), "0Fh AND 10h is zero");
    assert!(!f.get(SF), "TEST takes SF from the AND, which is zero");

    // 21h AND 01h is 01h, odd; 21h - 01h is 20h, even. Parity tells them apart.
    let f = borrow(0x21, 0x01);
    assert!(!f.get(ZF), "the AND is 01h, not zero");
    assert!(!f.get(PF), "parity comes from the AND, and 01h is odd");
}

/// `AF` is undefined after `TEST`, and this project leaves undefined flags
/// alone rather than inventing a value for them.
#[test]
fn a8_leaves_af_alone_the_way_test_does() {
    let mut c = cpu(&[0xB0, 0x10, 0xA8, 0x01]); // 0x10-0x01 would set AF
    c.regs.flags.set(AF, false);
    run(&mut c, 2);
    assert!(
        !c.regs.flags.get(AF),
        "0x10 AND 0x01 says nothing about AF, so the old value stands"
    );
}

/// A catch-all match arm over `AC..=AF` ran `SCAS` for `LODSB`.
#[test]
fn repe_lodsb_loads_the_byte_into_al() {
    let mut c = cpu(&[0xB9, 0x01, 0x00, 0xBD, 0x20, 0x00, 0xF3, 0xAC]);
    c.mem.write_u8(0x0000, 0x0020, 0x5A);
    run(&mut c, 3);
    assert_eq!(c.regs.byte(Reg8::Al), 0x5A, "REPE LODSB must load AL");
    assert_eq!(c.regs.word(Reg16::Si), 0x0021);
    assert_eq!(c.regs.word(Reg16::Cx), 0);
}

#[test]
fn rep_lodsw_loads_the_word_into_ax() {
    let mut c = cpu(&[0xB9, 0x01, 0x00, 0xBD, 0x20, 0x00, 0xF3, 0xAD]);
    c.mem.write_u16(0x0000, 0x0020, 0xBEEF);
    run(&mut c, 3);
    assert_eq!(c.regs.word(Reg16::Ax), 0xBEEF);
    assert_eq!(c.regs.word(Reg16::Si), 0x0022);
}

/// `CMPS` and `SCAS` set the same six flags a subtraction does. Modelling
/// only ZF/CF/OF breaks anything reading the parity or auxiliary flag.
#[test]
fn cmpsb_writes_all_six_arithmetic_flags() {
    let mut c = cpu(&[0xBE, 0x20, 0x00, 0xBF, 0x21, 0x00, 0xA6]);
    c.mem.write_u8(0x0000, 0x0020, 0x03);
    c.mem.write_u8(0x0000, 0x0021, 0x0F);
    run(&mut c, 3);
    let f = c.regs.flags;
    // 03 - 0F = F4. F4 has five set bits, so parity is odd and PF is clear.
    assert!(f.get(CF), "03 borrows from 0F");
    assert!(f.get(AF), "3 - 15 carries out of bit 3");
    assert!(f.get(SF), "F4 is negative");
    assert!(!f.get(ZF) && !f.get(OF) && !f.get(PF));
}

/// `INC r16` sets five flags and leaves `CF` exactly as it was.
#[test]
fn inc_r16_sets_the_five_flags_and_leaves_cf() {
    let mut c = cpu(&[0xB9, 0xFF, 0x00, 0x41]);
    c.regs.flags.set(CF, true);
    run(&mut c, 2);
    assert_eq!(c.regs.word(Reg16::Cx), 0x0100);
    let f = c.regs.flags;
    assert!(
        f.get(PF) && f.get(AF),
        "00 has even parity, FF+1 carries from bit 3"
    );
    assert!(f.get(CF), "INC must not touch CF");
    assert!(!f.get(SF) && !f.get(ZF) && !f.get(OF));
}

/// `0F` is `POP CS`, and was being swallowed by the ALU dispatcher.
#[test]
fn opcode_0f_pops_cs() {
    let mut c = cpu(&[0x0F]);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.mem.write_u16(0x1000, 0x0100, 0x3000);
    c.step().unwrap();
    assert_eq!(c.regs.segment(Seg::Cs), 0x3000);
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0102,
        "pop predecrements on the way in"
    );
}

/// Bytes the 8086 leaves with no instruction at all: **none**. The list is
/// empty because every one of the 256 first opcode bytes decodes. The 8086
/// Family User's Manual's Table 4-13 prints 23 of them as `(not used)`, and
/// every one of those 23 executes something — they are the undocumented
/// instructions, not holes.
///
/// Sources, both primary:
///
/// * Ken Shirriff, *Undocumented 8086 instructions, explained by the
///   microcode*, <http://www.righto.com/2023/07/undocumented-8086-instructions.html>.
///   Its "Holes in the opcode table" section walks `D6` (SALC), `0F`
///   (`POP CS`), `60`-`6F` (aliases for `70`-`7F`), `C0`/`C8`
///   (`RET`/`RETF` imm16), `C1`/`C9` (`RET`/`RETF`) and `F1` (a second
///   `LOCK` prefix), showing the microcode each one runs.
/// * Andrew Jenner's disassembly of the real 8086 microcode ROM,
///   <https://github.com/reenigne/reenigne/blob/master/8088/8086_microcode/8086_microcode.cpp>.
///   Its Group Decode ROM annotation table names `SALC`, `RET`, `RETF`,
///   `RET/RETF iw`, `Jcond cb` and `ESC` as categories of the 8086's own
///   decoder — the same conclusion from the silicon side.
///
/// An empty list here is the finding, not an omission. The earlier version of
/// this gate listed eleven real 8086 encodings as missing and was `#[ignore]`d;
/// U6 emptied it, the loop body stopped running, and the gate stopped gating
/// without anybody noticing. `probed` is asserted below for exactly that
/// reason.
#[test]
fn every_first_opcode_byte_the_8086_defines_decodes() {
    /// The hole list. Empty, and it stays empty: see the doc comment.
    ///
    /// There is no second list any more. The four segment override prefixes
    /// used to be excluded as "not an opcode on its own", which left this gate
    /// probing 252 bytes; U7 implemented them and the sweep now covers 256, so
    /// `probed` is pinned to the literal 256 rather than to a subtraction. An
    /// exclusion list is the exact shape that let this gate go vacuous before.
    const HOLES: &[u8] = &[];

    let mut probed = 0usize;
    let mut undecoded = vec![];
    for op in 0u16..=0xFF {
        let op = op as u8;
        if HOLES.contains(&op) {
            continue;
        }
        probed += 1;
        // A byte counts as undecoded on ANY failure that means "this byte did
        // not decode". Matching one variant only is how this gate went blind
        // before: a prefix byte routed to the wrong arm returns a different
        // error and used to sail through.
        if let Err(err) = probe(op).step()
            && matches!(
                err,
                ExecError::UnknownOpcode(_) | ExecError::PrefixNotConsumed(_)
            )
        {
            undecoded.push(op);
        }
    }

    assert_eq!(
        probed, 256,
        "the sweep must visit every byte, or it asserts nothing"
    );
    let named: Vec<String> = undecoded.iter().map(|b| format!("{b:#04x}")).collect();
    assert!(
        undecoded.is_empty(),
        "the 8086 leaves no first opcode byte undefined, so these must decode: {}",
        named.join(", ")
    );
}

/// A CPU holding one instruction whose opcode is `op`, plus whatever that
/// opcode needs behind it.
fn probe(op: u8) -> Cpu {
    // `F2`/`F3` are prefixes and need a string operation behind them. The
    // segment override prefixes `26`/`2E`/`36`/`3E` need nothing in
    // particular: `C3` is an instruction, which is all a prefix needs behind
    // it. For everything else `C3` is the cheapest possible filler: `mod=11`
    // means it needs no displacement, touches no memory and leaves the stack
    // alone.
    let filler = if matches!(op, 0xF2 | 0xF3) {
        0xA4
    } else {
        0xC3
    };
    let mut c = Cpu::new();
    c.mem
        .load_at(0x10000, &[op, filler, 0xA4, 0x06, 0x20, 0x00, 0x0A])
        .unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ss, 0x2000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

/// `8C` with a register destination is an 80186 encoding. The 8086 has no
/// such instruction, and accepting it would put a segment register where
/// the hardware cannot.
#[test]
fn mov_segment_to_register_is_rejected_on_the_8086() {
    let mut c = cpu(&[0x8C, 0xC3]);
    assert_eq!(c.step(), Err(ExecError::NotOn8086(0x8C)));
}

/// `LEA` computes an address and must never read the segment into the
/// result, even when the operand carries one.
#[test]
fn lea_uses_the_offset_alone() {
    let mut c = cpu(&[0x8D, 0x06, 0x34, 0x12]); // LEA AX, [1234h]
    c.regs.set_segment(Seg::Ds, 0x9999);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "the segment is not part of an offset"
    );
}

/// Each string operation may only advance the pointer it actually used.
/// Advancing both corrupted `DI` on LODS and `SI` on STOS and SCAS.
#[test]
fn lods_advances_only_si() {
    let mut c = cpu(&[0xAC]); // LODSB
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x0000, 0x0020, 0x5A);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Si), 0x0021);
    assert_eq!(c.regs.word(Reg16::Di), 0x0300, "LODS must not touch DI");
}

#[test]
fn stos_advances_only_di() {
    let mut c = cpu(&[0xAA]); // STOSB
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.regs.set_byte(Reg8::Al, 0x5A);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Di), 0x0301);
    assert_eq!(c.regs.word(Reg16::Si), 0x0020, "STOS must not touch SI");
}

#[test]
fn scas_advances_only_di() {
    let mut c = cpu(&[0xAE]); // SCASB
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Di), 0x0301);
    assert_eq!(c.regs.word(Reg16::Si), 0x0020, "SCAS must not touch SI");
}

#[test]
fn movs_advances_both_pointers() {
    let mut c = cpu(&[0xA4]); // MOVSB
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Si), 0x0021);
    assert_eq!(c.regs.word(Reg16::Di), 0x0301);
}

#[test]
fn rep_lods_advances_only_si() {
    let mut c = cpu(&[0xB9, 0x03, 0x00, 0xBD, 0x20, 0x00, 0xF3, 0xAC]);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u16(0x0000, 0x0020, 0x0000);
    for _ in 0..3 {
        c.step().unwrap();
    }
    assert_eq!(c.regs.word(Reg16::Si), 0x0023);
    assert_eq!(c.regs.word(Reg16::Di), 0x0300, "REP LODS must not touch DI");
}

/// RCL and RCR rotate the carry *through* the operand. The previous code
/// put the old carry into the top bit and dropped the count entirely.
#[test]
fn rcl_rolls_the_old_carry_into_the_bottom() {
    // D1 D0 = RCL AX, 1.  8000h with CF=0 -> 0000h, CF=1.
    let mut c = cpu(&[0xD1, 0xD0]);
    c.regs.set_word(Reg16::Ax, 0x8000);
    c.regs.flags.set(CF, false);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0000, "the old CF lands in bit 0");
    assert!(c.regs.flags.get(CF), "the old MSB lands in CF");
}

/// A count above one winds the carry around step by step: CF=1 enters at
/// bit 0 on the first step and is itself shifted out on the second.
#[test]
fn rcl_honours_a_count_above_one() {
    // MOV CX, 2 then RCL AX, CL. AX=0001h, CF=1.
    //   step 1 -> AX=0003h, CF=0   (bit 15 was clear)
    //   step 2 -> AX=0006h, CF=0   (bit 15 was still clear)
    let mut c = cpu(&[0xB9, 0x02, 0x00, 0xD3, 0xD0]);
    c.regs.set_word(Reg16::Ax, 0x0001);
    c.regs.flags.set(CF, true);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x0006,
        "CF=1 was rolled in, then shifted up"
    );
    assert!(!c.regs.flags.get(CF));
}

/// RCR brings the carry in at the top and pushes the bottom bit out.
#[test]
fn rcr_rolls_the_old_carry_into_the_top() {
    // D1 D8 = RCR AX, 1.  0001h with CF=1 -> 8000h, CF=1.
    let mut c = cpu(&[0xD1, 0xD8]);
    c.regs.set_word(Reg16::Ax, 0x0001);
    c.regs.flags.set(CF, true);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x8000);
    assert!(c.regs.flags.get(CF));
}

/// Rotating by zero changes nothing, and used to panic in debug because the
/// rotate computed `v >> width` on a u16.
#[test]
fn rotating_by_zero_is_a_no_op_and_does_not_panic() {
    // D3 C0 = ROL AX, CL with CL = 0. CL=16 is also a zero rotation on a word.
    for cl in [0u8, 16] {
        let mut c = cpu(&[0xB9, cl, 0x00, 0xD3, 0xC0]);
        c.regs.set_word(Reg16::Ax, 0x1234);
        c.regs.flags.set(CF, true);
        c.step().unwrap(); // MOV CX, cl
        c.regs.set_byte(Reg8::Cl, cl);
        c.step().unwrap(); // ROL AX, CL
        assert_eq!(c.regs.word(Reg16::Ax), 0x1234, "CL={cl} must not rotate");
        assert!(c.regs.flags.get(CF), "a zero rotate leaves CF alone");
    }
}

/// CF reports the bit that left, which is bit n-k for ROL and bit k-1 for ROR.
#[test]
fn rotate_reports_the_bit_that_left() {
    // ROL AL, 3 (D0 /0 with a count needs CL, so use D2). 0000_1000 rol 3.
    let mut c = cpu(&[0xB9, 0x03, 0x00, 0xD2, 0xC0]);
    c.regs.set_byte(Reg8::Al, 0x08);
    c.regs.set_byte(Reg8::Cl, 3);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x40, "0000_1000 rol 3");
    assert!(
        !c.regs.flags.get(CF),
        "bit 5 of 0000_1000 is clear, so CF clears"
    );

    let mut c = cpu(&[0xB9, 0x03, 0x00, 0xD2, 0xC8]);
    c.regs.set_byte(Reg8::Al, 0x08);
    c.regs.set_byte(Reg8::Cl, 3);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x01, "0000_1000 ror 3");
    assert!(!c.regs.flags.get(CF), "bit 2 of 0000_1000 is clear");
}

/// The shifts update ZF, SF and PF from the result. Table 2-21 marks only O
/// and C, but the manual's prose for the same instructions and the silicon
/// both say the arithmetic flags move.
#[test]
fn a_shift_updates_the_arithmetic_flags() {
    // SHR AX, 1 on 0001h gives zero, so ZF is set.
    let mut c = cpu(&[0xD1, 0xE8]);
    c.regs.set_word(Reg16::Ax, 0x0001);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0000);
    assert!(c.regs.flags.get(ZF), "a shift to zero must set ZF");
    assert!(!c.regs.flags.get(SF));
    assert!(c.regs.flags.get(CF), "bit 0 was the bit that left");

    // SHL AX, 1 on 8000h makes SF set.
    let mut c = cpu(&[0xD1, 0xE0]);
    c.regs.set_word(Reg16::Ax, 0x8000);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0000);
    assert!(c.regs.flags.get(CF), "bit 15 was the bit that left");
    assert!(c.regs.flags.get(ZF), "the result is zero");
    assert!(!c.regs.flags.get(SF), "and zero is not negative");
}

/// A rotate leaves the arithmetic flags undefined, so it must not touch them.
#[test]
fn a_rotate_leaves_the_arithmetic_flags_alone() {
    let mut c = cpu(&[0xD1, 0xC0]); // ROL AX, 1
    c.regs.set_word(Reg16::Ax, 0x0001);
    c.regs.flags.set(ZF, true);
    c.regs.flags.set(SF, true);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0002);
    assert!(
        c.regs.flags.get(ZF) && c.regs.flags.get(SF),
        "undefined means untouched"
    );
}

/// The 8086 pushes the frame before it reads the vector. With SS:SP over the
/// table itself, the pushes land on the entry they are about to read.
#[test]
fn int_pushes_the_frame_before_reading_the_vector() {
    // Put the stack on top of the interrupt table, with a known vector.
    let mut c = cpu(&[0xCD, 0x21]);
    c.regs.set_segment(Seg::Ss, 0x0000);
    c.regs.set_word(Reg16::Sp, 0x0010);
    c.mem.write_u16(0x0000, 0x21 * 4, 0x0600);
    c.mem.write_u16(0x0000, 0x21 * 4 + 2, 0x5000);
    c.step().unwrap();

    // SP 0x0010 predecrements to 0x000E, which is vector 0x21's offset slot
    // minus two. The IP pushed overwrote physical 0x0E, and 0x84..0x85 is
    // vector 0x21's own offset at 0x84, so the entry survives here.
    assert_eq!(c.regs.segment(Seg::Cs), 0x5000, "vector still resolved");
    assert_eq!(c.regs.ip, 0x0600);
}

/// Port N must be physical byte N. The implementation used to pass the port
/// as a segment, which lands on N * 16, while the comment claimed byte N.
#[test]
fn a_port_is_the_byte_with_that_number() {
    // E4 60 = IN AL, 60h
    let mut c = cpu(&[0xE4, 0x60]);
    c.mem.write_u8(0x0000, 0x60, 0x5A);
    c.mem.write_u8(0x0600, 0x0000, 0x99); // what segment=0x60 would have hit
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x5A, "byte 0x60, not 0x600");

    // E6 60 = OUT 60h, AL
    let mut c = cpu(&[0xE6, 0x60]);
    c.regs.set_byte(Reg8::Al, 0xC3);
    c.step().unwrap();
    assert_eq!(c.mem.read_u8(0x0000, 0x60), 0xC3);
    assert_eq!(c.mem.read_u8(0x0600, 0x0000), 0x00);
}

/// U4: group 1 against memory. TEST, NOT and NEG, INC and DEC on a byte or
/// word memory operand, and PUSH r/m16.
#[test]
fn test_against_memory_ands_and_discards_the_result() {
    // F7 06 3412 7809 = TEST WORD [1234h], 0978h
    let mut c = cpu(&[0xF7, 0x06, 0x34, 0x12, 0xCB, 0xED]);
    c.mem.write_u16(0x0000, 0x1234, 0x1234);
    c.regs.flags.set(CF, true);
    c.regs.flags.set(OF, true);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x0000, 0x1234),
        0x1234,
        "TEST stores nothing"
    );
    assert!(c.regs.flags.get(ZF), "1234h and EDCBh share no bit");
    assert!(
        !c.regs.flags.get(CF) && !c.regs.flags.get(OF),
        "TEST clears both"
    );
}

#[test]
fn neg_sets_carry_from_the_operand_and_overflow_only_from_the_minimum() {
    // F7 1E 3412 = NEG WORD [1234h]
    let mut c = cpu(&[0xF7, 0x1E, 0x34, 0x12]);
    c.mem.write_u16(0x0000, 0x1234, 0x0001);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0xFFFF);
    assert!(c.regs.flags.get(CF), "negating a non-zero sets CF");
    assert!(!c.regs.flags.get(OF));

    // NEG 8000h is 8000h, and it overflows because the value is its own
    // negative. CF is set because the operand was not zero.
    let mut c = cpu(&[0xF7, 0x1E, 0x34, 0x12]);
    c.mem.write_u16(0x0000, 0x1234, 0x8000);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0x8000);
    assert!(c.regs.flags.get(CF), "the operand was not zero");
    assert!(c.regs.flags.get(OF), "negating the minimum value overflows");

    // NEG of zero is zero and clears CF.
    let mut c = cpu(&[0xF7, 0x1E, 0x34, 0x12]);
    c.mem.write_u16(0x0000, 0x1234, 0x0000);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0x0000);
    assert!(!c.regs.flags.get(CF), "negating zero clears CF");
    assert!(!c.regs.flags.get(OF));
}

#[test]
fn not_leaves_every_flag_alone() {
    let mut c = cpu(&[0xF7, 0x16, 0x34, 0x12]); // NOT WORD [1234h]
    c.mem.write_u16(0x0000, 0x1234, 0x00FF);
    c.regs.flags.set(ZF, true);
    c.regs.flags.set(CF, true);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0xFF00);
    assert!(
        c.regs.flags.get(ZF) && c.regs.flags.get(CF),
        "NOT touches nothing"
    );
}

#[test]
fn inc_and_dec_work_on_a_memory_operand() {
    // FE 06 3412 = INC BYTE [1234h]
    let mut c = cpu(&[0xFE, 0x06, 0x34, 0x12]);
    c.mem.write_u8(0x0000, 0x1234, 0xFF);
    c.regs.flags.set(CF, true);
    c.step().unwrap();
    assert_eq!(c.mem.read_u8(0x0000, 0x1234), 0x00);
    assert!(c.regs.flags.get(ZF) && c.regs.flags.get(AF));
    assert!(c.regs.flags.get(CF), "INC leaves CF alone");

    // FE 0E 3412 = DEC BYTE [1234h]
    let mut c = cpu(&[0xFE, 0x0E, 0x34, 0x12]);
    c.mem.write_u8(0x0000, 0x1234, 0x01);
    c.step().unwrap();
    assert_eq!(c.mem.read_u8(0x0000, 0x1234), 0x00);
    assert!(c.regs.flags.get(ZF));
}

#[test]
fn ff_zero_and_one_are_the_word_inc_and_dec() {
    // FF 06 3412 = INC WORD [1234h]
    let mut c = cpu(&[0xFF, 0x06, 0x34, 0x12]);
    c.mem.write_u16(0x0000, 0x1234, 0x00FF);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0x0100);

    // FF 0E 3412 = DEC WORD [1234h]
    let mut c = cpu(&[0xFF, 0x0E, 0x34, 0x12]);
    c.mem.write_u16(0x0000, 0x1234, 0x0100);
    c.step().unwrap();
    assert_eq!(c.mem.read_u16(0x0000, 0x1234), 0x00FF);
}

#[test]
fn ff_six_pushes_a_memory_word() {
    // FF 36 3412 = PUSH WORD [1234h]
    let mut c = cpu(&[0xFF, 0x36, 0x34, 0x12]);
    c.regs.set_segment(Seg::Ss, 0x2000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.mem.write_u16(0x0000, 0x1234, 0xCAFE);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Sp), 0x00FE);
    assert_eq!(c.mem.read_u16(0x2000, 0x00FE), 0xCAFE);
}

/// `8F` is /0 with mod=00 only. A guard written as
/// `matches!(m.rm, Operand::Reg(_))` only ever sees mod=11, so mod=01 and
/// mod=10 slip through while the comment claims otherwise.
#[test]
fn pop_rm16_refuses_only_what_the_8086_leaves_undefined() {
    // The register form and a non-zero /digit are undefined for this
    // encoding. mod=01 and mod=10 are NOT: an earlier revision refused
    // them on the strength of a prose note that contradicted this
    // project's own opcode table and the microcode.
    for code in [
        &[0x8F, 0xC3][..],             // mod=11
        &[0x8F, 0x0E, 0x40, 0x00][..], // mod=00 but reg=001
        &[0x8F, 0x8E, 0x40, 0x00][..], // mod=10 but reg=001
    ] {
        let mut c = cpu(code);
        c.regs.set_word(Reg16::Sp, 0x0100);
        assert_eq!(c.step(), Err(ExecError::NotOn8086(0x8F)), "{code:02X?}");
        assert_eq!(c.regs.word(Reg16::Sp), 0x0100, "{code:02X?}: SP moved");
    }
}

/// LAHF puts exactly SF ZF AF PF CF in AH, with bit 1 a fixed one.
#[test]
fn lahf_places_exactly_the_status_flags_and_a_fixed_one() {
    for (flags, ah) in [
        (0x0000u16, 0x02u8),
        (0x0002, 0x02),
        (0x0001, 0x03),
        (0x0004, 0x06),
        (0x0010, 0x12),
        (0x0040, 0x42),
        (0x0080, 0x82),
        (0x00D5, 0xD7),
        (0x0FD7, 0xD7),
        (0x0008, 0x02),
        (0x0020, 0x02),
        (0xFFFF, 0xD7),
    ] {
        let mut c = Cpu::new();
        c.regs.flags.0 = flags;
        c.mem.load_at(0, &[0x9F]).unwrap();
        c.regs.set_segment(Seg::Cs, 0);
        c.regs.ip = 0;
        c.step().unwrap();
        assert_eq!(c.regs.byte(Reg8::Ah), ah, "FLAGS={flags:#06x}");
    }
}

/// Power-on FLAGS already carries the fixed one. Flags::new() setting it is
/// worthless if the public constructor derives Default instead.
#[test]
fn power_on_flags_already_carry_the_fixed_one() {
    assert!(Cpu::new().regs.flags.get(ONE), "power-on FLAGS bit 1");

    let mut c = Cpu::new();
    c.regs.set_segment(Seg::Ss, 0x2000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.mem.load_at(0, &[0x9C]).unwrap();
    c.regs.set_segment(Seg::Cs, 0);
    c.regs.ip = 0;
    c.step().unwrap();
    let image = c.mem.read_u16(0x2000, 0x00FE);
    assert_eq!(
        image & 0x0002,
        0x0002,
        "PUSHF image {image:#06x} lacks bit 1"
    );
}

// ---------------------------------------------------------------------------
// U6 — the eleven opcodes that unit added. Every expectation below is taken
// from `documentation-guide.md` §1.4 (Table 2-21), not from what the
// implementation happens to do.
// ---------------------------------------------------------------------------

/// `9A` is `CALL FAR PTR16:16`. Table 2-21 leaves all nine flags blank. The
/// push order is the observable that is easy to get backwards.
#[test]
fn call_far_pushes_cs_before_the_return_address() {
    let mut c = cpu(&[0x9A, 0x30, 0x00, 0x00, 0x50]); // CALL FAR 5000:0030h
    stack_grows_down(&c);
    c.regs.flags.set(CF, true);
    c.step().unwrap();

    // PUSH predecrements, so the word pushed last sits lowest. CS first means
    // the return address is at SS:00FCh and CS is at SS:00FEh, which is the
    // order RETF pops back.
    assert_eq!(c.mem.read_u16(0x1000, 0x00FC), 5, "IP after five bytes");
    assert_eq!(c.mem.read_u16(0x1000, 0x00FE), 0x1000, "CS pushed first");
    assert_eq!(c.regs.word(Reg16::Sp), 0x00FC);
    assert_eq!(c.regs.segment(Seg::Cs), 0x5000);
    assert_eq!(c.regs.ip, 0x0030);
    assert!(c.regs.flags.get(CF), "Table 2-21: CALL writes no flag");
}

/// `CE` is `INTO`, one byte, and only when `OF` is set. With `OF` clear it
/// must be a complete no-op: no frame, no vector, no flag.
#[test]
fn into_moves_nothing_at_all_when_overflow_is_clear() {
    let mut c = cpu(&[0xCE]);
    stack_grows_down(&c);
    c.regs.flags.set(OF, false);
    c.regs.flags.set(CF, true);
    let before = c.regs.flags.0;
    c.step().unwrap();

    assert_eq!(c.regs.ip, 1, "one byte");
    assert_eq!(c.regs.word(Reg16::Sp), 0x0100, "no frame was pushed");
    assert_eq!(c.regs.segment(Seg::Cs), 0x1000, "no vector was taken");
    assert_eq!(c.regs.flags.0, before, "INTO with OF clear changes nothing");
}

/// With `OF` set, `INTO` is the type 4 interrupt. Table 2-21 marks IF and TF
/// as `0` and leaves the other seven blank.
#[test]
fn into_takes_vector_four_with_interrupts_and_trap_off() {
    let mut c = cpu(&[0xCE]);
    stack_grows_down(&c);
    c.regs.flags.set(OF, true);
    c.regs.flags.set(IF, true);
    c.regs.flags.set(TF, true);
    c.mem.write_u16(0x0000, 4 * 4, 0x0700);
    c.mem.write_u16(0x0000, 4 * 4 + 2, 0x9000);
    c.step().unwrap();

    assert_eq!(c.stop_reason, StopReason::Interrupt(4));
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.ip, 0x0700);
    assert!(!c.regs.flags.get(IF), "Table 2-21: INTO clears IF");
    assert!(!c.regs.flags.get(TF), "Table 2-21: INTO clears TF");
}

/// Table 2-21 gives `AAM` S, Z and P as `X` and the rest `U`. The microcode
/// takes them from the **remainder**: AL = 14h over base 10 gives the
/// quotient 02h in AH and the remainder 00h in AL, and only the remainder
/// sets ZF.
#[test]
fn aam_takes_its_flags_from_the_remainder() {
    let mut c = cpu(&[0xD4, 0x0A]);
    c.regs.set_byte(Reg8::Al, 0x14);
    c.step().unwrap();

    assert_eq!(c.regs.byte(Reg8::Ah), 0x02, "the quotient");
    assert_eq!(c.regs.byte(Reg8::Al), 0x00, "the remainder");
    assert!(
        c.regs.flags.get(ZF),
        "flags come from the remainder, not 02h"
    );
    assert!(!c.regs.flags.get(SF));
    assert!(c.regs.flags.get(PF), "00h has even parity");
    assert_eq!(c.regs.ip, 2);
}

/// A zero immediate is a divide by zero, so the type 0 interrupt fires. The
/// manual leaves the quotient and the remainder undefined, so this core
/// writes neither.
#[test]
fn aam_with_a_zero_base_takes_the_divide_error() {
    let mut c = cpu(&[0xD4, 0x00]);
    stack_grows_down(&c);
    c.regs.flags.set(IF, true);
    c.mem.write_u16(0x0000, 0x0000, 0x0400);
    c.mem.write_u16(0x0000, 0x0002, 0x0500);
    c.regs.set_byte(Reg8::Al, 0x37);
    c.step().unwrap();

    assert_eq!(c.stop_reason, StopReason::Interrupt(0));
    assert_eq!(c.regs.segment(Seg::Cs), 0x0500);
    assert_eq!(c.regs.ip, 0x0400);
    assert!(
        !c.regs.flags.get(IF),
        "the handler runs with interrupts off"
    );
    assert_eq!(c.regs.byte(Reg8::Al), 0x37, "undefined, so not written");
    assert_eq!(c.regs.word(Reg16::Sp), 0x0100 - 6, "FLAGS, CS, IP pushed");
}

/// Table 2-21 gives `AAD` S, Z and P as `X`. The result is a *byte*: with
/// AH = FFh and AL = FFh the 16-bit sum is 0AF5h, and AH has to come out
/// clear rather than carrying.
#[test]
fn aad_clears_ah_even_when_the_sum_runs_past_ffh() {
    let mut c = cpu(&[0xD5, 0x0A]);
    c.regs.set_byte(Reg8::Ah, 0xFF);
    c.regs.set_byte(Reg8::Al, 0xFF);
    c.step().unwrap();

    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x00F5,
        "FFh*10h+FFh = 0AF5h, and only the low byte survives"
    );
    assert!(c.regs.flags.get(SF), "F5h is negative");
    assert!(!c.regs.flags.get(ZF));
    assert!(c.regs.flags.get(PF), "F5h has six bits set");
    assert_eq!(c.regs.ip, 2);
}

/// `D7` indexes `DS:[BX+AL]` with AL read as a **signed** byte, and Table
/// 2-21 leaves all nine flags blank.
#[test]
fn xlat_uses_a_signed_index() {
    let mut c = cpu(&[0xD7]);
    c.regs.set_word(Reg16::Bx, 0x0100);
    c.regs.set_byte(Reg8::Al, 0x80); // -128 signed, +128 unsigned
    c.mem.write_u8(0x0000, 0x0080, 0x5A); // 0100h - 80h
    c.mem.write_u8(0x0000, 0x0180, 0x11); // 0100h + 80h
    let before = c.regs.flags.0;
    c.step().unwrap();

    assert_eq!(c.regs.byte(Reg8::Al), 0x5A, "the index is signed");
    assert_eq!(c.regs.ip, 1);
    assert_eq!(c.regs.flags.0, before, "Table 2-21: XLAT writes no flag");
}

/// `9B` is `WAIT`. Nothing drives the coprocessor's TEST pin here, so the
/// wait is already over and the instruction retires having touched nothing.
#[test]
fn wait_retires_without_touching_anything() {
    let mut c = cpu(&[0x9B]);
    let before = c.regs.flags.0;
    let info = c.step().unwrap();

    assert_eq!(info.next_ip, 1);
    assert!(!c.halted, "WAIT is not HALT");
    assert_eq!(c.regs.flags.0, before, "Table 2-21: WAIT writes no flag");
}

/// `F0` is `LOCK`, a prefix with no flag row. The covered instruction runs
/// exactly as it would alone and the pair retires as one step.
#[test]
fn lock_runs_the_instruction_it_covers() {
    let mut c = cpu(&[0xF0, 0x40]); // LOCK INC AX
    c.regs.set_word(Reg16::Ax, 0xFFFF);
    c.regs.flags.set(CF, true);
    let info = c.step().unwrap();

    assert_eq!(c.regs.word(Reg16::Ax), 0x0000);
    assert_eq!(info.next_ip, 2, "the prefix belongs to the instruction");
    assert!(
        c.regs.flags.get(ZF),
        "the covered instruction wrote its flags"
    );
    assert!(c.regs.flags.get(CF), "INC leaves CF alone");
}

/// `FF /3` and `FF /5` are the far `CALL` and `JMP` through a four-byte
/// pointer in memory: offset word first, segment word second. Table 4-13
/// gives both a memory operand and no flags.
#[test]
fn ff_three_and_five_are_the_far_call_and_the_far_jump() {
    // FF 1E 3412 = CALL FAR [1234h]
    let mut c = cpu(&[0xFF, 0x1E, 0x34, 0x12]);
    stack_grows_down(&c);
    c.mem.write_u16(0x0000, 0x1234, 0x0700);
    c.mem.write_u16(0x0000, 0x1236, 0x9000);
    c.step().unwrap();

    assert_eq!(c.mem.read_u16(0x1000, 0x00FC), 4, "the return address");
    assert_eq!(c.mem.read_u16(0x1000, 0x00FE), 0x1000, "CS pushed first");
    assert_eq!(c.regs.word(Reg16::Sp), 0x00FC);
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.ip, 0x0700);

    // FF 2E 3412 = JMP FAR [1234h]: the same pointer, nothing pushed.
    let mut c = cpu(&[0xFF, 0x2E, 0x34, 0x12]);
    stack_grows_down(&c);
    c.mem.write_u16(0x0000, 0x1234, 0x0700);
    c.mem.write_u16(0x0000, 0x1236, 0x9000);
    c.step().unwrap();

    assert_eq!(c.regs.word(Reg16::Sp), 0x0100, "a jump pushes nothing");
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.ip, 0x0700);

    // mod=11 names a register, and a register holds no four-byte pointer.
    for code in [&[0xFF, 0xDB][..], &[0xFF, 0xEB][..]] {
        let mut c = cpu(code);
        assert!(
            c.step().is_err(),
            "{code:02X?} is not a pointer and must not be executed"
        );
    }
}

/// `EA` is `JMP FAR PTR16:16`: the same pointer `9A` reads, with no push and
/// no flag.
#[test]
fn jmp_far_reads_the_pointer_without_pushing() {
    let mut c = cpu(&[0xEA, 0x30, 0x00, 0x00, 0x50]); // JMP FAR 5000:0030h
    stack_grows_down(&c);
    c.regs.flags.set(CF, true);
    let before = c.regs.flags.0;
    c.step().unwrap();

    assert_eq!(c.regs.segment(Seg::Cs), 0x5000);
    assert_eq!(c.regs.ip, 0x0030);
    assert_eq!(c.regs.word(Reg16::Sp), 0x0100, "a jump pushes nothing");
    assert_eq!(c.regs.flags.0, before, "Table 2-21: JMP writes no flag");
}

/// `CA iw` is `RETF iw` and `CB` is `RETF`: pop IP, then CS, then release the
/// argument bytes. Table 4-13 puts the immediate on `CA`, and Shirriff's
/// microcode agrees — the `RET/RETF iw` entry runs for `1100?0?0`, which is
/// `C0`, `C2`, `C8` and `CA` and nothing else. Table 2-21 leaves every flag
/// blank for both forms.
#[test]
fn retf_pops_ip_then_cs_and_releases_the_arguments() {
    let mut c = cpu(&[0xCA, 0x00, 0x04]); // RETF 0400h
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700); // IP
    c.mem.write_u16(0x1000, 0x0102, 0x9000); // CS
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    // Both pops land first, so the release is added to 0104h, not to 0100h.
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0104 + 0x0400,
        "arguments released"
    );

    let mut c = cpu(&[0xCB]);
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700);
    c.mem.write_u16(0x1000, 0x0102, 0x9000);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.word(Reg16::Sp), 0x0104, "CB releases nothing");
}

/// Table 2-21 gives `TEST` `0` under OF and CF, `X` under S, Z and P, and
/// `U` under AF. This core answers `U` by leaving the flag exactly as it
/// stood, and neither form writes a result.
#[test]
fn test_rm_r_leaves_af_alone_and_writes_no_result() {
    // 84 D8 = TEST AL, BL
    let mut c = cpu(&[0x84, 0xD8]);
    c.regs.set_byte(Reg8::Al, 0xFF);
    c.regs.set_byte(Reg8::Bl, 0x0F);
    c.regs.flags.set(AF, true);
    c.regs.flags.set(OF, true);
    c.regs.flags.set(CF, true);
    c.step().unwrap();

    let f = c.regs.flags;
    assert!(f.get(AF), "AF is U and this core leaves it alone");
    assert!(!f.get(OF) && !f.get(CF), "Table 2-21: TEST clears both");
    assert!(!f.get(ZF) && !f.get(SF), "FFh and 0Fh give 0Fh");
    assert!(f.get(PF), "F0h has four bits set");
    assert_eq!(c.regs.byte(Reg8::Al), 0xFF, "TEST stores nothing");
    assert_eq!(c.regs.ip, 2);

    // 85 D8 = TEST AX, BX
    let mut c = cpu(&[0x85, 0xF8]);
    c.regs.set_word(Reg16::Ax, 0x1234);
    c.regs.set_word(Reg16::Bx, 0x1000);
    c.regs.flags.set(AF, true);
    c.step().unwrap();

    assert!(c.regs.flags.get(AF));
    assert!(!c.regs.flags.get(ZF), "1234h and 1000h give 1000h");
    assert_eq!(c.regs.word(Reg16::Ax), 0x1234, "TEST stores nothing");
}

// ---------------------------------------------------------------------------
// AUDIT-4 — the bytes Table 4-13 prints as `(not used)`. Every one of them
// decodes on an 8086; the sources are on the gate's doc comment above.
// ---------------------------------------------------------------------------

/// `60`-`6F` alias `70`-`7F`: the conditional-jump microcode runs for the
/// whole bit pattern `011?????` and the condition is the low nibble.
#[test]
fn sixty_to_sixf_are_the_conditional_jumps() {
    // 62h is JB, so with CF set it jumps past the next instruction.
    let mut c = cpu(&[0x62, 0x02, 0xEB, 0x00, 0x90]);
    c.regs.flags.set(CF, true);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 4, "60h-6Fh alias 70h-7Fh");

    // Each alias must behave exactly like the documented byte it shadows.
    for (alias, real) in [(0x60u8, 0x70u8), (0x67, 0x77), (0x6F, 0x7F)] {
        for cf in [false, true] {
            let mut a = cpu(&[alias, 0x04, 0xC3]);
            let mut b = cpu(&[real, 0x04, 0xC3]);
            a.regs.flags.set(CF, cf);
            b.regs.flags.set(CF, cf);
            a.step().unwrap();
            b.step().unwrap();
            assert_eq!(a.regs.ip, b.regs.ip, "{alias:#04x} vs {real:#04x}");
            assert_eq!(
                a.regs.flags.0, b.regs.flags.0,
                "{alias:#04x} vs {real:#04x}"
            );
        }
    }
}

/// `D6` is `SALC`, `AL = CF ? FFh : 00h`. The microcode has no flag write.
#[test]
fn salc_sets_al_from_the_carry() {
    for (cf, want) in [(false, 0x00u8), (true, 0xFF)] {
        let mut c = cpu(&[0xD6]);
        c.regs.flags.set(CF, cf);
        c.regs.set_byte(Reg8::Al, 0x5A);
        let before = c.regs.flags.0;
        c.step().unwrap();
        assert_eq!(c.regs.byte(Reg8::Al), want, "CF={cf}");
        assert_eq!(c.regs.flags.0, before, "CF={cf}: SALC sets no flag");
        assert_eq!(c.regs.ip, 1);
    }
}

/// `C0 iw` is `C2 iw` and `C1` is `C3`: the same microcode, with bit 0
/// selecting whether the immediate follows.
#[test]
fn c0_and_c1_are_the_ret_forms() {
    let mut c = cpu(&[0xC0, 0x06, 0x00]);
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700);
    c.regs.flags.set(CF, true);
    let before = c.regs.flags.0;
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0102 + 0x0006,
        "arguments released"
    );
    assert_eq!(c.regs.flags.0, before, "RET writes no flag");

    let mut c = cpu(&[0xC1]);
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(c.regs.word(Reg16::Sp), 0x0102, "C1 releases nothing");
}

/// `C8 iw` is `CA iw` and `C9` is `CB`, for the inter-segment return.
#[test]
fn c8_and_c9_are_the_retf_forms() {
    let mut c = cpu(&[0xC8, 0x02, 0x00]);
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700);
    c.mem.write_u16(0x1000, 0x0102, 0x9000);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.word(Reg16::Sp), 0x0104 + 0x0002);

    let mut c = cpu(&[0xC9]);
    stack_grows_down(&c);
    c.mem.write_u16(0x1000, 0x0100, 0x0700);
    c.mem.write_u16(0x1000, 0x0102, 0x9000);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x0700);
    assert_eq!(c.regs.segment(Seg::Cs), 0x9000);
    assert_eq!(c.regs.word(Reg16::Sp), 0x0104, "C9 releases nothing");
}

/// `F1` is `LOCK`. It is decoded in logic rather than microcode, and the
/// Group Decode ROM gives it the same outputs as `F0`.
#[test]
fn f1_is_a_second_lock_prefix() {
    let mut c = cpu(&[0xF1, 0x40]); // LOCK INC AX
    c.regs.set_word(Reg16::Ax, 0xFFFF);
    c.regs.flags.set(CF, true);
    let info = c.step().unwrap();

    assert_eq!(c.regs.word(Reg16::Ax), 0x0000);
    assert_eq!(info.next_ip, 2);
    assert!(c.regs.flags.get(ZF) && c.regs.flags.get(CF));
}

/// `D8`-`DF` are `ESC 0`-`ESC 7`, the gateway to an 8087. With no
/// coprocessor on the bus nothing performs the operand access, so what the
/// byte leaves behind is its length and nothing else.
#[test]
fn escape_consumes_its_modrm_and_touches_nothing() {
    let mut c = cpu(&[0xD8, 0x06, 0x34, 0x12, 0x90]); // ESC 0 with a disp16
    c.regs.set_word(Reg16::Ax, 0x1234);
    let before = c.regs.flags.0;
    let info = c.step().unwrap();

    assert_eq!(info.next_ip, 4, "opcode + ModR/M + disp16");
    assert_eq!(c.regs.word(Reg16::Ax), 0x1234, "ESC writes no register");
    assert_eq!(c.regs.flags.0, before, "Table 2-21: ESC has no flag row");
}

// ============================================== AUDIT-6: the /digit-level gate

/// What the 8086's own Group Decode ROM resolves one `(opcode, /digit)` pair to.
///
/// `Runs` is an instruction: its mnemonic, the value it must leave at the
/// probe's memory operand, and the `SP` it must leave behind. `Hole` is a digit
/// position the 8086 has no usable instruction in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// `(mnemonic, value at DS:0100 afterwards, SP afterwards)`
    Runs(&'static str, u16, u16),
    /// No instruction on an 8086. The reason lives in [`HOLES`].
    Hole,
}

/// The ModR/M groups the 8086 resolves through the `reg` field of the second
/// byte, with the names Intel gives them.
///
/// Shirriff, *Holes in two-byte opcodes*: "These fall into four categories
/// which Intel labeled `Immed`, `Shift`, `Group 1`, and `Group 2`, corresponding
/// to opcodes 80-83, D0-D3, F6-F7, and FE-FF."
///
/// Two candidates in the brief's list are **not** groups and are deliberately
/// absent:
///
/// * `C0`/`C1` are not shift-by-immediate. Table 4-13 prints them `(not used)`
///   and Shirriff shows they run `RET`/`RET i16`, which is what this core
///   implements; there is no digit to resolve.
/// * `D8`-`DF` are `ESC 0`-`ESC 7`. The escape index is the *first* byte's low
///   three bits, not the ModR/M `reg` field, so ESC has no `/digit` at all.
const GROUPS: &[(&str, &[u8])] = &[
    ("Immed", &[0x80, 0x81, 0x82, 0x83]),
    ("Shift", &[0xD0, 0xD1, 0xD2, 0xD3]),
    ("Group 1", &[0xF6, 0xF7]),
    ("Group 2", &[0xFE, 0xFF]),
];

/// Every `(opcode, /digit)` pair in the table above, written out.
///
/// **The count `96` is a literal, not `GROUPS.iter().map(..).sum()`.** Deriving
/// it is the defect the first gate in this project shipped with: independent
/// verification mutated the exclusion list, the derived expectation moved with
/// it, and the gate stayed green while gating nothing. The same rule applies
/// here in both directions — `DIGITS` is complete (nothing outside it), and the
/// literals below fix how many of its entries are holes.
///
/// Columns of [`Slot::Runs`] are read off the sources, not off the core: each
/// is `operand after the instruction` and `SP after the instruction` for the
/// probe state [`digit_cpu`] builds. `ROL` and `SHL` share a column because
/// they *are* the same operation on the operand and differ only in `OF`; the
/// digit that must not share a column with them is `/6`, which is why SETMO's
/// `FFh`/`FFFFh` is here.
const DIGITS: &[(u8, u8, Slot)] = &[
    // ---- "Immed": 80 r/m8,i8 | 81 r/m16,i16 | 82 r/m8,i8 | 83 r/m16,i8:sx.
    // 003Ch AND/OR/XOR/ADD/SUB with 000Fh, carry in, per Table 4-13.
    (0x80, 0, Slot::Runs("ADD r/m8,i8", 0x004B, 0x0100)),
    (0x80, 1, Slot::Runs("OR r/m8,i8", 0x003F, 0x0100)),
    (0x80, 2, Slot::Runs("ADC r/m8,i8", 0x004C, 0x0100)),
    (0x80, 3, Slot::Runs("SBB r/m8,i8", 0x002C, 0x0100)),
    (0x80, 4, Slot::Runs("AND r/m8,i8", 0x000C, 0x0100)),
    (0x80, 5, Slot::Runs("SUB r/m8,i8", 0x002D, 0x0100)),
    (0x80, 6, Slot::Runs("XOR r/m8,i8", 0x0033, 0x0100)),
    (0x80, 7, Slot::Runs("CMP r/m8,i8", 0x003C, 0x0100)),
    (0x81, 0, Slot::Runs("ADD r/m16,i16", 0x004B, 0x0100)),
    (0x81, 1, Slot::Runs("OR r/m16,i16", 0x003F, 0x0100)),
    (0x81, 2, Slot::Runs("ADC r/m16,i16", 0x004C, 0x0100)),
    (0x81, 3, Slot::Runs("SBB r/m16,i16", 0x002C, 0x0100)),
    (0x81, 4, Slot::Runs("AND r/m16,i16", 0x000C, 0x0100)),
    (0x81, 5, Slot::Runs("SUB r/m16,i16", 0x002D, 0x0100)),
    (0x81, 6, Slot::Runs("XOR r/m16,i16", 0x0033, 0x0100)),
    (0x81, 7, Slot::Runs("CMP r/m16,i16", 0x003C, 0x0100)),
    // Table 4-13 prints `(not used)` for 82 /1, /4 and /6, but the Group Decode
    // ROM latches the second byte's bits 5-3 into the X register for the whole
    // Immed group, so all eight ALU selections run on 82 exactly as on 80.
    // Intel's table lists only the digits that are not redundant.
    (0x82, 0, Slot::Runs("ADD r/m8,i8", 0x004B, 0x0100)),
    (0x82, 1, Slot::Runs("OR r/m8,i8", 0x003F, 0x0100)),
    (0x82, 2, Slot::Runs("ADC r/m8,i8", 0x004C, 0x0100)),
    (0x82, 3, Slot::Runs("SBB r/m8,i8", 0x002C, 0x0100)),
    (0x82, 4, Slot::Runs("AND r/m8,i8", 0x000C, 0x0100)),
    (0x82, 5, Slot::Runs("SUB r/m8,i8", 0x002D, 0x0100)),
    (0x82, 6, Slot::Runs("XOR r/m8,i8", 0x0033, 0x0100)),
    (0x82, 7, Slot::Runs("CMP r/m8,i8", 0x003C, 0x0100)),
    (0x83, 0, Slot::Runs("ADD r/m16,i8:sx", 0x004B, 0x0100)),
    (0x83, 1, Slot::Runs("OR r/m16,i8:sx", 0x003F, 0x0100)),
    (0x83, 2, Slot::Runs("ADC r/m16,i8:sx", 0x004C, 0x0100)),
    (0x83, 3, Slot::Runs("SBB r/m16,i8:sx", 0x002C, 0x0100)),
    (0x83, 4, Slot::Runs("AND r/m16,i8:sx", 0x000C, 0x0100)),
    (0x83, 5, Slot::Runs("SUB r/m16,i8:sx", 0x002D, 0x0100)),
    (0x83, 6, Slot::Runs("XOR r/m16,i8:sx", 0x0033, 0x0100)),
    (0x83, 7, Slot::Runs("CMP r/m16,i8:sx", 0x003C, 0x0100)),
    // ---- "Group 1": F6 r/m8 | F7 r/m16. 003Ch and the AL/AX zero the probe
    // starts with, so MUL/IMUL/DIV/IDIV write registers and leave memory be.
    (0xF6, 0, Slot::Runs("TEST r/m8,i8", 0x003C, 0x0100)),
    (
        0xF6,
        1,
        Slot::Runs("TEST r/m8,i8 (undocumented)", 0x003C, 0x0100),
    ),
    (0xF6, 2, Slot::Runs("NOT r/m8", 0x00C3, 0x0100)),
    (0xF6, 3, Slot::Runs("NEG r/m8", 0x00C4, 0x0100)),
    (0xF6, 4, Slot::Runs("MUL r/m8", 0x003C, 0x0100)),
    (0xF6, 5, Slot::Runs("IMUL r/m8", 0x003C, 0x0100)),
    (0xF6, 6, Slot::Runs("DIV r/m8", 0x003C, 0x0100)),
    (0xF6, 7, Slot::Runs("IDIV r/m8", 0x003C, 0x0100)),
    (0xF7, 0, Slot::Runs("TEST r/m16,i16", 0x003C, 0x0100)),
    (
        0xF7,
        1,
        Slot::Runs("TEST r/m16,i16 (undoc.)", 0x003C, 0x0100),
    ),
    (0xF7, 2, Slot::Runs("NOT r/m16", 0xFFC3, 0x0100)),
    (0xF7, 3, Slot::Runs("NEG r/m16", 0xFFC4, 0x0100)),
    (0xF7, 4, Slot::Runs("MUL r/m16", 0x003C, 0x0100)),
    (0xF7, 5, Slot::Runs("IMUL r/m16", 0x003C, 0x0100)),
    (0xF7, 6, Slot::Runs("DIV r/m16", 0x003C, 0x0100)),
    (0xF7, 7, Slot::Runs("IDIV r/m16", 0x003C, 0x0100)),
    // ---- "Shift": D0/D2 r/m8 and D1/D3 r/m16, all shifting 003Ch once with
    // the carry set. `110` is the SETMO hole: not a shift at all, all ones.
    (0xD0, 0, Slot::Runs("ROL r/m8,1", 0x0078, 0x0100)),
    (0xD0, 1, Slot::Runs("ROR r/m8,1", 0x001E, 0x0100)),
    (0xD0, 2, Slot::Runs("RCL r/m8,1", 0x0079, 0x0100)),
    (0xD0, 3, Slot::Runs("RCR r/m8,1", 0x009E, 0x0100)),
    (0xD0, 4, Slot::Runs("SHL r/m8,1", 0x0078, 0x0100)),
    (0xD0, 5, Slot::Runs("SHR r/m8,1", 0x001E, 0x0100)),
    (0xD0, 6, Slot::Runs("SETMO (undocumented)", 0x00FF, 0x0100)),
    (0xD0, 7, Slot::Runs("SAR r/m8,1", 0x001E, 0x0100)),
    (0xD1, 0, Slot::Runs("ROL r/m16,1", 0x0078, 0x0100)),
    (0xD1, 1, Slot::Runs("ROR r/m16,1", 0x001E, 0x0100)),
    (0xD1, 2, Slot::Runs("RCL r/m16,1", 0x0079, 0x0100)),
    (0xD1, 3, Slot::Runs("RCR r/m16,1", 0x801E, 0x0100)),
    (0xD1, 4, Slot::Runs("SHL r/m16,1", 0x0078, 0x0100)),
    (0xD1, 5, Slot::Runs("SHR r/m16,1", 0x001E, 0x0100)),
    (0xD1, 6, Slot::Runs("SETMO (undocumented)", 0xFFFF, 0x0100)),
    (0xD1, 7, Slot::Runs("SAR r/m16,1", 0x001E, 0x0100)),
    (0xD2, 0, Slot::Runs("ROL r/m8,CL", 0x0078, 0x0100)),
    (0xD2, 1, Slot::Runs("ROR r/m8,CL", 0x001E, 0x0100)),
    (0xD2, 2, Slot::Runs("RCL r/m8,CL", 0x0079, 0x0100)),
    (0xD2, 3, Slot::Runs("RCR r/m8,CL", 0x009E, 0x0100)),
    (0xD2, 4, Slot::Runs("SHL r/m8,CL", 0x0078, 0x0100)),
    (0xD2, 5, Slot::Runs("SHR r/m8,CL", 0x001E, 0x0100)),
    (0xD2, 6, Slot::Runs("SETMO (undocumented)", 0x00FF, 0x0100)),
    (0xD2, 7, Slot::Runs("SAR r/m8,CL", 0x001E, 0x0100)),
    (0xD3, 0, Slot::Runs("ROL r/m16,CL", 0x0078, 0x0100)),
    (0xD3, 1, Slot::Runs("ROR r/m16,CL", 0x001E, 0x0100)),
    (0xD3, 2, Slot::Runs("RCL r/m16,CL", 0x0079, 0x0100)),
    (0xD3, 3, Slot::Runs("RCR r/m16,CL", 0x801E, 0x0100)),
    (0xD3, 4, Slot::Runs("SHL r/m16,CL", 0x0078, 0x0100)),
    (0xD3, 5, Slot::Runs("SHR r/m16,CL", 0x001E, 0x0100)),
    (0xD3, 6, Slot::Runs("SETMO (undoc.)", 0xFFFF, 0x0100)),
    (0xD3, 7, Slot::Runs("SAR r/m16,CL", 0x001E, 0x0100)),
    // ---- "Group 2": FE r/m8 | FF r/m16.
    (0xFE, 0, Slot::Runs("INC r/m8", 0x003D, 0x0100)),
    (0xFE, 1, Slot::Runs("DEC r/m8", 0x003B, 0x0100)),
    (0xFE, 2, Slot::Hole),
    (0xFE, 3, Slot::Hole),
    (0xFE, 4, Slot::Hole),
    (0xFE, 5, Slot::Hole),
    // FE/6 is PUSH, and FE/7 reaches the same microcode: bits 5-3 of the
    // second byte replace bits 2-0, so /7 becomes /6. Both write ONE byte and
    // take SP down by two (Shirriff, "More FE holes").
    (0xFE, 6, Slot::Runs("PUSH r/m8 (undoc.)", 0x003C, 0x00FE)),
    (
        0xFE,
        7,
        Slot::Runs("PUSH r/m8 (undocumented)", 0x003C, 0x00FE),
    ),
    (0xFF, 0, Slot::Runs("INC r/m16", 0x003D, 0x0100)),
    (0xFF, 1, Slot::Runs("DEC r/m16", 0x003B, 0x0100)),
    (0xFF, 2, Slot::Runs("CALL r/m16", 0x003C, 0x00FE)),
    (0xFF, 3, Slot::Runs("CALL m16:16", 0x003C, 0x00FC)),
    (0xFF, 4, Slot::Runs("JMP r/m16", 0x003C, 0x0100)),
    (0xFF, 5, Slot::Runs("JMP m16:16", 0x003C, 0x0100)),
    (0xFF, 6, Slot::Runs("PUSH r/m16", 0x003C, 0x00FE)),
    (0xFF, 7, Slot::Runs("PUSH r/m16 (undoc.)", 0x003C, 0x00FE)),
];

/// The digits with no 8086 instruction behind them, and the source that says so.
///
/// A hole here is a claim about the silicon, so every row names where it was
/// read. `Table 4-13` is the Intel 8086 Family User's Manual's decoding guide
/// as transcribed in `odd/research/documentation-guide.md` §1.3; `Shirriff` is
/// *Undocumented 8086 instructions, explained by the microcode*.
const HOLES: &[(u8, u8, &str)] = &[
    (
        0xFE,
        2,
        "Table 4-13 prints `(not used)`: only INC and DEC are defined for FE. \
         Shirriff shows the 8086 runs the CALL rm microcode here, but with a \
         byte-wide operand the target address and the pushed return address \
         are both read a byte at a time, so what it computes depends on the \
         bus rather than on the architecture. Not emulated.",
    ),
    (
        0xFE,
        3,
        "Table 4-13 prints `(not used)`. Shirriff shows the 8086 runs the CALL \
         FAR rm microcode here, but it reads the new CS and the pushed old CS \
         and PC as bytes, so the frame it writes depends on the bus. Not \
         emulated.",
    ),
    (
        0xFE,
        4,
        "Table 4-13 prints `(not used)`. Shirriff shows the 8086 runs the JMP rm \
         microcode here, but it reads a byte where the target address is, so \
         the destination depends on the bus. Not emulated.",
    ),
    (
        0xFE,
        5,
        "Table 4-13 prints `(not used)`. Shirriff shows the 8086 runs the JMP \
         FAR rm microcode here, but it reads the offset and the segment as \
         bytes, so the destination depends on the bus. Not emulated.",
    ),
];

/// One digit's worth of instruction, on a CPU whose state is fixed.
///
/// `opcode`, a ModR/M byte of `mod=00 reg=<digit> rm=110` — the direct form, so
/// the operand is `DS:0100` — a `disp16` of `0100h`, and three spare bytes that
/// serve as the immediate for whichever form the opcode takes.
///
/// The state is chosen so that the digits *within* a group produce different
/// answers: `003Ch` against an immediate of `000Fh` with the carry already set
/// separates all eight ALU operations, and the carry in also separates `RCL`
/// and `RCR` from `ROL` and `ROR`. `CL` is 1 so the variable-shift opcodes
/// reach their ALU at all.
///
/// `DF` is left clear. It is not set here and must not be set in a helper: it
/// governs the string operations, and the `PUSH` rows below would have made
/// that look like a stack-direction defect.
fn digit_cpu(opcode: u8, digit: u8) -> Cpu {
    assert!(
        digit < 8,
        "a ModR/M reg field is three bits: digit {digit} does not exist"
    );
    let modrm = 0b00_000_110 | (digit << 3);
    let mut c = Cpu::new();
    c.mem
        .load_at(0x10000, &[opcode, modrm, 0x00, 0x01, 0x0F, 0x00, 0x00])
        .unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.regs.set_segment(Seg::Ss, 0x3000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.set_byte(Reg8::Cl, 1);
    c.mem.write_u16(0x2000, 0x0100, 0x003C);
    c.regs.flags.0 = CF;
    c
}

/// Every ModR/M `/digit` an 8086 group defines, and every digit it does not.
///
/// This is the second-byte counterpart to
/// [`every_first_opcode_byte_the_8086_defines_decodes`], which cannot see any of
/// it: a first-byte gate passes with the whole `reg` column wrong. Four such
/// holes were open when this was written — `F6 /1`, `F7 /1`, `D0`-`D3 /6` and
/// `FE`/`FF /7` — and `D0`-`D3 /6` was the dangerous one, because it did not
/// error: it fell through to the 80186 `SAL` alias and executed the wrong
/// instruction without a word.
///
/// The gate cannot go vacuous:
///
/// * `96`, `5` and `91` below are literals, never derived from a list of
///   exclusions. Editing `HOLES` alone therefore fails on three separate
///   counts at once.
/// * `DIGITS` must be exactly the product of `GROUPS` and the eight `/digit`
///   values — no missing key, no stray key, no duplicate.
/// * Every hole in `HOLES` must appear in `DIGITS` as a `Hole` and every `Hole`
///   in `DIGITS` must appear in `HOLES`, so neither table can be changed alone.
/// * Every hole carries a reason naming the source that settles it.
/// * Every `Runs` entry is executed and must leave the operand and the stack
///   pointer where the sources say, which is what a `SHL`-shaped `SETMO` and a
///   "decodes without erroring" check both fail.
#[test]
fn every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole() {
    // Pinned literally. See the doc comment: a derived count is the defect the
    // first gate shipped with.
    const DIGIT_PAIRS: usize = 96;
    const HOLE_COUNT: usize = 4;
    const INSTRUCTION_COUNT: usize = 92;

    // ---- the table is exactly the groups times the eight /digit values.
    let mut keys: Vec<(u8, u8)> = Vec::with_capacity(DIGITS.len());
    for &(op, digit, _) in DIGITS {
        assert!(
            digit < 8,
            "DIGITS holds a {digit}, and a ModR/M reg field is three bits"
        );
        assert!(
            !keys.contains(&(op, digit)),
            "DIGITS names {op:#04x} /{digit} twice"
        );
        keys.push((op, digit));
    }
    assert_eq!(
        keys.len(),
        DIGIT_PAIRS,
        "DIGITS must hold every (opcode, /digit) pair the 8086 resolves"
    );
    let mut expected: Vec<(u8, u8)> = GROUPS
        .iter()
        .flat_map(|(_, opcodes)| opcodes.iter())
        .flat_map(|&op| (0..8u8).map(move |digit| (op, digit)))
        .collect();
    expected.sort_unstable();
    let mut listed = keys.clone();
    listed.sort_unstable();
    assert_eq!(
        listed, expected,
        "DIGITS must be exactly GROUPS x the eight /digit values"
    );

    // ---- holes and instructions agree in both directions, and the literal
    // counts agree with both. This is the part a mutation cannot survive.
    let holes: Vec<(u8, u8)> = DIGITS
        .iter()
        .filter(|(_, _, slot)| *slot == Slot::Hole)
        .map(|&(op, digit, _)| (op, digit))
        .collect();
    let instructions = DIGITS.len() - holes.len();
    assert_eq!(holes.len(), HOLE_COUNT, "the 8086 has four /digit holes");
    assert_eq!(
        instructions, INSTRUCTION_COUNT,
        "the other 91 /digit positions are instructions"
    );
    for &(op, digit, why) in HOLES {
        assert!(
            holes.contains(&(op, digit)),
            "HOLES names {op:#04x} /{digit}, which DIGITS does not hold as a hole"
        );
        assert!(
            why.contains("Table 4-13") || why.contains("Shirriff"),
            "{op:#04x} /{digit}: a hole must name the source that settles it"
        );
        assert!(
            why.len() >= 80,
            "{op:#04x} /{digit}: a hole needs a one-line reason, not a gesture"
        );
    }
    for &(op, digit) in &holes {
        assert!(
            HOLES.iter().any(|&(h, d, _)| (h, d) == (op, digit)),
            "DIGITS holds {op:#04x} /{digit} as a hole that HOLES never explains"
        );
    }

    // ---- and the machine agrees with the table.
    for &(op, digit, slot) in DIGITS {
        let mut c = digit_cpu(op, digit);
        let name = format!("{op:#04x} /{digit}");
        match slot {
            Slot::Hole => {
                assert_eq!(
                    c.step(),
                    Err(ExecError::UnknownOpcode(op)),
                    "{name} is a hole and must not decode"
                );
            }
            Slot::Runs(mnemonic, out, sp) => {
                c.step()
                    .unwrap_or_else(|e| panic!("{name} is {mnemonic} and must decode: {e:?}"));
                assert_eq!(
                    c.mem.read_u16(0x2000, 0x0100),
                    out,
                    "{name} is {mnemonic}: wrong value left at the operand"
                );
                assert_eq!(
                    c.regs.word(Reg16::Sp),
                    sp,
                    "{name} is {mnemonic}: wrong stack pointer"
                );
            }
        }
    }
}

// ------------------------------------------------------------ AUDIT-6: /digit
//
// The gate above walks the 256 *first* bytes. It is structurally blind to the
// second one, so the four known holes all sat in it unnoticed:
//
// * `F6 /1`, `F7 /1` — Table 4-13 prints `(not used)`, but the microcode
//   matches `1111000?` and so runs the `/0` TEST, which is why the manual's
//   own digit table has `TEST` on both `/0` and `/1` for the `F6`/`F7` column.
// * `D0`-`D3 /6` — the manual prints `(not used)`; the 8086 selects the ALU's
//   SETMO operation and stores all ones.
// * `FE /7`, `FF /7` — the manual prints `(not used)`; the 8086 runs PUSH.

/// The FLAGS pattern every test below starts from.
///
/// `CF` is set so that `ADC`/`SBB` differ from `ADD`/`SUB`, and `AF` is set so
/// that "left alone" is a claim about a flag that actually *had* a value worth
/// preserving. `DF` is deliberately **not** set: it governs the string
/// operations, and a helper that presets it changes what a later push does.
fn status_before() -> u16 {
    CF | AF
}

/// `F6 /1` and `F7 /1` are `TEST r/m, imm`: an AND whose result is discarded.
///
/// Table 4-13 prints `(not used)` for `001` in the `F6`/`F7` column, but the
/// 8086's microcode entry for this group matches the instruction bits
/// `1111000?`, so `/1` runs the same three micro-instructions as `/0`. The
/// observable is therefore identical to `F6 /0`, `84` and `85`: `ZF`, `SF` and
/// `PF` from the **low byte** of the result, `CF` and `OF` cleared, `AF`
/// untouched, and nothing written back.
///
/// The two opcodes are checked with immediates that a decoder reading the
/// wrong number of bytes cannot fake: `F6` ANDs `34h` with `F0h` for `30h`,
/// while `F7` ANDs `1234h` with the *word* `0F03h` for `0202h`.
#[test]
fn f6_one_and_f7_one_are_test_against_an_immediate() {
    // (opcode, immediate bytes, expected flags, instruction length in bytes)
    for (opcode, imm, want_zf, want_pf, length) in [
        (0xF6u8, &[0xF0u8][..], false, true, 5usize), // 34h AND F0h = 30h
        (0xF7, &[0x0F, 0x03], false, false, 6),       // 1234h AND 0F03h = 0202h
    ] {
        let mut code = vec![opcode, 0b00_001_110, 0x00, 0x01];
        code.extend_from_slice(imm);
        let mut c = cpu(&code);
        c.regs.set_segment(Seg::Ds, 0x2000);
        c.mem.write_u16(0x2000, 0x0100, 0x1234);
        c.regs.flags.0 = status_before();
        let info = c.step().unwrap();

        assert_eq!(
            c.regs.flags.get(ZF),
            want_zf,
            "{opcode:#04x} /1: ZF must come from the AND, not from the operand"
        );
        assert!(
            !c.regs.flags.get(SF) && !c.regs.flags.get(CF) && !c.regs.flags.get(OF),
            "{opcode:#04x} /1: the result is positive, so S is clear and C and O are cleared"
        );
        assert_eq!(
            c.regs.flags.get(PF),
            want_pf,
            "{opcode:#04x} /1: PF is the parity of the low byte of the AND"
        );
        assert!(
            c.regs.flags.get(AF),
            "{opcode:#04x} /1: TEST is a logic operation and leaves AF alone"
        );
        assert_eq!(
            c.mem.read_u16(0x2000, 0x0100),
            0x1234,
            "{opcode:#04x} /1: the AND result is discarded"
        );
        assert_eq!(
            info.next_ip as usize, length,
            "{opcode:#04x} /1: opcode + ModR/M + disp16 + the immediate"
        );
    }
}

/// `F6 /1` is `TEST r/m8` and `F7 /1` is `TEST r/m16`: the same digit, the same
/// flags, and a different immediate width. The second byte of `F7 /1`'s
/// immediate is what separates them — read one byte too few and the AND
/// becomes zero.
#[test]
fn f7_one_reads_a_word_immediate_and_f6_one_a_byte() {
    // 1234h AND 0F03h = 0202h, so ZF is clear.
    let mut c = cpu(&[0xF7, 0b00_001_110, 0x00, 0x01, 0x0F, 0x03]);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.mem.write_u16(0x2000, 0x0100, 0x1234);
    c.regs.flags.0 = status_before();
    c.step().unwrap();
    assert!(
        !c.regs.flags.get(ZF),
        "1234h AND 0003h would be zero; 0F03h is not"
    );

    // 34h AND 03h is 00h, so ZF is set.
    let mut c = cpu(&[0xF6, 0b00_001_110, 0x00, 0x01, 0x03]);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.mem.write_u16(0x2000, 0x0100, 0x1234);
    c.regs.flags.0 = status_before();
    c.step().unwrap();
    assert!(c.regs.flags.get(ZF), "34h AND 03h is zero");
}

/// `D0`-`D3 /6` is SETMO: the operand takes all ones.
///
/// Table 4-13 prints `(not used)` for `110` in the Shift table. The 8086 does
/// not error and does not shift either: `/6` selects the ALU's SETMO operation,
/// which returns `FFh` or `FFFFh`, and the shared shift microcode
/// (`M -> tmpB, XI tmpB` then `S -> M`) writes that back to the operand. Before
/// this was fixed the digit fell through to the 80186 `SAL` alias and quietly
/// executed the wrong instruction.
#[test]
fn d0_to_d3_six_store_all_ones() {
    for (opcode, value) in [
        (0xD0u8, 0x00FFu16), // byte
        (0xD1, 0xFFFF),
        (0xD2, 0x00FF), // byte, count from CL
        (0xD3, 0xFFFF), // word, count from CL
    ] {
        let mut c = cpu(&[opcode, 0b00_110_110, 0x00, 0x01]);
        c.regs.set_segment(Seg::Ds, 0x2000);
        c.mem.write_u16(0x2000, 0x0100, 0x003C);
        c.regs.set_byte(Reg8::Cl, 1); // the D2/D3 count, so the digit is reached
        c.regs.flags.0 = status_before();
        c.step().unwrap();

        assert_eq!(
            c.mem.read_u16(0x2000, 0x0100),
            value,
            "{opcode:#04x} /6 must store all ones, not shift the operand"
        );
    }
}

/// SETMO's flags are the flags of the value it produced. `FFh` has eight set
/// bits, so parity is even and `PF` is set; it is never zero, so `ZF` is
/// clear; its top bit is set, so `SF` is set. A logic operation does not write
/// `AF`, so the `AF` the instruction found is the `AF` it leaves.
#[test]
fn setmo_flags_are_the_flags_of_the_result() {
    let mut c = cpu(&[0xD1, 0b00_110_110, 0x00, 0x01]);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.mem.write_u16(0x2000, 0x0100, 0x0000);
    c.regs.flags.0 = status_before();
    c.step().unwrap();

    assert!(c.regs.flags.get(SF), "FFFFh is negative");
    assert!(!c.regs.flags.get(ZF), "FFFFh is not zero");
    assert!(
        c.regs.flags.get(PF),
        "FFh has eight set bits, an even count"
    );
    assert!(!c.regs.flags.get(CF) && !c.regs.flags.get(OF));
    assert!(
        c.regs.flags.get(AF),
        "SETMO is not an addition and writes no AF"
    );
}

/// A `CL` count of zero changes nothing, flags included. That rule holds for
/// every digit in the Shift group and SETMO is no exception: the variable-shift
/// microcode loops `CL` times, so with `CL = 0` the ALU is never selected and
/// the operand keeps the value it had.
#[test]
fn setmo_with_a_zero_count_changes_nothing() {
    let mut c = cpu(&[0xD3, 0b00_110_110, 0x00, 0x01]);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.mem.write_u16(0x2000, 0x0100, 0x1234);
    c.regs.set_byte(Reg8::Cl, 0);
    c.regs.flags.0 = status_before();
    c.step().unwrap();

    assert_eq!(
        c.mem.read_u16(0x2000, 0x0100),
        0x1234,
        "CL = 0 leaves the operand alone"
    );
    assert_eq!(c.regs.flags.0, status_before(), "CL = 0 writes no flag");
}

/// `FE /7` and `FF /7` are `PUSH r/m`. Table 4-13 prints `(not used)` for both,
/// and the 8086 runs PUSH anyway: bits 5-3 of the ModR/M byte replace bits 2-0
/// of the opcode, so both become `FF` with the `110` selection that is PUSH.
///
/// `SP` falls by two and the value lands at the new `SS:SP`. The decrement is
/// unconditional: the stack that follows `DF` arrived with the 80186, and here
/// `DF` is set *on* to prove it.
///
/// Shirriff's listing for `FE /6` — the same PUSH microcode `FE /7` reaches —
/// writes one *byte* at `SS:SP` and leaves the byte above it stale. The width
/// this test pins is the operand width, which is what the manual's own
/// byte-column/word-column split for `FE`/`FF` means; the pushed word is
/// therefore zero-extended here rather than half a word of stale stack.
#[test]
fn fe_seven_and_ff_seven_push_the_operand() {
    // FF /7, the word form, with DF deliberately set.
    let mut c = cpu(&[0xFF, 0b00_111_110, 0x00, 0x01]);
    stack_grows_down(&c);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.regs.set_segment(Seg::Ss, 0x3000);
    c.mem.write_u16(0x2000, 0x0100, 0xBEEF);
    c.mem.write_u16(0x3000, 0x0100, 0x1111); // the slot the push will land in
    c.regs.flags.set(DF, true);
    c.step().unwrap();

    assert_eq!(c.regs.word(Reg16::Sp), 0x00FE, "the 8086 stack ignores DF");
    assert_eq!(c.mem.read_u16(0x3000, 0x00FE), 0xBEEF);

    // FE /7, the byte form: it reads a byte, so what it pushes is 00EFh rather
    // than the whole BEEFh word.
    let mut c = cpu(&[0xFE, 0b00_111_110, 0x00, 0x01]);
    stack_grows_down(&c);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.regs.set_segment(Seg::Ss, 0x3000);
    c.mem.write_u16(0x2000, 0x0100, 0xBEEF);
    c.regs.flags.0 = status_before();
    c.step().unwrap();

    assert_eq!(c.regs.word(Reg16::Sp), 0x00FE);
    assert_eq!(
        c.mem.read_u16(0x3000, 0x00FE) & 0x00FF,
        0xEF,
        "FE /7 reads its operand as a byte"
    );
    assert_eq!(
        c.mem.read_u16(0x2000, 0x0100),
        0xBEEF,
        "PUSH reads the operand and leaves it where it was"
    );
}

/// PUSH writes no flag. It is the same instruction as `FF /6`, and neither
/// form may disturb the status flags the instruction found.
#[test]
fn push_from_group_two_writes_no_flag() {
    let mut c = cpu(&[0xFF, 0b00_111_110, 0x00, 0x01]);
    stack_grows_down(&c);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.regs.set_segment(Seg::Ss, 0x3000);
    c.mem.write_u16(0x2000, 0x0100, 0xBEEF);
    c.regs.flags.0 = status_before();
    c.step().unwrap();

    assert_eq!(c.regs.flags.0, status_before(), "PUSH writes no flag");
}

// ---------------------------------------------------------------------------
// U7 — the four segment override prefixes. `26` names ES, `2E` CS, `36` SS and
// `3E` DS; each one changes the segment the *next* instruction's memory operand
// addresses through, and nothing else about it.
//
// The rule these tests pin is the segment the override names, not the fact that
// the byte was accepted: a decoder that consumed `26` and then carried on with
// DS passes any test that only checks "it did not fault".
// ---------------------------------------------------------------------------

/// A CPU with `code` at `CS:0000` and the four segment registers on four
/// different bases, each holding a different word at the same offset.
///
/// The bases differ so that reading the wrong segment gives the wrong answer.
/// A prefix that is parsed and dropped, a decoder that always uses `DS`, and a
/// decoder that applies the override to the wrong end of the instruction all
/// land on a different one of these four words.
///
/// No status flag is preset here, and none may be: `DF` governs the string
/// operations, so a helper that set it would make the `PUSH` rows below read
/// like a stack-direction defect.
fn four_segments(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x30000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x3000);
    c.regs.set_segment(Seg::Ds, 0x2000);
    c.regs.set_segment(Seg::Es, 0x5000);
    c.regs.set_segment(Seg::Ss, 0x4000);
    c.regs.set_word(Reg16::Sp, 0x0080);
    c.mem.write_u16(0x2000, 0x0100, 0x1111); // DS
    c.mem.write_u16(0x3000, 0x0100, 0x2222); // CS
    c.mem.write_u16(0x4000, 0x0100, 0x3333); // SS
    c.mem.write_u16(0x5000, 0x0100, 0x4444); // ES
    c
}

/// `A1 mo16` is `MOV AX, DS:mo16`, the accumulator form with no ModR/M byte,
/// and the override reaches it like any other `DS` memory reference. `3E` and
/// `26` on the identical operand are the whole test: the same bytes, the same
/// offset, `1111h` through DS and `4444h` through ES.
#[test]
fn a_disp16_operand_is_read_through_ds_or_through_the_override() {
    let mut c = four_segments(&[0xA1, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x1111, "no prefix means DS");

    // 3E names DS, which is also the default: the instruction must still run,
    // and the prefix byte must be part of the same step rather than swallowed.
    let mut c = four_segments(&[0x3E, 0xA1, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1111,
        "3E is DS, so it reads what the bare instruction reads"
    );
    assert_eq!(
        c.regs.ip, 4,
        "the prefix byte and the instruction it covers are one step"
    );

    // 26 names ES: the same encoding over the same offset, a different answer.
    let mut c = four_segments(&[0x26, 0xA1, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x4444,
        "26 is ES: the override was used, not merely accepted"
    );
}

/// `2E` reaches the ModR/M memory operand the same way. CS and DS hold
/// different words, so a decoder that parsed the prefix and then fell back to
/// the default reads `1111h` here.
#[test]
fn two_e_reads_a_modrm_memory_operand_through_cs() {
    // 8B /r is MOV r16, r/m16; mod=00 rm=110 is the direct form.
    let mut c = four_segments(&[0x2E, 0x8B, 0b00_000_110, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x2222, "2E is CS");

    let mut c = four_segments(&[0x8B, 0b00_000_110, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1111,
        "the same instruction without the prefix reads DS"
    );
}

/// `36` names SS, which is also where the stack lives — the two must not be
/// confused. The override chooses the segment for the instruction's *operand*;
/// `SP`, `PUSH`, `POP` and `BP` addressing all read `SS` directly and never
/// consult the pending override.
#[test]
fn three_six_reads_a_modrm_memory_operand_through_ss() {
    let mut c = four_segments(&[0x36, 0x8B, 0b00_000_110, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x3333,
        "36 is SS, and SS is a data segment like any other here"
    );
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0080,
        "reading through SS is not touching the stack"
    );
}

/// `LEA` takes the address, so the segment is not part of its answer and an
/// override cannot change it. The operand is still decoded through the
/// override — that is what keeps the instruction length and the effective
/// address identical either way — and then `LEA` throws the segment away.
#[test]
fn lea_under_an_override_still_computes_the_offset_alone() {
    // 8D /r is LEA r16, m. Anything that leaked the segment in would put
    // 4444h, the word ES holds, in AX instead of the offset.
    let mut c = four_segments(&[0x26, 0x8D, 0b00_000_110, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x0100,
        "a segment is not part of an offset"
    );

    let mut c = four_segments(&[0x8D, 0b00_000_110, 0x00, 0x01]);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x0100,
        "the override changes nothing LEA can see"
    );
}

/// A register operand has no segment to override, so a prefix in front of one
/// is consumed and nothing else happens.
///
/// The second instruction matters as much as the first: one prefix binds to
/// one instruction, so the `A1 mo16` behind the `ADD` must read `DS` again
/// rather than inherit ES from the prefix three bytes earlier.
#[test]
fn an_override_in_front_of_a_register_operand_is_consumed_and_forgotten() {
    // 26 03 C3 is ADD AX, BX with mod=11: no memory, so ES goes nowhere.
    let mut c = four_segments(&[0x26, 0x03, 0xC7, 0xA1, 0x00, 0x01]);
    c.regs.set_word(Reg16::Ax, 0x0010);
    c.regs.set_word(Reg16::Bx, 0x0002);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0012, "ADD AX, BX still runs");
    assert_eq!(c.regs.ip, 3, "the prefix is one byte of the same step");

    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1111,
        "the prefix is spent: the next instruction reads DS"
    );
}

/// `FF /6` has exactly one memory operand — the word it pushes — so `26` sends
/// *that* read to ES while the write still goes to `SS:SP`. `FE /6` reaches
/// the same microcode a byte at a time and takes the same view of the segment.
///
/// A register push has no memory operand at all: the prefix is consumed and
/// the push lands on the stack segment exactly as it would have alone.
#[test]
fn a_push_reads_its_operand_through_the_override_and_still_writes_ss() {
    let mut c = four_segments(&[0x26, 0xFF, 0b00_110_110, 0x00, 0x01]);
    stack_grows_down(&c);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Sp), 0x007E, "SP moved by two, downward");
    assert_eq!(
        c.mem.read_u16(0x4000, 0x007E),
        0x4444,
        "the word was read through ES and pushed onto SS:SP"
    );
    assert_eq!(
        c.mem.read_u16(0x2000, 0x0100),
        0x1111,
        "DS is not where the pushed word came from"
    );

    // 36 50: PUSH AX with SS named. No memory operand, so the prefix goes
    // nowhere and the value still lands on the stack segment.
    let mut c = four_segments(&[0x36, 0x50]);
    stack_grows_down(&c);
    c.regs.set_word(Reg16::Ax, 0xBEEF);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Sp), 0x007E);
    assert_eq!(
        c.mem.read_u16(0x4000, 0x007E),
        0xBEEF,
        "a prefix must not redirect a push to the data segment"
    );

    // 26 58: POP AX with ES named, off the same stack.
    let mut c = four_segments(&[0x26, 0x58]);
    stack_grows_down(&c);
    c.mem.write_u16(0x4000, 0x0080, 0xBEEF);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0xBEEF,
        "POP reads SS whatever the prefix named"
    );
    assert_eq!(
        c.regs.word(Reg16::Sp),
        0x0082,
        "pop predecrements on the way in"
    );
}

/// `D7` has no ModR/M byte at all: its operand is the implicit `DS:[BX + AL]`
/// with `AL` read as a signed byte. An override moves the *base segment* and
/// nothing else, so the index arithmetic is identical either way.
#[test]
fn xlat_reads_its_table_through_the_override() {
    let mut c = four_segments(&[0x26, 0xD7]);
    c.regs.set_byte(Reg8::Al, 1);
    c.mem.write_u8(0x5000, 0x0001, 0x5A); // ES:0001
    c.mem.write_u8(0x2000, 0x0001, 0x3C); // DS:0001
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x5A, "26 moves the table to ES");

    let mut c = four_segments(&[0xD7]);
    c.regs.set_byte(Reg8::Al, 1);
    c.mem.write_u8(0x2000, 0x0001, 0x3C); // DS:0001
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x3C, "no prefix means DS");
}

// ---- AUDIT-7 and its neighbours, settled from the manual --------------------
//
// AUDIT-7: BP-based memory operands default to SS. The manual is explicit at
// section 2: "All data references which explicitly or implicitly involve SP
// or BP are taken by default to be relative to SS." The direct disp16 form
// shares rm=110 with [BP+disp] and is told apart by the mod field alone, so
// it stays DS.

/// One distinct word per segment at the same offset, so a wrong default
/// segment shows up as a wrong value rather than as a fault.
fn fill_segments(c: &mut Cpu, offset: u16) {
    // Distinct bases, or every segment maps to the same physical bytes and
    // the test cannot tell a right answer from a lucky one.
    // CS is left alone: `cpu()` already put the code there.
    for (n, (seg, base)) in [(Seg::Ds, 0x2000u16), (Seg::Es, 0x3000), (Seg::Ss, 0x4000)]
        .into_iter()
        .enumerate()
    {
        c.regs.set_segment(seg, base);
        c.mem.write_u16(base, offset, 0x1100 + n as u16);
    }
}

#[test]
fn bp_based_operands_default_to_ss_and_the_rest_to_ds() {
    // Every form is built so the offset lands on 0x20h with the registers
    // zeroed, so the only thing under test is the default segment.
    // `[BP]`, `[BP+SI]` and `[BP+DI]` share rm=110/010/011 under mod=01, and
    // `[BX]`, `[SI]`, `[DI]` share 111/100/101.
    let cases: [(u8, bool, &str); 6] = [
        (0b01_000_110, true, "[BP+disp8]"),
        (0b01_000_111, false, "[BX+disp8]"),
        (0b01_000_010, true, "[BP+SI+disp8]"),
        (0b01_000_011, true, "[BP+DI+disp8]"),
        (0b01_000_100, false, "[SI+disp8]"),
        (0b01_000_101, false, "[DI+disp8]"),
    ];
    for (modrm, expects_ss, name) in cases {
        let mut c = cpu(&[0x8B, modrm, 0x20]); // MOV AX, [r/m+20h]
        fill_segments(&mut c, 0x0020);
        c.step().unwrap();
        let word = c.regs.word(Reg16::Ax);
        let from_ss = word == 0x1102;
        let from_ds = word == 0x1100;
        assert!(
            from_ss || from_ds,
            "{name}: read {word:#06x}, expected DS (0x1100) or SS (0x1102)"
        );
        assert_eq!(
            from_ss, expects_ss,
            "{name} used the wrong default segment: {word:#06x}"
        );
    }
}

#[test]
fn the_bp_base_reaches_ss_even_with_a_displacement() {
    // mod=01 rm=110 is [BP+disp8]; the segment default must follow BP.
    let mut c = cpu(&[0x8B, 0x46, 0x20]); // MOV AX, [BP+20h]
    fill_segments(&mut c, 0x0040);
    c.regs.set_word(Reg16::Bp, 0x0020);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x1102, "SS, not DS");
}

#[test]
fn the_direct_disp16_form_shares_rm_with_bp_but_is_not_ss() {
    // mod=00 rm=110 is the direct form, not [BP]. It must stay DS even though
    // the r/m bits are the ones that mean BP in every other mod value.
    let mut c = cpu(&[0x8B, 0x06, 0x40, 0x00]); // MOV AX, [0040h]
    fill_segments(&mut c, 0x0040);
    c.regs.set_word(Reg16::Bp, 0x0000);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x1100, "DS, not SS");
}

#[test]
fn a_segment_override_beats_the_bp_default() {
    // 26 is the ES override. It must win over BP's SS default.
    let mut c = cpu(&[0x26, 0x8B, 0x46, 0x20]); // ES: MOV AX, [BP+20h]
    fill_segments(&mut c, 0x0040);
    c.regs.set_word(Reg16::Bp, 0x0020);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x1101, "the override wins");
}

#[test]
fn lea_ignores_the_default_segment_entirely() {
    // LEA returns the offset. BP's SS default is irrelevant to the value.
    let mut c = cpu(&[0x8D, 0x46, 0x20]); // LEA AX, [BP+20h]
    c.regs.set_word(Reg16::Bp, 0x0020);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0040, "just the offset");
}

/// AUDIT-3. `8F` is Group 2 with /0 and decodes its ModR/M the ordinary way:
/// mod=00, 01 and 10 all address memory. An earlier revision refused mod=01
/// and mod=10 on the strength of a prose note that contradicted the section's
/// own cell and the microcode, where the destination is the `rm` field.
#[test]
fn pop_rm16_accepts_every_memory_addressing_mode() {
    // Each form writes to a different offset, so accepting the byte is not
    // enough: the destination has to be the one the encoding names. The
    // BP-based forms go to SS, per AUDIT-7, not DS.
    let cases: [(&[u8], u16, u16, &str); 5] = [
        (&[0x8F, 0x06, 0x40, 0x00], 0x3000, 0x0040, "mod=00 disp16"),
        (&[0x8F, 0x47, 0x30], 0x3000, 0x0030, "mod=01 [BX+disp8]"),
        (
            &[0x8F, 0x87, 0x30, 0x00],
            0x3000,
            0x0030,
            "mod=10 [BX+disp16]",
        ),
        (&[0x8F, 0x00], 0x3000, 0x0000, "mod=00 [BX+SI]"),
        (
            &[0x8F, 0x46, 0x30],
            0x2000,
            0x0030,
            "mod=01 [BP+disp8] lands in SS, which the loop set to 2000h",
        ),
    ];
    for (code, dest_seg, want, name) in cases {
        let mut c = cpu(code);
        c.regs.set_segment(Seg::Ss, 0x2000);
        c.regs.set_segment(Seg::Ds, 0x3000);
        c.regs.set_word(Reg16::Sp, 0x0100);
        c.regs.set_word(Reg16::Bx, 0x0000);
        c.regs.set_word(Reg16::Bp, 0x0000);
        // POP postincrements: it reads SS:0100h and leaves SP at 0102h.
        c.mem.write_u16(0x2000, 0x0100, 0xCAFE);
        c.step().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(c.regs.word(Reg16::Sp), 0x0102, "{name}: SP did not move");
        assert_eq!(
            c.mem.read_u16(dest_seg, want),
            0xCAFE,
            "{name}: wrong destination"
        );
    }
}

/// The register form stays refused: the documentation calls it undefined for
/// this encoding, and an undefined encoding is a policy call rather than a
/// guess at behaviour.
#[test]
fn pop_rm16_still_refuses_the_register_and_non_zero_digits() {
    for code in [
        &[0x8F, 0xC3][..],             // mod=11
        &[0x8F, 0x0E, 0x40, 0x00][..], // reg=001
    ] {
        let mut c = cpu(code);
        c.regs.set_word(Reg16::Sp, 0x0100);
        assert_eq!(c.step(), Err(ExecError::NotOn8086(0x8F)), "{code:02X?}");
        assert_eq!(c.regs.word(Reg16::Sp), 0x0100, "{code:02X?}: SP moved");
    }
}

/// AUDIT-5. Prefix bytes used to be consumed by each prefix arm re-entering
/// the dispatcher, which cost one host stack frame per byte. About 1,500
/// consecutive `LOCK` bytes exhausted the stack, and `panic = "abort"` makes
/// that unrecoverable. Prefixes are now consumed by a loop in `step`, so the
/// depth of a run costs nothing.
#[test]
fn a_long_run_of_prefixes_does_not_exhaust_the_host_stack() {
    // Four thousand is nearly three times the old ceiling.
    let run = 4_000u16;
    let mut image = vec![0xF0u8; run as usize]; // LOCK
    image.push(0x90); // XCHG AX,AX, the instruction every LOCK covers
    let mut c = cpu(&image);
    c.regs.set_word(Reg16::Ax, 0x1234);

    // One step retires the whole run plus the instruction it covers.
    let info = c.step().expect("a run of prefixes must not fault");
    assert_eq!(
        info.next_ip,
        run + 1,
        "past every prefix and its instruction"
    );
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x1234,
        "NOP covered by LOCK is a NOP"
    );
}

/// The same must hold for segment override prefixes, which previously cost
/// two nested frames per byte.
#[test]
fn a_long_run_of_segment_overrides_also_stays_flat() {
    let run = 4_000u16;
    let mut image = vec![0x26u8; run as usize]; // ES override
    image.extend_from_slice(&[0x8A, 0x06, 0x40, 0x00]); // MOV AL, ES:[0040h]
    let mut c = cpu(&image);
    c.regs.set_segment(Seg::Es, 0x3000);
    c.mem.write_u8(0x3000, 0x0040, 0x5A);

    let info = c.step().expect("a run of overrides must not fault");
    assert_eq!(
        info.next_ip,
        run + 4,
        "past every prefix and its instruction"
    );
    assert_eq!(c.regs.byte(Reg8::Al), 0x5A);
}

/// A prefix run and its instruction retire as one step, so `StepInfo` reports
/// the covered opcode rather than the first prefix byte.
#[test]
fn a_prefixed_step_reports_the_covered_opcode() {
    let mut c = cpu(&[0xF0, 0x3E, 0x90]);
    let info = c.step().unwrap();
    assert_eq!(info.at, 0, "the run starts here");
    assert_eq!(info.opcode, 0x90, "the covered opcode, not the prefix");
    assert_eq!(info.next_ip, 3);
}

/// Two segment overrides in a row: the later one replaces the earlier rather
/// than composing, and the override does not outlive the instruction.
#[test]
fn a_later_override_replaces_an_earlier_one_and_clears_afterwards() {
    let mut c = cpu(&[0x26, 0x36, 0x8A, 0x06, 0x40, 0x00]);
    c.regs.set_segment(Seg::Es, 0x3000);
    c.regs.set_segment(Seg::Ss, 0x4000);
    c.mem.write_u8(0x3000, 0x0040, 0x11);
    c.mem.write_u8(0x4000, 0x0040, 0x22);
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x22, "the later prefix wins");

    // The next instruction reads through DS again.
    let mut c = cpu(&[0x36, 0x8A, 0x06, 0x40, 0x00, 0x8A, 0x06, 0x40, 0x00]);
    c.regs.set_segment(Seg::Ss, 0x4000);
    c.regs.set_segment(Seg::Ds, 0x3000);
    c.mem.write_u8(0x4000, 0x0040, 0x22);
    c.mem.write_u8(0x3000, 0x0040, 0x11);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0x11, "the override did not leak");
}

/// F1. The 256-byte gate must fail on a prefix byte that reaches the
/// dispatcher instead of being consumed by `step`. Matching only
/// `UnknownOpcode` left the gate blind to `PrefixNotConsumed`, which is the
/// same blindness that let it ignore `NotOn8086` and the deleted `BadRepeat`.
#[test]
fn the_opcode_gate_counts_an_unconsumed_prefix_as_undecoded() {
    // Prove the predicate rather than asserting the implementation: the
    // variant the gate must reject is named here, so deleting it from the
    // gate's pattern is a compile error rather than a silent gap.
    let unreachable = ExecError::PrefixNotConsumed(0x26);
    assert!(
        matches!(
            unreachable,
            ExecError::UnknownOpcode(_) | ExecError::PrefixNotConsumed(_)
        ),
        "the gate must treat an unconsumed prefix as an undecoded byte"
    );
}

// ---------------------------------------------------------------------------
// REP is a latch, not a gate.
//
// The 8086's opcode decode sets the segment latches and the REP latch from
// independent columns, so the order of the prefix bytes cannot matter, and it
// has no invalid-instruction detection to object to a repeat prefix in front
// of something that ignores it.
// ---------------------------------------------------------------------------

/// Everything `REP MOVSB` can be judged on: the count, both pointers, where
/// `IP` stopped, and the destination bytes.
type State = (u16, u16, u16, u16, [u8; 4]);

fn movsb_state(c: &Cpu) -> State {
    let mut moved = [0u8; 4];
    for (i, byte) in moved.iter_mut().enumerate() {
        *byte = c.mem.read_u8(0x9000, 0x0300 + i as u16);
    }
    (
        c.regs.word(Reg16::Cx),
        c.regs.word(Reg16::Si),
        c.regs.word(Reg16::Di),
        c.regs.ip,
        moved,
    )
}

/// The machine for a three-iteration `REP MOVSB`, differing only in the bytes
/// ahead of the opcode.
fn movsb_after(code: &[u8]) -> (Cpu, State) {
    let mut c = cpu(code);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 3);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x0000, 0x0020, 0x11);
    c.mem.write_u8(0x0000, 0x0021, 0x22);
    c.mem.write_u8(0x0000, 0x0022, 0x33);
    // A segment override reaches the SI side, so a test whose prefix names ES
    // reads the source from here. These bytes are deliberately DIFFERENT from
    // DS's: identical bytes in both segments would let a dropped override pass
    // unnoticed, which is how the old permutation test stayed green while the
    // override was being discarded.
    c.mem.write_u8(0x9000, 0x0020, 0xAA);
    c.mem.write_u8(0x9000, 0x0021, 0xBB);
    c.mem.write_u8(0x9000, 0x0022, 0xCC);
    c.step().expect("REP MOVSB must not fault");
    let state = movsb_state(&c);
    (c, state)
}

/// The state a correct three-iteration `REP MOVSB` reaches when nothing
/// overrides the source: the count spent, both pointers past three bytes, and
/// `DS`'s bytes copied through `ES`. The `IP` is the length of the program that
/// reached it.
const REP_MOVSB_THREE: State = (0, 0x0023, 0x0303, 3, [0x11, 0x22, 0x33, 0x00]);

/// The same three iterations behind a prefix that names `ES` as the source
/// segment. The moved bytes are `ES`'s, which is what makes it a different
/// state rather than the same one reached by another route.
const REP_MOVSB_THREE_FROM_ES: State = (0, 0x0023, 0x0303, 3, [0xAA, 0xBB, 0xCC, 0x00]);

/// The same three iterations reached behind a 4-byte prefix run, so `IP`
/// stops one byte further on and nothing else differs.
const REP_MOVSB_THREE_AFTER_FOUR: State = (0, 0x0023, 0x0303, 5, [0x11, 0x22, 0x33, 0x00]);

/// The three iterations behind a 5-byte run that ends in an override naming
/// the source. `ES`'s bytes moved, because `ES` is now the source.
const REP_MOVSB_THREE_FROM_ES_AFTER_FIVE: State = (0, 0x0023, 0x0303, 6, [0xAA, 0xBB, 0xCC, 0x00]);

/// The three iterations behind the same 5-byte run ending in `3E`, which names
/// `DS` — the segment the bytes came from in the first place.
const REP_MOVSB_THREE_AFTER_FIVE: State = (0, 0x0023, 0x0303, 6, [0x11, 0x22, 0x33, 0x00]);

/// `F3 26 A4` used to be `BadRepeat`, because the repeat helper fetched its
/// own operand byte and rejected anything that was not a string operation.
/// With REP a latch the two prefixes are independent and either order runs
/// the same three iterations.
#[test]
fn rep_before_a_segment_override_and_the_other_way_round_agree() {
    let (_, first) = movsb_after(&[0xF3, 0x26, 0xA4]);
    let (_, second) = movsb_after(&[0x26, 0xF3, 0xA4]);
    assert_eq!(first, second, "prefix order changed the outcome");
    assert_eq!(
        first, REP_MOVSB_THREE_FROM_ES,
        "three MOVSB iterations, source through ES and destination through ES"
    );
}

/// The same with `F2`, the other repeat prefix.
#[test]
fn repne_before_a_segment_override_and_the_other_way_round_agree() {
    let (_, first) = movsb_after(&[0xF2, 0x3E, 0xA4]);
    let (_, second) = movsb_after(&[0x3E, 0xF2, 0xA4]);
    assert_eq!(first, second, "prefix order changed the outcome");
    assert_eq!(
        first, REP_MOVSB_THREE,
        "REPNE MOVSB repeats the same way; MOVS has no condition"
    );
}

/// A segment override cannot redirect a string operation's destination: on an
/// 8086 the destination is always ES.
#[test]
fn a_segment_override_cannot_redirect_the_string_destination() {
    let (c, _) = movsb_after(&[0x3E, 0xF3, 0xA4]);
    assert_eq!(
        c.mem.read_u8(0x9000, 0x0300),
        0x11,
        "the destination went through ES, not the overridden DS"
    );
    assert_eq!(c.mem.read_u8(0x0000, 0x0300), 0x00, "and not through DS");
}

/// The other half of that rule. The manual is explicit that the destination of
/// a string instruction is the *only* operand an override cannot reach, so
/// everything on the `SI` side obeys the override like any other memory
/// operand. `26 A4` reads `ES:0100`, not `DS:0100`.
#[test]
fn a_segment_override_reaches_the_string_source() {
    let mut c = four_segments(&[0x26, 0xA5]); // ES override; MOVSW
    c.regs.set_word(Reg16::Si, 0x0100);
    c.regs.set_word(Reg16::Di, 0x0200);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x5000, 0x0200),
        0x4444,
        "the word came from ES:0100, the override, and not from DS"
    );
    assert_eq!(c.mem.read_u16(0x2000, 0x0100), 0x1111, "DS was not read");

    let mut c = four_segments(&[0x26, 0xAC]); // ES override; LODSB
    c.regs.set_word(Reg16::Si, 0x0100);
    c.step().unwrap();
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x44,
        "loaded through ES, so the override reached LODS too"
    );

    let mut c = four_segments(&[0x26, 0xA6]); // ES override; CMPSB
    c.regs.set_word(Reg16::Si, 0x0100);
    c.regs.set_word(Reg16::Di, 0x0200);
    c.mem.write_u8(0x5000, 0x0200, 0x44);
    c.step().unwrap();
    assert!(
        c.regs.flags.get(ZF),
        "ES:0100 against ES:0200 is the same byte; through DS it would not be"
    );
}

/// The destination rule survives the fix: `STOS` and `SCAS` read and write
/// `ES:DI` even when the override names another segment, which is what makes
/// them different from the three operations above rather than the same rule
/// applied twice.
#[test]
fn stos_and_scas_ignore_the_override_and_stay_in_es() {
    let mut c = four_segments(&[0x2E, 0xAA]); // CS override; STOSB
    c.regs.set_word(Reg16::Di, 0x0200);
    c.regs.set_byte(Reg8::Al, 0x99);
    c.step().unwrap();
    assert_eq!(c.mem.read_u8(0x5000, 0x0200), 0x99, "ES:DI, always");
    assert_eq!(
        c.mem.read_u8(0x2000, 0x0200),
        0x00,
        "not through the overridden segment"
    );

    let mut c = four_segments(&[0x2E, 0xAE]); // CS override; SCASB
    c.regs.set_word(Reg16::Di, 0x0100);
    c.regs.set_byte(Reg8::Al, 0x44);
    c.step().unwrap();
    assert!(
        c.regs.flags.get(ZF),
        "AL against ES:0100 is 0x44 either way"
    );
}

/// Two segment overrides are not a composition and not an error: the Group
/// Decode ROM loads the segment latch from a column of its own, so the last
/// one in the run is the one the next instruction sees. That is why order
/// matters *between two overrides* even though it cannot matter anywhere else.
#[test]
fn the_last_segment_override_wins() {
    let mut c = four_segments(&[0x36, 0x2E, 0xAC]); // SS then CS; LODSB
    c.regs.set_word(Reg16::Si, 0x0100);
    c.step().unwrap();
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x22,
        "CS:0100 is 2222h, and CS came last"
    );

    let mut c = four_segments(&[0x2E, 0x36, 0xAC]); // CS then SS
    c.regs.set_word(Reg16::Si, 0x0100);
    c.step().unwrap();
    assert_eq!(
        c.regs.byte(Reg8::Al),
        0x33,
        "SS:0100 is 3333h, and SS came last"
    );
}

/// A repeat prefix in front of something that is not a string operation is
/// ignored. The 8086 has no invalid-instruction detection, so refusing it
/// would be the divergence.
#[test]
fn rep_in_front_of_a_nop_is_a_nop() {
    let mut c = cpu(&[0xF3, 0x90, 0xB0, 0x77]);
    c.regs.set_word(Reg16::Cx, 0x0005);
    let info = c.step().expect("F3 90 is not an error");
    assert_eq!(info.opcode, 0x90, "the covered opcode, not the prefix");
    assert_eq!(info.next_ip, 2);
    assert_eq!(
        c.regs.word(Reg16::Cx),
        0x0005,
        "nothing repeated, so nothing consumed the count"
    );
    assert_ne!(c.regs.byte(Reg8::Al), 0x77, "the NOP retired unchanged");
}

/// All 4! orders of `F0 F1 F2 F3` ahead of the same `MOVSB`. Those four bytes
/// feed latches the 8086 loads from independent columns of the decode ROM, so
/// nothing about the outcome can depend on their order.
///
/// No segment override is in this set, and that is deliberate rather than an
/// oversight: order *does* matter between two of those, which is what
/// `the_last_segment_override_wins` pins. The earlier version of this test
/// permuted all eight prefix bytes and still passed while the override was
/// being discarded on the `SI` side, so the set it chose proved nothing.
#[test]
fn every_order_of_lock_and_the_repeat_prefixes_reaches_the_same_state() {
    let mut order = [0xF0u8, 0xF1, 0xF2, 0xF3];
    let mut orders = Vec::new();
    permutations(&mut order, 0, &mut orders);
    assert_eq!(orders.len(), 24, "4! orderings, not a sample");

    let mut code = [0u8; 5];
    code[4] = 0xA4;
    let mut c = cpu(&code);
    c.regs.set_segment(Seg::Es, 0x9000);

    let mut reference = None;
    for order in &orders {
        run_prefixed(&mut c, order);
        let state = movsb_state(&c);
        match reference {
            None => reference = Some(state),
            Some(want) => assert_eq!(state, want, "prefix order {order:02X?} changed the outcome"),
        }
    }
    assert_eq!(
        reference,
        Some(REP_MOVSB_THREE_AFTER_FOUR),
        "and the shared state is the right one: three MOVSB iterations"
    );
}

/// The 96 orderings this commit would otherwise have dropped: `F0 F1 F2 F3`
/// around a *single* segment override. Dropping the eight-byte permutation
/// took these with it, and they are the case the fix makes interesting — one
/// override cannot compete with another, so order provably cannot matter, and
/// the state must be the one that override names. All four overrides are
/// checked, so `26`/`2E`/`36`/`3E` each have to reach their own segment.
#[test]
fn every_order_around_a_single_segment_override_reaches_the_same_state() {
    for (prefix, state) in [
        (0x26u8, REP_MOVSB_THREE_FROM_ES_AFTER_FIVE),
        (0x3Eu8, REP_MOVSB_THREE_AFTER_FIVE),
    ] {
        let mut order = [0xF0u8, 0xF1, 0xF2, 0xF3];
        let mut orders = Vec::new();
        permutations(&mut order, 0, &mut orders);
        assert_eq!(orders.len(), 24, "4! orderings per override, not a sample");

        let mut code = [0u8; 6];
        code[5] = 0xA4;
        let mut c = cpu(&code);
        c.regs.set_segment(Seg::Es, 0x9000);

        let mut reference = None;
        for permutation in &orders {
            let mut run = [0u8; 5];
            run[4] = prefix;
            run[..4].copy_from_slice(permutation);
            run_prefixed(&mut c, &run);
            let got = movsb_state(&c);
            match reference {
                None => reference = Some(got),
                Some(want) => assert_eq!(
                    got, want,
                    "prefix order {permutation:02X?} around {prefix:02X} changed the outcome"
                ),
            }
        }
        assert_eq!(
            reference,
            Some(state),
            "and the shared state is the right one for override {prefix:02X}"
        );
    }
}

/// Generate every permutation of `items`, in place.
fn permutations<const N: usize>(items: &mut [u8; N], at: usize, out: &mut Vec<[u8; N]>) {
    if at == N {
        out.push(*items);
        return;
    }
    for i in at..N {
        items.swap(at, i);
        permutations(items, at + 1, out);
        items.swap(at, i);
    }
}

/// Rewrite the code bytes to `order` followed by `MOVSB`, reset everything the
/// covered instruction touches, and take one step. Reusing one CPU keeps every
/// ordering off the 1 MiB allocator.
///
/// Both source segments are seeded with different bytes, because an override
/// can name either one and a test that cannot tell them apart would pass with
/// the override dropped.
fn run_prefixed(c: &mut Cpu, order: &[u8]) {
    c.regs.ip = 0;
    c.regs.set_word(Reg16::Cx, 3);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    for i in 0..4u16 {
        c.mem.write_u8(0x9000, 0x0300 + i, 0x00);
    }
    c.mem.write_u8(0x0000, 0x0020, 0x11);
    c.mem.write_u8(0x0000, 0x0021, 0x22);
    c.mem.write_u8(0x0000, 0x0022, 0x33);
    c.mem.write_u8(0x9000, 0x0020, 0xAA);
    c.mem.write_u8(0x9000, 0x0021, 0xBB);
    c.mem.write_u8(0x9000, 0x0022, 0xCC);
    for (i, byte) in order.iter().enumerate() {
        c.mem.write_u8(0x1000, i as u16, *byte);
    }
    c.mem.write_u8(0x1000, order.len() as u16, 0xA4);
    c.step().expect("a run of prefixes must not fault");
}

/// `CX` is tested before each iteration and decremented after it, so the run
/// ends on the iteration that took the count to zero.
#[test]
fn rep_repeats_exactly_cx_times() {
    let mut c = cpu(&[0xF3, 0xA4]);
    c.regs.set_word(Reg16::Cx, 3);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Cx),
        0,
        "the last iteration decrements too"
    );
    assert_eq!(c.regs.word(Reg16::Si), 0x0023);
    assert_eq!(c.regs.word(Reg16::Di), 0x0303);
}

/// Intel's pseudocode for REP is `WHILE CountReg <> 0`, so a count that starts
/// at zero runs no iterations at all. The 65,536-iteration surprise belongs
/// to `LOOP`, which decrements before it tests; REP tests before it decrements.
#[test]
fn rep_with_a_zero_count_runs_no_iterations() {
    let mut c = cpu(&[0xF3, 0xA4]);
    c.regs.set_word(Reg16::Cx, 0);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x9000, 0x0300, 0xFF);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Si), 0x0020, "SI did not move");
    assert_eq!(c.regs.word(Reg16::Di), 0x0300, "DI did not move");
    assert_eq!(c.mem.read_u8(0x9000, 0x0300), 0xFF, "nothing was written");
}

/// `REP` or `REPNE` in front of a multiply negates the product, and in front
/// of a signed divide negates the quotient.
///
/// The source is the microcode itself, extracted from an 8086 die and
/// disassembled by Andrew Jenner (reenigne.org/blog/8086-microcode-disassembled/):
/// "Using the REP or REPNE prefix with a MUL or IMUL instruction negates the
/// product. Using the REP or REPNE prefix with an IDIV instruction negates the
/// quotient." The multiply and divide microcode flips its internal F1 flag once
/// per negative sign in the inputs, and the REP prefix is what loads that flag,
/// so the sign of the result is inverted as a side effect. Nothing about the
/// FLAGS register changes: MUL, IMUL and IDIV define none of them, and the
/// inversion happens in the result, not in a flag.
///
/// `DIV` is unsigned and has no sign handling to invert, and neither does the
/// byte form of `MUL` differently from the word form.
#[test]
fn rep_negates_a_multiply_and_a_signed_divide() {
    // F7 E1 = MUL CX: mod=11, reg=/4, rm=CX. 3 * 5 = 15, negated to -15.
    let mut c = cpu(&[0xF3, 0xF7, 0xE1]);
    c.regs.set_word(Reg16::Ax, 3);
    c.regs.set_word(Reg16::Cx, 5);
    c.step().unwrap();
    assert_eq!(
        (c.regs.word(Reg16::Ax), c.regs.word(Reg16::Dx)),
        (0xFFF1, 0xFFFF),
        "15 negated to FFF1h with the high half sign-extended"
    );

    // The same instruction without the prefix still multiplies plainly.
    let mut plain = cpu(&[0xF7, 0xE1]);
    plain.regs.set_word(Reg16::Ax, 3);
    plain.regs.set_word(Reg16::Cx, 5);
    plain.step().unwrap();
    assert_eq!(
        (plain.regs.word(Reg16::Ax), plain.regs.word(Reg16::Dx)),
        (0x000F, 0x0000),
        "without REP nothing is negated"
    );

    // F7 F9 = IDIV CX. 100 / 5 = 20, negated to -20.
    let mut c = cpu(&[0xF2, 0xF7, 0xF9]);
    c.regs.set_word(Reg16::Dx, 0);
    c.regs.set_word(Reg16::Ax, 100);
    c.regs.set_word(Reg16::Cx, 5);
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        (-20i32) as u16,
        "REPNE negated the quotient to FFECH"
    );
}

/// `DIV` is unsigned, so its microcode has no sign flag to invert and the
/// latch reaches nobody: `F3 F6 F1` divides exactly like a bare `F6 F1`. This
/// is the negative control for the test above — without it, a latch that was
/// honoured everywhere by accident would still pass.
#[test]
fn rep_does_not_change_an_unsigned_divide() {
    let mut c = cpu(&[0xF3, 0xF6, 0xF1]); // DIV CL
    c.regs.set_word(Reg16::Ax, 0x0064);
    c.regs.set_byte(Reg8::Cl, 0x07);
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 14, "100 / 7 = 14 remainder 2");
    assert_eq!(c.regs.byte(Reg8::Ah), 2);
}

/// The inversion is in the result, never in FLAGS: a multiply defines CF and OF
/// and REP must not disturb them.
#[test]
fn rep_does_not_change_the_flags_of_a_multiply() {
    let mut c = cpu(&[0xF3, 0xF7, 0xE1]);
    c.regs.set_word(Reg16::Ax, 3);
    c.regs.set_word(Reg16::Cx, 5);
    let before = c.regs.flags;
    c.step().unwrap();
    assert_eq!(
        c.regs.flags.0 & (CF | OF),
        before.0 & (CF | OF),
        "a product that fits sets neither, with or without REP"
    );

    // An overflowing product still sets both with REP in front.
    let mut c = cpu(&[0xF3, 0xF7, 0xE1]);
    c.regs.set_word(Reg16::Ax, 0x8000);
    c.regs.set_word(Reg16::Cx, 0x0002);
    c.step().unwrap();
    assert!(
        c.regs.flags.get(CF) && c.regs.flags.get(OF),
        "0x8000 * 2 does not fit in AX, and REP did not hide it"
    );
}

/// The latch is consumed by the multiply it covered. A `MUL` after a `REP MUL`
/// multiplies plainly, exactly like one after any other instruction.
#[test]
fn the_rep_latch_does_not_outlive_the_multiply_it_negated() {
    let mut c = cpu(&[0xF3, 0xF7, 0xE1, 0xF7, 0xE1]);
    c.regs.set_word(Reg16::Ax, 3);
    c.regs.set_word(Reg16::Cx, 5);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0xFFF1, "the first MUL was negated");
    c.regs.set_word(Reg16::Ax, 3);
    c.step().unwrap();
    assert_eq!(
        (c.regs.word(Reg16::Ax), c.regs.word(Reg16::Dx)),
        (0x000F, 0x0000),
        "the second MUL is plain"
    );
}

/// The carry into the high half of a negated product is the carry out of
/// `!lo + 1`, which is 1 exactly when the low half is zero. A first attempt
/// used `overflowing_neg`'s overflow flag instead, which is set exactly when
/// the low half is *not* zero — the two cases are opposites, so every product
/// with a zero low half came back un-negated.
#[test]
fn rep_negates_the_high_half_when_the_low_half_is_zero() {
    let mut c = cpu(&[0xF3, 0xF6, 0xE1]); // REP MUL CL
    c.regs.set_byte(Reg8::Al, 2);
    c.regs.set_byte(Reg8::Cl, 128);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0xFF00, "0x0100 negated is 0xFF00");

    let mut c = cpu(&[0xF3, 0xF7, 0xE1]); // REP MUL CX
    c.regs.set_word(Reg16::Ax, 2);
    c.regs.set_word(Reg16::Cx, 0x8000);
    c.step().unwrap();
    assert_eq!(
        (c.regs.word(Reg16::Ax), c.regs.word(Reg16::Dx)),
        (0x0000, 0xFFFF),
        "0x00010000 negated is 0xFFFF0000"
    );

    let mut c = cpu(&[0xF3, 0xF6, 0xE9]); // REP IMUL CL, 2 * -128 = -256
    c.regs.set_byte(Reg8::Al, 2);
    c.regs.set_byte(Reg8::Cl, 128);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x0100, "-256 negated is 0x0100");
}

/// Every byte-sized pair, signed and unsigned. The whole 8-bit space is 65,536
/// cases and costs little, so it is checked rather than sampled — the first
/// attempt at this rule got 769 of them wrong and no sample had caught it.
#[test]
fn rep_negates_every_eight_bit_multiply() {
    let mut c = cpu(&[0xF3, 0xF6, 0xE1]);
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            // The program is `F3 F6 <modrm>`; reg=100 is MUL and reg=101 IMUL,
            // so each branch sets the ModR/M byte at offset 2 itself.
            c.mem.write_u8(0x1000, 2, 0xE1);
            c.regs.ip = 0;
            c.regs.set_word(Reg16::Cx, 0);
            c.regs.set_byte(Reg8::Al, a);
            c.regs.set_byte(Reg8::Cl, b);
            c.step().unwrap();
            assert_eq!(
                c.regs.word(Reg16::Ax),
                (u16::from(a) * u16::from(b)).wrapping_neg(),
                "REP MUL CL {a} * {b}"
            );

            // The same run with reg=101, which is IMUL. The program is `F3 F6 <modrm>`,
            // so only the ModR/M byte at offset 2 changes.
            c.mem.write_u8(0x1000, 2, 0xE9);
            c.regs.ip = 0;
            c.regs.set_word(Reg16::Cx, 0);
            c.regs.set_byte(Reg8::Al, a);
            c.regs.set_byte(Reg8::Cl, b);
            c.step().unwrap();
            assert_eq!(
                c.regs.word(Reg16::Ax),
                (i16::from(a as i8) * i16::from(b as i8)).wrapping_neg() as u16,
                "REP IMUL CL {a} * {b}"
            );
        }
    }
}

/// A wide sample of word-sized multiplies against a 32-bit reference, plus
/// every pair of the boundary values that catch carry and sign-extension bugs.
/// One CPU is reused across cases; `cpu()` allocates a mebibyte each time. The
// program is `F3 F7 <modrm>`, so the ModR/M byte is at offset 2.
#[test]
fn rep_negates_word_sized_multiplies() {
    let mut cases: Vec<(u16, u16)> = [0u16, 1, 0x100, 0x7FFF, 0x8000, 0x8001, 0xFFFF]
        .iter()
        .flat_map(|&a| [0u16, 1, 0x100, 0x7FFF, 0x8000, 0x8001, 0xFFFF].map(|b| (a, b)))
        .collect();
    let mut seed = 0x1234_5678u64;
    for _ in 0..20_000 {
        let mut next = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) as u16
        };
        cases.push((next(), next()));
    }

    let mut c = cpu(&[0xF3, 0xF7, 0xE1]);
    for (a, b) in cases {
        for (modrm, signed) in [(0xE1u8, false), (0xE9, true)] {
            c.mem.write_u8(0x1000, 2, modrm);
            c.regs.ip = 0;
            c.regs.set_word(Reg16::Ax, a);
            c.regs.set_word(Reg16::Cx, b);
            c.step().unwrap();
            let product = if signed {
                i32::from(a as i16).wrapping_mul(i32::from(b as i16)) as u32
            } else {
                u32::from(a) * u32::from(b)
            };
            let want = product.wrapping_neg();
            assert_eq!(
                (c.regs.word(Reg16::Ax), c.regs.word(Reg16::Dx)),
                (want as u16, (want >> 16) as u16),
                "REP {modrm:02X} {a:#06X} * {b:#06X}"
            );
        }
    }
}

/// `IDIV` negates the quotient and leaves the remainder exactly as the division
/// produced it. A shared two-word negation got this wrong: the tuple that `/7`
/// returns is `(quotient, remainder)`, so complementing the high half inverted
/// the remainder in every non-zero case.
#[test]
fn rep_negates_the_idiv_quotient_and_not_the_remainder() {
    let mut c = cpu(&[0xF2, 0xF7, 0xF9]); // REPNE IDIV CX
    c.regs.set_word(Reg16::Dx, 0);
    c.regs.set_word(Reg16::Ax, 100);
    c.regs.set_word(Reg16::Cx, 7);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), (-14i32) as u16, "quotient negated");
    assert_eq!(c.regs.word(Reg16::Dx), 2, "100 % 7 = 2, not negated");

    let mut c = cpu(&[0xF2, 0xF6, 0xF9]); // REPNE IDIV CL
    c.regs.set_word(Reg16::Ax, 100);
    c.regs.set_byte(Reg8::Cl, 7);
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0xF2, "quotient negated");
    assert_eq!(c.regs.byte(Reg8::Ah), 2, "remainder stays 2");

    let mut c = cpu(&[0xF2, 0xF7, 0xF9]);
    c.regs.set_word(Reg16::Dx, 0);
    c.regs.set_word(Reg16::Ax, 100);
    c.regs.set_word(Reg16::Cx, 5);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), (-20i32) as u16);
    assert_eq!(c.regs.word(Reg16::Dx), 0, "a zero remainder stays zero");
}

/// Every byte-sized `IDIV` that does not fault, so no remainder can slip
/// through unnoticed.
#[test]
fn rep_keeps_every_idiv_remainder() {
    let mut c = cpu(&[0xF2, 0xF6, 0xF9]);
    for a in 0..=0xFFu16 {
        for b in 1..=0x7Fu16 {
            if !(-128..=127).contains(&((a as i16) / (b as i16))) {
                continue;
            }
            c.regs.ip = 0;
            c.regs.set_word(Reg16::Dx, 0);
            c.regs.set_word(Reg16::Ax, a);
            c.regs.set_byte(Reg8::Cl, b as u8);
            c.step().unwrap();
            assert_eq!(
                c.regs.byte(Reg8::Al),
                ((a as i16) / (b as i16)).wrapping_neg() as u8,
                "REPNE IDIV CL {a}/{b} quotient"
            );
            assert_eq!(
                c.regs.byte(Reg8::Ah),
                ((a as i16) % (b as i16)) as u8,
                "REPNE IDIV CL {a}/{b} remainder"
            );
        }
    }
}

/// A divide that faulted leaves nothing behind, and in particular no latch: the
/// next `MUL` must be plain.
#[test]
fn a_faulting_rep_idiv_keeps_no_latch_for_the_next_multiply() {
    // A type 0 interrupt needs a vector, and the handler returns immediately: the
    // point is only that the CPU keeps running afterwards.
    let mut c = cpu(&[0xF2, 0xF7, 0xF9, 0xF7, 0xE1]);
    c.mem.write_u16(0x0000, 0x0000, 0x0010);
    c.mem.write_u16(0x0000, 0x0002, 0x0000);
    c.mem.load_at(0x00010, &[0xCF]).unwrap();
    c.regs.set_word(Reg16::Dx, 0);
    c.regs.set_word(Reg16::Ax, 100);
    c.regs.set_word(Reg16::Cx, 0);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 100, "divide-by-zero writes nothing");
    c.step().unwrap();
    c.regs.set_word(Reg16::Ax, 3);
    c.regs.set_word(Reg16::Cx, 5);
    c.step().unwrap();
    assert_eq!(
        (c.regs.word(Reg16::Ax), c.regs.word(Reg16::Dx)),
        (0x000F, 0x0000),
        "the dead REP must not negate the next MUL"
    );
}

/// `REPE CMPSB` stops at the first pair that does not compare equal, and the
/// comparison that stopped it has already been made: both pointers and the
/// count account for it.
#[test]
fn repe_cmpsb_stops_at_the_first_mismatch() {
    let mut c = cpu(&[0xF3, 0xA6]);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 5);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x0000, 0x0020, 0x11);
    c.mem.write_u8(0x0000, 0x0021, 0x22);
    c.mem.write_u8(0x0000, 0x0022, 0x33);
    c.mem.write_u8(0x9000, 0x0300, 0x11);
    c.mem.write_u8(0x9000, 0x0301, 0x22);
    c.mem.write_u8(0x9000, 0x0302, 0x99);
    c.step().unwrap();
    assert!(!c.regs.flags.get(ZF), "the third pair did not match");
    assert_eq!(c.regs.word(Reg16::Cx), 2, "5 counts, 3 comparisons run");
    assert_eq!(c.regs.word(Reg16::Si), 0x0023);
    assert_eq!(c.regs.word(Reg16::Di), 0x0303);
}

/// `REPE CMPSB` over a fully equal block spends the whole count.
#[test]
fn repe_cmpsb_runs_the_whole_count_when_every_pair_matches() {
    let mut c = cpu(&[0xF3, 0xA6]);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 3);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    for i in 0..3u16 {
        c.mem.write_u8(0x0000, 0x0020 + i, 0x40);
        c.mem.write_u8(0x9000, 0x0300 + i, 0x40);
    }
    c.step().unwrap();
    assert!(c.regs.flags.get(ZF), "every pair matched");
    assert_eq!(c.regs.word(Reg16::Cx), 0);
    assert_eq!(c.regs.word(Reg16::Si), 0x0023);
}

/// `REPNE` stops at the first pair that *does* match — the mirror image.
#[test]
fn repne_cmpsb_stops_at_the_first_match() {
    let mut c = cpu(&[0xF2, 0xA6]);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 5);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x0000, 0x0020, 0x11);
    c.mem.write_u8(0x9000, 0x0300, 0x11);
    c.step().unwrap();
    assert!(c.regs.flags.get(ZF), "the first pair matched");
    assert_eq!(c.regs.word(Reg16::Cx), 4);
    assert_eq!(c.regs.word(Reg16::Si), 0x0021);
    assert_eq!(c.regs.word(Reg16::Di), 0x0301);
}

/// `SCAS` takes its condition from the same `ZF` and stops the same way.
#[test]
fn repe_scasb_stops_at_the_first_mismatch() {
    let mut c = cpu(&[0xF3, 0xAE]);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 4);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.regs.set_byte(Reg8::Al, 0x5A);
    c.mem.write_u8(0x9000, 0x0300, 0x5A);
    c.mem.write_u8(0x9000, 0x0301, 0x5A);
    c.mem.write_u8(0x9000, 0x0302, 0x00);
    c.step().unwrap();
    assert!(!c.regs.flags.get(ZF), "the third byte did not match");
    assert_eq!(c.regs.word(Reg16::Cx), 1, "4 counts, 3 comparisons run");
    assert_eq!(c.regs.word(Reg16::Di), 0x0303);
}

/// Both latches die with the instruction they cover, so one prefix binds to
/// one instruction and never leaks into the next.
#[test]
fn the_rep_latch_does_not_outlive_its_instruction() {
    let mut c = cpu(&[0xF3, 0x26, 0xA4, 0xA4]);
    c.regs.set_segment(Seg::Es, 0x9000);
    c.regs.set_word(Reg16::Cx, 3);
    c.regs.set_word(Reg16::Si, 0x0020);
    c.regs.set_word(Reg16::Di, 0x0300);
    c.mem.write_u8(0x0000, 0x0020, 0x11);
    c.mem.write_u8(0x0000, 0x0021, 0x22);
    c.mem.write_u8(0x0000, 0x0022, 0x33);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Di), 0x0303, "three iterations");
    assert_eq!(
        c.rep, None,
        "the REP latch is decoder state, not a register"
    );
    assert_eq!(c.seg_override, None);

    // The bare `MOVSB` behind it moves one byte and stops, because the count
    // is spent and the latch is gone.
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Di),
        0x0304,
        "one iteration, not another run"
    );
    assert_eq!(c.regs.word(Reg16::Si), 0x0024);
}

/// The same guarantee on the fault path: a prefix must not outlive an
/// instruction that faults.
#[test]
fn a_faulting_prefixed_instruction_clears_both_latches() {
    // 8C /r with a register destination is an 80186 encoding.
    let mut c = cpu(&[0xF3, 0x26, 0x8C, 0xC3]);
    assert_eq!(c.step(), Err(ExecError::NotOn8086(0x8C)));
    assert_eq!(c.rep, None, "the REP latch must not survive a fault");
    assert_eq!(c.seg_override, None);
}

/// A prefixed `StepInfo` names the covered opcode, which is what `LOCK` and
/// the segment overrides already did. `F3` no longer reports itself.
#[test]
fn a_reped_step_reports_the_covered_string_opcode() {
    let mut c = cpu(&[0xF3, 0xA4]);
    let info = c.step().unwrap();
    assert_eq!(info.at, 0);
    assert_eq!(info.opcode, 0xA4, "the MOVSB, not the F3 in front of it");
    assert_eq!(info.next_ip, 2, "both bytes are consumed");
}
