//! A program, assembled.

use emo8067_asm::program::assemble_program;

#[test]
fn a_program_knows_where_it_starts_and_what_it_holds() {
    let program = assemble_program("org 100h\nmov ax, 3\ndb 'H', 0\n").unwrap();
    assert_eq!(program.origin, 0x100);
    assert_eq!(program.bytes, vec![0xB8, 0x03, 0x00, b'H', 0x00]);
}

#[test]
fn a_word_is_two_bytes_little_endian() {
    let program = assemble_program("dw 1234h\ndd 1\n").unwrap();
    assert_eq!(program.bytes, vec![0x34, 0x12, 0x01, 0x00, 0x00, 0x00]);
}

#[test]
fn a_label_is_remembered_where_it_is() {
    let program = assemble_program("org 100h\nmov ax, 1\nhere:\nnop\n").unwrap();
    assert_eq!(program.names, vec![("here".to_string(), 0x103)]);
}

#[test]
fn a_constant_is_folded_into_the_operand_that_names_it() {
    let program = assemble_program("SIZE EQU 4\nmov cx, SIZE\n").unwrap();
    assert_eq!(program.bytes, vec![0xB9, 0x04, 0x00]);
}

#[test]
fn the_directives_that_say_nothing_are_passed_over() {
    let program = assemble_program(
        "name \"a-program\"\nsegment code\nassume cs:code\nstart:\nmov ax, 0\nend\n",
    )
    .unwrap();
    assert_eq!(program.bytes, vec![0xB8, 0x00, 0x00]);
    assert_eq!(program.names, vec![("start".to_string(), 0)]);
}

#[test]
fn a_jump_knows_where_it_is_going() {
    // The program is walked twice: the first time the names are collected
    // and the addresses counted, the second time the bytes are made.
    let program = assemble_program("org 100h\nmov ax, 1\nhere:\njmp here\n").unwrap();
    assert_eq!(program.bytes, vec![0xB8, 0x01, 0x00, 0xE9, 0xFD, 0xFF]);
}

#[test]
fn a_conditional_jump_is_the_short_form() {
    let program = assemble_program("org 0\nje there\nnop\nthere:\nnop\n").unwrap();
    // The jump is at 0 and ends at 2; the label is after the nop, at 3, so
    // the displacement is one. The canonical opcode is 74h: the guide's row
    // 60 is the chip's alias of the 70h row, and the assembler writes the
    // canonical one.
    assert_eq!(program.bytes, vec![0x74, 0x01, 0x90, 0x90]);
}
