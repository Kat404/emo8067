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
    /// A conditional jump that is out of range, in the reference's own words.
    OutOfRange { message: &'static str },
}

/// Assemble one line of source.
pub fn assemble(source: &str) -> Result<Vec<u8>, Error> {
    let line = line::parse(source).map_err(|_| Error::Empty)?;
    let operands: Vec<Operand> = line
        .operands
        .iter()
        .map(|text| operand(text))
        .collect::<Result<Vec<_>, _>>()?;
    assemble_parts(&line, &operands, 0)
}

/// Assemble a line that has already been taken apart.
pub fn assemble_parts(
    line: &line::Line,
    operands: &[Operand],
    address: u16,
) -> Result<Vec<u8>, Error> {
    assemble_parts_at(line, operands, address, true)
}

/// The same, with the range of a short jump's displacement left unchecked.
///
/// The first walk of a program does not know where a jump goes yet, and a
/// jump's size does not depend on where it goes, so the first walk can
/// count it with a target of zero.
pub fn assemble_parts_at(
    line: &line::Line,
    operands: &[Operand],
    address: u16,
    check_range: bool,
) -> Result<Vec<u8>, Error> {
    // The prefixes come before the instruction they cover, so the opcode is
    // not where the line starts: a jump's displacement is measured from the
    // end of the whole of it.
    let prefixes = line.prefix_bytes();
    let mut bytes = assemble_inner(
        line,
        operands,
        address.wrapping_add(prefixes.len() as u16),
        check_range,
    )?;
    let mut out = prefixes;
    out.append(&mut bytes);
    Ok(out)
}

/// The instruction itself, without the prefixes in front of it.
fn assemble_inner(
    line: &line::Line,
    operands: &[Operand],
    address: u16,
    check_range: bool,
) -> Result<Vec<u8>, Error> {
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
    checks::check_pair(operands).map_err(|message| Error::Type { message })?;
    let options = options_for(&line.mnemonic);
    // The reference's own checks come first: they say why a pair of
    // operands is refused, in its words, and the form is only chosen when
    // the pair survives them.
    let mut complaint: Option<&'static str> = None;
    let mut exact: Option<&'static str> = None;
    let mut chosen: Option<(&u8, &String)> = None;
    for (opcode, takes, form) in &options {
        // The source's own qualifier says which form it wants: `CALL FAR`
        // wants the form that crosses to another segment.
        if !qualifier_allows(line.qualifier.as_deref(), takes) {
            continue;
        }
        match checks::check(takes, operands) {
            Ok(()) => {
                if form_takes(takes, operands) {
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
                if exact.is_none() && form_takes(takes, operands) {
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
    operands::encode_at(
        *opcode,
        &line.mnemonic,
        form,
        operands,
        address,
        check_range,
    )
    .map_err(|error| match error {
        operands::Error::Type { form, position } => Error::BadOperand {
            text: format!("{form}, operand {position}"),
        },
        operands::Error::OutOfRange { message } => Error::OutOfRange { message },
        _ => Error::BadOperand {
            text: line.operands.join(", "),
        },
    })
}

/// The forms an instruction's name can mean: the table's own cells, and the
/// group's cells, which name the operation instead of the form.
///
/// Each option carries the form the matcher takes and the form that says
/// which operands the form takes. For a group they differ, because the
/// matcher resolves the group's digit from the operation.
pub fn options_for_pub(mnemonic: &str) -> Vec<(u8, String, String)> {
    options_for(mnemonic)
}
fn options_for(mnemonic: &str) -> Vec<(u8, String, String)> {
    let mut options: Vec<(u8, String, String)> = Vec::new();
    for cell in table::forms() {
        // The guide's row 60 is the chip's alias: the microcode for the
        // conditional jumps runs for the whole bit pattern 011?????, so 60h
        // runs JO, 61h runs JNO and so on (the guide's footnote 8). The
        // canonical opcodes are the 70h-7Fh row, so the alias is left out
        // and the assembler writes the opcodes people write.
        if (0x60..=0x6F).contains(&cell.opcode) {
            continue;
        }
        // The reference's own `RETF` is the guide's `RET (inter)`: the far
        // return, the one that pops the segment as well.
        let far_return = mnemonic.eq_ignore_ascii_case("RETF")
            && cell.mnemonic.eq_ignore_ascii_case("RET")
            && cell.operands.contains("(inter)");
        if names_match(cell.mnemonic, mnemonic)
            || alias_of(mnemonic) == Some(cell.mnemonic)
            || string_alias(&cell, mnemonic)
            || far_return
        {
            options.push((
                cell.opcode,
                cell.operands.to_string(),
                cell.operands.to_string(),
            ));
        }
    }
    for group in groups::forms() {
        if group
            .names()
            .any(|name| name.eq_ignore_ascii_case(mnemonic))
        {
            options.push((
                group.opcode,
                group.operands().to_string(),
                group.text.to_string(),
            ));
        }
    }
    options
}

/// Whether the source's qualifier asks for this form.
///
/// The guide's own cells carry the qualifiers: `JMP far` crosses to another
/// segment, `JMP short (intra)` stays in this one, and the group's cells
/// say the same thing - `CALL m16 (inter)` crosses, `CALL r16/m16 (intra)`
/// stays. A source that writes `FAR` wants the one that crosses.
fn qualifier_allows(qualifier: Option<&str>, form: &str) -> bool {
    let Some(qualifier) = qualifier else {
        return true;
    };
    match qualifier.to_ascii_uppercase().as_str() {
        "FAR" => form.contains("(inter)") || form.eq_ignore_ascii_case("far"),
        "NEAR" => form.contains("(intra)") || form.eq_ignore_ascii_case("near"),
        "SHORT" => form.to_ascii_uppercase().contains("SHORT"),
        _ => true,
    }
}

/// The cell a name is the other name of.
///
/// The guide's cells carry two names for one instruction - `JE/JZ` - and
/// the source may use either. The reference has a few more of its own:
/// `JC` is the carry jump and `JNC` is the one that is not.
fn alias_of(mnemonic: &str) -> Option<&'static str> {
    match mnemonic.to_ascii_uppercase().as_str() {
        "JZ" => Some("JE/JZ"),
        "JNZ" => Some("JNE/JNZ"),
        "JNAE" => Some("JB/JNAE"),
        "JNB" | "JAE" => Some("JNB/JAE"),
        "JC" => Some("JB/JNAE"),
        "JNC" => Some("JNB/JAE"),
        "JNBE" | "JA" => Some("JNBE/JA"),
        "JNA" | "JBE" => Some("JBE/JNA"),
        "JNG" | "JLE" => Some("JLE/JNG"),
        "JNGE" | "JL" => Some("JL/JNGE"),
        "JGE" | "JNL" => Some("JNL/JGE"),
        "JG" | "JNLE" => Some("JNLE/JG"),
        "JPE" => Some("JP/JPE"),
        "JPO" => Some("JNP/JPO"),
        "LOOPNZ" => Some("LOOPNE/LOOPNZ"),
        "LOOPZ" => Some("LOOPE/LOOPZ"),
        _ => None,
    }
}

/// Whether a name is the other name of a cell whose width is a separate
/// part.
///
/// The guide writes the string instructions with their width apart -
/// `MOVS b` and `MOVS w` - and the reference writes them as one word:
/// `MOVSB` and `MOVSW`.
fn string_alias(cell: &table::Form, mnemonic: &str) -> bool {
    let upper = mnemonic.to_ascii_uppercase();
    // The guide's `XLAT` has no width of its own, and the reference writes
    // it `XLATB`.
    if upper == "XLATB" {
        return cell.mnemonic.eq_ignore_ascii_case("XLAT");
    }
    let (stem, width) = match upper.strip_suffix('B') {
        Some(stem) => (stem, "b"),
        None => match upper.strip_suffix('W') {
            Some(stem) => (stem, "w"),
            None => return false,
        },
    };
    cell.mnemonic.eq_ignore_ascii_case(stem) && cell.operands.eq_ignore_ascii_case(width)
}

/// Whether a name is one a cell answers to.
///
/// The table's cells carry two names for the same instruction - `JE/JZ`,
/// `JB/JNAE` - and the source may use either.
fn names_match(cell: &str, mnemonic: &str) -> bool {
    cell.split('/')
        .any(|part| part.trim().eq_ignore_ascii_case(mnemonic))
}

/// Whether a form takes these operands.
///
/// A mnemonic can name many forms - ADD r/m8,r8, ADD r/m16,r16, ADD AL,i8 and
/// ADD AX,i16 are all ADD - and the operands say which one was meant.
fn form_takes(form: &str, operands: &[Operand]) -> bool {
    if form.is_empty() {
        return operands.is_empty();
    }
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    // The guide's qualifiers count for nothing: `RET (intra)` is a return
    // with no operand, and the `(intra)` is not one.
    let wanted = parts
        .iter()
        .filter(|part| !matches!(**part, "(intra)" | "(inter)" | "b" | "w"))
        .count();
    if wanted != operands.len() {
        return false;
    }
    parts
        .iter()
        .filter(|part| !matches!(**part, "(intra)" | "(inter)" | "b" | "w"))
        .zip(operands)
        .all(|(part, operand)| part_takes(part.trim(), operand))
}

/// Whether one part of a form takes one operand.
pub fn part_takes(part: &str, operand: &Operand) -> bool {
    // A part that names a register takes that register and nothing else:
    // the table's cells say `MOV CL,i8` for the register itself.
    if is_register(part) {
        return matches!(operand, Operand::Register(name) if name == part);
    }
    let is_segment = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "CS" | "DS" | "ES" | "SS"));
    // The REG field names all eight byte registers; the R/M field of the
    // 8086 lost its high half, and the 80186 gave it back.
    let is_byte = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "AL" | "CL" | "DL" | "BL" | "AH" | "CH" | "DH" | "BH"));
    let is_rm_byte = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "AL" | "CL" | "DL" | "BL" | "AH" | "CH" | "DH" | "BH"));
    let is_word = matches!(operand, Operand::Register(name) if matches!(name.as_str(), "AX" | "CX" | "DX" | "BX" | "SP" | "BP" | "SI" | "DI"));
    match part {
        // A register or a reference, of the width the part names.
        "r/m8" => match operand {
            Operand::Register(_) => is_rm_byte,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Word),
            _ => false,
        },
        "r/m16" => match operand {
            Operand::Register(_) => is_word,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Byte),
            _ => false,
        },
        // A reference and nothing else. The guide's `m8` and `m16` are a
        // direct address: the word that follows is the address itself.
        "m8" => matches!(
            operand,
            Operand::Memory(memory) if memory.width != Some(crate::modrm::Width::Word)
        ),
        "m16" => matches!(
            operand,
            Operand::Memory(memory) if memory.width != Some(crate::modrm::Width::Byte)
        ),
        "r8" => is_byte,
        "r16" => is_word,
        // The guide's `MEM16`: the manual prints it for the word forms of
        // the group, and the register mode of it is a register form.
        "MEM16" => match operand {
            Operand::Register(_) => is_word,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Byte),
            _ => false,
        },
        "i8" | "i16" | "1" => matches!(operand, Operand::Immediate(_)),
        // The interrupt three is the number three.
        "3" => matches!(operand, Operand::Immediate(3)),
        // A jump's target: a name, whose address the program knows, or a
        // number, which the program already folded.
        "short" | "near" | "short (intra)" => {
            matches!(
                operand,
                Operand::Relative(_) | Operand::Immediate(_) | Operand::Name(_)
            )
        }
        // A far pointer: the guide's `JMP far` and `CALL far` carry an
        // offset and a segment.
        "far" => matches!(operand, Operand::Far(_, _)),
        // The guide's own indirect calls and jumps: `CALL r16/m16 (intra)`
        // is a call inside the segment, and `CALL m16 (inter)` is one that
        // crosses to another.
        "r16/m16 (intra)" | "r/m16 (intra)" => match operand {
            Operand::Register(_) => is_word,
            Operand::Memory(memory) => memory.width != Some(crate::modrm::Width::Byte),
            _ => false,
        },
        "m16 (inter)" => matches!(
            operand,
            Operand::Memory(memory) if memory.width != Some(crate::modrm::Width::Byte)
        ),
        "sreg" => is_segment,
        // The guide's qualifiers: `RET (intra)` is a return with no operand,
        // and `RET i16 (intra)` is one with an immediate.
        "(intra)" | "(inter)" => false,
        _ => true,
    }
}

/// One operand, as the source wrote it.
///
/// A word that is neither a register, a number nor a reference is a name:
/// the program assembler knows which names it has.
pub fn operand(text: &str) -> Result<Operand, Error> {
    let text = text.trim();
    let upper = text.to_ascii_uppercase();
    // A reference: it has brackets, or it says PTR, or it names a segment.
    if text.contains('[') || upper.contains("PTR") || has_segment(text) {
        let reference = addressing::parse(text).map_err(|_| Error::BadOperand {
            text: text.to_string(),
        })?;
        return Ok(Operand::Memory(Memory {
            label: reference.address.label.clone(),
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
    // A name on its own is a name, and what it means depends on the form
    // the instruction takes: a jump's target, or the address of a label.
    if !upper.is_empty()
        && !upper.chars().all(|c| c.is_ascii_digit())
        && upper
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '@' || c == '.')
        && literals::parse(text).is_err()
    {
        return Ok(Operand::Name(text.to_string()));
    }
    // A number.
    match literals::parse(text) {
        Ok(value) => Ok(Operand::Immediate(value)),
        // A name: the program assembler knows which names it has, and a
        // name it does not have is a jump's target.
        Err(_) => Ok(Operand::Relative(0)),
    }
}

/// Whether a word is a name rather than a number.
#[must_use]
pub fn is_name(text: &str) -> bool {
    literals::parse(text).is_err()
}

/// Whether a reference names a segment: `CS:temp1`.
#[must_use]
pub fn has_segment(text: &str) -> bool {
    let upper = text.trim().to_ascii_uppercase();
    ["CS:", "DS:", "ES:", "SS:"]
        .iter()
        .any(|segment| upper.starts_with(segment))
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
