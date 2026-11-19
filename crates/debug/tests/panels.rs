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
