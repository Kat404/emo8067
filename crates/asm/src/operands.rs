//! Operands, and the forms the document's table gives them.

use crate::modrm::{Mod, Width, byte_for};

/// One operand of a source line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Operand {
    /// A register, by name: `AX`, `CL`, `CS`.
    Register(String),
    /// An immediate, by value.
    Immediate(i64),
    /// A memory reference, by its parts.
    Memory(Memory),
    /// A relative jump's target, by address.
    Relative(i64),
}

/// A memory reference, in the parts the 8086's addressing needs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Memory {
    /// The segment the reference is in, when the source named one.
    pub segment: Option<String>,
    /// The base register, when the source named one.
    pub base: Option<String>,
    /// The index register, when the source named one.
    pub index: Option<String>,
    /// The displacement, when the source gave one.
    pub displacement: Option<i64>,
    /// The width the source asked for, when it said: `BYTE PTR` or `WORD PTR`.
    pub width: Option<Width>,
}

impl From<Memory> for Operand {
    fn from(memory: Memory) -> Self {
        Operand::Memory(memory)
    }
}

impl Memory {
    /// The MOD, the R/M and the displacement bytes this reference needs.
    pub fn encode(&self) -> Result<(crate::modrm::Mod, u8, Vec<u8>), crate::addressing::Error> {
        let address = crate::addressing::Address {
            base: self.base.as_deref().and_then(crate::addressing::register),
            index: self.index.as_deref().and_then(crate::addressing::register),
            displacement: self.displacement,
        };
        crate::addressing::encode(&address)
    }

    /// Whether the reference names no register at all, which is the direct
    /// form: `[1234h]`.
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.base.is_none() && self.index.is_none() && self.displacement.is_some()
    }
}

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The source gave the wrong number of operands.
    Count { form: String, got: usize },
    /// The source gave an operand the form does not take.
    Type { form: String, position: usize },
    /// The form is one the assembler cannot encode yet.
    Unsupported { form: String },
    /// The addressing does not work.
    Addressing { form: String, reason: String },
}

/// Encode one instruction: the form from the document's table, and the
/// operands the source gave.
///
/// The form is the document's own: `r/m8,r8`, `AL,i8`, `r16,r/m16`. Which
/// operand goes in the `REG` field and which in the `R/M` field is the
/// instruction's business, not the field's, so it is decided here.
pub fn encode(
    opcode: u8,
    mnemonic: &str,
    form: &str,
    operands: &[Operand],
    address: u16,
) -> Result<Vec<u8>, Error> {
    encode_inner(opcode, mnemonic, form, operands, None, address)
}

/// The encoding, with the REG field pinned when a group's digit chose the
/// form.
fn encode_inner(
    opcode: u8,
    mnemonic: &str,
    form: &str,
    operands: &[Operand],
    digit: Option<u8>,
    address: u16,
) -> Result<Vec<u8>, Error> {
    if form.starts_with("Group") || form.starts_with("Shift") {
        // A group is chosen by the operation the source asked for: ADD in
        // Group 1 is the digit whose cell says ADD.
        let group = crate::groups::find(opcode, mnemonic).ok_or(Error::Unsupported {
            form: form.to_string(),
        })?;
        return encode_inner(
            opcode,
            mnemonic,
            group.operands(),
            operands,
            Some(group.digit),
            address,
        );
    }
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    if parts.len() != operands.len() {
        return Err(Error::Count {
            form: form.to_string(),
            got: operands.len(),
        });
    }
    let mut out = vec![opcode];
    // A named register and an immediate: the opcode carries the register,
    // so `MOV CX,i16` is the `B9` opcode with the word that follows.
    if parts.len() == 2 && (parts[1] == "i8" || parts[1] == "i16") && is_register_name(parts[0]) {
        let width = if parts[1] == "i8" {
            Width::Byte
        } else {
            Width::Word
        };
        let value = immediate(&operands[1], width)?;
        out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
        return Ok(out);
    }
    match parts.as_slice() {
        // A register against a register: the first is the REG field and the
        // second the R/M field.
        ["r/m8", "r8"] | ["r8", "r/m8"] | ["r/m16", "r16"] | ["r16", "r/m16"] => {
            let (reg_part, rm_part) = if parts[0].starts_with("r/m") {
                (1, 0)
            } else {
                (0, 1)
            };
            let width = if parts[0].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let reg = match digit {
                Some(digit) => digit,
                None => register(&operands[reg_part], width)?,
            };
            // The R/M field is a register or a memory reference, and a
            // memory reference brings its own MOD and its displacement.
            let (modrm, rm, displacement) = match &operands[rm_part] {
                Operand::Register(name) => {
                    let rm = width.field_of(name).ok_or_else(|| Error::Type {
                        form: form.to_string(),
                        position: rm_part,
                    })?;
                    (Mod::Register, rm, Vec::new())
                }
                Operand::Memory(memory) => {
                    let (modrm, rm, displacement) =
                        memory.encode().map_err(|error| Error::Addressing {
                            form: form.to_string(),
                            reason: format!("{error:?}"),
                        })?;
                    (modrm, rm, displacement)
                }
                _ => {
                    return Err(Error::Type {
                        form: form.to_string(),
                        position: rm_part,
                    });
                }
            };
            out.push(byte_for(modrm, reg, rm));
            out.extend(displacement);
        }
        // An immediate against a register or a reference: the group's form,
        // where the digit is in the REG field, and the `MOV r/m,i` form,
        // where the digit is ignored and the REG field is zero.
        ["r/m8", "i8"] | ["r/m16", "i16"] | ["m8", "i8"] | ["m16", "i16"] => {
            let width = if parts[0].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let (modrm, rm, displacement) = match &operands[0] {
                Operand::Register(name) => {
                    let rm = width.field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: 0,
                    })?;
                    (Mod::Register, rm, Vec::new())
                }
                Operand::Memory(memory) if digit.is_none() => {
                    let (modrm, rm, displacement) =
                        memory.encode().map_err(|error| Error::Addressing {
                            form: form.to_string(),
                            reason: format!("{error:?}"),
                        })?;
                    (modrm, rm, displacement)
                }
                _ => {
                    return Err(Error::Unsupported {
                        form: form.to_string(),
                    });
                }
            };
            out.push(byte_for(modrm, digit.unwrap_or(0), rm));
            out.extend(displacement);
            let value = immediate(&operands[1], width)?;
            out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
        }
        // A jump to a label: the displacement is where the target is, less
        // where the instruction ends.
        ["short"] | ["short (intra)"] | ["near"] => {
            let width = if parts[0] == "near" { 2 } else { 1 };
            let target = match operands[0] {
                Operand::Relative(target) => target,
                Operand::Immediate(target) => target,
                _ => {
                    return Err(Error::Type {
                        form: form.to_string(),
                        position: 0,
                    });
                }
            };
            let end = i64::from(address) + 1 + width as i64;
            let displacement = target - end;
            if width == 1 && !(-128..=127).contains(&displacement) {
                return Err(Error::Type {
                    form: "a displacement that fits in a byte".to_string(),
                    position: 0,
                });
            }
            // The opcode is already the first byte.
            out.extend_from_slice(&(displacement as i16).to_le_bytes()[..width]);
        }
        // An immediate against the accumulator.
        ["AL", "i8"] | ["AX", "i16"] => {
            let width = if parts[0] == "AL" {
                Width::Byte
            } else {
                Width::Word
            };
            let value = immediate(&operands[1], width)?;
            out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
        }
        // An immediate on its own: the accumulator forms of the ALU groups.
        ["i8"] | ["i16"] => {
            let width = if parts[0] == "i8" {
                Width::Byte
            } else {
                Width::Word
            };
            let value = immediate(&operands[0], width)?;
            out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
        }
        _ => {
            return Err(Error::Unsupported {
                form: form.to_string(),
            });
        }
    }
    Ok(out)
}

/// Whether a word names a register, of any width.
fn is_register_name(word: &str) -> bool {
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
    )
}

/// The field value of a register operand.
fn register(operand: &Operand, width: Width) -> Result<u8, Error> {
    match operand {
        Operand::Register(name) => width.field_of(name).ok_or(Error::Type {
            form: width.registers().join(","),
            position: 0,
        }),
        _ => Err(Error::Type {
            form: width.registers().join(","),
            position: 0,
        }),
    }
}

/// The bytes of an immediate operand, at the width the form asks for.
fn immediate(operand: &Operand, width: Width) -> Result<i64, Error> {
    let Operand::Immediate(value) = operand else {
        return Err(Error::Type {
            form: "immediate".to_string(),
            position: 0,
        });
    };
    let limit = if width == Width::Byte { 0xFF } else { 0xFFFF };
    if *value < 0 || *value > limit {
        return Err(Error::Type {
            form: format!("a value that fits in {limit:#x}"),
            position: 0,
        });
    }
    Ok(*value)
}
