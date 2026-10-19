//! The addressing modes, and the bytes they need.

use emo8067_asm::addressing::{Address, Error, Reg, default_segment, encode, parse};
use emo8067_asm::modrm::{Mod, Width};

fn address(base: Option<Reg>, index: Option<Reg>, displacement: Option<i64>) -> Address {
    Address {
        base,
        index,
        displacement,
    }
}

#[test]
fn the_eight_forms_name_the_rm_field_the_manual_gives_them() {
    let cases = [
        (address(Some(Reg::Bx), Some(Reg::Si), None), 0b000),
        (address(Some(Reg::Bx), Some(Reg::Di), None), 0b001),
        (address(Some(Reg::Bp), Some(Reg::Si), None), 0b010),
        (address(Some(Reg::Bp), Some(Reg::Di), None), 0b011),
        (address(Some(Reg::Si), None, None), 0b100),
        (address(Some(Reg::Di), None, None), 0b101),
        (address(Some(Reg::Bp), None, None), 0b110),
        (address(Some(Reg::Bx), None, None), 0b111),
    ];
    for (reference, rm) in cases {
        let (modrm, got, bytes) = encode(&reference).unwrap();
        assert_eq!(modrm, Mod::Memory);
        assert_eq!(got, rm, "the R/M field");
        assert!(bytes.is_empty(), "and no displacement");
    }
}

#[test]
fn a_displacement_says_how_wide_it_is() {
    let (modrm, rm, bytes) = encode(&address(Some(Reg::Bx), None, Some(0x40))).unwrap();
    assert_eq!(modrm, Mod::Byte);
    assert_eq!(rm, 0b111);
    assert_eq!(bytes, vec![0x40]);

    let (modrm, rm, bytes) = encode(&address(Some(Reg::Bx), None, Some(0x1234))).unwrap();
    assert_eq!(modrm, Mod::Word);
    assert_eq!(rm, 0b111);
    assert_eq!(bytes, vec![0x34, 0x12], "little endian");
}

#[test]
fn the_direct_form_carries_a_word() {
    let (modrm, rm, bytes) = encode(&address(None, None, Some(0x1234))).unwrap();
    assert_eq!(modrm, Mod::Memory, "the direct form is MOD 00 with R/M 110");
    assert_eq!(rm, 0b110);
    assert_eq!(bytes, vec![0x34, 0x12]);
    assert_eq!(bytes, vec![0x34, 0x12]);
}

#[test]
fn bp_and_sp_are_relative_to_ss() {
    assert_eq!(default_segment(&address(Some(Reg::Bp), None, None)), "SS");
    assert_eq!(default_segment(&address(Some(Reg::Sp), None, None)), "SS");
    assert_eq!(default_segment(&address(Some(Reg::Bx), None, None)), "DS");
    assert_eq!(default_segment(&address(Some(Reg::Si), None, None)), "DS");
}

#[test]
fn the_two_illegal_pairs_are_refused() {
    assert!(matches!(
        encode(&address(Some(Reg::Bx), Some(Reg::Bp), None)),
        Err(Error::WrongAddressing { .. })
    ));
    assert!(matches!(
        encode(&address(Some(Reg::Si), Some(Reg::Di), None)),
        Err(Error::WrongAddressing { .. })
    ));
}

#[test]
fn a_reference_is_read_the_way_the_source_wrote_it() {
    let reference = parse("[BX+SI+4]").unwrap();
    assert_eq!(reference.address.base, Some(Reg::Bx));
    assert_eq!(reference.address.index, Some(Reg::Si));
    assert_eq!(reference.address.displacement, Some(4));
    assert_eq!(reference.width, None);

    let reference = parse("WORD PTR [1234h]").unwrap();
    assert_eq!(reference.width, Some(Width::Word));
    assert_eq!(reference.address.displacement, Some(0x1234));

    let reference = parse("BYTE PTR DS:[BX]").unwrap();
    assert_eq!(reference.width, Some(Width::Byte));
    assert_eq!(reference.address.base, Some(Reg::Bx));

    // A sign in front of the number belongs to the number.
    let reference = parse("[BX-4]").unwrap();
    assert_eq!(reference.address.displacement, Some(-4));
}
