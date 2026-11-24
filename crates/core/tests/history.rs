//! The journal: the steps the machine can undo.
//!
//! The step back is the reference's: the machine goes back to the state it
//! had before the instruction, which means the bytes that instruction
//! wrote have to go back too. These tests prove that through the public
//! interface — the machine is stepped and rewound, and what it holds
//! afterwards is read back.
//!
//! **These are the implementer's own cases**, the ones that went from
//! red to green with the journal. The independent verifier's probes live
//! in `journal.rs`, and they stay apart: a guarantee that two files
//! prove is a guarantee with two owners.

use emo8067_core::Cpu;
use emo8067_core::flags::ZF;
use emo8067_core::history::{MAXIMUM_STEPS_BACK, MAXIMUM_WRITES};
use emo8067_core::memory::Memory;
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

/// A step that writes more than the journal can hold breaks the journal.
///
/// The guest chooses how much it writes: a `rep stosw` with a count of
/// FFFFh writes 131 070 bytes, and the journal cannot hold them all.
#[test]
fn a_step_that_writes_more_than_the_cap_breaks_the_journal() {
    // F3 AB   rep stosw
    let mut cpu = machine(&[0xF3, 0xAB]);
    cpu.regs.set_word(Reg16::Cx, 0xFFFF);
    cpu.regs.set_word(Reg16::Di, 0x0300);
    cpu.enable_history();
    cpu.step().expect("the store runs");
    assert!(
        cpu.history.is_broken(),
        "the journal is broken: the step cannot be undone"
    );
    assert!(
        !cpu.step_back(),
        "and the step back refuses: the memory moved past the step"
    );
    assert_eq!(cpu.history.len(), 0, "and the journal holds nothing");
}

/// The journal's total stays inside the bound, whatever the guest writes.
///
/// Every step writes a lot, and the journal drops the oldest records
/// rather than grow past the bound.
#[test]
fn the_journal_keeps_its_total_inside_the_bound() {
    // B9 00 10   mov cx, 1000h
    // F3 AB      rep stosw
    // EB F9      jmp back to the mov
    let mut cpu = machine(&[0xB9, 0x00, 0x10, 0xF3, 0xAB, 0xEB, 0xF9]);
    cpu.regs.set_word(Reg16::Di, 0x0300);
    cpu.enable_history();
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

/// The oldest step goes off the journal when it is full.
#[test]
fn the_oldest_step_goes_off_the_journal_when_it_is_full() {
    // EB FE   jmp back to itself
    let mut cpu = machine(&[0xEB, 0xFE]);
    cpu.enable_history();
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
    assert!(!cpu.step_back(), "and the oldest step went off the journal");
}

/// A machine with the journal off keeps no write log at all.
///
/// The log belongs to the journal: the headless runner never rewinds the
/// machine, and it must not pay for a log it does not read.
#[test]
fn a_memory_that_keeps_no_log_keeps_nothing() {
    let mut mem = Memory::new();
    mem.write_u8(0x1000, 0x0300, 0x11);
    assert!(
        mem.take_writes().is_empty(),
        "the log is empty: the journal is off"
    );
    mem.set_logging(true);
    mem.write_u8(0x1000, 0x0300, 0x22);
    assert_eq!(
        mem.take_writes().len(),
        1,
        "and the log is kept when the journal is on"
    );
}

/// The journal's own log holds the writes the instructions made.
#[test]
fn the_journal_turns_the_write_log_on() {
    // A3 00 01   mov [0100], ax
    let mut cpu = machine(&[0xA3, 0x00, 0x01]);
    cpu.step().expect("the store runs");
    assert_eq!(
        cpu.history.writes(),
        0,
        "the journal is off: it keeps nothing"
    );
    cpu.enable_history();
    cpu.step().expect("the store runs again");
    assert_eq!(
        cpu.history.writes(),
        1,
        "and the journal keeps the write when it is on"
    );
}

/// The restore puts back the value a byte held before the instruction.
///
/// A byte the instruction wrote twice holds the value it had before the
/// first write, and the restore must undo the last write first.
#[test]
fn the_restore_puts_back_the_value_a_byte_held_first() {
    let mut mem = Memory::new();
    mem.set_logging(true);
    mem.write_u8(0x1000, 0x0300, 0x11);
    mem.write_u8(0x1000, 0x0300, 0x22);
    let writes = mem.take_writes();
    mem.restore(&writes);
    assert_eq!(
        mem.read_u8(0x1000, 0x0300),
        0x00,
        "the byte goes back to the value it held first"
    );
    assert!(
        mem.last_write.is_none(),
        "and the last write is not the one that was undone"
    );
}
