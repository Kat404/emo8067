//! The group tables, read out of the document.

use emo8067_asm::groups::{find, forms};

#[test]
fn the_group_tables_are_read() {
    let forms = forms();
    // Group 1 has six opcodes, the shift group four and Group 2 two, and each
    // has eight digits: 96 forms in all.
    assert_eq!(forms.len(), 96);
    // Group 1 has eight digits for each of its six opcodes.
    let group_one = forms.iter().filter(|f| f.opcode == 0x80).count();
    assert_eq!(group_one, 8, "Group 1's digit per operation");
}

#[test]
fn an_operation_chooses_its_digit() {
    // ADD in Group 1 is the digit zero.
    let add = find(0x80, "ADD").expect("ADD is in Group 1");
    assert_eq!(add.digit, 0);
    assert_eq!(add.operands(), "r/m8,i8");

    // AND in Group 1 is the digit four.
    let and = find(0x80, "AND").expect("AND is in Group 1");
    assert_eq!(and.digit, 4);

    // And the shift group has its own table: ROL is digit zero.
    let rol = find(0xD0, "ROL").expect("ROL is in the shift group");
    assert_eq!(rol.digit, 0);
    assert_eq!(rol.operands(), "r/m8,1");
}
