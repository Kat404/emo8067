//! The journal: the steps the machine can undo.
//!
//! The step back is the reference's: the machine goes back to the state it
//! had before the instruction, which means the bytes that instruction
//! wrote have to go back too. These tests prove that through the public
//! interface — the machine is stepped and rewound, and what it holds
//! afterwards is read back.

use emo8067_core::Cpu;
use emo8067_core::flags::ZF;
use emo8067_core::regs::{Reg16, Seg};

/// A machine with a program in it.
///
/// The code and the data live in the same segment, at `1000:0000`, and the
/// stack is in it too.
fn machine(code: &[u8]) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem.load_at(0x10000, code).expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_segment(Seg::Ds, 0x1000);
    cpu.regs.set_segment(Seg::Ss, 0x1000);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu
}

#[test]
fn the_journal_restores_the_registers_the_flags_and_the_bytes() {
    // MOV AX, 1234h ; MOV [0100], AX ; SUB AX, AX
    let mut cpu = machine(&[0xB8, 0x34, 0x12, 0xA3, 0x00, 0x01, 0x2B, 0xC0]);
    cpu.enable_history();
    cpu.step().expect("the first instruction runs");
    cpu.step().expect("the store runs");
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0100),
        0x34,
        "the store wrote the byte"
    );
    cpu.step().expect("the subtraction runs");
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x0000, "the machine moved on");
    assert!(cpu.regs.flags.get(ZF), "and the flag says so");

    assert!(cpu.step_back(), "the journal has a step to undo");
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x1234, "the register is back");
    assert_eq!(cpu.regs.ip, 0x0006, "the instruction pointer is back");
    assert!(!cpu.regs.flags.get(ZF), "and the flags are back");

    assert!(cpu.step_back(), "and the step before it");
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0100),
        0x00,
        "the byte the store wrote is back"
    );
    assert_eq!(cpu.regs.ip, 0x0003, "the instruction pointer is back too");
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x1234, "and the register is back");
}

#[test]
fn the_depth_counts_a_call_and_a_return() {
    // CALL +1 ; RET
    let mut cpu = machine(&[0xE8, 0x01, 0x00, 0x00, 0xC3]);
    assert_eq!(cpu.call_depth, 0, "the machine is not in a call");
    cpu.step().expect("the call runs");
    assert_eq!(cpu.call_depth, 1, "the call went in");
    assert_eq!(cpu.regs.ip, 0x0004, "and the machine is inside it");
    cpu.step().expect("the return runs");
    assert_eq!(cpu.call_depth, 0, "the return came out");
}

#[test]
fn a_step_back_restores_the_depth_of_the_calls() {
    // CALL +1 ; RET
    let mut cpu = machine(&[0xE8, 0x01, 0x00, 0x00, 0xC3]);
    cpu.enable_history();
    assert_eq!(cpu.call_depth, 0, "the machine is not in a call");
    cpu.step().expect("the call runs");
    assert_eq!(cpu.call_depth, 1, "the call went in");
    assert!(cpu.step_back(), "the journal has a step to undo");
    assert_eq!(
        cpu.call_depth, 0,
        "and the depth is the one the machine had before the call"
    );
    assert_eq!(cpu.regs.ip, 0x0000, "and the machine is where it was");
}

#[test]
fn a_step_back_with_nothing_to_rewind_changes_nothing() {
    let mut cpu = machine(&[0xB8, 0x34, 0x12]);
    cpu.step().expect("the instruction runs");
    let ip = cpu.regs.ip;
    let ax = cpu.regs.word(Reg16::Ax);
    assert!(!cpu.step_back(), "there is nothing to rewind");
    assert_eq!(cpu.regs.ip, ip, "the machine is where it was");
    assert_eq!(cpu.regs.word(Reg16::Ax), ax, "and it holds what it held");
}

#[test]
fn a_journal_that_is_off_keeps_nothing() {
    let mut cpu = machine(&[0xB8, 0x34, 0x12]);
    cpu.step().expect("the instruction runs");
    assert!(!cpu.history.is_enabled(), "the journal is off");
    assert_eq!(cpu.history.len(), 0, "and it kept nothing");
    assert!(!cpu.step_back(), "so there is nothing to rewind");
}
