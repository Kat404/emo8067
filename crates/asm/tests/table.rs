//! The opcode map, read out of the document.

use emo8067_asm::table::{forms, forms_of};

#[test]
fn the_map_covers_every_opcode_and_every_column() {
    let forms = forms();
    assert_eq!(forms.len(), 256, "one cell per opcode");
    for opcode in 0..=255u8 {
        assert_eq!(
            forms_of(opcode).len(),
            1,
            "opcode {opcode:#04x} has one cell"
        );
    }
}

#[test]
fn a_cell_is_a_mnemonic_and_an_operand_form() {
    let add = forms_of(0x00)[0];
    assert_eq!(add.opcode, 0x00);
    assert_eq!(add.column, 0);
    assert_eq!(add.mnemonic, "ADD");
    assert_eq!(add.operands, "r/m8,r8");
    assert!(add.is_instruction());
}

#[test]
fn the_next_opcode_of_a_row_is_the_next_cell() {
    let row = forms_of(0x01)[0];
    assert_eq!(row.opcode, 0x01, "the row's second opcode");
    assert_eq!(row.mnemonic, "ADD");
    assert_eq!(row.operands, "r/m16,r16");
}

#[test]
fn a_group_cell_is_not_an_instruction() {
    // 80 is Group 1, and the guide names it as a group.
    let group = forms_of(0x80)[0];
    assert_eq!(group.mnemonic, "Group");
    assert!(!group.is_instruction());

    // 84 is TEST, and it is one.
    let test = forms_of(0x84)[0];
    assert_eq!(test.mnemonic, "TEST");
    assert!(test.is_instruction());
}

#[test]
fn the_documented_forms_are_where_they_are() {
    // A few cells from the manual's own table, so that a reader of the
    // document can check the reader.
    assert_eq!(forms_of(0x90)[0].mnemonic, "XCHG");
    assert_eq!(forms_of(0x90)[0].operands, "AX,AX");
    assert_eq!(forms_of(0xB8)[0].operands, "AX,i16");
    assert_eq!(forms_of(0xE8)[0].operands, "near");
    assert_eq!(forms_of(0x9A)[0].operands, "far");
    assert_eq!(forms_of(0xA0)[0].operands, "AL,m8");
    assert_eq!(forms_of(0x06)[0].mnemonic, "PUSH");
    assert_eq!(forms_of(0x06)[0].operands, "ES");
}
