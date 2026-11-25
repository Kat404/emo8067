//! The step back and the list of what has run.
//!
//! The list holds the addresses the machine has been at, and the step
//! back takes the address of the instruction it undid off the list —
//! the address of the instruction that ran, and not the address the
//! machine was at when the step back was pressed.

use emo8067_core::regs::Seg;
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

/// The step back takes the address of the instruction that ran.
#[test]
fn the_step_back_takes_the_address_that_ran_off_the_list() {
    // 90         nop
    // 90         nop
    let mut app = app_with_bytes(&[0x90, 0x90]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.executed, vec![0x0000], "the first instruction ran");
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.executed, vec![0x0000, 0x0001], "and the second one ran");
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.executed,
        vec![0x0000],
        "and the step back takes the address that ran off the list"
    );
}

/// The step back after a step that failed takes the failed address.
///
/// The instruction was attempted, and the journal holds a record for it,
/// so its address is the one that goes off the list.
#[test]
fn the_step_back_after_a_failing_step_takes_the_failed_address() {
    // 90         nop
    // C4 C0      les ax, ax -- an encoding the 8086 does not have
    let mut app = app_with_bytes(&[0x90, 0xC4, 0xC0]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.executed, vec![0x0000], "the nop ran");
    press(&mut app, &[KeyCode::F(8)]);
    assert!(
        app.status
            .as_deref()
            .is_some_and(|status| status.starts_with("the machine stopped")),
        "the bad encoding stopped the machine: {:?}",
        app.status
    );
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.executed,
        vec![0x0000],
        "and the nop's mark is still there: the step back undid the \
         instruction that failed"
    );
}
