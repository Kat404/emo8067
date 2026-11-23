//! The panel model: the registers and the flags it shows.
//!
//! The model is the product (`features-and-ux.md` §4.3), so what the TUI
//! draws is here: the registers in the order the panel shows them, and the
//! nine flags by name and by value.

use emo8067_core::Cpu;
use emo8067_core::flags::CF;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::panels::{Flag, Panels, Register, Ui};

/// A machine with a little code in it.
fn machine() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x02000, &[0xB8, 0x34, 0x12, 0xCD, 0x21, 0x90])
        .unwrap();
    cpu.regs.set_segment(Seg::Cs, 0x0200);
    cpu.regs.set_word(Reg16::Ax, 0x1234);
    cpu.regs.set_word(Reg16::Si, 0x0050);
    cpu
}

#[test]
fn the_registers_come_out_of_the_model_in_the_order_the_panel_shows_them() {
    let panels = Panels::of(&machine(), &Ui::default());
    let cells = panels.registers.cells();

    let names: Vec<&str> = cells.iter().map(|cell| cell.register.name()).collect();
    assert_eq!(
        names,
        [
            "AX", "BX", "CX", "DX", "SI", "DI", "BP", "SP", "CS", "DS", "ES", "SS", "IP"
        ],
        "the panel's own order"
    );
    assert_eq!(cells.len(), Register::ALL.len(), "and no more than that");
    assert_eq!(cells[0].value, 0x1234, "AX holds what the machine holds");
    assert_eq!(cells[4].value, 0x0050, "SI too");
    assert_eq!(cells[12].value, 0, "IP starts where it was");
}

#[test]
fn the_flags_are_read_out_of_the_flags_word() {
    let mut cpu = machine();
    cpu.regs.flags.set(CF, true);
    let panels = Panels::of(&cpu, &Ui::default());

    assert!(panels.registers.flag(Flag::Cf), "CF was set");
    assert!(!panels.registers.flag(Flag::Zf), "ZF was not");
    assert!(panels.registers.flag(Flag::If), "interrupts start enabled");
    assert_eq!(Flag::ALL.len(), 9, "the 8086 has nine flags");
    assert_eq!(Flag::Cf.name(), "CF");
    assert_eq!(Flag::If.name(), "IF");
}

#[test]
fn a_register_can_be_written() {
    let mut cpu = machine();

    Register::Word(Reg16::Ax).write(&mut cpu, 0xBEEF);
    assert_eq!(cpu.regs.word(Reg16::Ax), 0xBEEF);

    Register::Segment(Seg::Ds).write(&mut cpu, 0x1234);
    assert_eq!(cpu.regs.segment(Seg::Ds), 0x1234);

    Register::Ip.write(&mut cpu, 0x0100);
    assert_eq!(cpu.regs.ip, 0x0100);
}

#[test]
fn the_stack_window_is_where_the_ui_says_and_not_where_the_pointer_is() {
    let mut cpu = Cpu::new();
    cpu.regs.set_segment(Seg::Ss, 0x0200);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu.mem.write_u16(0x0200, 0x0100, 0xAAAA);
    cpu.mem.write_u16(0x0200, 0x0080, 0xBBBB);
    let ui = Ui {
        stack_at: 0x0080,
        stack_len: 4,
        ..Ui::default()
    };

    let panels = Panels::of(&cpu, &ui);
    let stack = &panels.stack;

    assert_eq!(
        stack.words[0], 0xBBBB,
        "the window starts where the UI says and not at the pointer"
    );
    assert_eq!(stack.words[1], 0x0000, "and the next word is above it");
    assert_eq!(stack.sp, 0x0100, "the pointer is still reported");
    assert_eq!(stack.segment, 0x0200, "and the segment it is in");
}

#[test]
fn the_stack_window_wraps_inside_the_segment() {
    let mut cpu = Cpu::new();
    cpu.regs.set_segment(Seg::Ss, 0x0200);
    cpu.mem.write_u16(0x0200, 0xFFFF, 0x1111);
    cpu.mem.write_u16(0x0200, 0x0001, 0x2222);
    let ui = Ui {
        stack_at: 0xFFFF,
        stack_len: 2,
        ..Ui::default()
    };

    let stack = &Panels::of(&cpu, &ui).stack;

    assert_eq!(stack.words[0], 0x1111, "the word at the window's start");
    assert_eq!(
        stack.words[1], 0x2222,
        "and the offset wrapped inside the segment"
    );
}

#[test]
fn the_memory_panel_is_a_window_around_the_cursor() {
    let mut cpu = Cpu::new();
    cpu.mem.load_at(0x02010, &[0xAB]).unwrap();
    let ui = Ui {
        memory_at: 0x0010,
        memory_segment: 0x0200,
        memory_len: 16,
        ..Ui::default()
    };

    let panels = Panels::of(&cpu, &ui);
    let memory = &panels.memory;

    assert_eq!(memory.at, 0x0010, "the cursor is at the address");
    assert_eq!(memory.window_at, 0x0008, "the window starts before it");
    assert_eq!(memory.cursor, 8, "and the cursor is in the window");
    assert_eq!(memory.bytes.len(), 16, "the window holds what it says");
    assert_eq!(
        memory.bytes[8], 0xAB,
        "and the byte under the cursor is there"
    );
}
