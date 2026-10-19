//! The ModR/M byte, and the order the fields name the registers in.

use emo8067_asm::modrm::{Mod, REG8, REG16, Width, byte_for};

#[test]
fn the_sixteen_bit_field_names_the_8086s_order() {
    // Table 4-9 of the 1979 manual: AX CX DX BX SP BP SI DI. The 80186
    // moved SP and BX along; the hardware did not.
    assert_eq!(REG16, ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"]);
    assert_eq!(Width::Word.field_of("SP"), Some(4));
    assert_eq!(Width::Word.field_of("BX"), Some(3));
    assert_eq!(Width::Word.field_of("DI"), Some(7));
}

#[test]
fn the_eight_bit_field_cannot_reach_the_high_registers() {
    assert_eq!(REG8, ["AL", "CL", "DL", "BL"]);
    assert_eq!(Width::Byte.field_of("AL"), Some(0));
    assert_eq!(Width::Byte.field_of("BL"), Some(3));
    assert_eq!(
        Width::Byte.field_of("AH"),
        None,
        "the 8086's field cannot name it"
    );
    assert_eq!(Width::Byte.field_of("BH"), None);
}

#[test]
fn a_modrm_byte_is_three_fields() {
    // 8B E0 is MOV SP,AX: mod=11, reg=100 (SP), rm=000 (AX).
    assert_eq!(byte_for(Mod::Register, 4, 0), 0xE0);
    // 01 07 is ADD [BX],AX: mod=00, reg=000 (AX), rm=111 (BX).
    assert_eq!(byte_for(Mod::Memory, 0, 7), 0x07);
    // 01 47 is ADD [BX+disp8],AX: mod=01, reg=000, rm=111.
    assert_eq!(byte_for(Mod::Byte, 0, 7), 0x47);
    // 01 87 is ADD [BX+disp16],AX: mod=10.
    assert_eq!(byte_for(Mod::Word, 0, 7), 0x87);
}
