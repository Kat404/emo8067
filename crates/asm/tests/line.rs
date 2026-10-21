//! A source line, taken apart.

use emo8067_asm::line::{Error, Prefix, parse};

#[test]
fn a_line_is_a_mnemonic_and_its_operands() {
    let line = parse("MOV AX, BX").unwrap();
    assert_eq!(line.mnemonic, "MOV");
    assert_eq!(line.operands, vec!["AX", "BX"]);
    assert!(line.prefixes.is_empty());
}

#[test]
fn a_comma_inside_brackets_does_not_split_the_operands() {
    let line = parse("MOV AX, [BX+SI+4]").unwrap();
    assert_eq!(line.operands, vec!["AX", "[BX+SI+4]"]);
    let line = parse("MOV [BX+DI], WORD 1234h").unwrap();
    assert_eq!(line.operands, vec!["[BX+DI]", "WORD 1234h"]);
}

#[test]
fn the_prefixes_come_before_the_instruction() {
    let line = parse("REPE MOVSB").unwrap();
    assert_eq!(line.prefixes, vec![Prefix::RepeatEqual]);
    assert_eq!(line.mnemonic, "MOVSB");

    let line = parse("LOCK INC WORD [BX]").unwrap();
    assert_eq!(line.prefixes, vec![Prefix::Lock]);

    let line = parse("CS: MOV AX, [SI]").unwrap();
    assert_eq!(line.prefixes, vec![Prefix::Segment("CS".to_string())]);
    assert_eq!(line.mnemonic, "MOV");
}

#[test]
fn a_segment_override_may_carry_a_dot() {
    // The reference's own list of the prefixes it accepts names the
    // segment overrides both ways: `cs:` and `.cs:`.
    let line = parse(".CS: MOV AX, [SI]").unwrap();
    assert_eq!(line.prefixes, vec![Prefix::Segment("CS".to_string())]);
    assert_eq!(line.mnemonic, "MOV");
}

#[test]
fn a_comment_is_not_code() {
    let line = parse("MOV AX, BX ; the whole point").unwrap();
    assert_eq!(line.operands, vec!["AX", "BX"]);
}

#[test]
fn an_empty_line_is_not_a_line() {
    assert!(matches!(parse("   "), Err(Error::Empty)));
    assert!(matches!(parse("; only a comment"), Err(Error::Empty)));
}
