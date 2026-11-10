//! The breakpoints, the watchpoints and the panels.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::breakpoints::{Access, Breakpoints, Missed};
use emo8067_debug::expr;
use emo8067_debug::machine::Machine;
use emo8067_debug::panels::{Panels, Ui};

/// A machine with a little code in it and the stack where it was left.
fn machine() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x02000, &[0xB8, 0x34, 0x12, 0xCD, 0x21, 0x90])
        .unwrap();
    cpu.regs.set_segment(Seg::Cs, 0x0200);
    cpu.regs.set_segment(Seg::Ss, 0x1000);
    cpu.regs.set_segment(Seg::Ds, 0x1000);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu.regs.set_word(Reg16::Ax, 0x1234);
    cpu
}

#[test]
fn a_breakpoint_with_a_condition_stops_only_where_the_condition_holds() {
    let cpu = machine();
    let mut breakpoints = Breakpoints::new();
    breakpoints
        .add(0x0200, 0x0010, Some("AX==0x1234 && CS==0x0200"))
        .unwrap();
    let machine = Machine::new(&cpu);

    assert!(
        breakpoints.stops_at(&machine, 0x0200, 0x0010),
        "the condition holds there"
    );
    assert!(
        !breakpoints.stops_at(&machine, 0x0200, 0x0011),
        "and only there"
    );

    // The same condition, a machine whose `AX` is another value.
    let mut other = cpu.clone();
    other.regs.set_word(Reg16::Ax, 0x4321);
    let machine = Machine::new(&other);
    assert!(
        !breakpoints.stops_at(&machine, 0x0200, 0x0010),
        "and not when the machine is another one"
    );
}

#[test]
fn a_breakpoint_without_a_condition_always_stops() {
    let cpu = machine();
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, None).unwrap();
    assert!(breakpoints.stops_at(&Machine::new(&cpu), 0x0200, 0x0010));
}

#[test]
fn a_condition_that_cannot_be_read_is_refused_where_it_is_written() {
    let mut breakpoints = Breakpoints::new();
    assert!(breakpoints.add(0x0200, 0x0010, Some("AX ==")).is_err());
    assert!(breakpoints.breakpoints().is_empty());
}

#[test]
fn a_watchpoint_fires_on_a_write() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.watch(0x0100, 0x0004, Access::Write);

    let report = breakpoints.written(Some((0x0100, 0x0004, 1)));
    assert_eq!(report.hit, vec![0x0004]);
    assert!(report.missed.is_empty());
}

#[test]
fn a_watchpoint_on_a_write_does_not_fire_on_a_read_and_says_why() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.watch(0x0100, 0x0004, Access::Write);

    let report = breakpoints.read(0x0100, 0x0004);
    assert!(report.hit.is_empty(), "a read is not a write");
    assert_eq!(report.missed, vec![(0x0004, Missed::NotAWrite)]);
}

#[test]
fn a_word_stored_at_the_top_of_a_segment_does_not_overflow_the_watchpoint() {
    // The offset wraps inside the segment, exactly as it does on the chip:
    // a word at `FFFFh` writes the byte before it and the byte at the bottom.
    let mut breakpoints = Breakpoints::new();
    breakpoints.watch(0x0100, 0xFFFE, Access::Write);
    breakpoints.watch(0x0100, 0xFFFF, Access::Write);

    let report = breakpoints.written(Some((0x0100, 0xFFFE, 2)));
    assert_eq!(report.hit, vec![0xFFFE, 0xFFFF]);
}

#[test]
fn a_watchpoint_the_access_did_not_touch_says_so() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.watch(0x0100, 0x0004, Access::Write);
    let report = breakpoints.written(Some((0x0100, 0x0008, 1)));
    assert!(report.hit.is_empty());
    assert_eq!(report.missed, vec![(0x0004, Missed::NotTheAddress)]);
}

#[test]
fn the_memory_says_where_its_last_write_landed() {
    // A watchpoint asks the memory, because a write of the value that was
    // already there is still a write.
    let mut cpu = machine();
    cpu.mem.write_u8(0x0100, 0x0004, 0xAB);
    assert_eq!(cpu.mem.last_write, Some((0x0100, 0x0004, 1)));
    cpu.mem.write_u16(0x0100, 0x0006, 0x1234);
    assert_eq!(cpu.mem.last_write, Some((0x0100, 0x0006, 2)));
}

#[test]
fn the_panels_are_drawn_from_the_machine() {
    let cpu = machine();
    let ui = Ui {
        memory_at: 0x0000,
        memory_segment: 0x1000,
        memory_len: 4,
        disassembly_at: 0x0000,
        disassembly_len: 2,
        stack_len: 2,
    };
    let panels = Panels::of(&cpu, &ui);

    assert_eq!(panels.registers.ax, 0x1234);
    assert_eq!(panels.registers.cs, 0x0200);
    assert_eq!(panels.registers.ip, 0);
    assert_eq!(panels.disassembly.len(), 2);
    assert!(
        panels.disassembly[0].text.contains("MOV"),
        "the first instruction is the MOV: {}",
        panels.disassembly[0].text
    );
    assert_eq!(panels.disassembly[0].bytes, vec![0xB8, 0x34, 0x12]);
    assert_eq!(panels.memory.at, 0x0000);
    assert_eq!(panels.memory.bytes.len(), 4);
    assert_eq!(panels.stack.sp, 0x0100);
    assert_eq!(panels.stack.words.len(), 2);
}

#[test]
fn an_expression_reads_the_machine_it_is_given() {
    let cpu = machine();
    let value = expr::evaluate("AX", &Machine::new(&cpu)).unwrap();
    assert_eq!(value, 0x1234);
}
