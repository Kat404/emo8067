//! Regression probes for defects found in independent verification of T11.
//!
//! Each one is a case that passed a 96-test suite while the emulator was
//! wrong, so each is pinned here with the bytes that trigger it.

use emo8067_core::Cpu;
use emo8067_core::exec::ExecError;
use emo8067_core::flags::{AF, CF, OF, PF, SF, ZF};
use emo8067_core::regs::{Reg8, Reg16, Seg};

fn cpu(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ss, 0x1000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

fn run(c: &mut Cpu, n: usize) {
    for _ in 0..n {
        c.step().expect("the program must not fault");
    }
}

/// `A8` is `CMP AL, imm8` and carries no ModR/M byte. Routing it through the
/// shared r/m,imm helper consumed a phantom one and stored to memory.
#[test]
fn a8_is_cmp_al_imm8_and_touches_no_memory() {
    let mut c = cpu(&[0xB0, 0x10, 0xA8, 0x10]);
    c.mem.write_u8(0x0000, 0x0020, 0x77);
    run(&mut c, 2);
    assert_eq!(
        c.mem.read_u8(0x0000, 0x0020),
        0x77,
        "A8 must not write memory"
    );
    assert_eq!(c.regs.ip, 4, "A8 ib is two bytes");
    assert_eq!(c.regs.byte(Reg8::Al), 0x10, "CMP stores nothing");
    assert!(c.regs.flags.get(ZF));
}

/// `A9` is `CMP AX, imm16`, three bytes, still no memory access.
#[test]
fn a9_is_cmp_ax_imm16_and_touches_no_memory() {
    let mut c = cpu(&[0xB8, 0x34, 0x12, 0xA9, 0x34, 0x12]);
    c.mem.write_u8(0x0000, 0x0020, 0x77);
    run(&mut c, 2);
    assert_eq!(c.mem.read_u8(0x0000, 0x0020), 0x77);
    assert_eq!(c.regs.ip, 6, "B8 iw (3) then A9 iw (3)");
    assert!(c.regs.flags.get(ZF));
}

/// A catch-all match arm over `AC..=AF` ran `SCAS` for `LODSB`.
#[test]
fn repe_lodsb_loads_the_byte_into_al() {
    let mut c = cpu(&[0xB9, 0x01, 0x00, 0xBE, 0x20, 0x00, 0xF3, 0xAC]);
    c.mem.write_u8(0x0000, 0x0020, 0x5A);
    run(&mut c, 3);
    assert_eq!(c.regs.byte(Reg8::Al), 0x5A, "REPE LODSB must load AL");
    assert_eq!(c.regs.word(Reg16::Si), 0x0021);
    assert_eq!(c.regs.word(Reg16::Cx), 0);
}

#[test]
fn rep_lodsw_loads_the_word_into_ax() {
    let mut c = cpu(&[0xB9, 0x01, 0x00, 0xBE, 0x20, 0x00, 0xF3, 0xAD]);
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

/// A sweep over the encodings still absent, so the gap is visible rather than
/// assumed. This fails until T11 completes; it is the to-do list, not a claim.
#[test]
#[ignore = "T11 is incomplete: these 8086 encodings are not implemented yet"]
fn the_encodings_that_are_still_missing_are_listed_here() {
    let missing: Vec<u8> = vec![0x86, 0x8F, 0x9E, 0x9F, 0xC4, 0xC5, 0xCE, 0xD4, 0xD5];
    for op in missing {
        let mut c = Cpu::new();
        assert!(
            c.mem
                .load_at(0x10000, &[op, 0xC3, 0x06, 0x20, 0x00, 0x0A])
                .is_ok(),
            "the probe program must fit"
        );
        c.regs.set_segment(Seg::Cs, 0x1000);
        c.regs.set_segment(Seg::Ss, 0x1000);
        c.regs.set_word(Reg16::Sp, 0x0100);
        assert_ne!(
            c.step(),
            Err(ExecError::UnknownOpcode(op)),
            "{op:#04x} is a real 8086 opcode and should not be reported unknown"
        );
    }
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
#[ignore = "OPEN: LEA AX,[BX] with BX=1234h does not land on 1234h in this build. Undiagnosed; do not trust LEA until it is."]
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
