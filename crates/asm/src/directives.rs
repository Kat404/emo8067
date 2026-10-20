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
    /// `END`: the program ends here, and the rest of the source is prose.
    End,
    /// A directive that only matters to the assembler, not to the bytes.
    Ignored,
}

/// The names of the directives the assembler knows.
pub const NAMES: [&str; 23] = [
    "ORG", "DB", "DW", "DD", "EQU", "LABEL", "MACRO", "ENDM", "LOCAL", "INCLUDE", "END", "START",
    "SEGMENT", "ENDS", "ASSUME", "PROC", "ENDP", "STACK", "NONE", "NEAR", "FAR", "SHORT", "NAME",
];

/// The directives that say nothing to the bytes.
const IGNORED: [&str; 12] = [
    "NAME", "SEGMENT", "ENDS", "ASSUME", "PROC", "ENDP", "STACK", "NEAR", "FAR", "COMMENT", "PAGE",
    "TITLE",
];

/// The MASM-style sections the reference's own examples use.
///
/// The reference writes them with a dot in front - `.data`, `.code`,
/// `.stack 64h` - and they are the segments of the program, which this
/// assembler lays out as one.
const SECTIONS: [&str; 3] = ["DATA", "CODE", "STACK"];

/// The dotted words that say nothing to the bytes.
const DOTTED: [&str; 7] = ["DATA", "CODE", "STACK", "MODEL", "EXIT", "PUBLIC", "EXTRN"];

/// Whether a name is one of the program's sections.
#[must_use]
pub fn is_segment(name: &str) -> bool {
    SECTIONS.contains(&name.to_ascii_uppercase().as_str())
}

/// Read a directive, if the line is one.
pub fn parse(mnemonic: &str, operands: &[String]) -> Option<Directive> {
    let name = mnemonic.to_ascii_uppercase();
    // The reference writes the sections with a dot in front: `.data`.
    if let Some(dotted) = name.strip_prefix('.') {
        return DOTTED.contains(&dotted).then_some(Directive::Ignored);
    }
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
        "DUP" => Some(Directive::Db(values(operands)?)),
        "LABEL" => Some(Directive::Label(operands.first()?.clone())),
        "END" => Some(Directive::End),
        _ => None,
    }
}

/// The numbers a data directive lists.
///
/// The reference writes `len dup(0)` for a run of bytes, `?` for a byte it
/// does not care about, and a string for the characters themselves.
fn values(operands: &[String]) -> Option<Vec<i64>> {
    let mut out: Vec<i64> = Vec::new();
    for text in operands {
        let text = text.trim();
        if text == "?" {
            out.push(0);
            continue;
        }
        // A string is a run of bytes.
        if let Some(string) = string_of(text) {
            out.extend(string.bytes().map(i64::from));
            continue;
        }
        if let Some((count, value)) = text.split_once(" dup").or_else(|| text.split_once(" DUP")) {
            let count = literals::parse(count.trim()).ok()?;
            let value = value
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim();
            // A byte the source does not care about.
            if value == "?" {
                out.extend(std::iter::repeat_n(0i64, count as usize));
                continue;
            }
            // A string, repeated.
            if let Some(string) = string_of(value) {
                for _ in 0..count {
                    out.extend(string.bytes().map(i64::from));
                }
                continue;
            }
            let value = literals::parse(value).ok()?;
            for _ in 0..count {
                out.push(value);
            }
            continue;
        }
        out.push(literals::parse(text).ok()?);
    }
    Some(out)
}

/// The characters of a string the source wrote, if it wrote one.
fn string_of(text: &str) -> Option<String> {
    let trimmed = text.trim();
    for quote in ['"', '\''] {
        if trimmed.len() >= 2 && trimmed.starts_with(quote) && trimmed.ends_with(quote) {
            return Some(trimmed[1..trimmed.len() - 1].to_string());
        }
    }
    None
}
