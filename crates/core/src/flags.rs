//! The FLAGS register.
//!
//! Bit layout of the 8086:
//!
//! ```text
//!  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!   -  -  -  -  OF DF IF TF SF ZF -  AF -  PF 1 CF
//! ```
//!
//! Bits 1 and 3 are not writable: bit 1 always reads back as 1 and bit 3
//! always reads back as 0. Bits 12..=15 are undefined on an 8086 and read
//! back as 1 through `POPF`; the 8086 has no 8086-flag source that ever
//! writes them.

/// Carry — unsigned overflow out of the top bit.
pub const CF: u16 = 1 << 0;
/// Fixed 1. Not writable.
pub const ONE: u16 = 1 << 1;
/// Parity — 1 when the low byte has an even number of set bits.
pub const PF: u16 = 1 << 2;
/// Auxiliary carry — carry from bit 3 to bit 4 (BCD).
pub const AF: u16 = 1 << 4;
/// Zero — result is zero.
pub const ZF: u16 = 1 << 6;
/// Sign — copy of the top bit of the result.
pub const SF: u16 = 1 << 7;
/// Trap — single-step.
pub const TF: u16 = 1 << 8;
/// Interrupt enable.
pub const IF: u16 = 1 << 9;
/// Direction — 1 decrements `SI`/`DI`, 0 increments.
pub const DF: u16 = 1 << 10;
/// Overflow — signed overflow.
pub const OF: u16 = 1 << 11;

/// Bits 12..=15, which are undefined on an 8086.
pub const UNDEFINED: u16 = 0xF000;

/// Every bit an arithmetic or logical instruction is allowed to write.
pub const STATUS_MASK: u16 = CF | PF | AF | ZF | SF | OF;
/// Status bits plus the two the string and scalar instructions touch.
pub const WRITABLE_MASK: u16 = STATUS_MASK | DF;

/// Even-parity bit for a low byte: set when the byte has an even popcount.
#[must_use]
pub fn parity_even(byte: u8) -> bool {
    byte.count_ones() % 2 == 0
}

/// The FLAGS register.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags(pub u16);

impl Flags {
    /// The power-on state: interrupts enabled, everything else clear.
    #[must_use]
    pub const fn new() -> Self {
        Self(IF)
    }

    #[must_use]
    pub const fn get(self, mask: u16) -> bool {
        self.0 & mask != 0
    }

    /// Set or clear `mask` according to `value`.
    pub const fn set(&mut self, mask: u16, value: bool) {
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Apply a full status result in one go.
    pub const fn apply_status(
        &mut self,
        cf: bool,
        pf: bool,
        af: bool,
        zf: bool,
        sf: bool,
        of: bool,
    ) {
        self.set(CF, cf);
        self.set(PF, pf);
        self.set(AF, af);
        self.set(ZF, zf);
        self.set(SF, sf);
        self.set(OF, of);
    }

    /// Clear the six arithmetic status bits, as `CLC`-style pre-clears do.
    pub const fn clear_status(&mut self) {
        self.0 &= !WRITABLE_MASK;
    }

    /// The value as `PUSHF` pushes it: undefined bits read as 1.
    #[must_use]
    pub const fn pushed(self) -> u16 {
        self.0 | UNDEFINED
    }

    /// Consume a value popped by `POPF`, honouring the non-writable bits.
    #[must_use]
    pub const fn popped(word: u16) -> Self {
        // Bit 1 is fixed at 1, bit 3 fixed at 0, bits 12..=15 ignored.
        Self((word & 0x0FD5) | ONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_interrupts_enabled_and_nothing_else() {
        let f = Flags::new();
        assert!(f.get(IF));
        for bit in [CF, PF, AF, ZF, SF, OF, TF, DF] {
            assert!(!f.get(bit), "{bit:#06x} should start clear");
        }
    }

    #[test]
    fn parity_is_even_not_odd() {
        // 0b1010_0000 has two bits set -> even -> PF set.
        assert!(parity_even(0xA0));
        // 0b1111_1111 has eight -> even -> PF set.
        assert!(parity_even(0xFF));
        // 0b0000_0001 has one set bit -> odd -> PF clear.
        assert!(!parity_even(0x01));
        // Zero set bits is an even count too, so PF is set for a zero result.
        assert!(parity_even(0x00));
    }

    #[test]
    fn fixed_bits_survive_a_pop() {
        // Bit 1 cannot be cleared, bit 3 cannot be set.
        let f = Flags::popped(0x0000);
        assert!(f.get(ONE));
        assert_eq!(f.0 & 0x0008, 0);

        let f = Flags::popped(0xFFFF);
        assert!(f.get(ONE));
        assert_eq!(f.0 & 0x0008, 0);
        assert!(f.get(CF) && f.get(PF) && f.get(OF) && f.get(DF));
    }

    #[test]
    fn push_reads_the_undefined_bits_as_one() {
        let f = Flags(0);
        assert_eq!(f.pushed(), 0xF000);
        let f = Flags(CF);
        assert_eq!(f.pushed(), 0xF001);
    }
}
