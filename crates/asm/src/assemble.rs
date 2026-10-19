//! Assembling a line.
//!
//! The whole thread in one place: a line of source, the instruction's name,
//! the form the document's table gives that instruction, the operands the
//! source wrote, and the bytes.

use crate::addressing;
use crate::checks;
use crate::groups;
use crate::line;
use crate::literals;
use crate::operands::{self, Memory, Operand};
use crate::table;

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The line has no instruction on it.
    Empty,
    /// The instruction is not in the document's table.
    UnknownInstruction { name: String },
    /// An operand is not a register, a number or a reference.
    BadOperand { text: String },
    /// The operands are of the wrong type, in the reference's own words.
    Type { message: &'static str },
}

/// Assemble one line of source.
pub fn assemble(source: &str) -> Result<Vec<u8>, Error> {
    let line = line::parse(source).map_err(|_| Error::Empty)?;
    assemble_parts(&line, &line.operands)
}

/// Assemble a line that has already been taken apart.
pub fn assemble_parts(line: &line::Line, operand_texts: &[String]) -> Result<Vec<u8>, Error> {
    let operands: Vec<Operand> = operand_texts
        .iter()
        .map(|text| operand(text))
        .collect::<Result<Vec<_>, _>>()?;
    // The manual has no NOP: the 8086's 90h is XCHG AX,AX, and that is what
    // a NOP is.
    if line.mnemonic.eq_ignore_ascii_case("NOP") && operands.is_empty() {
        return Ok(vec![0x90]);
    }
    // The instruction's form, out of the document's table. A mnemonic can
    // name more than one form - MOV r/m8,r8 and MOV r/m16,r16 are both MOV -
    // so the operands pick the one. A group's cell names the operation
    // rather than a form, so the group's own table is searched too.
    // The checks that need no form come first: their answer does not depend
    // on which instruction the source meant.
    checks::check_pair(&operands).map_err(|message| Error::Type { message })?;
    let options = options_for(&line.mnemonic);
    // The reference's own checks come first: they say why a pair of
    // operands is refused, in its words, and the form is only chosen when
    // the pair survives them.
    let mut complaint: Option<&'static str> = None;
    let mut exact: Option<&'static str> = None;
    let mut chosen: Option<(&u8, &String)> = None;
    for (opcode, takes, form) in &options {
        match checks::check(takes, &operands) {
            Ok(()) => {
                if form_takes(takes, &operands) {
                    chosen = Some((opcode, form));
                    break;
                }
            }
            Err(message) => {
                // The complaint that counts is the one from the form that
                // matched the operands: that is the instruction the source
                // meant, and its check is the reason it does not fit.
                if complaint.is_none() {
                    complaint = Some(message);
                }
                if exact.is_none() && form_takes(takes, &operands) {
                    exact = Some(message);
                }
            }
        }
    }
    let Some((opcode, form)) = chosen else {
        return Err(match exact.or(complaint) {
            Some(message) => Error::Type { message },
            None => Error::UnknownInstruction {
                name: line.mnemonic.clone(),
            },
        });
    };
    operands::encode(*opcode, &line.mnemonic, form, &operands).map_err(|_| Error::BadOperand {
        text: line.operands.join(", "),
    })
}

/// The forms an instruction's name can mean: the table's own cells, and the
/// group's cells, which name the operation instead of the form.
///
/// Each option carries the form the matcher takes and the form that says
/// which operands the form takes. For a group they differ, because the
/// matcher resolves the group's digit from the operation.
fn options_for(mnemonic: &str) -> Vec<(u8, String, String)> {
    let mut options: Vec<(u8, String, String)> = Vec::new();
    for cell in table::forms() {
        if cell.mnemonic.eq_ignore_ascii_case(mnemonic) {
            options.push((
                cell.opcode,
                cell.operands.to_string(),
                cell.operands.to_string(),
            ));
        }
    }
    for group in groups::forms() {
        if group.mnemonic().eq_ignore_ascii_case(mnemonic) {
            options.push((
                group.opcode,
                group.operands().to_string(),
                "Group".to_string(),
            ));
        }
    }
    options
}

/// Whether a form takes these operands.
///
/// A mnemonic can name many forms - ADD r/m8,r8, ADD r/m16,r16, ADD AL,i8 and
/// ADD AX,i16 are all ADD - and the operands say which one was meant.
fn form_takes(form: &str, operands: &[Operand]) -> bool {
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    if parts.len() != operands.len() {
        return false;
    }
    parts
        .iter()
        .zip(operands)
        .all(|(part, operand)| part_takes(part.trim(), operand))
}

/// Whether one part of a form takes one operand.
fn part_takes(part: &str, operand: &Operand) -> bool {
    // A part that names a register takes that register and nothing else:
    // the table's cells say `MOV CL,i8` for the register itself.
    if is_register(part) {
        return matches!(operand, Operand::Register(name) if name == part);
    }
    let is_segment = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "CS" | "DS" | "ES" | "SS"));
    let is_byte = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "AL" | "CL" | "DL" | "BL" | "AH" | "CH" | "DH" | "BH"));
    let is_word = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "AX" | "CX" | "DX" | "BX" | "SP" | "BP" | "SI" | "DI"));
    match part {
        // A register or a reference, of the width the part names.
        "r/m8" => match operand {
            Operand::Register(_) => is_byte,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Word),
            _ => false,
        },
        "r/m16" => match operand {
            Operand::Register(_) => is_word,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Byte),
            _ => false,
        },
        // A reference and nothing else.
        "m8" => {
            matches!(operand, Operand::Memory(memory) if memory.width != Some(crate::modrm::Width::Word))
        }
        "m16" => {
            matches!(operand, Operand::Memory(memory) if memory.width != Some(crate::modrm::Width::Byte))
        }
        "r8" => is_byte,
        "r16" => is_word,
        "i8" | "i16" | "1" => matches!(operand, Operand::Immediate(_)),
        "sreg" => is_segment,
        _ => true,
    }
}

/// One operand, as the source wrote it.
fn operand(text: &str) -> Result<Operand, Error> {
    let text = text.trim();
    let upper = text.to_ascii_uppercase();
    // A reference: it has brackets, or it says PTR.
    if text.contains('[') || upper.contains("PTR") {
        let reference = addressing::parse(text).map_err(|_| Error::BadOperand {
            text: text.to_string(),
        })?;
        return Ok(Operand::Memory(Memory {
            segment: None,
            base: reference.address.base.map(|reg| reg.name().to_string()),
            index: reference.address.index.map(|reg| reg.name().to_string()),
            displacement: reference.address.displacement,
            width: reference.width,
        }));
    }
    // A register.
    if is_register(&upper) {
        return Ok(Operand::Register(upper));
    }
    // A number.
    let value = literals::parse(text).map_err(|_| Error::BadOperand {
        text: text.to_string(),
    })?;
    Ok(Operand::Immediate(value))
}

/// Whether a word names a register.
fn is_register(word: &str) -> bool {
    matches!(
        word,
        "AL" | "CL"
            | "DL"
            | "BL"
            | "AH"
            | "CH"
            | "DH"
            | "BH"
            | "AX"
            | "CX"
            | "DX"
            | "BX"
            | "SP"
            | "BP"
            | "SI"
            | "DI"
            | "CS"
            | "DS"
            | "ES"
            | "SS"
            | "IP"
    )
}
