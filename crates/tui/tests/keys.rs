//! The keys that come with the core: the watchpoint, the step back and the
//! step over.
//!
//! The TUI is driven through its own keys and the machine is read back
//! through the core's own doors.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use emo8067_core::regs::{Reg16, Seg};
use emo8067_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The counter that tells the directories of one run apart.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory of this run's own, and the guard that takes it away again.
///
/// The name carries the process id and a counter, so two runs never share
/// one, and the directory is made exclusively: another local user cannot
/// have made it before us, and the program inside it cannot be a symlink
/// somebody else left behind.
struct Directory {
    path: PathBuf,
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A directory of this run's own, made exclusively.
fn directory(name: &str) -> Directory {
    let path = std::env::temp_dir().join(format!(
        "emo8067-tui-keys-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&path).expect("the directory");
    Directory { path }
}

/// The app, with that program of the test's own words open.
///
/// The program is read while it is there, and the directory goes when the
/// app is built: nothing reads it again.
fn app_of(name: &str, source: &str) -> App {
    let dir = directory(name);
    let path = dir.path.join("program.asm");
    std::fs::write(&path, source).expect("the program");
    App::new(Some(path.to_string_lossy().to_string())).expect("the program loads")
}

/// Two runs of the same test do not share a directory.
#[test]
fn two_runs_of_the_same_test_do_not_collide() {
    let first = directory("collide");
    let second = directory("collide");
    assert_ne!(
        first.path, second.path,
        "the two runs have a directory of their own: {:?}",
        first.path
    );
    assert!(
        first.path.starts_with(std::env::temp_dir()),
        "and it is under the system's temporary directory: {:?}",
        first.path
    );
}

/// The keys, one at a time.
fn press(app: &mut App, keys: &[KeyCode]) {
    for key in keys {
        app.key(KeyEvent::new(*key, KeyModifiers::NONE));
    }
}

/// The screen the app draws, one string per row.
fn rows(app: &App, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(app, frame))
        .expect("the drawing");
    let buffer = terminal.backend().buffer().clone();
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer.cell((x, y)).map_or("", |cell| cell.symbol()))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// A program that writes a word to 0300h and then runs on.
const STORES: &str = "org 100h\n\
                      mov ax, 1234h\n\
                      mov [0300h], ax\n\
                      mov bx, 5678h\n\
                      hlt\n";

/// The watchpoint fires at the write, and the machine stops there.
#[test]
fn the_watchpoint_fires_at_the_write() {
    let mut app = app_of("watchpoint-fires", STORES);
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0300"),
        "the status names the address that was written"
    );
    // The store is at 0103h and it is four bytes long: the machine stops
    // at 0107h, the instruction after the write, and not later.
    assert_eq!(app.cpu.regs.ip, 0x0107, "and it stopped there");
    assert!(!app.finished, "and the machine did not run on");
    assert_eq!(
        app.cpu.mem.read_u8(0x0000, 0x0300),
        0x34,
        "and the byte was written"
    );
}

/// A write elsewhere does not fire the watchpoint.
#[test]
fn a_write_elsewhere_does_not_fire_the_watchpoint() {
    let mut app = app_of("watchpoint-miss", STORES);
    app.ui.memory_at = 0x0400;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(app.status, None, "the watchpoint did not fire");
    assert!(app.finished, "and the machine ran on to the halt");
}

/// A word write fires the watchpoint on either of its halves.
#[test]
fn a_word_write_fires_the_watchpoint_on_either_half() {
    let mut app = app_of("watchpoint-word", STORES);
    app.ui.memory_at = 0x0301;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0300"),
        "the status names the address of the word that was written"
    );
}

/// The watchpoint comes off with `w` again, and then it does not fire.
#[test]
fn the_watchpoint_comes_off_with_w_again() {
    let mut app = app_of("watchpoint-off", STORES);
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    assert_eq!(app.breakpoints.watchpoints().len(), 1, "it is set");
    press(&mut app, &[KeyCode::Char('w')]);
    assert!(
        app.breakpoints.watchpoints().is_empty(),
        "and w again takes it away"
    );
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(app.status, None, "and it does not fire");
    assert!(app.finished, "and the machine ran on");
}

/// The conditions pane draws the watchpoint: it is the model's own.
#[test]
fn the_conditions_pane_draws_the_watchpoint() {
    let mut app = app_of("watchpoint-pane", STORES);
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::Char('b')]);
    let text = rows(&app, 80, 24).join("\n");
    assert!(
        text.contains("0300"),
        "the conditions pane shows the watchpoint: {text}"
    );
    assert!(text.contains("watch"), "and it says what it is: {text}");
}

/// The step back puts the bytes the instruction wrote back.
#[test]
fn the_step_back_puts_the_bytes_back() {
    let mut app = app_of("step-back-bytes", STORES);
    press(&mut app, &[KeyCode::F(8), KeyCode::F(8)]);
    assert_eq!(
        app.cpu.mem.read_u8(0x0000, 0x0300),
        0x34,
        "the word was written"
    );
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.cpu.mem.read_u8(0x0000, 0x0300),
        0x00,
        "and the step back puts the byte back"
    );
    assert_eq!(app.cpu.regs.ip, 0x0103, "and IP goes back");
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0x1234, "and the registers");
}

/// A step back with nothing to rewind is refused, and nothing moves.
#[test]
fn a_step_back_with_nothing_to_rewind_is_refused() {
    let mut app = app_of("step-back-empty", STORES);
    let ip = app.cpu.regs.ip;
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.status.as_deref(),
        Some("There are no recorded back steps ...."),
        "the refusal is the reference's own words"
    );
    assert_eq!(app.cpu.regs.ip, ip, "and nothing moved");
    assert_eq!(app.steps, 0, "and the counter did not move");
    assert!(app.executed.is_empty(), "and nothing ran");
}

/// The step back over a call puts the depth of the calls back.
#[test]
fn the_step_back_over_a_call_puts_the_depth_back() {
    let mut app = app_of(
        "step-back-call",
        "org 100h\ncall f\nhlt\nf:\nmov ax, 1234h\nret\n",
    );
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.cpu.call_depth, 1, "the call went in");
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(app.cpu.call_depth, 0, "and the depth goes back");
    assert_eq!(app.cpu.regs.ip, 0x0100, "and IP goes back");
}

/// The step back over a halt leaves a machine that can still be stepped.
#[test]
fn the_step_back_over_a_halt_leaves_a_machine_that_can_be_stepped() {
    let mut app = app_of("step-back-halt", "org 100h\nmov ax, 1234h\nhlt\n");
    press(&mut app, &[KeyCode::F(8), KeyCode::F(8)]);
    assert!(app.finished, "the machine halted");
    press(&mut app, &[KeyCode::F(6)]);
    assert!(
        !app.finished,
        "the step back takes the machine out of the halt"
    );
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0104,
        // The probe said 0102h: `mov ax, 1234h` is three bytes, so the
        // halt is at 0103h and the machine ends at 0104h.
        "and the machine can be stepped again"
    );
}

/// The step over runs a near call to its end, and stops where it returns.
#[test]
fn the_step_over_runs_a_near_call_to_its_end() {
    let mut app = app_of(
        "step-over-near",
        "org 100h\ncall f\nhlt\nf:\nmov ax, 1234h\nret\n",
    );
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0103,
        "the machine is where the call returns"
    );
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0x1234, "and the call ran");
    assert_eq!(app.cpu.call_depth, 0, "and the depth came back out");
}

/// The step over runs a far call to its end.
#[test]
fn the_step_over_runs_a_far_call_to_its_end() {
    let mut app = app_of("step-over-far", "org 100h\ncall 0000:0200h\nhlt\n");
    // The far callee, with a far return in it.
    app.cpu.mem.write_u8(0x0000, 0x0200, 0xCB);
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0105,
        "the machine is where the call returns"
    );
    assert_eq!(app.cpu.call_depth, 0, "and the depth came back out");
}

/// The step over runs a call through a register to its end.
#[test]
fn the_step_over_runs_an_indirect_call_to_its_end() {
    let mut app = app_of(
        "step-over-indirect",
        "org 100h\nmov bx, 0200h\ncall bx\nhlt\n",
    );
    app.cpu.mem.write_u8(0x0000, 0x0200, 0xB8);
    app.cpu.mem.write_u8(0x0000, 0x0201, 0x34);
    app.cpu.mem.write_u8(0x0000, 0x0202, 0x12);
    app.cpu.mem.write_u8(0x0000, 0x0203, 0xC3);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.cpu.regs.ip, 0x0103, "the machine is at the call");
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0105,
        "the machine is where the call returns"
    );
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0x1234, "and the call ran");
}

/// The step over of an instruction that is not a call is one step.
#[test]
fn the_step_over_of_an_instruction_that_is_not_a_call_is_one_step() {
    let mut app = app_of("step-over-plain", STORES);
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0103,
        "the machine took one step, like F8 does"
    );
    assert_eq!(
        app.cpu.regs.word(Reg16::Ax),
        0x1234,
        "and the register moved"
    );
}

/// The step over of an interrupt runs the handler and stops where it
/// returns.
#[test]
fn the_step_over_of_an_interrupt_runs_the_handler() {
    let mut app = app_of("step-over-int", "org 100h\nint 10h\nmov ax, 1234h\nhlt\n");
    // The vector of 10h, and the handler that ends with an iret.
    app.cpu.mem.write_u8(0x0000, 0x0040, 0x00);
    app.cpu.mem.write_u8(0x0000, 0x0041, 0x02);
    app.cpu.mem.write_u8(0x0000, 0x0042, 0x00);
    app.cpu.mem.write_u8(0x0000, 0x0043, 0x00);
    app.cpu.mem.write_u8(0x0000, 0x0200, 0xCF);
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0102,
        "the machine is where the interrupt returns"
    );
    assert_eq!(app.cpu.call_depth, 0, "and the depth came back out");
}

/// A step over that runs out of its budget says so.
#[test]
fn a_step_over_that_runs_out_of_its_budget_says_so() {
    let mut app = app_of("step-over-budget", "org 100h\ncall f\nhlt\nf:\njmp f\n");
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the budget of 100000 instructions ran out"),
        "the budget is reported the way F5 reports it"
    );
}

/// The console's `p` steps over, the way F7 does.
#[test]
fn the_console_p_steps_over() {
    let mut app = app_of(
        "console-p",
        "org 100h\ncall f\nhlt\nf:\nmov ax, 1234h\nret\n",
    );
    press(&mut app, &[KeyCode::Char(':')]);
    for character in "p".chars() {
        press(&mut app, &[KeyCode::Char(character)]);
    }
    press(&mut app, &[KeyCode::Enter]);
    assert_eq!(
        app.cpu.regs.ip, 0x0103,
        "the console's p steps over, like F7 does"
    );
    assert_eq!(app.cpu.regs.word(Reg16::Ax), 0x1234, "and the call ran");
    let said = app.console.output.join("\n");
    assert!(
        !said.contains("call tracking"),
        "and the old refusal is gone: {said}"
    );
}

/// The help lists every key at eighty columns, and none of them is cut.
///
/// The list is the screen's own: the test reads the production list, so
/// a key that is added and not shown fails the test.
#[test]
fn the_help_lists_every_key_at_eighty_columns() {
    let mut app = app_of("help-keys", STORES);
    press(&mut app, &[KeyCode::Char('?')]);
    let text = rows(&app, 80, 24).join("\n");
    for entry in emo8067_tui::ui::KEYS {
        assert!(text.contains(entry), "the help lists {entry}: {text}");
    }
    // No line of the help is longer than the screen.
    for line in rows(&app, 80, 24) {
        assert!(line.chars().count() <= 80, "the line fits: {line:?}");
    }
}

/// The help falls back to one column when the width cannot hold two.
///
/// Below seventy-eight columns the two columns do not fit, and a key that
/// is cut is a key that is lost: the keys go down one column instead.
#[test]
fn the_help_falls_back_to_one_column_at_a_narrow_width() {
    let mut app = app_of("help-narrow", STORES);
    press(&mut app, &[KeyCode::Char('?')]);
    let rows = rows(&app, 60, 24);
    let row = rows
        .iter()
        .find(|row| row.contains("F5   run to the next breakpoint"))
        .expect("the first key is on the screen");
    assert!(
        !row.contains(":    the console"),
        "the keys are in one column, not two: {row:?}"
    );
    assert!(
        row.ends_with("F5   run to the next breakpoint"),
        "and the key is whole: {row:?}"
    );
}

/// The help keeps every key it had before the three new ones.
#[test]
fn the_help_keeps_the_keys_it_had() {
    let mut app = app_of("help-old-keys", STORES);
    press(&mut app, &[KeyCode::Char('?')]);
    let text = rows(&app, 80, 24).join("\n");
    for key in [
        "F5", "F8", "F9", "Tab", "1 ", "2 ", "3 ", "4 ", "5 ", "b ", ": ", "e ", "g ", "PgUp",
        "Enter", "Esc", "?", "q ",
    ] {
        assert!(text.contains(key), "the help still lists {key}: {text}");
    }
    assert!(
        text.contains("the stack (5 again closes it)"),
        "the stack's own words: {text}"
    );
}

/// An instruction that writes nothing does not fire the watchpoint.
///
/// The core clears the last write at the start of every instruction, so
/// the write of the step before is not this step's: an instruction that
/// writes nothing at all must not fire the watchpoint.
#[test]
fn an_instruction_that_writes_nothing_does_not_fire_the_watchpoint() {
    let mut app = app_of("watchpoint-silent", "org 100h\nmov [0300h], ax\nnop\nhlt\n");
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0300"),
        "the store fired the watchpoint"
    );
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(
        app.status, None,
        "and the instruction that wrote nothing did not fire it"
    );
}

/// The step over stops at a breakpoint, the way the run does.
///
/// A breakpoint inside the procedure being stepped over is not a thing
/// the step over may run past.
#[test]
fn the_step_over_stops_at_a_breakpoint() {
    let mut app = app_of(
        "step-over-breakpoint",
        "org 100h\ncall f\nhlt\nf:\nmov ax, 1234h\nret\n",
    );
    // The breakpoint is inside the procedure the step over runs.
    app.breakpoints
        .add(0x0000, 0x0104, None)
        .expect("the breakpoint is set");
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0104,
        "the machine stopped at the breakpoint"
    );
    assert_eq!(
        app.status.as_deref(),
        Some("the machine stopped at a breakpoint"),
        "and it says so"
    );
}

/// An app with raw bytes in it: the assembler does not know the string
/// operations, and the bulk writes are what the journal's bound is about.
fn app_with_bytes(code: &[u8]) -> App {
    let mut app = App::new(None).expect("the app");
    app.cpu.mem.load_at(0x0000, code).expect("the code loads");
    app.cpu.regs.set_segment(Seg::Cs, 0x0000);
    app.cpu.regs.set_segment(Seg::Es, 0x0000);
    app.cpu.regs.ip = 0x0000;
    app
}

/// The step back refuses a step that was too big to undo.
#[test]
fn the_step_back_after_a_too_big_step_says_so() {
    // F3 AB   rep stosw, with 65535 words: 131070 bytes.
    let mut app = app_with_bytes(&[0xF3, 0xAB, 0x90]);
    app.cpu.regs.set_word(Reg16::Cx, 0xFFFF);
    app.cpu.regs.set_word(Reg16::Di, 0x0300);
    press(&mut app, &[KeyCode::F(8)]);
    assert!(app.cpu.history.is_broken(), "the journal is broken");
    press(&mut app, &[KeyCode::F(6)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the step was too big to undo"),
        "the refusal says it"
    );
}

/// An instruction that writes nothing fires no watchpoint.
#[test]
fn an_instruction_that_writes_nothing_leaves_the_watchpoint_silent() {
    let mut app = app_of(
        "watchpoint-silent",
        "org 100h\nmov ax, 1234h\nmov [0300h], ax\nnop\nhlt\n",
    );
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.status, None, "the mov wrote nothing");
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0300"),
        "the store fired it"
    );
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(app.status, None, "and the nop did not fire it again");
}

/// The watchpoint works with the journal off.
#[test]
fn the_watchpoint_fires_with_the_journal_off() {
    let mut app = app_of("watchpoint-no-journal", "org 100h\nmov [0300h], ax\nhlt\n");
    app.cpu.disable_history();
    app.ui.memory_at = 0x0300;
    press(&mut app, &[KeyCode::Char('w')]);
    press(&mut app, &[KeyCode::F(8)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the machine wrote 0000:0300"),
        "the watchpoint does not need the journal"
    );
}

/// The step over stops at a breakpoint inside the procedure.
#[test]
fn the_step_over_stops_at_a_breakpoint_inside_the_procedure() {
    //0100 call f / 0103 hlt / 0104 f: nop / 0105 mov ax,1234h / 0108 ret
    let mut app = app_of(
        "step-over-breakpoint",
        "org 100h\ncall f\nhlt\nf:\nnop\nmov ax, 1234h\nret\n",
    );
    app.breakpoints
        .add(0x0000, 0x0105, None)
        .expect("the breakpoint is set");
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.cpu.regs.ip, 0x0105,
        "the machine stopped at the breakpoint"
    );
    assert_eq!(
        app.status.as_deref(),
        Some("the machine stopped at a breakpoint"),
        "and it says so"
    );
}

/// The run and the step over say the budget the same way.
#[test]
fn the_run_and_the_step_over_say_the_budget_the_same_way() {
    let mut app = app_of("run-budget", "org 100h\njmp $\n");
    press(&mut app, &[KeyCode::F(5)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the budget of 100000 instructions ran out"),
        "the run says it"
    );
    let mut app = app_of("over-budget", "org 100h\ncall f\nhlt\nf:\njmp f\n");
    press(&mut app, &[KeyCode::F(7)]);
    assert_eq!(
        app.status.as_deref(),
        Some("the budget of 100000 instructions ran out"),
        "and the step over says it the same way"
    );
}

/// The help keeps every key whole, at every width.
#[test]
fn the_help_keeps_every_key_whole_at_every_width() {
    for width in [40u16, 50, 60, 70, 77, 78, 80, 100, 140] {
        let mut app = app_of("help-widths", "org 100h\nhlt\n");
        press(&mut app, &[KeyCode::Char('?')]);
        let text = rows(&app, width, 40).join("\n");
        for key in emo8067_tui::ui::KEYS {
            assert!(
                text.contains(key),
                "at {width} columns the help loses {key:?}"
            );
        }
    }
}
