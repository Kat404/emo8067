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
    /// A name the program gives a value to.
    Name(String),
    /// A far pointer: an offset and a segment, as `0FFFFh:0000h`.
    Far(u16, u16),
}

/// A memory reference, in the parts the 8086's addressing needs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Memory {
    /// The segment the reference is in, when the source named one.
    pub segment: Option<String>,
    /// A name the program gives a value to, as in `num1[bx]`.
    pub label: Option<String>,
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
            label: self.label.clone(),
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
    /// A conditional jump whose displacement does not fit in a byte, in the
    /// reference's own words.
    OutOfRange { message: &'static str },
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
    encode_inner(opcode, mnemonic, form, operands, None, address, true)
}

/// The same, with the range of a short jump's displacement left unchecked.
pub fn encode_at(
    opcode: u8,
    mnemonic: &str,
    form: &str,
    operands: &[Operand],
    address: u16,
    check_range: bool,
) -> Result<Vec<u8>, Error> {
    encode_inner(opcode, mnemonic, form, operands, None, address, check_range)
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
    check_range: bool,
) -> Result<Vec<u8>, Error> {
    if let Some(group) = crate::groups::find_text(opcode, mnemonic, form) {
        // A group is chosen by the operation the source asked for: ADD in
        // Group 1 is the digit whose cell says ADD. The form says which
        // cell it is, so the digit is known.
        return encode_inner(
            opcode,
            mnemonic,
            group.operands(),
            operands,
            Some(group.digit),
            address,
            check_range,
        );
    }
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    // The guide's qualifiers count for nothing: `RET (intra)` is a return
    // with no operand, and the `(intra)` is not one.
    let parts: Vec<&str> = parts
        .into_iter()
        .filter(|part| !matches!(*part, "(intra)" | "(inter)" | "b" | "w"))
        .collect();
    // A cell with no form at all is an instruction with no operands:
    // `PUSHF`, `CLD`, `HLT`.
    let parts: Vec<&str> = if parts.len() == 1 && parts[0].is_empty() {
        Vec::new()
    } else {
        parts
    };
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
        // An instruction with no operands: the opcode is the whole of it.
        [] => {}
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
                    let rm = width.rm_field_of(name).ok_or_else(|| Error::Type {
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
                    // The R/M field of the 8086 reaches only four of the
                    // eight byte registers.
                    let rm = width.rm_field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: 0,
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
                    return Err(Error::Unsupported {
                        form: form.to_string(),
                    });
                }
            };
            // The group's digit is the operation, and it goes in the REG
            // field; the table's own `MOV m8,i8` has no digit and its REG
            // field is zero.
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
            if check_range && width == 1 && !(-128..=127).contains(&displacement) {
                // The reference's own words for a jump that is out of range.
                return Err(Error::OutOfRange {
                    message: "conditional jump is out of range: allowed values are from -128 to 127",
                });
            }
            // The opcode is already the first byte.
            out.extend_from_slice(&(displacement as i16).to_le_bytes()[..width]);
        }
        // A far pointer: the offset first, then the segment.
        ["far"] => {
            let Operand::Far(offset, segment) = operands[0] else {
                return Err(Error::Type {
                    form: form.to_string(),
                    position: 0,
                });
            };
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&segment.to_le_bytes());
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
        // A shift: the count is one or the count register, and the
        // operation is in the digit.
        ["r/m8", "1"] | ["r/m16", "1"] | ["r/m8", "CL"] | ["r/m16", "CL"] => {
            let width = if parts[0].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let reg = digit.ok_or(Error::Unsupported {
                form: form.to_string(),
            })?;
            let (modrm, rm, displacement) = match &operands[0] {
                Operand::Register(name) => {
                    let rm = width.rm_field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: 0,
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
                        position: 0,
                    });
                }
            };
            out.push(byte_for(modrm, reg, rm));
            out.extend(displacement);
            // The count: one, or the count register.
            if parts[1] == "CL" {
                out.push(
                    0xC0 | width.field_of("CL").ok_or(Error::Type {
                        form: form.to_string(),
                        position: 1,
                    })?,
                );
            }
        }
        // A named register and an immediate: the opcode carries the
        // register, so `MOV CL,i8` is B1h with the byte that follows.
        [register, imm] if is_register_name(register) && (*imm == "i8" || *imm == "i16") => {
            if !operand_names(&operands[0], register) {
                return Err(Error::Type {
                    form: form.to_string(),
                    position: 0,
                });
            }
            let width = if *imm == "i8" {
                Width::Byte
            } else {
                Width::Word
            };
            let value = immediate(&operands[1], width)?;
            out.extend_from_slice(&value.to_le_bytes()[..if width == Width::Byte { 1 } else { 2 }]);
        }
        // A direct address: the opcode and a word of address, with no ModR/M
        // byte at all - the guide's `m8` and `m16`.
        ["AL", "m8"] | ["AX", "m16"] | ["m8", "AL"] | ["m16", "AX"] => {
            let address = direct_address(&operands[0]).ok_or(Error::Type {
                form: form.to_string(),
                position: 0,
            })?;
            out.extend_from_slice(&address.to_le_bytes());
        }
        // A direct address, the way the reference writes a label's address
        // in place of a register: `mov m, al`.
        ["r8", "m8"] | ["r16", "m16"] | ["m8", "r8"] | ["m16", "r16"] => {
            let (address_part, other_part) = if parts[0].starts_with("m") {
                (0, 1)
            } else {
                (1, 0)
            };
            let address = direct_address(&operands[address_part]).ok_or(Error::Type {
                form: form.to_string(),
                position: address_part,
            })?;
            let width = if parts[other_part].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let other = match &operands[other_part] {
                Operand::Register(name) => {
                    let rm = width.rm_field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: other_part,
                    })?;
                    (Mod::Register, rm, Vec::<u8>::new())
                }
                _ => {
                    return Err(Error::Type {
                        form: form.to_string(),
                        position: other_part,
                    });
                }
            };
            out.push(byte_for(other.0, 0, other.1));
            out.extend(other.2);
            out.extend_from_slice(&address.to_le_bytes());
        }
        // An immediate and a register, or a register and a port: the opcode
        // carries both.
        ["i8", "AL"] | ["i8", "AX"] => {
            let value = immediate(&operands[0], Width::Byte)?;
            out.extend_from_slice(&value.to_le_bytes()[..1]);
        }
        [_, _] if is_register_name(parts[0]) && is_register_name(parts[1]) => {
            // `IN AL,DX` and `OUT DX,AL`: the opcode carries both.
            let _ = &operands[0];
        }
        // A register or a segment on its own: the opcode carries it.
        [only] if is_register_name(only) || is_segment_name(only) => {
            if !operand_names(&operands[0], only) {
                return Err(Error::Type {
                    form: form.to_string(),
                    position: 0,
                });
            }
        }
        // A return, near or far, with or without an immediate.
        ["(intra)"] | ["(inter)"] => {}
        ["i16 (intra)"] | ["i16 (inter)"] => {
            let value = immediate(&operands[0], Width::Word)?;
            out.extend_from_slice(&value.to_le_bytes());
        }
        // One operand in the R/M field: the group puts the operation in the
        // REG field and the operand in the R/M field. `INC AL` is FE C0h
        // and `MUL BL` is F6 E3h. The guide's own indirect calls and jumps
        // are here too: `CALL m16 (inter)` is FF /3.
        ["r/m8"]
        | ["r/m16"]
        | ["r16/m16"]
        | ["MEM16"]
        | ["r16/m16 (intra)"]
        | ["r/m16 (intra)"]
        | ["m16 (inter)"]
        | ["m8"]
        | ["m16"] => {
            let width = if parts[0].ends_with('8') {
                Width::Byte
            } else {
                Width::Word
            };
            let (modrm, rm, displacement) = match &operands[0] {
                Operand::Register(name) => {
                    let rm = width.rm_field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: 0,
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
                        position: 0,
                    });
                }
            };
            out.push(byte_for(modrm, digit.unwrap_or(0), rm));
            out.extend(displacement);
        }
        // A segment register and a memory operand, either way round: the REG
        // field carries the segment.
        ["r/m16", "sreg"] | ["r16/m16", "sreg"] | ["sreg", "r/m16"] | ["sreg", "r16/m16"] => {
            let (segment_part, memory_part) = if matches!(parts[0], "sreg") {
                (0, 1)
            } else {
                (1, 0)
            };
            let segment = segment_field(&operands[segment_part]).ok_or(Error::Type {
                form: form.to_string(),
                position: segment_part,
            })?;
            let (modrm, rm, displacement) = match &operands[memory_part] {
                Operand::Register(name) => {
                    let rm = Width::Word.rm_field_of(name).ok_or(Error::Type {
                        form: form.to_string(),
                        position: memory_part,
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
                        position: memory_part,
                    });
                }
            };
            out.push(byte_for(modrm, segment, rm));
            out.extend(displacement);
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

/// Whether a word names a segment register.
fn is_segment_name(word: &str) -> bool {
    matches!(word, "ES" | "CS" | "SS" | "DS")
}

/// Whether the operand is the register or segment the form names.
fn operand_names(operand: &Operand, name: &str) -> bool {
    matches!(operand, Operand::Register(other) if other == name)
}

/// The field value of a segment register: `00` ES, `01` CS, `10` SS, `11` DS.
fn segment_field(operand: &Operand) -> Option<u8> {
    let Operand::Register(name) = operand else {
        return None;
    };
    match name.as_str() {
        "ES" => Some(0),
        "CS" => Some(1),
        "SS" => Some(2),
        "DS" => Some(3),
        _ => None,
    }
}

/// The address of a direct reference, when the operand names one.
fn direct_address(operand: &Operand) -> Option<u16> {
    match operand {
        Operand::Immediate(value) => u16::try_from(*value).ok(),
        Operand::Memory(memory) => {
            if memory.base.is_some() || memory.index.is_some() {
                return None;
            }
            memory
                .displacement
                .and_then(|value| u16::try_from(value).ok())
        }
        _ => None,
    }
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
///
/// A negative value is two's complement: `-5` is `FBh` in a byte and
/// `FFF3h` in a word.
fn immediate(operand: &Operand, width: Width) -> Result<i64, Error> {
    let Operand::Immediate(value) = operand else {
        return Err(Error::Type {
            form: "immediate".to_string(),
            position: 0,
        });
    };
    let (low, high) = if width == Width::Byte {
        (-0x80, 0xFF)
    } else {
        (-0x8000, 0xFFFF)
    };
    if *value < low || *value > high {
        return Err(Error::Type {
            form: format!("a value that fits in {high:#x}"),
            position: 0,
        });
    }
    Ok(*value)
}
