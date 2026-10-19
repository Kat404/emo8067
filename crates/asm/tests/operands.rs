//! Matching a form from the document to the bytes it encodes.

use emo8067_asm::operands::{Error, Operand, encode};

fn reg(name: &str) -> Operand {
    Operand::Register(name.to_string())
}

fn imm(value: i64) -> Operand {
    Operand::Immediate(value)
}

#[test]
fn a_register_against_a_register_is_the_opcode_and_a_modrm_byte() {
    // ADD r/m8,r8 with AL and CL: the REG field names CL and the R/M field
    // names AL.
    assert_eq!(
        encode(0x00, "r/m8,r8", &[reg("AL"), reg("CL")]).unwrap(),
        vec![0x00, 0xC8]
    );
    // MOV r/m16,r16 with AX and BX: the REG field names the source, BX, and
    // the R/M field the destination, AX. That is 89 D8h.
    assert_eq!(
        encode(0x89, "r/m16,r16", &[reg("AX"), reg("BX")]).unwrap(),
        vec![0x89, 0xD8]
    );
    // MOV r8,r/m8: here the REG field names the first operand.
    assert_eq!(
        encode(0x8A, "r8,r/m8", &[reg("CL"), reg("AL")]).unwrap(),
        vec![0x8A, 0xC8]
    );
}

#[test]
fn an_immediate_is_little_endian_and_the_width_the_form_says() {
    assert_eq!(
        encode(0x04, "AL,i8", &[reg("AL"), imm(1)]).unwrap(),
        vec![0x04, 0x01]
    );
    assert_eq!(
        encode(0xB8, "AX,i16", &[reg("AX"), imm(0x1234)]).unwrap(),
        vec![0xB8, 0x34, 0x12]
    );
    // An immediate that does not fit is an error, not a truncation.
    assert!(matches!(
        encode(0x04, "AL,i8", &[reg("AL"), imm(0x100)]),
        Err(Error::Type { .. })
    ));
}

#[test]
fn the_wrong_number_of_operands_is_an_error() {
    assert!(matches!(
        encode(0x00, "r/m8,r8", &[reg("AL")]),
        Err(Error::Count { got: 1, .. })
    ));
}

#[test]
fn a_register_of_the_wrong_width_is_an_error() {
    // AL is not a 16-bit register.
    assert!(matches!(
        encode(0x89, "r/m16,r16", &[reg("AL"), reg("BX")]),
        Err(Error::Type { .. })
    ));
    // And AH cannot be named by a ModR/M field at all.
    assert!(matches!(
        encode(0x00, "r/m8,r8", &[reg("AH"), reg("CL")]),
        Err(Error::Type { .. })
    ));
}

#[test]
fn the_forms_that_are_not_encoded_yet_say_so() {
    // A group form needs the /digit, which lives in the guide's group tables.
    assert!(matches!(
        encode(0x80, "Group 1", &[imm(1)]),
        Err(Error::Unsupported { .. })
    ));
    // And so does a memory operand, until the addressing modes land.
    assert!(matches!(
        encode(
            0x00,
            "r/m8,r8",
            &[emo8067_asm::operands::Memory::default().into(), reg("CL")]
        ),
        Err(Error::Unsupported { .. })
    ));
}
