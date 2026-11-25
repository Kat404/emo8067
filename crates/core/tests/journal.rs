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
//!
//! **These are the independent verifier's probes**, written without the
//! implementer's own cases in sight. They stay in a file of their own:
//! the implementer's red-to-green cases live in `history.rs`, and a
//! guarantee that two files prove is a guarantee with two owners.

use emo8067_core::Cpu;
use emo8067_core::flags::ZF;
use emo8067_core::history::{MAXIMUM_STEPS_BACK, MAXIMUM_WRITES};
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

/// The write log stops at the bound and says so.
#[test]
fn the_write_log_stops_at_the_bound_and_says_so() {
    // The log belongs to the journal, and the journal is on: the machine
    // keeps the writes.
    let mut cpu = machine(&[0x90]);
    for i in 0..100_000u32 {
        cpu.mem.write_u8(0x1000, (i & 0xFFFF) as u16, 0x55);
    }
    assert_eq!(
        cpu.mem.take_writes().len(),
        MAXIMUM_WRITES,
        "the log stops at the bound"
    );
    assert!(cpu.mem.is_truncated(), "and it says so");
}

/// The journal's total stays inside the bound; the oldest records go.
#[test]
fn the_journal_total_stays_inside_the_bound() {
    // B9 00 10   mov cx, 1000h
    // F3 AB      rep stosw
    // EB F9      jmp back to the mov
    let mut cpu = machine(&[0xB9, 0x00, 0x10, 0xF3, 0xAB, 0xEB, 0xF9]);
    cpu.regs.set_word(Reg16::Di, 0x0300);
    for _ in 0..40 {
        cpu.step().expect("the program runs");
    }
    assert!(
        cpu.history.writes() <= MAXIMUM_WRITES,
        "the total is inside the bound: {}",
        cpu.history.writes()
    );
    assert!(
        cpu.history.len() < 40,
        "and the oldest records went off the journal: {}",
        cpu.history.len()
    );
}

/// The biggest bulk write the journal can hold is still undoable.
#[test]
fn the_biggest_bulk_write_the_journal_can_hold_is_still_undoable() {
    // F3 A5   rep movsw
    let mut cpu = machine(&[0xF3, 0xA5]);
    cpu.regs.set_word(Reg16::Cx, 0x8000);
    cpu.regs.set_word(Reg16::Si, 0x0300);
    cpu.regs.set_word(Reg16::Di, 0x0400);
    cpu.mem.write_u8(0x1000, 0x0400, 0x5A);
    cpu.mem.write_u8(0x1000, 0x0401, 0xA5);
    cpu.step().expect("the move runs");
    assert!(
        !cpu.history.is_broken(),
        "65536 entries are inside the bound"
    );
    assert_eq!(cpu.history.writes(), 65536, "the journal holds them all");
    assert!(cpu.step_back(), "and the step can be undone");
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0400),
        0x5A,
        "and the bytes go back"
    );
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0401),
        0xA5,
        "and the other half too"
    );
}

/// Two entries over the bound break the journal, and nothing moves.
#[test]
fn a_step_that_writes_more_than_the_bound_breaks_the_journal() {
    // F3 A5   rep movsw, with 32769 words: 65538 bytes.
    let mut cpu = machine(&[0xF3, 0xA5]);
    cpu.regs.set_word(Reg16::Cx, 0x8001);
    cpu.regs.set_word(Reg16::Si, 0x0300);
    cpu.regs.set_word(Reg16::Di, 0x0400);
    cpu.step().expect("the move runs");
    assert!(cpu.history.is_broken(), "the journal is broken");
    assert_eq!(cpu.history.len(), 0, "and it holds nothing");
    let ip = cpu.regs.ip;
    assert!(!cpu.step_back(), "and the step back refuses");
    assert_eq!(cpu.regs.ip, ip, "and nothing moved");
}

/// The restore undoes the last write first.
#[test]
fn the_restore_undoes_the_last_write_first() {
    // The log belongs to the journal, and the journal is on.
    let mut cpu = machine(&[0x90]);
    cpu.mem.write_u8(0x1000, 0x0300, 0x11);
    cpu.mem.write_u8(0x1000, 0x0301, 0x22);
    cpu.mem.write_u8(0x1000, 0x0300, 0x33);
    cpu.mem.write_u8(0x1000, 0x0300, 0x44);
    let writes = cpu.mem.take_writes();
    assert_eq!(writes.len(), 4, "the log holds the four writes");
    cpu.mem.restore(&writes);
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0300),
        0x00,
        "the first value the byte held survives"
    );
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0301),
        0x00,
        "and the other byte too"
    );
}

/// The journal's boundary: 257 steps, 256 records, 256 rewinds.
#[test]
fn the_journal_keeps_256_steps_and_refuses_the_257th() {
    // EB FE   jmp back to itself
    let mut cpu = machine(&[0xEB, 0xFE]);
    for _ in 0..=MAXIMUM_STEPS_BACK {
        cpu.step().expect("the program runs");
    }
    assert_eq!(
        cpu.history.len(),
        MAXIMUM_STEPS_BACK,
        "the journal keeps its steps"
    );
    for _ in 0..MAXIMUM_STEPS_BACK {
        assert!(cpu.step_back(), "the journal has a step to undo");
    }
    assert!(!cpu.step_back(), "and the 257th refuses");
}

/// The write of the step before is not this step's.
#[test]
fn an_instruction_that_writes_nothing_leaves_no_last_write() {
    // A3 00 03   mov [0300h], ax
    // 90          nop
    let mut cpu = machine(&[0xA3, 0x00, 0x03, 0x90]);
    cpu.step().expect("the store runs");
    assert_eq!(
        cpu.mem.last_write,
        Some((0x1000, 0x0300, 2)),
        "the store wrote"
    );
    cpu.step().expect("the nop runs");
    assert!(cpu.mem.last_write.is_none(), "the nop wrote nothing");
    assert!(cpu.step_back(), "there is a step to undo");
    assert!(
        cpu.mem.last_write.is_none(),
        "and the rewind names no write"
    );
}

/// The log belongs to the journal; the watchpoint does not need it.
#[test]
fn the_journal_off_keeps_no_log_but_the_write_is_still_named() {
    let mut cpu = machine(&[0xA3, 0x00, 0x03]);
    cpu.disable_history();
    cpu.step().expect("the store runs");
    assert!(cpu.mem.take_writes().is_empty(), "the log keeps nothing");
    assert_eq!(
        cpu.mem.last_write,
        Some((0x1000, 0x0300, 2)),
        "and the write is still named"
    );
}
