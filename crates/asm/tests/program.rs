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

#[test]
fn an_offset_can_be_part_of_an_expression() {
    let program = assemble_program(
        "org 100h\nmsg db 'hello'\nmsg_end:\nmov cx, offset msg_end - offset msg\n",
    )
    .unwrap();
    // The message is five bytes long, and MOV CX,5 is B9 05 00.
    assert_eq!(
        program.bytes,
        vec![b'h', b'e', b'l', b'l', b'o', 0xB9, 0x05, 0x00]
    );
}

#[test]
fn an_offset_can_have_an_addition() {
    let program =
        assemble_program("org 100h\nfilename db 'ab'\nmov dx, offset filename+2\n").unwrap();
    // The name is at 100h, and MOV DX,102h is BA 02 01.
    assert_eq!(program.bytes, vec![b'a', b'b', 0xBA, 0x02, 0x01]);
}

#[test]
fn a_conditional_jump_out_of_range_is_refused_in_the_reference_words() {
    let mut source = String::from("org 0\nje there\n");
    for _ in 0..200 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nnop\n");
    let error = assemble_program(&source).unwrap_err();
    // The refusal says which line it happened on.
    let inner = match error {
        emo8067_asm::program::Error::AtLine { error, .. } => *error,
        other => panic!("expected the refusal, got {other:?}"),
    };
    match inner {
        emo8067_asm::program::Error::JumpOutOfRange { message } => {
            assert_eq!(
                message,
                "conditional jump is out of range: allowed values are from -128 to 127"
            );
        }
        other => panic!("expected the refusal, got {other:?}"),
    }
}
