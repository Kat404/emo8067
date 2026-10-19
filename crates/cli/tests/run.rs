//! End-to-end: real programs, run through the public API.
//!
//! These are the tests that catch the class of bug unit tests miss — an
//! instruction that decodes but does not do the right thing to memory, the
//! stack, or the flags only shows up when the whole thing runs.

use emo8067_core::flags::{CF, ZF};
use emo8067_core::regs::{Reg16, Seg};
use emo8067_core::{Cpu, StopReason};

/// Load `image` at `1000:0000` and run until it halts or the budget runs out.
fn run(image: &[u8], budget: u64) -> Cpu {
    let mut cpu = Cpu::new();
    assert!(cpu.mem.load_at(0x10000, image).is_ok());
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_segment(Seg::Ss, 0x2000);
    cpu.regs.set_segment(Seg::Ds, 0);
    cpu.regs.set_word(Reg16::Sp, 0xFFFE);
    let mut n = 0;
    while n < budget {
        cpu.step().expect("the program must not fault");
        n += 1;
        if cpu.stop_reason == StopReason::Halted {
            break;
        }
    }
    cpu
}

#[test]
fn a_com_program_computes_a_sum_and_halts() {
    let cpu = run(
        &[
            0xB8, 0x34, 0x12, // MOV AX, 1234h
            0xBB, 0x11, 0x11, // MOV BX, 1111h
            0x01, 0xD8, // ADD AX, BX
            0x2D, 0x00, 0x01, // SUB AX, 0100h
            0xF4, // HLT
        ],
        100,
    );
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x1234 + 0x1111 - 0x0100);
    assert!(cpu.halted);
    assert!(!cpu.regs.flags.get(CF));
}

#[test]
fn a_loop_that_never_terminates_stops_on_the_budget() {
    let cpu = run(&[0xE2, 0xFE], 10);
    assert!(!cpu.halted);
    assert_eq!(cpu.regs.word(Reg16::Cx), 0xFFF6, "decremented ten times");
}

#[test]
fn a_subroutine_returns_to_the_caller() {
    let cpu = run(
        &[
            // CALL is relative to the end of itself, which is offset 3.
            0xE8, 0x03, 0x00, // 0000: CALL 0006
            0xF4, // 0003: HLT
            0x90, 0x90, // 0004, 0005: padding
            0xBB, 0x05, 0x00, // 0006: MOV BX, 5   (BB is MOV BX; BC is MOV SP)
            0xC3, // 0009: RET
        ],
        100,
    );
    assert!(cpu.halted);
    assert_eq!(cpu.regs.word(Reg16::Bx), 5);
    assert_eq!(cpu.regs.word(Reg16::Sp), 0xFFFE, "the stack is balanced");
}

#[test]
fn rep_stosb_fills_a_block_and_an_interrupt_returns_from_it() {
    let mut cpu = Cpu::new();
    let program = [
        0xFC, // CLD
        0xBF, 0x00, 0x30, // MOV DI, 3000h   (BF is MOV DI; BB is MOV BX)
        0xB0, 0x20, // MOV AL, 20h
        0xB9, 0x10, 0x00, // MOV CX, 10h
        0xF3, 0xAA, // REP STOSB
        0xCD, 0x21, // INT 21h
        0xF4, // HLT after the handler returns
    ];
    assert!(cpu.mem.load_at(0x10000, &program).is_ok());
    assert!(cpu.mem.load_at(0x20000, &[0xCF]).is_ok()); // the handler is just IRET
    cpu.mem.write_u16(0x0000, 0x21 * 4, 0x0000);
    cpu.mem.write_u16(0x0000, 0x21 * 4 + 2, 0x2000);
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_segment(Seg::Ss, 0x3000);
    cpu.regs.set_segment(Seg::Ds, 0);
    cpu.regs.set_segment(Seg::Es, 0);
    cpu.regs.set_word(Reg16::Sp, 0xFFFE);

    for _ in 0..100 {
        cpu.step().expect("the program must not fault");
        if cpu.halted {
            break;
        }
        if cpu.stop_reason == StopReason::Interrupt(0x21) && cpu.regs.segment(Seg::Cs) == 0x2000 {
            cpu.step().expect("IRET must not fault");
        }
    }

    assert!(cpu.halted, "execution must come back from the handler");
    assert_eq!(cpu.mem.read_u16(0, 0x3000), 0x2020);
    assert_eq!(cpu.mem.read_u16(0, 0x300E), 0x2020);
    assert_eq!(cpu.regs.word(Reg16::Di), 0x3010);
    assert_eq!(cpu.regs.word(Reg16::Sp), 0xFFFE, "IRET unwound the frame");
}

#[test]
fn a_compare_that_matches_sets_zero_and_does_not_store() {
    let mut cpu = Cpu::new();
    assert!(cpu.mem.load_at(0x10000, &[0x3D, 0x34, 0x12, 0xF4]).is_ok());
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_word(Reg16::Ax, 0x1234);
    cpu.step().unwrap();
    assert!(cpu.regs.flags.get(ZF));
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x1234, "CMP stores nothing");
}
