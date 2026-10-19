//! A line of source, all the way to bytes.

use emo8067_asm::assemble::assemble;

#[test]
fn a_register_against_a_register() {
    // MOV AX,BX is 89 D8h: the REG field names the source and the R/M field
    // the destination.
    assert_eq!(assemble("MOV AX, BX").unwrap(), vec![0x89, 0xD8]);
}

#[test]
fn an_immediate_against_the_accumulator() {
    // ADD AL,5 is 04 05h.
    assert_eq!(assemble("ADD AL, 5").unwrap(), vec![0x04, 0x05]);
    // MOV AX,1234h is B8 34 12h.
    assert_eq!(assemble("MOV AX, 1234h").unwrap(), vec![0xB8, 0x34, 0x12]);
}

#[test]
fn a_group_form_takes_its_digit_from_the_operation() {
    // ADD BL,1 is in Group 1, and ADD is the digit zero: 80 C3 01h.
    assert_eq!(assemble("ADD BL, 1").unwrap(), vec![0x80, 0xC3, 0x01]);
    // AND BL,1 is the digit four: 80 E3 01h.
    assert_eq!(assemble("AND BL, 1").unwrap(), vec![0x80, 0xE3, 0x01]);
}

#[test]
fn a_memory_operand_brings_its_displacement() {
    // MOV [BX+SI],CL is 88 08h.
    assert_eq!(assemble("MOV [BX+SI], CL").unwrap(), vec![0x88, 0x08]);
    // MOV [BX+4],BX is 89 5F 04h.
    assert_eq!(assemble("MOV [BX+4], BX").unwrap(), vec![0x89, 0x5F, 0x04]);
    // MOV [1234h],AX is the direct form: 89 06 34 12h.
    assert_eq!(
        assemble("MOV [1234h], AX").unwrap(),
        vec![0x89, 0x06, 0x34, 0x12]
    );
}

#[test]
fn an_instruction_that_is_not_in_the_table_is_refused() {
    assert!(assemble("FROB [BX]").is_err());
}

#[test]
fn a_reference_with_an_immediate_keeps_the_reference() {
    // MOV WORD PTR [BX],1234h is C7 07 34 12h: the reference is in the R/M
    // field and the digit the opcode carries is ignored.
    assert_eq!(
        assemble("MOV WORD PTR [BX], 1234h").unwrap(),
        vec![0xC7, 0x07, 0x34, 0x12]
    );
}

#[test]
fn a_group_picks_the_opcode_the_operation_uses() {
    // ADD BX,1 is in Group 1's word column: 81 C3 01 00h.
    assert_eq!(assemble("ADD BX, 1").unwrap(), vec![0x81, 0xC3, 0x01, 0x00]);
}
