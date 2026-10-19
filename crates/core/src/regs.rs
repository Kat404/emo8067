//! The 8086 register file.

use crate::flags::Flags;

/// An 8-bit register, numbered as the instruction encoding numbers them.
///
/// `AH`, `CH`, `DH` and `BH` have no encoding in a ModR/M `reg` field — the
/// 8086 simply cannot name them there. [`Reg8::encodable`] is the gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg8 {
    Al = 0,
    Cl = 1,
    Dl = 2,
    Bl = 3,
    Ah = 4,
    Ch = 5,
    Dh = 6,
    Bh = 7,
}

impl Reg8 {
    /// Decode from the 3-bit field used by ModR/M and opcode bytes.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b111 {
            0 => Self::Al,
            1 => Self::Cl,
            2 => Self::Dl,
            3 => Self::Bl,
            4 => Self::Ah,
            5 => Self::Ch,
            6 => Self::Dh,
            _ => Self::Bh,
        }
    }

    /// Whether the 8086 can encode this register in a ModR/M `reg` field.
    #[must_use]
    pub const fn encodable(self) -> bool {
        (self as u8) < 4
    }

    /// Lowercase assembly name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"][self as usize]
    }
}

/// A 16-bit general register, in encoding order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg16 {
    Ax = 0,
    Cx = 1,
    Dx = 2,
    Bx = 3,
    Sp = 4,
    Bp = 5,
    Si = 6,
    Di = 7,
}

impl Reg16 {
    /// Decode the 3-bit REG or R/M field of an instruction to a 16-bit
    /// register, in the order the 8086's Table 4-9 gives.
    ///
    /// The order is `AX CX DX BX SP BP SI DI`, and it is the order this enum's
    /// discriminants run in, which is why the function exists at all: it is
    /// **not** the order the later processors use. Table 4-9 of the 1979
    /// manual prints two columns over the same eight slots:
    ///
    /// ```text
    ///     REG   8-bit   16-bit
    ///     000   AL      AX
    ///     001   CL      CX
    ///     010   DL      DX
    ///     011   BL      BX
    ///     100   AH      SP
    ///     101   CH      BP
    ///     110   DH      SI
    ///     111   BH      DI
    /// ```
    ///
    /// The 80186 gave the 8-bit field its high half back — `AH`, `CH`, `DH`
    /// and `BH` become reachable through it — and moved the 16-bit registers
    /// to `AX CX DX SP BP SI DI BX` to make room. Every reference that quotes
    /// the table today quotes the 80186's version, and so did this project
    /// until the hardware disagreed: the captures from a real P80C86A-2 say
    /// `011` is `BX` and `100` is `SP`, and the 1979 manual's own table says
    /// the same.
    ///
    /// Anything that wants the *discriminant* — the index into
    /// [`Registers::words`] — must not come here; index `words` directly.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b111 {
            0 => Self::Ax,
            1 => Self::Cx,
            2 => Self::Dx,
            3 => Self::Bx,
            4 => Self::Sp,
            5 => Self::Bp,
            6 => Self::Si,
            _ => Self::Di,
        }
    }

    /// Lowercase assembly name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"][self as usize]
    }
}

/// A segment register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Seg {
    Es = 0,
    Cs = 1,
    Ss = 2,
    Ds = 3,
}

impl Seg {
    /// Decode from the 3-bit field used by ModR/M and `PUSH`/`POP`.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Es,
            1 => Self::Cs,
            2 => Self::Ss,
            _ => Self::Ds,
        }
    }

    /// Lowercase assembly name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        ["es", "cs", "ss", "ds"][self as usize]
    }
}

/// The complete register file.
#[derive(Clone, Copy, Debug, Default)]
pub struct Registers {
    /// The eight 16-bit general registers, indexed by [`Reg16`].
    pub words: [u16; 8],
    /// The four segment registers, indexed by [`Seg`].
    pub segments: [u16; 4],
    /// `CS:IP`, the instruction pointer.
    pub ip: u16,
    /// FLAGS.
    pub flags: Flags,
}

impl Registers {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            words: [0; 8],
            segments: [0; 4],
            ip: 0,
            flags: Flags::new(),
        }
    }

    #[must_use]
    pub const fn word(&self, reg: Reg16) -> u16 {
        self.words[reg as usize]
    }

    pub const fn set_word(&mut self, reg: Reg16, value: u16) {
        self.words[reg as usize] = value;
    }

    /// Read an 8-bit register, folding `reg + 4` into the high half.
    ///
    /// The parent word is found by indexing `words` directly, because
    /// [`Reg8`]'s order (`AL CL DL BL AH CH DH BH`) matches the *discriminant*
    /// order of [`Reg16`] (`AX CX DX BX ...`), not the hardware REG field order
    /// that [`Reg16::from_bits`] decodes.
    #[must_use]
    pub const fn byte(&self, reg: Reg8) -> u8 {
        let idx = reg as usize;
        if idx < 4 {
            (self.words[idx] & 0x00FF) as u8
        } else {
            (self.words[idx - 4] >> 8) as u8
        }
    }

    /// Write an 8-bit register into either half of its parent word.
    pub fn set_byte(&mut self, reg: Reg8, value: u8) {
        let idx = reg as usize;
        let parent = idx & 0b11;
        let wide = self.words[parent];
        self.words[parent] = if idx < 4 {
            (wide & 0xFF00) | (value as u16)
        } else {
            (wide & 0x00FF) | ((value as u16) << 8)
        };
    }

    #[must_use]
    pub const fn segment(&self, seg: Seg) -> u16 {
        self.segments[seg as usize]
    }

    pub const fn set_segment(&mut self, seg: Seg, value: u16) {
        self.segments[seg as usize] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_halves_map_onto_the_right_words() {
        let mut r = Registers::new();
        r.set_byte(Reg8::Al, 0x11);
        r.set_byte(Reg8::Ah, 0x22);
        assert_eq!(r.word(Reg16::Ax), 0x2211);
        assert_eq!(r.byte(Reg8::Al), 0x11);
        assert_eq!(r.byte(Reg8::Ah), 0x22);

        // Writing one half must not disturb the other.
        r.set_byte(Reg8::Al, 0x33);
        assert_eq!(r.byte(Reg8::Ah), 0x22);
        r.set_byte(Reg8::Ah, 0x44);
        assert_eq!(r.byte(Reg8::Al), 0x33);
    }

    #[test]
    fn encoding_numbers_round_trip() {
        for bits in 0..8u8 {
            assert_eq!(Reg8::from_bits(bits) as u8, bits);
        }
        for bits in 0..4u8 {
            assert_eq!(Seg::from_bits(bits) as u8, bits);
        }
    }

    /// Intel Table 4-9, 16-bit column: `011` is `BX` and `100` is `SP`. This
    /// is the encoding that differs from the 8-bit one, and getting it wrong
    /// permutes five of the eight registers without any other test noticing.
    ///
    /// The 80186 moved the 16-bit registers to `AX CX DX SP BP SI DI BX` to
    /// make room for `AH CH DH BH` in the 8-bit field, and every modern
    /// reference quotes that later table. The 1979 manual's own table, and a
    /// real P80C86A-2, both say `BX` at `011`: the captures show `MOV SP,AX`
    /// as `8B E0` and `ADD CX,SP` as `01 E1`, and neither is possible under
    /// the later order.
    #[test]
    fn the_word_register_field_follows_intel_table_4_9() {
        let expected = [
            Reg16::Ax,
            Reg16::Cx,
            Reg16::Dx,
            Reg16::Bx,
            Reg16::Sp,
            Reg16::Bp,
            Reg16::Si,
            Reg16::Di,
        ];
        for (bits, want) in expected.iter().enumerate() {
            assert_eq!(Reg16::from_bits(bits as u8), *want, "field {bits:03b}");
        }
    }

    #[test]
    fn high_byte_registers_are_not_encodable() {
        for reg in [Reg8::Al, Reg8::Cl, Reg8::Dl, Reg8::Bl] {
            assert!(reg.encodable());
        }
        for reg in [Reg8::Ah, Reg8::Ch, Reg8::Dh, Reg8::Bh] {
            assert!(!reg.encodable());
        }
    }
}
