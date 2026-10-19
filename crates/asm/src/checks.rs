//! The type checks, in the reference's own words.
//!
//! The reference refused some operand pairs with a message of its own, and
//! those messages are the specification of what it refused. Two of them say
//! the same thing from the two sides - an eight-bit register against a
//! sixteen-bit address - and which one comes out depends on which operand
//! came first.

use crate::operands::Operand;

/// What kind of thing an operand is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// An eight-bit register.
    Register8,
    /// A sixteen-bit register.
    Register16,
    /// A segment register.
    Segment,
    /// The instruction pointer.
    Ip,
    /// An address whose width the source did not say.
    Address,
    /// An eight-bit address.
    Address8,
    /// A sixteen-bit address.
    Address16,
    /// An immediate value.
    Immediate,
}

/// The kind an operand is.
#[must_use]
pub fn kind_of(operand: &Operand) -> Kind {
    match operand {
        Operand::Register(name) => match name.as_str() {
            "AL" | "CL" | "DL" | "BL" | "AH" | "CH" | "DH" | "BH" => Kind::Register8,
            "AX" | "CX" | "DX" | "BX" | "SP" | "BP" | "SI" | "DI" => Kind::Register16,
            "CS" | "DS" | "ES" | "SS" => Kind::Segment,
            "IP" => Kind::Ip,
            _ => Kind::Register16,
        },
        Operand::Immediate(_) => Kind::Immediate,
        Operand::Memory(memory) => match memory.width {
            Some(crate::modrm::Width::Byte) => Kind::Address8,
            Some(crate::modrm::Width::Word) => Kind::Address16,
            None => Kind::Address,
        },
        Operand::Relative(_) => Kind::Immediate,
    }
}

/// The reference's own refusal: its message, word for word.
pub type Error = &'static str;

/// Check the operands against the form that would encode them.
///
/// The form is the document's own, so the check knows what it wanted: an
/// eight-bit register, a sixteen-bit address, an immediate, a segment.
pub fn check(form: &str, operands: &[Operand]) -> Result<(), Error> {
    let parts: Vec<&str> = form.split(',').map(str::trim).collect();
    if parts.len() != operands.len() {
        return Ok(());
    }
    // The first operand of a form that does not take one.
    if !matches!(parts[0], "i8" | "i16" | "1") && kind_of(&operands[0]) == Kind::Immediate {
        return Err("first operand cannot be an immediate value");
    }
    // The second operand, when the form wants eight bits of it.
    if parts[1] == "i8" {
        if let Operand::Immediate(value) = operands[1] {
            if value > 0xFF {
                return Err("second operand is over 8 bits!");
            }
        }
    }
    for (index, (part, operand)) in parts.iter().zip(operands).enumerate() {
        check_part(part, operand, index)?;
    }
    Ok(())
}

/// The checks that need no form: they look at the pair of operands alone.
///
/// These are the ones whose answer does not depend on which instruction the
/// source meant, so they are the same whatever form is being looked at.
pub fn check_pair(operands: &[Operand]) -> Result<(), Error> {
    // Two segment registers.
    if operands
        .iter()
        .filter(|o| kind_of(o) == Kind::Segment)
        .count()
        > 1
    {
        return Err("segment registers cannot go together!");
    }
    // Memory to memory.
    if operands.len() > 1 && operands.iter().all(|o| matches!(o, Operand::Memory(_))) {
        return Err("memory to memory is not allowed for 8086:");
    }
    // A segment register with an immediate value.
    if operands.iter().any(|o| kind_of(o) == Kind::Segment)
        && operands.iter().any(|o| kind_of(o) == Kind::Immediate)
    {
        return Err("cannot use segment register with an immediate value");
    }
    // The instruction pointer: the reference would not write it and would
    // not read it either.
    for (index, operand) in operands.iter().enumerate() {
        if kind_of(operand) == Kind::Ip {
            return Err(ip_message(index));
        }
    }
    // CS is the one segment that cannot be written to at all.
    if let Some(Operand::Register(name)) = operands.first() {
        if name == "CS" {
            return Err("CS cannot be modified directly (use far JMP or CALL)");
        }
    }
    Ok(())
}

/// Check one operand against one part of a form.
///
/// The two wordings of a mismatch - an eight-bit register against a
/// sixteen-bit address - belong to the position: the first operand says it
/// one way and the second says it the other.
fn check_part(part: &str, operand: &Operand, index: usize) -> Result<(), Error> {
    let kind = kind_of(operand);
    match part {
        // A segment register's own form.
        "sreg" => Ok(()),
        // An immediate.
        "i8" | "i16" | "1" => match kind {
            Kind::Segment => Err("cannot use segment register with an immediate value"),
            _ => Ok(()),
        },
        // Eight bits.
        "r8" | "r/m8" | "m8" | "AL" => match kind {
            Kind::Register8 | Kind::Address8 | Kind::Address => Ok(()),
            Kind::Register16 => Err(if index == 0 {
                "16 bit and 8 bit register"
            } else {
                "8 bit and 16 bit register"
            }),
            Kind::Address16 => Err(if index == 0 {
                "16 bit register and 8 bit address"
            } else {
                "8 bit address and 16 bit register"
            }),
            Kind::Segment => Err(segment_message(operand, index)),
            Kind::Ip => Err(ip_message(index)),
            Kind::Immediate => Ok(()),
        },
        // Sixteen bits.
        "r16" | "r/m16" | "m16" | "AX" => match kind {
            Kind::Register16 | Kind::Address16 | Kind::Address => Ok(()),
            Kind::Register8 => Err(if index == 0 {
                "8 bit and 16 bit register"
            } else {
                "16 bit and 8 bit register"
            }),
            Kind::Address8 => Err(if index == 0 {
                "8 bit register and 16 bit address (add 'b.' before ...)"
            } else {
                "16 bit address and 8 bit register (add 'b.' before ...)"
            }),
            Kind::Segment => Err(segment_message(operand, index)),
            Kind::Ip => Err(ip_message(index)),
            Kind::Immediate => Ok(()),
        },
        _ => Ok(()),
    }
}

/// What the reference says about a segment register in the wrong place.
fn segment_message(operand: &Operand, index: usize) -> Error {
    // CS is the one segment that cannot be written to at all.
    if index == 0 && matches!(operand, Operand::Register(name) if name == "CS") {
        return "CS cannot be modified directly (use far JMP or CALL)";
    }
    "wrong use of segment register"
}

/// What the reference says about the instruction pointer.
fn ip_message(index: usize) -> Error {
    if index == 0 {
        "IP cannot be modified directly (use JMP or CALL)"
    } else {
        "cannot get value of IP register (use POP after CALL)"
    }
}
