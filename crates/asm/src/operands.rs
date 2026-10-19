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
    /// A relative jump's target, by name.
    Relative(String),
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
}

impl From<Memory> for Operand {
    fn from(memory: Memory) -> Self {
        Operand::Memory(memory)
    }
}

impl Memory {
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
) -> Result<Vec<u8>, Error> {
    encode_inner(opcode, mnemonic, form, operands, None)
}

/// The encoding, with the REG field pinned when a group's digit chose the
/// form.
fn encode_inner(
    opcode: u8,
    mnemonic: &str,
    form: &str,
    operands: &[Operand],
    digit: Option<u8>,
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
            let rm = match &operands[rm_part] {
                Operand::Register(name) => width.field_of(name).ok_or_else(|| Error::Type {
                    form: form.to_string(),
                    position: rm_part,
                })?,
                Operand::Memory(_) => {
                    // The addressing modes are the next task.
                    return Err(Error::Unsupported {
                        form: form.to_string(),
                    });
                }
                _ => {
                    return Err(Error::Type {
                        form: form.to_string(),
                        position: rm_part,
                    });
                }
            };
            out.push(byte_for(Mod::Register, reg, rm));
        }
        // A group's form: the digit is in the REG field and the operand
        // follows it.
        ["r/m8", "i8"] | ["r/m16", "i16"] => {
            let width = if parts[0].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let reg = digit.ok_or(Error::Unsupported {
                form: form.to_string(),
            })?;
            let rm = match &operands[0] {
                Operand::Register(name) => width.field_of(name).ok_or(Error::Type {
                    form: form.to_string(),
                    position: 0,
                })?,
                _ => {
                    return Err(Error::Unsupported {
                        form: form.to_string(),
                    });
                }
            };
            out.push(byte_for(Mod::Register, reg, rm));
            let value = immediate(&operands[1], width)?;
            out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
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
