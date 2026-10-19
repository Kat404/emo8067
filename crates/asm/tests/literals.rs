//! Numeric literals, and the reference's rules about them.

use emo8067_asm::literals::{Error, parse, parse_word};

#[test]
fn the_four_ways_of_writing_a_number() {
    assert_eq!(parse("&HFF").unwrap(), 0xFF);
    assert_eq!(parse("&hff").unwrap(), 0xFF);
    assert_eq!(parse("1234h").unwrap(), 0x1234);
    assert_eq!(
        parse("16$FF").unwrap(),
        0xFF,
        "MASM's radix: the base, then the $, then the digits"
    );
    assert_eq!(parse("8$17").unwrap(), 15, "base eight, seventeen");
    assert_eq!(parse("1234").unwrap(), 1234);
}

#[test]
fn a_hex_value_that_would_look_decimal_wants_a_zero_prefix() {
    // "0FFh" reads as a name, so the reference wanted "00FFh".
    assert!(matches!(parse("FFh"), Err(Error::NeedsZeroPrefix { .. })));
    assert_eq!(parse("0FFh").unwrap(), 0xFF);
    assert_eq!(parse("1234h").unwrap(), 0x1234, "it begins with a digit");
}

#[test]
fn a_value_that_will_not_fit_is_refused() {
    assert!(matches!(
        parse_word("10000h"),
        Err(Error::TooBig { value: 0x10000, .. })
    ));
    assert_eq!(parse_word("0FFFFh").unwrap(), 0xFFFF);
}

#[test]
fn something_that_is_not_a_number_is_not_a_number() {
    assert!(matches!(parse("AX"), Err(Error::NotANumber { .. })));
    assert!(matches!(parse(""), Err(Error::NotANumber { .. })));
}
