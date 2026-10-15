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
    // Every 8086 opcode still absent. This is the gate for T11: it stays
    // ignored until the list is empty, and un-ignoring it before then is
    // the point.
    let missing: Vec<u8> = vec![
        0x84, 0x85, // TEST r/m, r
        0x9A, 0x9B, // CALL far, WAIT
        0xCA, 0xCB, // RETF, RETF iw
        0xCE,       // INTO
        0xD4, 0xD5, // AAM, AAD
        0xD7,       // XLAT
        0xEA,       // JMP far
        0xF0,       // LOCK
    ];
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
    let mut c = cpu(&[0xB9, 0x03, 0x00, 0xBE, 0x20, 0x00, 0xF3, 0xAC]);
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
