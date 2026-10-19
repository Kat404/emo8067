//! Assembling a program.
//!
//! A program is a list of lines. The directives say where it is and what it
//! holds, the instructions are the bytes, and the names are collected as the
//! program is walked.
//!
//! The program is walked twice. The first time the names are collected and
//! the addresses are counted; the second time the bytes are made, and by
//! then a jump knows where it is going. A jump's size does not depend on
//! where it goes, so the first walk can count it with a target of zero.

use crate::assemble;
use crate::directives::{self, Directive};
use crate::line;
use crate::literals;
use crate::operands::Operand;

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// A line is not a line.
    Empty,
    /// The instruction is not in the document's table.
    UnknownInstruction { name: String },
    /// The operands do not fit.
    BadOperand { text: String },
    /// A name that the program does not define.
    UnknownName { name: String },
}

/// A program, assembled.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Program {
    /// Where the program starts.
    pub origin: u16,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// The names and the address they are at.
    pub names: Vec<(String, u16)>,
}

/// What a line does to the program.
enum Effect {
    /// Nothing at all.
    Nothing,
    /// A name at this address.
    Label(String),
    /// A name and its value.
    Constant(String, i64),
    /// The program starts here.
    Origin(u16),
    /// These bytes go into the program.
    Bytes(Vec<u8>),
    /// An instruction, the label it carries, the address it is at, and how
    /// long it is.
    Instruction(String, Vec<String>, u16, usize),
}

/// Assemble a program.
pub fn assemble_program(source: &str) -> Result<Program, Error> {
    let lines: Vec<line::Line> = source
        .lines()
        .map(line::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Empty)?;
    // The first walk: the names, the constants, and how far the program goes.
    let mut origin = 0u16;
    let mut address = 0u16;
    let mut names: Vec<(String, u16)> = Vec::new();
    let mut constants: Vec<(String, i64)> = Vec::new();
    for parsed in &lines {
        match effect(parsed, &constants, &names, address, false)? {
            Effect::Nothing => {}
            Effect::Label(name) => names.push((name, address)),
            Effect::Constant(name, value) => constants.push((name, value)),
            Effect::Origin(start) => {
                origin = start;
                address = start;
            }
            Effect::Bytes(bytes) => address = address.wrapping_add(bytes.len() as u16),
            Effect::Instruction(label, _, at, size) => {
                if !label.is_empty() {
                    names.push((label, at));
                }
                address = at.wrapping_add(size as u16);
            }
        }
    }
    // The second walk: the bytes, with the names known.
    let mut program = Program {
        origin,
        bytes: Vec::new(),
        names: names.clone(),
    };
    let mut address = origin;
    for parsed in &lines {
        match effect(parsed, &constants, &names, address, true)? {
            Effect::Nothing | Effect::Label(..) | Effect::Constant(..) | Effect::Origin(..) => {}
            Effect::Bytes(bytes) => {
                program.bytes.extend_from_slice(&bytes);
                address = address.wrapping_add(bytes.len() as u16);
            }
            Effect::Instruction(_, texts, at, size) => {
                let bytes = assemble_instruction(parsed, &texts, at)?;
                program.bytes.extend_from_slice(&bytes);
                address = at.wrapping_add(size as u16);
            }
        }
    }
    Ok(program)
}

/// What a line does, with the names and the constants the program has.
fn effect(
    parsed: &line::Line,
    constants: &[(String, i64)],
    names: &[(String, u16)],
    address: u16,
    strict: bool,
) -> Result<Effect, Error> {
    // A name and its value: `SIZE EQU 10`.
    if let Some((name, value)) = equ(parsed) {
        return Ok(Effect::Constant(name, value));
    }
    // A directive.
    if let Some(directive) = directives::parse(&parsed.mnemonic, &parsed.operands) {
        return Ok(match directive {
            Directive::Org(origin) => Effect::Origin(origin),
            Directive::Db(values) => Effect::Bytes(bytes_of(&values, 1)),
            Directive::Dw(values) => Effect::Bytes(bytes_of(&values, 2)),
            Directive::Dd(values) => Effect::Bytes(bytes_of(&values, 4)),
            Directive::Equ(name, value) => Effect::Constant(name, value),
            Directive::Label(name) => Effect::Label(name),
            Directive::Ignored => Effect::Nothing,
        });
    }
    // A line that carries nothing but a label has no instruction.
    if parsed.mnemonic.is_empty() {
        return Ok(match &parsed.label {
            Some(name) => Effect::Label(name.clone()),
            None => Effect::Nothing,
        });
    }
    // An instruction. Its size is what the assembler says, and a jump's
    // size does not depend on where it goes, so a target of zero is enough
    // to count it.
    let texts = fold(&parsed.operands, constants, names);
    let operands = operands_of(parsed, &texts, strict)?;
    let bytes =
        assemble::assemble_parts(parsed, &operands, address).map_err(|error| bad(parsed, error))?;
    let size = bytes.len();
    Ok(Effect::Instruction(
        parsed.label.clone().unwrap_or_default(),
        texts,
        address,
        size,
    ))
}

/// Assemble one instruction, with the names the program has.
fn assemble_instruction(
    parsed: &line::Line,
    texts: &[String],
    address: u16,
) -> Result<Vec<u8>, Error> {
    let operands = operands_of(parsed, texts, true)?;
    assemble::assemble_parts(parsed, &operands, address).map_err(|error| bad(parsed, error))
}

/// The operands of a line, and a word that names nothing.
fn operands_of(parsed: &line::Line, texts: &[String], strict: bool) -> Result<Vec<Operand>, Error> {
    let operands: Vec<Operand> = texts
        .iter()
        .map(|text| assemble::operand(text))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::BadOperand {
            text: parsed.operands.join(", "),
        })?;
    // A word that is still a name is a name the program does not have.
    for (text, operand) in texts.iter().zip(&operands) {
        if strict && matches!(operand, Operand::Relative(_)) {
            return Err(Error::UnknownName {
                name: text.trim().to_string(),
            });
        }
    }
    Ok(operands)
}

/// The error the assembler gave, in the program's own words.
fn bad(parsed: &line::Line, error: assemble::Error) -> Error {
    match error {
        assemble::Error::UnknownInstruction { name } => Error::UnknownInstruction { name },
        assemble::Error::Type { message } => Error::BadOperand {
            text: message.to_string(),
        },
        _ => Error::BadOperand {
            text: parsed.operands.join(", "),
        },
    }
}

/// Put a name and its value back together: `SIZE EQU 10`.
fn equ(parsed: &line::Line) -> Option<(String, i64)> {
    let first = parsed.operands.first()?;
    let rest = first.trim();
    let upper = rest.to_ascii_uppercase();
    let value = upper.strip_prefix("EQU")?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let name = parsed.mnemonic.trim_end_matches(':').to_string();
    if name.is_empty() {
        return None;
    }
    Some((name, literals::parse(value).ok()?))
}

/// Replace the operands that name a constant or a label with its value.
fn fold(operands: &[String], constants: &[(String, i64)], names: &[(String, u16)]) -> Vec<String> {
    operands
        .iter()
        .map(|text| {
            let upper = text.trim().to_ascii_uppercase();
            if let Some((_, value)) = constants
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&upper))
            {
                return value.to_string();
            }
            if let Some((_, at)) = names
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&upper))
            {
                return at.to_string();
            }
            text.clone()
        })
        .collect()
}

/// The bytes of a data directive's values.
fn bytes_of(values: &[i64], width: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        let of = value.to_le_bytes();
        bytes.extend_from_slice(&of[..width]);
    }
    bytes
}
