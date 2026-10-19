//! Assembling a program.
//!
//! A program is a list of lines. The directives say where it is and what it
//! holds, the instructions are the bytes, and the names are collected as the
//! program is walked so that a label can be used after it was written.

use crate::assemble;
use crate::directives::{self, Directive};
use crate::line;
use crate::literals;

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

/// Assemble a program.
pub fn assemble_program(source: &str) -> Result<Program, Error> {
    let mut program = Program::default();
    let mut constants: Vec<(String, i64)> = Vec::new();
    for raw in source.lines() {
        let parsed = line::parse(raw).map_err(|_| Error::Empty)?;
        // A label the line carries: `start:`.
        if let Some(name) = &parsed.label {
            program.names.push((name.clone(), address(&program)));
        }
        // A name and its value: `SIZE EQU 10`.
        if let Some((name, value)) = equ(&parsed) {
            constants.push((name, value));
            continue;
        }
        // A directive.
        if let Some(directive) = directives::parse(&parsed.mnemonic, &parsed.operands) {
            match directive {
                Directive::Org(origin) => {
                    program.origin = origin;
                    program.bytes.clear();
                    program.names.clear();
                }
                Directive::Db(values) => push(&mut program.bytes, &values, 1),
                Directive::Dw(values) => push(&mut program.bytes, &values, 2),
                Directive::Dd(values) => push(&mut program.bytes, &values, 4),
                Directive::Equ(name, value) => constants.push((name, value)),
                Directive::Label(name) => program.names.push((name, address(&program))),
                Directive::Ignored => {}
            }
            continue;
        }
        // A line that carries nothing but a label has no instruction.
        if parsed.mnemonic.is_empty() {
            continue;
        }
        // An instruction, with the constants folded into its operands.
        let folded = fold(&parsed.operands, &constants);
        let bytes = assemble::assemble_parts(&parsed, &folded).map_err(|error| match error {
            assemble::Error::UnknownInstruction { name } => Error::UnknownInstruction { name },
            assemble::Error::Type { message } => Error::BadOperand {
                text: message.to_string(),
            },
            _ => Error::BadOperand {
                text: parsed.operands.join(", "),
            },
        })?;
        program.bytes.extend_from_slice(&bytes);
    }
    Ok(program)
}

/// The address the program has reached.
fn address(program: &Program) -> u16 {
    program.origin.wrapping_add(program.bytes.len() as u16)
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

/// Replace the operands that name a constant with its value.
fn fold(operands: &[String], constants: &[(String, i64)]) -> Vec<String> {
    operands
        .iter()
        .map(|text| {
            let upper = text.trim().to_ascii_uppercase();
            match constants.iter().find(|(name, _)| *name == upper) {
                Some((_, value)) => value.to_string(),
                None => text.clone(),
            }
        })
        .collect()
}

/// Put the values of a data directive into the bytes.
fn push(bytes: &mut Vec<u8>, values: &[i64], width: usize) {
    for value in values {
        let bytes_of = value.to_le_bytes();
        bytes.extend_from_slice(&bytes_of[..width]);
    }
}
