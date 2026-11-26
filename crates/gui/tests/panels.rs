//! The panels: what each one shows, and that every one of them is there.
//!
//! The widgets' logic is a pure function of the panel model, so the tree's
//! tests read them without a display — the shape T1 established.

use emo8067_core::Cpu;
use emo8067_core::flags::CF;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::breakpoints::Breakpoints;
use emo8067_debug::console::Console;
use emo8067_debug::panels::{Edit, Panels, Register, Ui};
use emo8067_gui::app::App;
use emo8067_gui::ui::{
    condition_rows, console_view, flags_rows, memory_rows, registers_rows, source_rows, stack_rows,
};

/// A machine with a little code in it.
fn machine() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x02000, &[0xB8, 0x34, 0x12, 0xCD, 0x21, 0x90])
        .unwrap();
    cpu.regs.set_segment(Seg::Cs, 0x0200);
    cpu.regs.set_segment(Seg::Ds, 0x0200);
    cpu.regs.set_segment(Seg::Ss, 0x0200);
    cpu.regs.set_word(Reg16::Ax, 0x1234);
    cpu.regs.set_word(Reg16::Sp, 0x0100);
    cpu
}

/// The panels of that machine.
fn panels() -> Panels {
    Panels::of(&machine(), &Ui::default())
}

#[test]
fn the_registers_panel_shows_the_machine_s_registers() {
    let panels = panels();

    let rows = registers_rows(&panels, &Ui::default());

    assert_eq!(rows.len(), 13, "the panel shows the thirteen registers");
    assert_eq!(rows[0].name, "AX", "the first is AX");
    assert_eq!(rows[0].value, "1234", "and it holds what the machine holds");
    assert_eq!(
        rows[12].name, "IP",
        "and the last is the instruction pointer"
    );
}

#[test]
fn the_register_under_the_cursor_is_the_one_the_ui_says() {
    let panels = panels();
    let ui = Ui {
        register_cursor: 2,
        ..Ui::default()
    };

    let rows = registers_rows(&panels, &ui);

    assert!(rows[2].cursor, "the cursor is on the register it names");
    assert!(!rows[0].cursor, "and on no other");
}

#[test]
fn the_value_being_typed_over_a_register_is_shown_in_its_place() {
    let panels = panels();
    let ui = Ui {
        edit: Some(Edit::Register {
            register: Register::Word(Reg16::Ax),
            text: "BEEF".to_string(),
        }),
        ..Ui::default()
    };

    let rows = registers_rows(&panels, &ui);

    assert_eq!(rows[0].value, "BEEF", "the text is shown over the value");
    assert_eq!(
        rows[1].value, "0000",
        "and the other registers are as they were"
    );
}

#[test]
fn the_flags_panel_shows_the_nine_flags() {
    let mut cpu = machine();
    cpu.regs.flags.set(CF, true);
    let panels = Panels::of(&cpu, &Ui::default());

    let rows = flags_rows(&panels, &Ui::default());

    assert_eq!(rows.len(), 9, "the 8086 has nine flags");
    assert_eq!(rows[0].name, "CF", "the first is the carry flag");
    assert_eq!(rows[0].value, "1", "and it is set");
    assert_eq!(rows[1].value, "0", "and the parity flag is not");
}

#[test]
fn the_memory_window_shows_the_bytes_around_the_cursor() {
    let ui = Ui {
        memory_at: 0x0010,
        ..Ui::default()
    };
    let panels = Panels::of(&machine(), &ui);

    let rows = memory_rows(&panels, &ui, 8);

    assert_eq!(rows.len(), 32, "the window holds 256 bytes, eight to a row");
    assert_eq!(
        rows[0].bytes.join(" "),
        "B8 34 12 CD 21 90 00 00",
        "the bytes are the model's"
    );
    assert_eq!(
        rows[0].printable, ".4..!...",
        "and the printable side is beside them"
    );
    assert_eq!(rows[0].cursor, None, "the cursor is not in this row");
}

#[test]
fn the_byte_under_the_memory_cursor_is_the_one_the_ui_says() {
    let ui = Ui {
        memory_at: 0x0000,
        ..Ui::default()
    };
    let panels = Panels::of(&machine(), &ui);

    let rows = memory_rows(&panels, &ui, 8);

    assert_eq!(
        rows[0].cursor,
        Some(0),
        "the cursor's byte is the one the window says"
    );
}

#[test]
fn the_stack_panel_shows_the_words_on_the_stack() {
    let mut cpu = machine();
    cpu.mem.write_u16(0x0200, 0x0100, 0xAAAA);
    cpu.mem.write_u16(0x0200, 0x0102, 0xBBBB);
    let panels = Panels::of(&cpu, &Ui::default());

    let rows = stack_rows(&panels, &Ui::default(), &cpu);

    assert_eq!(rows.len(), 8, "the panel shows the words it was asked for");
    assert_eq!(rows[0].address, "0100", "the first word is at the pointer");
    assert_eq!(rows[0].value, "AAAA", "and it holds what the machine holds");
    assert!(rows[0].pointer, "and the pointer is marked");
    assert!(!rows[1].pointer, "and the other word is not");
}

#[test]
fn the_conditions_panel_shows_the_breakpoints_and_the_watchpoints() {
    let mut breakpoints = Breakpoints::new();
    breakpoints
        .add(0x0200, 0x0100, None)
        .expect("the breakpoint is added");
    breakpoints.watch(0x0200, 0x0200, emo8067_debug::Access::Write);

    let rows = condition_rows(&breakpoints.listing(), &Ui::default());

    assert_eq!(rows.len(), 2, "the panel shows the model's own listing");
    assert_eq!(rows[0].kind, "breakpoint", "the first is a breakpoint");
    assert_eq!(rows[0].address, "0200:0100", "at the address it was set");
    assert_eq!(rows[0].detail, "no condition", "and it carries none");
    assert_eq!(rows[1].kind, "watchpoint", "the second is a watchpoint");
    assert_eq!(rows[1].detail, "write", "and it says what it watches");
}

#[test]
fn the_console_panel_shows_the_log_and_the_command_line() {
    let mut console = Console::default();
    console.output.push("> r AX".to_string());
    console.output.push("AX 0000".to_string());
    console.text = "r BX".to_string();

    let view = console_view(&console);

    assert_eq!(view.log.len(), 2, "the log says what the commands said");
    assert_eq!(
        view.log[0], "> r AX",
        "the echo of the command is the first line"
    );
    assert_eq!(view.line, "r BX", "and the command line is what was typed");
}

#[test]
fn every_panel_is_in_the_window_without_a_keymap() {
    let mut app = App::new(None).expect("the machine is empty");
    app.breakpoints
        .add(0x0200, 0x0100, None)
        .expect("the breakpoint is added");
    app.console.text = "r".to_string();

    // Every panel's rows come from the app's own state: no panel of the
    // window is behind a key.
    assert_eq!(
        registers_rows(&app.panels, &app.ui).len(),
        13,
        "the registers are there"
    );
    assert_eq!(
        flags_rows(&app.panels, &app.ui).len(),
        9,
        "the flags are there"
    );
    assert!(
        !memory_rows(&app.panels, &app.ui, 8).is_empty(),
        "the memory window is there"
    );
    assert!(
        !stack_rows(&app.panels, &app.ui, &app.cpu).is_empty(),
        "the stack is there"
    );
    assert_eq!(
        condition_rows(&app.breakpoints.listing(), &app.ui).len(),
        1,
        "the conditions are there"
    );
    assert_eq!(console_view(&app.console).line, "r", "the console is there");
    assert_eq!(
        app.rows.len(),
        app.panels.disassembly.len(),
        "the disassembly is there"
    );
}

#[test]
fn the_source_strip_shows_the_program_s_own_lines() {
    let source = "org 100h\nmov ax, 1234h\nhlt\n";
    let program = emo8067_asm::program::assemble_program(source).expect("the program assembles");
    let mut cpu = machine();
    cpu.regs.ip = 0x0100;
    let view = emo8067_debug::source::SourceView::of(&program, &cpu, 3);

    let rows = source_rows(&view);

    assert_eq!(rows.len(), 2, "the strip shows the lines it was asked for");
    assert!(
        rows[0].text.contains("mov ax"),
        "the program's own text is there"
    );
    assert!(
        rows.iter().any(|row| row.current),
        "and the machine's line is marked"
    );
}
