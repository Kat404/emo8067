//! The ModR/M byte, and the fields that go into it.
//!
//! The 8086 names the 16-bit registers in the order `AX CX DX BX SP BP SI DI`
//! — the order of Table 4-9 of the 1979 manual, and the order the hardware
//! uses. The 80186 moved them to `AX CX DX SP BP SI DI BX` to make room for
//! `AH CH DH BH` in the 8-bit field, and every modern reference quotes that
//! later table. The captures from a real P80C86A-2 say otherwise, and so
//! does the manual: `MOV SP,AX` is `8B E0`, which is only possible under the
//! 8086's order.

/// The 16-bit registers, in the order the REG and R/M fields name them.
pub const REG16: [&str; 8] = ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"];

/// The 8-bit registers the REG and R/M fields can name.
///
/// The high four are not there: on the 8086 the 8-bit field cannot reach
/// `AH`, `CH`, `DH` or `BH`. A register operand names them, through the
/// opcode's own low bits — `MOV AH,1` is `B4 01` — but a ModR/M field does
/// not.
pub const REG8: [&str; 4] = ["AL", "CL", "DL", "BL"];

/// The width of an operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    /// Eight bits.
    Byte,
    /// Sixteen bits.
    Word,
}

impl Width {
    /// The registers the field can name at this width.
    #[must_use]
    pub fn registers(self) -> &'static [&'static str] {
        match self {
            Width::Byte => &REG8,
            Width::Word => &REG16,
        }
    }

    /// The field's value for a register, if it has one at this width.
    #[must_use]
    pub fn field_of(self, name: &str) -> Option<u8> {
        self.registers()
            .iter()
            .position(|r| *r == name)
            .map(|i| i as u8)
    }
}

/// The MOD field: how the operand is addressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mod {
    /// Memory, with no displacement.
    Memory,
    /// Memory, with an 8-bit displacement.
    Byte,
    /// Memory, with a 16-bit displacement.
    Word,
    /// A register, not memory.
    Register,
}

impl Mod {
    /// The MOD field's value.
    #[must_use]
    pub fn value(self) -> u8 {
        match self {
            Mod::Memory => 0b00,
            Mod::Byte => 0b01,
            Mod::Word => 0b10,
            Mod::Register => 0b11,
        }
    }
}

/// Build a ModR/M byte.
///
/// The REG field is the second operand for most instructions and the first
/// for the ones that read it, and the R/M field is the other; the caller
/// says which is which, because the instruction says so, not the field.
#[must_use]
pub fn byte_for(modrm: Mod, reg: u8, rm: u8) -> u8 {
    (modrm.value() << 6) | ((reg & 0b111) << 3) | (rm & 0b111)
}
