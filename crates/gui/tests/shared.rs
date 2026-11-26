//! The state the two front ends share: the GUI's own copy of it, and the
//! console's commands.
//!
//! The panel model is the product (`odd/research/features-and-ux.md` §4.3)
//! and the two front ends are two renderers of it. The state they share
//! lives in the debug crate, and this is what the GUI's own copy of it
//! looks like.

use emo8067_core::Cpu;
use emo8067_core::regs::Reg16;
use emo8067_debug::console::{self, Command};
use emo8067_debug::panels::{Edit, Goto, Register};
use emo8067_gui::app::App;

#[test]
fn the_gui_holds_the_state_the_two_front_ends_share() {
    let mut app = App::new(None).expect("the machine is empty");

    app.ui.cursor = 3;
    app.ui.register_cursor = 2;
    app.ui.flag_cursor = 4;
    app.ui.condition_cursor = 1;
    app.ui.stack_scroll = 6;
    app.ui.goto_segment = Some(0x0200);
    app.ui.edit = Some(Edit::Register {
        register: Register::Word(Reg16::Ax),
        text: "1234".to_string(),
    });
    app.ui.goto = Some(Goto {
        text: "0100".to_string(),
    });

    assert_eq!(app.ui.cursor, 3, "the listing's cursor is the shared state");
    assert_eq!(app.ui.register_cursor, 2, "and so is the registers'");
    assert_eq!(app.ui.flag_cursor, 4, "and the flags'");
    assert_eq!(app.ui.condition_cursor, 1, "and the conditions'");
    assert_eq!(app.ui.stack_scroll, 6, "and the stack window's scroll");
    assert_eq!(
        app.ui.goto_segment,
        Some(0x0200),
        "and the segment a goto named"
    );
    assert!(
        app.ui.edit.is_some(),
        "and the edit prompt is the shared state"
    );
    assert!(app.ui.goto.is_some(), "and so is the goto prompt");
}

#[test]
fn the_gui_reads_the_console_s_commands() {
    let cpu = Cpu::new();

    assert_eq!(
        console::parse("r", &cpu),
        Ok(Command::Registers),
        "the console's words are the shared ones"
    );
    assert_eq!(
        console::parse("x", &cpu),
        Err(console::Refusal::UnknownCommand("x".to_string())),
        "and so are its refusals"
    );
}
