//! The window's listing: the disassembly, drawn from the panel model.
//!
//! The GUI is a second renderer of the same model
//! (`odd/research/features-and-ux.md` §4.3): the rows the window shows are
//! the model's own disassembly, and the GUI reads the machine nowhere else.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::panels::{Panels, Ui};
use emo8067_gui::ui::{Here, listing};

/// A machine with a little code in it.
fn machine() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x02000, &[0xB8, 0x34, 0x12, 0xCD, 0x21, 0x90])
        .unwrap();
    cpu.regs.set_segment(Seg::Cs, 0x0200);
    cpu.regs.set_word(Reg16::Ax, 0x1234);
    cpu
}

/// Where the machine is.
fn here(cpu: &Cpu) -> Here {
    Here {
        segment: cpu.regs.segment(Seg::Cs),
        offset: cpu.regs.ip,
    }
}

#[test]
fn the_listing_is_the_panels_own_disassembly() {
    let mut cpu = machine();
    cpu.regs.ip = 0x0003;
    let panels = Panels::of(&cpu, &Ui::default());
    let rows = listing(&panels, here(&cpu));

    assert_eq!(
        rows.len(),
        panels.disassembly.len(),
        "one row per instruction the model found"
    );
    assert_eq!(
        rows[0].address, "0200:0000",
        "the address is the model's own"
    );
    assert_eq!(rows[0].bytes, "B8 34 12", "the bytes are the model's own");
    assert_eq!(
        rows[0].text, "MOV AX, 01234h",
        "the text is the model's own"
    );
    assert!(!rows[0].here, "the machine is not at this one");
    assert_eq!(rows[1].address, "0200:0003", "the next one follows");
    assert_eq!(rows[1].text, "INT 021h", "and its text is the model's own");
    assert!(rows[1].here, "the machine is at this one");
}

#[test]
fn an_empty_listing_marks_nothing() {
    let cpu = machine();
    let mut panels = Panels::of(&cpu, &Ui::default());
    panels.disassembly.clear();

    let rows = listing(&panels, here(&cpu));

    assert!(rows.is_empty(), "there is nothing to mark");
}

#[test]
fn a_listing_that_does_not_reach_the_machine_marks_nothing() {
    let cpu = machine();
    // The listing starts past the machine, so its rows are not its own.
    let ui = Ui {
        disassembly_at: cpu.regs.ip + 4,
        ..Ui::default()
    };
    let panels = Panels::of(&cpu, &ui);

    let rows = listing(&panels, here(&cpu));

    assert!(!rows.is_empty(), "the listing has rows of its own");
    assert!(
        !rows.iter().any(|row| row.here),
        "and none of them is the machine's own instruction"
    );
}
