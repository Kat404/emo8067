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

/// The 8-bit registers the REG field names.
///
/// All eight: the 8086's 8-bit field has its high half, and `AH`, `CH`, `DH`
/// and `BH` are reachable through it. The R/M field is the one that lost
/// its high half, and the 80186 gave it back — see [`RM8`].
pub const REG8: [&str; 8] = ["AL", "CL", "DL", "BL", "AH", "CH", "DH", "BH"];

/// The 8-bit registers the R/M field names.
///
/// The guide's Table 4-10 prints the register mode of the 8-bit column as
/// `AL CL DL BL AH CH DH BH` — the whole 8-bit field, high half and all —
/// and the reference's own examples use the high half there: `mov al, ah`
/// is one of them, and `xor ah, ah` is another.
pub const RM8: [&str; 8] = ["AL", "CL", "DL", "BL", "AH", "CH", "DH", "BH"];

/// The width of an operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    /// Eight bits.
    Byte,
    /// Sixteen bits.
    Word,
}

impl Width {
    /// The registers the REG field can name at this width.
    #[must_use]
    pub fn registers(self) -> &'static [&'static str] {
        match self {
            Width::Byte => &REG8,
            Width::Word => &REG16,
        }
    }

    /// The registers the R/M field can name at this width.
    ///
    /// The 8-bit R/M field of the 8086 lost its high half to the 80186.
    #[must_use]
    pub fn rm_registers(self) -> &'static [&'static str] {
        match self {
            Width::Byte => &RM8,
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

    /// The R/M field's value for a register, if it has one at this width.
    #[must_use]
    pub fn rm_field_of(self, name: &str) -> Option<u8> {
        self.rm_registers()
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
