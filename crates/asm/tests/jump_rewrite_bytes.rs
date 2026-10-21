//! A conditional jump whose target is too far is replaced the reference's way.
//!
//! S9 of `odd/tasks/assembler.md`: the `70h`-`7Fh` row becomes the opposite
//! opcode with a displacement of `+3`, then `E9h` and the near displacement -
//! five bytes; `JCXZ` becomes `OR CX,CX`, `JNZ` with a displacement of `+3`,
//! and `E9h` and the near displacement - seven bytes. The near displacement
//! must land exactly on the label the jump replaced.

use emo8067_asm::program::assemble_program;

/// A program whose conditional jump is out of the byte's reach.
fn program_with(jump: &str) -> emo8067_asm::program::Program {
    let mut source = format!("org 100h\n{jump}\n");
    for _ in 0..200 {
        source.push_str("nop\n");
    }
    source.push_str("far:\nret\n");
    assemble_program(&source).expect("the program assembles")
}

/// The address of a label the program declares.
fn address_of(program: &emo8067_asm::program::Program, name: &str) -> u16 {
    program
        .names
        .iter()
        .find(|(declared, _)| declared == name)
        .map(|(_, address)| *address)
        .expect("the label")
}

#[test]
fn a_conditional_jump_that_is_too_far_becomes_the_opposite_and_a_big_jump() {
    let program = program_with("jne far");
    // JNE is 75h, so the opposite is 74h, and the displacement is +3.
    assert_eq!(&program.bytes[0..2], &[0x74, 0x03]);
    // Then E9h and the near displacement.
    assert_eq!(program.bytes[2], 0xE9);
    let displacement = i16::from_le_bytes([program.bytes[3], program.bytes[4]]);
    // The E9 is at 102h and is three bytes long, so it lands at 105h + disp.
    let landed = 0x0105 + i32::from(displacement);
    assert_eq!(landed, i32::from(address_of(&program, "far")));
}

#[test]
fn a_jcxz_that_is_too_far_becomes_or_cx_cx_and_two_jumps() {
    let program = program_with("jcxz far");
    // OR CX,CX is 09h C9h, then JNZ with a displacement of +3.
    assert_eq!(&program.bytes[0..4], &[0x09, 0xC9, 0x75, 0x03]);
    // Then E9h and the near displacement.
    assert_eq!(program.bytes[4], 0xE9);
    let displacement = i16::from_le_bytes([program.bytes[5], program.bytes[6]]);
    // The E9 is at 104h and is three bytes long, so it lands at 107h + disp.
    let landed = 0x0107 + i32::from(displacement);
    assert_eq!(landed, i32::from(address_of(&program, "far")));
}

#[test]
fn a_conditional_jump_that_is_in_reach_stays_two_bytes() {
    let program =
        assemble_program("org 100h\njne far\nfar:\nret\n").expect("the program assembles");
    assert_eq!(&program.bytes[0..2], &[0x75, 0x00]);
    assert_eq!(program.bytes.len(), 3);
}
