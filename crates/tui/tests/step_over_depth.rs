//! The step over and the depth of the calls.
//!
//! The core keeps the depth of the calls, and the step over asks it how
//! deep the machine is: a procedure that calls another one goes one deep,
//! two, and back to one.

use emo8067_core::regs::{Reg16, Seg};
use emo8067_tui::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// An app with raw bytes in it.
fn app_with_bytes(code: &[u8]) -> App {
    let mut app = App::new(None).expect("the app");
    app.cpu.mem.load_at(0x0000, code).expect("the code loads");
    app.cpu.regs.set_segment(Seg::Cs, 0x0000);
    app.cpu.regs.set_segment(Seg::Es, 0x0000);
    app.cpu.regs.set_segment(Seg::Ds, 0x0000);
    app.cpu.regs.set_segment(Seg::Ss, 0x0000);
    app.cpu.regs.ip = 0x0000;
    app
}

/// The keys, one at a time.
fn press(app: &mut App, keys: &[KeyCode]) {
    for key in keys {
        app.key(KeyEvent::new(*key, KeyModifiers::NONE));
    }
}

/// The step over runs a procedure that calls another one.
#[test]
fn the_step_over_runs_a_procedure_that_calls_another() {
    // 0000 call 0004
    // 0003 hlt
    // 0004 f: call 0008
    // 0007 ret
    // 0008 g: mov ax, 1234h
    // 000B ret
    let mut app = app_with_bytes(&[
        0xE8, 0x01, 0x00, 0xF4, 0xE8, 0x01, 0x00, 0xC3, 0xB8, 0x34, 0x12, 0xC3,
    ]);
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0003,
        "the machine is where the call returns"
    );
    assert_eq!(
        app.cpu.regs.word(Reg16::Ax),
        0x1234,
        "and the inner call ran"
    );
    assert_eq!(app.cpu.call_depth, 0, "and the depth came back out");
}

/// The step over steps from an address a breakpoint sits on.
///
/// The place the user started from is not reached by the step over: the
/// single instruction key runs the instruction there, and so does F7.
#[test]
fn the_step_over_steps_from_an_address_with_a_breakpoint_on_it() {
    // 0000 mov ax, 1234h
    // 0003 mov bx, 5678h
    // 0006 hlt
    let mut app = app_with_bytes(&[0xB8, 0x34, 0x12, 0xBB, 0x78, 0x56, 0xF4]);
    app.breakpoints
        .add(0x0000, 0x0000, None)
        .expect("the breakpoint is set");
    press(&mut app, &[KeyCode::F(7)]);
    assert_ne!(
        app.cpu.regs.ip, 0x0000,
        "the step over advanced: it did not stop where it started"
    );
    assert_ne!(
        app.status.as_deref(),
        Some("the machine stopped at a breakpoint"),
        "and it did not answer the refusal"
    );
}
