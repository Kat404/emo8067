//! The breakpoints, the watchpoints and the panels.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::breakpoints::{Access, Breakpoints, ConditionError, Kind, Missed};
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
fn the_listing_holds_every_breakpoint_and_every_watchpoint() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, Some("AX==0x1234")).unwrap();
    breakpoints.add(0x0200, 0x0020, None).unwrap();
    breakpoints.watch(0x0100, 0x0004, Access::Write);
    breakpoints.watch(0x0100, 0x0008, Access::Both);

    let listing = breakpoints.listing();

    assert_eq!(listing.len(), 4, "every breakpoint and every watchpoint");
    assert_eq!(listing[0].segment, 0x0200);
    assert_eq!(listing[0].offset, 0x0010);
    assert_eq!(listing[0].condition.as_deref(), Some("AX==0x1234"));
    assert_eq!(listing[1].segment, 0x0200);
    assert_eq!(listing[1].offset, 0x0020);
    assert_eq!(listing[2].segment, 0x0100);
    assert_eq!(listing[2].offset, 0x0004);
    assert_eq!(listing[3].segment, 0x0100);
    assert_eq!(listing[3].offset, 0x0008);
}

#[test]
fn the_listing_tells_a_breakpoint_from_a_watchpoint() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, None).unwrap();
    breakpoints.watch(0x0100, 0x0004, Access::Write);

    let listing = breakpoints.listing();

    assert_eq!(listing[0].kind, Kind::Breakpoint);
    assert_eq!(listing[1].kind, Kind::Watchpoint(Access::Write));
}

#[test]
fn a_breakpoint_without_a_condition_carries_none() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, None).unwrap();

    let listing = breakpoints.listing();

    assert_eq!(listing[0].condition, None, "no condition at all");
    assert!(
        listing[0].condition.as_deref() != Some(""),
        "and not an empty text either"
    );
}

#[test]
fn a_condition_can_be_set_and_taken_away() {
    let cpu = machine();
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, None).unwrap();

    breakpoints
        .set_condition(0x0200, 0x0010, Some("AX==0x1234"))
        .unwrap();
    assert_eq!(
        breakpoints.listing()[0].condition.as_deref(),
        Some("AX==0x1234"),
        "the listing shows the new condition"
    );
    assert!(
        breakpoints.stops_at(&Machine::new(&cpu), 0x0200, 0x0010),
        "and the breakpoint stops with it"
    );

    breakpoints.set_condition(0x0200, 0x0010, None).unwrap();
    assert!(
        breakpoints.listing()[0].condition.is_none(),
        "the condition is gone when it is taken away"
    );
}

#[test]
fn a_condition_that_cannot_be_read_is_refused_and_the_old_one_stays() {
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, Some("AX==0x1234")).unwrap();

    let error = breakpoints
        .set_condition(0x0200, 0x0010, Some("AX =="))
        .unwrap_err();

    assert_eq!(
        error,
        ConditionError::Unreadable(expr::Error::BadExpression),
        "the condition does not read"
    );
    assert_eq!(
        breakpoints.listing()[0].condition.as_deref(),
        Some("AX==0x1234"),
        "the breakpoint keeps the condition it had"
    );
}

#[test]
fn a_condition_cannot_be_written_where_there_is_no_breakpoint() {
    let mut breakpoints = Breakpoints::new();

    let error = breakpoints
        .set_condition(0x0200, 0x0010, Some("AX"))
        .unwrap_err();

    assert_eq!(error, ConditionError::NoBreakpoint);
    assert!(breakpoints.listing().is_empty());
}

#[test]
fn the_first_breakpoint_at_an_address_is_the_one_that_is_written() {
    // The door that writes a condition and the door that takes a
    // breakpoint away agree on which breakpoint at an address is the one:
    // the first.
    let mut breakpoints = Breakpoints::new();
    breakpoints.add(0x0200, 0x0010, Some("AX")).unwrap();
    breakpoints.add(0x0200, 0x0010, Some("BX")).unwrap();

    breakpoints
        .set_condition(0x0200, 0x0010, Some("CX"))
        .unwrap();
    let taken = breakpoints.remove(0x0200, 0x0010).unwrap();

    assert_eq!(taken.condition.as_deref(), Some("CX"));
    assert_eq!(
        breakpoints.listing()[0].condition.as_deref(),
        Some("BX"),
        "and the second one is the one that is left"
    );
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
    // a word at `FFFFh` writes `FFFFh` and `0000h`.
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
        stack_at: 0x0000,
        screen_segment: 0xB800,
        screen_page: 0,
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

#[test]
fn the_loader_starts_the_program_where_it_says() {
    // The reference's `.binf` model: the load segment and the load offset
    // have their own lines, apart from `CS` and `IP`, and each falls back to
    // the other when the other is not given. A program that declares its
    // state starts in that state.
    let program = emo8067_asm::program::assemble_program(
        "#cs=2000#\n#ip=0010#\n#ss=3000#\n#sp=0100#\n#ds=4000#\n#es=5000#\n#al=99#\n\
         org 0010h\nmov ax, 1\n",
    )
    .unwrap();
    let cpu = emo8067_debug::machine::load(&program).unwrap();

    assert_eq!(cpu.regs.segment(Seg::Cs), 0x2000);
    assert_eq!(cpu.regs.ip, 0x0010);
    assert_eq!(cpu.regs.segment(Seg::Ss), 0x3000);
    assert_eq!(cpu.regs.word(Reg16::Sp), 0x0100);
    assert_eq!(cpu.regs.segment(Seg::Ds), 0x4000);
    assert_eq!(cpu.regs.segment(Seg::Es), 0x5000);
    assert_eq!(cpu.regs.byte(emo8067_core::regs::Reg8::Al), 0x99);
    // And the image is where the load address says.
    assert_eq!(cpu.mem.read_u8(0x2000, 0x0010), 0xB8, "the MOV is there");
}

#[test]
fn a_program_without_a_state_starts_in_the_boot_templates_default() {
    let program = emo8067_asm::program::assemble_program("org 100h\nmov ax, 1\n").unwrap();
    let cpu = emo8067_debug::machine::load(&program).unwrap();
    assert_eq!(cpu.regs.segment(Seg::Cs), 0x0000);
    assert_eq!(cpu.regs.ip, 0x0100);
    assert_eq!(cpu.regs.word(Reg16::Sp), 0xFFFE);
}

#[test]
fn the_loader_honours_the_index_registers_too() {
    // The reference's `.binf` file lists them beside the byte registers:
    // "EF12 ; SI", "3456 ; DI", "7890 ; BP".
    let program = emo8067_asm::program::assemble_program(
        "#si=1234#\n#di=5678#\n#bp=9abc#\norg 0010h\nmov ax, 1\n",
    )
    .unwrap();
    let cpu = emo8067_debug::machine::load(&program).unwrap();
    assert_eq!(cpu.regs.word(Reg16::Si), 0x1234);
    assert_eq!(cpu.regs.word(Reg16::Di), 0x5678);
    assert_eq!(cpu.regs.word(Reg16::Bp), 0x9ABC);
}
