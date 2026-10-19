//! The type checks, in the reference's own words.

use emo8067_asm::assemble::{Error, assemble};

/// The refusal the source gets, word for word.
fn refusal(source: &str) -> String {
    match assemble(source) {
        Err(Error::Type { message }) => message.to_string(),
        other => panic!("expected a refusal for {source:?}, got {other:?}"),
    }
}

#[test]
fn a_register_of_the_wrong_width_is_refused() {
    assert_eq!(refusal("MOV AX, BL"), "16 bit and 8 bit register");
    assert_eq!(refusal("MOV BL, AX"), "8 bit and 16 bit register");
}

#[test]
fn cs_cannot_be_written_to() {
    assert_eq!(
        refusal("MOV CS, AX"),
        "CS cannot be modified directly (use far JMP or CALL)"
    );
}

#[test]
fn the_instruction_pointer_can_neither_be_written_nor_read() {
    assert_eq!(
        refusal("MOV IP, AX"),
        "IP cannot be modified directly (use JMP or CALL)"
    );
    assert_eq!(
        refusal("MOV AX, IP"),
        "cannot get value of IP register (use POP after CALL)"
    );
}

#[test]
fn memory_does_not_go_to_memory() {
    assert_eq!(
        refusal("MOV [BX], [SI]"),
        "memory to memory is not allowed for 8086:"
    );
}

#[test]
fn two_segment_registers_do_not_go_together() {
    assert_eq!(
        refusal("MOV DS, ES"),
        "segment registers cannot go together!"
    );
}

#[test]
fn a_segment_register_does_not_go_with_an_immediate() {
    assert_eq!(
        refusal("MOV DS, 5"),
        "cannot use segment register with an immediate value"
    );
}

#[test]
fn an_immediate_that_does_not_fit_the_form_is_refused() {
    assert_eq!(refusal("ADD AL, 300"), "second operand is over 8 bits!");
}
