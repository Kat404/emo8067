//! The disassembler: machine code in, the reference's own text out.
//!
//! The mnemonic comes from the guide's §1.3 - the same table the assembler
//! reads - and the operands are printed in the formats the reference's own
//! disassembler uses: a leading zero, two-digit bytes and a lowercase `h`
//! for every hex value, a logical address as `SSSS:OOOO`, and the
//! eight-bit form as `0%Xh`.
//!
//! The reference's own effective-address table gives the sixteen-bit R/M
//! field its order, and the guide's table gives every form the operands it
//! takes, so the text the disassembler prints is the text the assembler
//! reads back.

use crate::groups;
use crate::modrm::{Mod, REG8, REG16, RM8, Width};
use crate::table::{self, Form};
use crate::x87;
use std::sync::OnceLock;

/// The reference's own words for an opcode the 8086 instruction set does
/// not define.
pub const UNDEFINED: &str = "no such opcode in 8086 instruction set";

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The opcode is not in the 8086 instruction set, in the reference's
    /// own words.
    Undefined { message: &'static str },
    /// The bytes run out before the instruction does.
    Truncated,
    /// The form is one the disassembler does not know.
    Unsupported { form: String },
}

/// One decoded instruction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Instruction {
    /// The instruction's text, in the reference's own formats.
    pub text: String,
    /// How many bytes the instruction takes.
    pub length: usize,
}

/// Decode the instruction at the start of the bytes.
///
/// The address is where the instruction is, because a jump's target is
/// relative to the end of the instruction and the text prints the target
/// as an absolute address.
pub fn disassemble(bytes: &[u8], address: u16) -> Result<Instruction, Error> {
    // The BIOS device interrupt: the FFFF prefix in front of CD is the
    // marker, and the CD that follows is the interrupt itself.
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xFF && bytes[2] == 0xCD {
        return Ok(Instruction {
            text: "BIOS DI".to_string(),
            length: 4,
        });
    }
    let mut cursor = Cursor::new(bytes);
    let mut prefixes = Vec::new();
    while let Some(prefix) = cursor.prefix() {
        prefixes.push(prefix);
    }
    let opcode = cursor.byte()?;
    let cell = cell_of(opcode).ok_or(Error::Undefined { message: UNDEFINED })?;
    let (mnemonic, operands) = decode(&mut cursor, opcode, cell, address)?;
    let mut text = String::new();
    for prefix in prefixes {
        text.push_str(prefix);
        text.push(' ');
    }
    text.push_str(&mnemonic);
    if !operands.is_empty() {
        text.push(' ');
        text.push_str(&operands.join(", "));
    }
    Ok(Instruction {
        text,
        length: cursor.at,
    })
}

/// Decode every instruction in the bytes, stopping at the first one that
/// cannot be decoded.
#[must_use]
pub fn disassemble_all(bytes: &[u8], address: u16) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let Ok(instruction) = disassemble(&bytes[at..], address.wrapping_add(at as u16)) else {
            break;
        };
        at += instruction.length;
        out.push(instruction);
    }
    out
}

/// The instruction the opcode names, if the 8086 instruction set has one.
///
/// The ESC range is not one of them: the 8086 has no instruction there,
/// and the x87 table answers for it.
#[must_use]
pub fn instruction_of(opcode: u8) -> Option<&'static Form> {
    let cell = cell_of(opcode)?;
    if cell.mnemonic.starts_with("ESC") {
        return None;
    }
    Some(cell)
}

/// The cell the guide's table gives the opcode.
fn cell_of(opcode: u8) -> Option<&'static Form> {
    static MAP: OnceLock<Vec<Form>> = OnceLock::new();
    MAP.get_or_init(table::forms)
        .iter()
        .find(|form| form.opcode == opcode)
}

/// The mnemonic and the operands of one instruction.
fn decode(
    cursor: &mut Cursor<'_>,
    opcode: u8,
    cell: &Form,
    address: u16,
) -> Result<(String, Vec<String>), Error> {
    // The ESC range is the coprocessor's, and the reference shows its
    // mnemonics for display only.
    if cell.mnemonic.starts_with("ESC") {
        return x87_text(cursor, opcode);
    }
    // A group cell names the operation rather than the form: the ModR/M
    // byte's digit says which one it is.
    if cell.mnemonic.starts_with("Group") || cell.mnemonic == "Shift" {
        let modrm = cursor.modrm()?;
        let Some(group) = groups::find_digit(opcode, modrm.reg) else {
            return Err(Error::Undefined { message: UNDEFINED });
        };
        let (name, form) = group.mnemonic_form();
        let operands = operands(cursor, form, Some(modrm), address)?;
        return Ok((mnemonic_of(name, form), operands));
    }
    let operands = operands(cursor, cell.operands, None, address)?;
    Ok((mnemonic_of(cell.mnemonic, cell.operands), operands))
}

/// The mnemonic the instruction is written with.
///
/// The guide's cells carry two names for one instruction - `JE/JZ` - and
/// the reference's own names for a few of them differ from the guide's.
fn mnemonic_of(name: &str, form: &str) -> String {
    let first = name.split('/').next().unwrap_or(name).trim();
    match (first, form) {
        // The string instructions carry their width apart - `MOVS b` - and
        // the reference writes them as one word.
        ("MOVS", "b") | ("CMPS", "b") | ("STOS", "b") | ("LODS", "b") | ("SCAS", "b") => {
            format!("{first}B")
        }
        ("MOVS", "w") | ("CMPS", "w") | ("STOS", "w") | ("LODS", "w") | ("SCAS", "w") => {
            format!("{first}W")
        }
        // The far return is the reference's `RETF`.
        ("RET", "(inter)") | ("RET", "i16 (inter)") => "RETF".to_string(),
        // The guide's `XLAT` has no width of its own, and the reference
        // writes it `XLATB`.
        ("XLAT", "") => "XLATB".to_string(),
        _ => first.to_string(),
    }
}

/// The operands of a form, in the order the form gives them.
fn operands(
    cursor: &mut Cursor<'_>,
    form: &str,
    modrm: Option<Modrm>,
    address: u16,
) -> Result<Vec<String>, Error> {
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    // The guide's qualifiers count for nothing: `RET (intra)` is a return
    // with no operand, and the `(intra)` is not one.
    let bases: Vec<&str> = parts.iter().map(|part| base_of(part)).collect();
    // The count of a shift is the CL of the `r/m8,CL` form; a CL anywhere
    // else is a register of its own.
    let count = bases.len() == 2 && bases[1] == "CL" && matches!(bases[0], "r/m8" | "r/m16");
    let mut kinds = Vec::with_capacity(bases.len());
    for part in &bases {
        let Some(kind) = part_of(part, is_direct(&bases), count) else {
            return Err(Error::Unsupported {
                form: form.to_string(),
            });
        };
        kinds.push(kind);
    }
    // The ModR/M byte comes before the immediate, and the displacement
    // with it.
    let modrm = match modrm {
        Some(modrm) => Some(modrm),
        None if kinds.iter().any(|kind| matches!(kind, Part::Rm(_))) => Some(cursor.modrm()?),
        None => None,
    };
    // The immediate follows the ModR/M byte, and the immediates are in the
    // order the form gives them.
    let mut immediates = Vec::new();
    for kind in &kinds {
        match kind {
            Part::Immediate(width) => immediates.push(match width {
                Width::Byte => u16::from(cursor.byte()?),
                Width::Word => cursor.word()?,
            }),
            // The count of a shift by CL is a byte of its own, after the
            // ModR/M byte.
            Part::Count => {
                cursor.byte()?;
            }
            _ => {}
        }
    }
    let modrm = modrm.as_ref();
    let mut out = Vec::new();
    let mut immediate = immediates.into_iter();
    for kind in &kinds {
        out.push(match kind {
            Part::None => continue,
            Part::Fixed(name) => (*name).to_string(),
            Part::Literal(text) => (*text).to_string(),
            Part::Reg(width) => register(*width, modrm.expect("the REG field")).to_string(),
            Part::Segment => segment(modrm.expect("the REG field")).to_string(),
            Part::Rm(width) => {
                let modrm = modrm.expect("the R/M field");
                if modrm.mod_ == Mod::Register {
                    rm_register(*width, modrm.rm).to_string()
                } else {
                    memory(modrm, *width, size_is_known(&kinds))
                }
            }
            Part::Immediate(width) => {
                immediate_text(*width, immediate.next().expect("the immediate"))
            }
            Part::Count => "CL".to_string(),
            Part::Direct => {
                let address = cursor.word()?;
                format!("[{}]", word(address))
            }
            Part::Target(width) => {
                let displacement = match width {
                    Width::Byte => i32::from(cursor.byte()? as i8),
                    Width::Word => i32::from(cursor.word()? as i16),
                };
                // The target is where the jump goes, and the displacement
                // is measured from the end of the instruction.
                let end = i32::from(address) + 1 + i32::from(width_bytes(*width));
                let target = (end + displacement) as u16;
                // The short jump says so: the near jump is the one the
                // assembler writes, and the two are told apart by the
                // qualifier the reference's own dialect has.
                if *width == Width::Byte {
                    format!("SHORT 0{target:04X}h")
                } else {
                    format!("0{target:04X}h")
                }
            }
            Part::Far => {
                let offset = cursor.word()?;
                let segment = cursor.word()?;
                format!("{}:{}", word(segment), word(offset))
            }
        });
    }
    Ok(out)
}

/// The part of a form without its qualifier.
fn base_of(part: &str) -> &str {
    part.split_once(' ').map_or(part, |(base, _)| base).trim()
}

/// How many bytes a width takes.
fn width_bytes(width: Width) -> u8 {
    match width {
        Width::Byte => 1,
        Width::Word => 2,
    }
}

/// Whether the form's `m8` or `m16` is a direct address rather than a
/// reference through the R/M field.
fn is_direct(parts: &[&str]) -> bool {
    parts
        .iter()
        .filter(|part| matches!(**part, "m8" | "m16"))
        .count()
        == 1
        && parts.iter().any(|part| matches!(*part, "AL" | "AX"))
}

/// Whether the form's own operands say how wide the reference is.
///
/// The `MOV r/m16,r16` form has a register that says, and the group's
/// `ADD r/m8,i8` has not: the reference is then printed with its size, so
/// that the assembler reads back the same width.
fn size_is_known(kinds: &[Part]) -> bool {
    kinds.iter().any(|kind| matches!(kind, Part::Reg(_)))
}

/// One operand of a form.
enum Part {
    /// The form takes no operand here.
    None,
    /// A register the form names: `AL`, `ES`.
    Fixed(&'static str),
    /// A literal: `1`, `CL`, `3`.
    Literal(&'static str),
    /// The REG field.
    Reg(Width),
    /// The R/M field.
    Rm(Width),
    /// A segment register, in the REG field.
    Segment,
    /// An immediate.
    Immediate(Width),
    /// The count of a shift by CL: the byte that carries it.
    Count,
    /// A direct address: the word that follows the opcode.
    Direct,
    /// The target of a jump.
    Target(Width),
    /// A far pointer: an offset and a segment.
    Far,
}

/// What one part of a form is, if the disassembler knows it.
fn part_of(text: &str, direct: bool, count: bool) -> Option<Part> {
    Some(match text {
        "" | "(intra)" | "(inter)" | "b" | "w" => Part::None,
        "r8" => Part::Reg(Width::Byte),
        "r16" => Part::Reg(Width::Word),
        "r/m8" => Part::Rm(Width::Byte),
        "r/m16" | "r16/m16" | "MEM16" => Part::Rm(Width::Word),
        "m8" => {
            if direct {
                Part::Direct
            } else {
                Part::Rm(Width::Byte)
            }
        }
        "m16" => {
            if direct {
                Part::Direct
            } else {
                Part::Rm(Width::Word)
            }
        }
        "sreg" => Part::Segment,
        "i8" | "i8:sx" => Part::Immediate(Width::Byte),
        "i16" => Part::Immediate(Width::Word),
        "short" => Part::Target(Width::Byte),
        "near" => Part::Target(Width::Word),
        "far" => Part::Far,
        "1" => Part::Literal("1"),
        "CL" if count => Part::Count,
        "3" => Part::Literal("3"),
        name => Part::Fixed(register_name(name)?),
    })
}

/// The name of a register, if the word is one.
fn register_name(text: &str) -> Option<&'static str> {
    Some(match text {
        "AL" => "AL",
        "CL" => "CL",
        "DL" => "DL",
        "BL" => "BL",
        "AH" => "AH",
        "CH" => "CH",
        "DH" => "DH",
        "BH" => "BH",
        "AX" => "AX",
        "CX" => "CX",
        "DX" => "DX",
        "BX" => "BX",
        "SP" => "SP",
        "BP" => "BP",
        "SI" => "SI",
        "DI" => "DI",
        "ES" => "ES",
        "CS" => "CS",
        "SS" => "SS",
        "DS" => "DS",
        _ => return None,
    })
}

/// The register the REG field names.
fn register(width: Width, modrm: &Modrm) -> &'static str {
    match width {
        Width::Byte => REG8[modrm.reg as usize],
        Width::Word => REG16[modrm.reg as usize],
    }
}

/// The segment register the REG field names.
fn segment(modrm: &Modrm) -> &'static str {
    match modrm.reg {
        0 => "ES",
        1 => "CS",
        2 => "SS",
        _ => "DS",
    }
}

/// The register the R/M field names.
fn rm_register(width: Width, rm: u8) -> &'static str {
    match width {
        Width::Byte => RM8[rm as usize],
        Width::Word => REG16[rm as usize],
    }
}

/// A memory reference, in the reference's own bracket form.
///
/// The reference's effective-address table gives the sixteen-bit R/M field
/// its order, and the MOD field says how much displacement follows.
fn memory(modrm: &Modrm, width: Width, size_known: bool) -> String {
    let mut out = String::new();
    if !size_known {
        out.push_str(match width {
            Width::Byte => "BYTE PTR ",
            Width::Word => "WORD PTR ",
        });
    }
    out.push('[');
    // The direct form: the R/M field says 110 and the word that follows is
    // the address itself.
    if modrm.mod_ == Mod::Memory && modrm.rm == 0b110 {
        let address = u16::from_le_bytes([modrm.displacement[0], modrm.displacement[1]]);
        out.push_str(&word(address));
        out.push(']');
        return out;
    }
    out.push_str(EA[modrm.rm as usize]);
    if let Some(displacement) = displacement_of(modrm) {
        out.push_str(&displacement);
    }
    out.push(']');
    out
}

/// The displacement of a reference, in the house style.
fn displacement_of(modrm: &Modrm) -> Option<String> {
    match modrm.mod_ {
        Mod::Memory => None,
        Mod::Byte => Some(displacement_text(
            i32::from(modrm.displacement[0] as i8),
            true,
        )),
        Mod::Word => {
            let bytes = [modrm.displacement[0], modrm.displacement[1]];
            Some(displacement_text(
                i32::from(i16::from_le_bytes(bytes)),
                false,
            ))
        }
        Mod::Register => None,
    }
}

/// A displacement in the house style.
///
/// The magnitude is the reference's own form and the sign comes in front of
/// it, so that the assembler reads the displacement back as it was.
fn displacement_text(value: i32, byte: bool) -> String {
    let magnitude = value.unsigned_abs();
    let text = if byte {
        format!("0{magnitude:X}h")
    } else {
        word(magnitude as u16)
    };
    if value < 0 {
        format!("-{text}")
    } else {
        format!("+{text}")
    }
}

/// The sixteen-bit effective addresses, in the R/M field's order.
const EA: [&str; 8] = ["BX+SI", "BX+DI", "BP+SI", "BP+DI", "SI", "DI", "BP", "BX"];

/// A word in the house style: `0%02X%02Xh`.
fn word(value: u16) -> String {
    format!("0{:02X}{:02X}h", value >> 8, value & 0xFF)
}

/// An immediate in the house style: the eight-bit form is `0%Xh`.
fn immediate_text(width: Width, value: u16) -> String {
    match width {
        Width::Byte => format!("0{value:X}h"),
        Width::Word => word(value),
    }
}

/// The x87 text, for display only.
fn x87_text(cursor: &mut Cursor<'_>, opcode: u8) -> Result<(String, Vec<String>), Error> {
    let modrm = cursor.modrm()?;
    let (operation, target) = x87::decode(opcode, modrm.byte);
    let mnemonic = match operation {
        x87::Operation::Named(name) => name.to_string(),
        x87::Operation::UnknownF => "F???".to_string(),
        x87::Operation::Unknown => "???".to_string(),
    };
    let operand = match target {
        x87::Target::None => None,
        x87::Target::Stack(index) => Some(format!("st{index}")),
        x87::Target::Memory => Some(memory(&modrm, Width::Word, true)),
    };
    Ok((mnemonic, operand.into_iter().collect()))
}

/// The ModR/M byte and the displacement it brings.
struct Modrm {
    /// The byte itself.
    byte: u8,
    /// The MOD field.
    mod_: Mod,
    /// The REG field.
    reg: u8,
    /// The R/M field.
    rm: u8,
    /// The displacement bytes.
    displacement: Vec<u8>,
}

/// The bytes, and how far they have been read.
struct Cursor<'a> {
    /// The bytes.
    bytes: &'a [u8],
    /// How many have been read.
    at: usize,
}

impl<'a> Cursor<'a> {
    /// The cursor over the bytes.
    fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, at: 0 }
    }

    /// The next byte.
    fn byte(&mut self) -> Result<u8, Error> {
        let byte = *self.bytes.get(self.at).ok_or(Error::Truncated)?;
        self.at += 1;
        Ok(byte)
    }

    /// The next word, little endian.
    fn word(&mut self) -> Result<u16, Error> {
        let low = self.byte()?;
        let high = self.byte()?;
        Ok(u16::from_le_bytes([low, high]))
    }

    /// The prefix the byte is, if it is one.
    fn prefix(&mut self) -> Option<&'static str> {
        let prefix = prefix_of(*self.bytes.get(self.at)?);
        if prefix.is_some() {
            self.at += 1;
        }
        prefix
    }

    /// The ModR/M byte and the displacement it brings.
    fn modrm(&mut self) -> Result<Modrm, Error> {
        let byte = self.byte()?;
        let mod_ = match byte >> 6 {
            0b00 => Mod::Memory,
            0b01 => Mod::Byte,
            0b10 => Mod::Word,
            _ => Mod::Register,
        };
        let displacement = match mod_ {
            // The direct form: the R/M field says 110 and the word that
            // follows is the address itself.
            Mod::Memory if byte & 0b111 == 0b110 => self.word()?.to_le_bytes().to_vec(),
            Mod::Memory => Vec::new(),
            Mod::Byte => vec![self.byte()?],
            Mod::Word => self.word()?.to_le_bytes().to_vec(),
            Mod::Register => Vec::new(),
        };
        Ok(Modrm {
            byte,
            mod_,
            reg: (byte >> 3) & 0b111,
            rm: byte & 0b111,
            displacement,
        })
    }
}

/// The prefix a byte is, if it is one.
///
/// The guide's own map gives the prefixes their bytes.
fn prefix_of(byte: u8) -> Option<&'static str> {
    Some(match byte {
        0x26 => "ES:",
        0x2E => "CS:",
        0x36 => "SS:",
        0x3E => "DS:",
        0xF0 | 0xF1 => "LOCK",
        0xF2 => "REPNE",
        0xF3 => "REP",
        _ => return None,
    })
}
