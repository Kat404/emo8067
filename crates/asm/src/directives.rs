//! The directives, and what they mean.
//!
//! The reference's own list of them: `LISTING` `LABEL` `PROC` `END` `ENDS`
//! `ENDP` `DUP` `FAR` `NEAR` `EQU` `ORG` `DB` `DW` `DD` `STACK` `NONE`
//! `SEGMENT` `ASSUME` `MACRO` `ENDM` `COMMENT` `PAGE` `TITLE` `NAME` `INC`.
//!
//! Most of them matter to the assembler and not to the bytes: the segments,
//! the procedures and the listing are the shape of the source, not of the
//! program. The rest say where the program is, what it holds, and what its
//! names mean.

use crate::literals;

/// What a directive says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Directive {
    /// `ORG`: where the program starts.
    Org(u16),
    /// `DB`: bytes.
    Db(Vec<i64>),
    /// `DW`: words.
    Dw(Vec<i64>),
    /// `DD`: double words.
    Dd(Vec<i64>),
    /// `EQU`: a name and its value.
    Equ(String, i64),
    /// `LABEL`: a name at this address.
    Label(String),
    /// A directive that only matters to the assembler, not to the bytes.
    Ignored,
}

/// The directives that say nothing to the bytes.
const IGNORED: [&str; 13] = [
    "NAME", "SEGMENT", "ENDS", "ASSUME", "PROC", "ENDP", "END", "STACK", "NEAR", "FAR", "COMMENT",
    "PAGE", "TITLE",
];

/// Read a directive, if the line is one.
pub fn parse(mnemonic: &str, operands: &[String]) -> Option<Directive> {
    let name = mnemonic.to_ascii_uppercase();
    if IGNORED.contains(&name.as_str()) {
        return Some(Directive::Ignored);
    }
    match name.as_str() {
        "ORG" => Some(Directive::Org(
            literals::parse_word(operands.first()?).ok()?,
        )),
        "DB" => Some(Directive::Db(values(operands)?)),
        "DW" => Some(Directive::Dw(values(operands)?)),
        "DD" => Some(Directive::Dd(values(operands)?)),
        "LABEL" => Some(Directive::Label(operands.first()?.clone())),
        _ => None,
    }
}

/// The numbers a data directive lists.
fn values(operands: &[String]) -> Option<Vec<i64>> {
    operands
        .iter()
        .map(|text| literals::parse(text).ok())
        .collect()
}
