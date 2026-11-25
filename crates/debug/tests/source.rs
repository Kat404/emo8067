//! The source panel: the program's own lines, and the one the machine is
//! stopped at.
//!
//! The panel is a pure function of the program and of the machine, the way
//! every other panel is a pure function of the machine and of the UI's
//! state.

use emo8067_asm::program::{Program, assemble_program};
use emo8067_core::Cpu;
use emo8067_debug::source::SourceView;

/// A program with a few instructions in it.
///
/// The origin is 100h, and every `mov` is three bytes and every `nop` is
/// one, so the lines land at 100h, 103h, 104h, 107h, 108h, 10Bh and 10Ch.
fn program() -> Program {
    assemble_program(
        "org 100h\n\
         mov ax, 1\n\
         nop\n\
         mov bx, 2\n\
         nop\n\
         mov cx, 3\n\
         nop\n\
         mov dx, 4\n",
    )
    .unwrap()
}

/// A machine stopped at an address.
fn stopped_at(address: u16) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.regs.ip = address;
    cpu
}

#[test]
fn the_view_finds_the_line_the_machine_is_stopped_at() {
    let program = program();
    // The machine is at 108h, which is the `mov cx, 3` on the sixth line of
    // the source.
    let view = SourceView::of(&program, &stopped_at(0x108), 3);

    assert_eq!(view.current, Some(1), "the current line is in the middle");
    assert_eq!(view.lines.len(), 3, "the strip shows three lines");
    assert_eq!(view.lines[0].number, 5, "the line before it");
    assert_eq!(view.lines[1].number, 6, "the line it is stopped at");
    assert_eq!(view.lines[2].number, 7, "and the line after it");
    assert_eq!(
        view.lines[1].text, "mov cx, 3",
        "with the text the source wrote"
    );
}

#[test]
fn the_strip_follows_the_machine_down_the_program() {
    let program = program();
    // The machine is at the last instruction, at 10Ch.
    let view = SourceView::of(&program, &stopped_at(0x10C), 3);

    assert_eq!(
        view.current,
        Some(2),
        "the last line is at the end of the strip"
    );
    assert_eq!(view.lines.len(), 3, "the strip is still three lines long");
    assert_eq!(
        view.lines
            .iter()
            .map(|line| line.number)
            .collect::<Vec<_>>(),
        vec![6, 7, 8],
        "the strip ends with the line the machine is at"
    );
}

#[test]
fn a_machine_that_is_not_in_any_line_shows_the_first_lines() {
    let program = program();
    // The machine is at 200h, which is nowhere in the program.
    let view = SourceView::of(&program, &stopped_at(0x200), 3);

    assert_eq!(view.current, None, "there is no line to be stopped at");
    assert_eq!(
        view.lines
            .iter()
            .map(|line| line.number)
            .collect::<Vec<_>>(),
        vec![2, 3, 4],
        "the strip shows the first lines of the program"
    );
}

#[test]
fn a_program_with_no_source_has_no_strip() {
    let program = assemble_program("org 100h\n").unwrap();
    let view = SourceView::of(&program, &stopped_at(0x100), 3);

    assert!(view.lines.is_empty(), "there is no source to show");
    assert_eq!(view.current, None);
}
