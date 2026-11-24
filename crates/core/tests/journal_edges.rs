//! The journal's edges: its bound, its restore, and its doors.
//!
//! These are the verifier's probes for the correction: the depth stops at
//! its bound, the restore refuses an address outside the address space,
//! the log belongs to the journal, a bulk write says that it threw the
//! recorded steps away, and the machine with the journal off keeps
//! nothing.

use emo8067_core::Cpu;
use emo8067_core::history::MAXIMUM_WRITES;
use emo8067_core::regs::{Reg16, Seg};

/// A machine with a program in it.
fn machine(code: &[u8]) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem.load_at(0x10100, code).expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, 0x1000);
    cpu.regs.set_segment(Seg::Ds, 0x1000);
    cpu.regs.set_segment(Seg::Es, 0x1000);
    cpu.regs.set_segment(Seg::Ss, 0x1000);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu.regs.ip = 0x0100;
    cpu
}

/// The depth stops at its bound: a return without a call is the guest's
/// business, and not a panic.
#[test]
fn the_depth_stops_at_its_bound() {
    // C3   ret
    let mut cpu = machine(&[0xC3]);
    cpu.enable_history();
    cpu.call_depth = i32::MIN;
    cpu.step().expect("the return runs");
    assert_eq!(
        cpu.call_depth,
        i32::MIN,
        "the depth stops at its bound and does not wrap"
    );
}

/// The restore refuses an address outside the address space.
#[test]
fn the_restore_refuses_an_address_outside_the_address_space() {
    let mut cpu = machine(&[0x90]);
    cpu.enable_history();
    cpu.mem.restore(&[(0x0010_0000, 0x55)]);
    assert_eq!(
        cpu.mem.read_u8(0x1000, 0x0010),
        0x00,
        "the byte outside the space is not written"
    );
}

/// The log belongs to the journal: a machine that never rewinds keeps
/// nothing, and the watchpoint does not need the log.
#[test]
fn the_log_belongs_to_the_journal() {
    // A3 00 03   mov [0300h], ax
    let mut cpu = machine(&[0xA3, 0x00, 0x03]);
    cpu.disable_history();
    cpu.step().expect("the store runs");
    assert!(
        cpu.mem.take_writes().is_empty(),
        "the log keeps nothing while the journal is off"
    );
    assert_eq!(
        cpu.mem.last_write,
        Some((0x1000, 0x0300, 2)),
        "and the write is still named: the watchpoint does not need the log"
    );
}

/// A bulk write that fills the budget says that it threw the recorded
/// steps away.
#[test]
fn a_bulk_write_says_that_it_threw_the_history_away() {
    // A3 00 01   mov [0100], ax
    // F3 A5      rep movsw
    let mut cpu = machine(&[0xA3, 0x00, 0x01, 0xF3, 0xA5]);
    cpu.enable_history();
    cpu.regs.set_word(Reg16::Cx, 0x8000);
    cpu.regs.set_word(Reg16::Si, 0x0300);
    cpu.regs.set_word(Reg16::Di, 0x0400);
    cpu.step().expect("the store runs");
    cpu.step().expect("the bulk write runs");
    assert!(
        cpu.history.is_discarded(),
        "the journal says its steps went: {}",
        cpu.history.len()
    );
    assert!(
        cpu.history.writes() <= MAXIMUM_WRITES,
        "and the total is inside the bound: {}",
        cpu.history.writes()
    );
}

/// A step that wrote too much leaves no log behind.
#[test]
fn a_step_that_wrote_too_much_leaves_no_log_behind() {
    // F3 A5      rep movsw
    let mut cpu = machine(&[0xF3, 0xA5]);
    cpu.enable_history();
    cpu.regs.set_word(Reg16::Cx, 0x8001);
    cpu.regs.set_word(Reg16::Si, 0x0300);
    cpu.regs.set_word(Reg16::Di, 0x0400);
    cpu.step().expect("the bulk write runs");
    assert!(cpu.history.is_broken(), "the journal is broken");
    assert!(
        cpu.mem.take_writes().is_empty(),
        "and the log keeps nothing: the step's writes are gone"
    );
}
