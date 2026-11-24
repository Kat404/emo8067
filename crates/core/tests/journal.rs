//! The journal: the steps the machine can undo, and the depth of the calls.
//!
//! The step back is the reference's: the machine goes back to the state it
//! had before the instruction, which means the bytes that instruction
//! wrote have to go back too. These tests prove that through the public
//! interface — the machine is stepped and rewound, and what it holds
//! afterwards is read back.
//!
//! The code lives at `1000:0100` and the data at `1000:0300`, so the two
//! never share a byte.

use emo8067_core::Cpu;
use emo8067_core::flags::ZF;
use emo8067_core::regs::{Reg16, Seg};

/// The address the code is loaded at.
const CODE: u32 = 0x10100;

/// A machine with a program in it, and the journal on.
fn machine(code: &[u8]) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem.load_at(CODE, code).expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_segment(Seg::Ds, 0x1000);
    cpu.regs.set_segment(Seg::Es, 0x1000);
    cpu.regs.set_segment(Seg::Ss, 0x1000);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu.regs.ip = 0x0100;
    cpu.enable_history();
    cpu
}

/// The bytes an instruction wrote go back when the step is undone.
#[test]
fn the_bytes_an_instruction_wrote_go_back() {
    // B0 42       mov al, 42h
    // A3 00 03    mov [0300h], ax
    // F4          hlt
    let mut cpu = machine(&[0xB0, 0x42, 0xA3, 0x00, 0x03, 0xF4]);
    // The high byte of the word holds something of its own.
    cpu.mem.write_u8(0x1000, 0x0301, 0xAB);
    cpu.step().expect("mov al");
    cpu.step().expect("mov [0300h], ax");
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0300),
        0x42,
        "the byte was written"
    );
    assert_eq!(cpu.mem.read_u8(0x1000, 0x0301), 0x00, "the high byte too");
    assert!(cpu.step_back(), "there is a step to undo");
    assert_eq!(cpu.mem.read_u8(0x1000, 0x0300), 0x00, "the byte goes back");
    assert_eq!(cpu.mem.read_u8(0x1000, 0x0301), 0xAB, "and the high byte");
    assert_eq!(cpu.regs.ip, 0x0102, "IP goes back");
    assert_eq!(cpu.regs.word(Reg16::Ax), 0x0042, "and the registers");
    assert!(!cpu.halted, "and the machine is not halted");
}

/// A word written at the wrap edge of the segment goes back, both halves.
#[test]
fn a_word_written_at_the_wrap_edge_of_the_segment_goes_back() {
    // B8 34 12    mov ax, 1234h
    // A3 FF FF    mov [ffffh], ax      -- the high byte lands at 0000h
    // F4          hlt
    let mut cpu = machine(&[0xB8, 0x34, 0x12, 0xA3, 0xFF, 0xFF, 0xF4]);
    cpu.mem.write_u8(0x1000, 0xFFFF, 0x77);
    cpu.mem.write_u8(0x1000, 0x0000, 0x88);
    cpu.step().expect("mov ax");
    cpu.step().expect("mov [ffffh], ax");
    assert_eq!(cpu.mem.read_u8(0x1000, 0xFFFF), 0x34, "the low byte");
    assert_eq!(cpu.mem.read_u8(0x1000, 0x0000), 0x12, "the high byte");
    assert!(cpu.step_back(), "there is a step to undo");
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0xFFFF),
        0x77,
        "the low byte goes back"
    );
    assert_eq!(cpu.mem.read_u8(0x1000, 0x0000), 0x88, "and the high byte");
}

/// The flags an instruction changed go back with it.
#[test]
fn the_flags_an_instruction_changed_go_back() {
    // 3B C3       cmp ax, bx      -- ax = bx = 0, so the zero flag is set
    // F4          hlt
    let mut cpu = machine(&[0x3B, 0xC3, 0xF4]);
    assert!(!cpu.regs.flags.get(ZF), "the flag starts clear");
    cpu.step().expect("cmp ax, bx");
    assert!(cpu.regs.flags.get(ZF), "the instruction set it");
    assert!(cpu.step_back(), "there is a step to undo");
    assert!(!cpu.regs.flags.get(ZF), "the flag goes back");
    assert_eq!(cpu.regs.ip, 0x0100, "and IP goes back");
}

/// A journal with nothing in it refuses to rewind, and changes nothing.
#[test]
fn a_journal_with_nothing_in_it_rewinds_nothing() {
    let mut cpu = machine(&[0x90, 0xF4]);
    let before = cpu.regs;
    assert!(!cpu.step_back(), "there is nothing to rewind");
    assert_eq!(cpu.regs.ip, before.ip, "nothing moved");
    assert_eq!(cpu.call_depth, 0, "and the depth is where it was");
    assert!(!cpu.halted, "and the machine is not halted");
}

/// The depth of the calls goes back with the step.
#[test]
fn the_depth_of_the_calls_goes_back() {
    // E8 02 00    call 0105h
    // 90 90       nop, nop
    // C3          ret
    let mut cpu = machine(&[0xE8, 0x02, 0x00, 0x90, 0x90, 0xC3]);
    cpu.step().expect("call");
    assert_eq!(cpu.call_depth, 1, "the call went in");
    assert_eq!(cpu.regs.ip, 0x0105, "and the machine is in the callee");
    assert!(cpu.step_back(), "there is a step to undo");
    assert_eq!(cpu.call_depth, 0, "the depth goes back");
    assert_eq!(cpu.regs.ip, 0x0100, "and so does IP");
}

/// The halt state goes back with the step.
#[test]
fn the_halt_state_goes_back() {
    let mut cpu = machine(&[0xF4]);
    cpu.step().expect("hlt");
    assert!(cpu.halted, "the machine halted");
    assert!(cpu.step_back(), "there is a step to undo");
    assert!(!cpu.halted, "the halt goes back");
}

/// The interrupt is a call: it goes in and the iret comes out.
#[test]
fn the_interrupt_is_a_call() {
    let mut cpu = machine(&[0xCD, 0x10, 0xF4]);
    cpu.step().expect("int 10h");
    assert_eq!(cpu.call_depth, 1, "the interrupt went in");
    assert!(cpu.step_back(), "there is a step to undo");
    assert_eq!(cpu.call_depth, 0, "the depth goes back");
}

/// A far call goes in and the far return comes out.
#[test]
fn a_far_call_goes_in_and_comes_out() {
    // 9A 07 01 00 10   call far 1000:0107
    // 90 90            nop, nop
    // CB               retf
    let mut cpu = machine(&[0x9A, 0x07, 0x01, 0x00, 0x10, 0x90, 0x90, 0xCB]);
    cpu.step().expect("call far");
    assert_eq!(cpu.call_depth, 1, "the far call went in");
    cpu.step().expect("retf");
    assert_eq!(cpu.call_depth, 0, "and the far return came out");
}

/// A call through a register goes in and comes out.
#[test]
fn an_indirect_call_goes_in_and_comes_out() {
    // BB 07 01     mov bx, 0107h
    // FF D3        call bx
    // 90 90        nop, nop
    // C3           ret
    let mut cpu = machine(&[0xBB, 0x07, 0x01, 0xFF, 0xD3, 0x90, 0x90, 0xC3]);
    cpu.step().expect("mov bx");
    cpu.step().expect("call bx");
    assert_eq!(cpu.call_depth, 1, "the call went in");
    cpu.step().expect("ret");
    assert_eq!(cpu.call_depth, 0, "and the return came out");
}
