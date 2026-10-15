//! The 8086 physical address space.
//!
//! The 8086 has a 20-bit physical bus: 1 MiB addressable, formed by
//! `physical = (segment << 4) + offset`, with the sum truncated to 20 bits.
//! That truncation is not an edge case to be normalised away — it is the
//! documented behaviour, and the reason the 640 KiB DOS limit exists.

/// Physical address space of the 8086: 2^20 bytes.
pub const ADDR_SPACE: usize = 1 << 20;

/// Mask for the 20-bit physical address bus.
const BUS_MASK: u32 = (ADDR_SPACE as u32) - 1;

/// A flat view of the 8086's physical memory.
#[derive(Clone)]
pub struct Memory {
    bytes: Box<[u8; ADDR_SPACE]>,
}

impl std::fmt::Debug for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Memory")
            .field("len", &ADDR_SPACE)
            .finish_non_exhaustive()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    /// A zero-filled address space.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: Box::new([0; ADDR_SPACE]),
        }
    }

    /// Translate a logical `segment:offset` pair onto the physical bus.
    ///
    /// Addition is done in `u32` and masked to 20 bits, so `FFFF:0010`
    /// wraps to physical `0x00000`, exactly like real silicon.
    #[must_use]
    pub fn physical(segment: u16, offset: u16) -> u32 {
        ((u32::from(segment) << 4) + u32::from(offset)) & BUS_MASK
    }

    /// Read one byte through a `segment:offset` pair.
    #[must_use]
    pub fn read_u8(&self, segment: u16, offset: u16) -> u8 {
        self.bytes[Self::physical(segment, offset) as usize]
    }

    /// Write one byte through a `segment:offset` pair.
    pub fn write_u8(&mut self, segment: u16, offset: u16, value: u8) {
        self.bytes[Self::physical(segment, offset) as usize] = value;
    }

    /// Read a little-endian word through a `segment:offset` pair.
    ///
    /// The offset wraps modulo 2^16 inside the segment, so the high byte of
    /// a word at `FFFFh` comes from offset `0000h` of the same segment. The
    /// 8086 does not carry from the offset into the segment base; that was
    /// the mistake corrected here, after an earlier revision assumed it did.
    #[must_use]
    pub fn read_u16(&self, segment: u16, offset: u16) -> u16 {
        let low = Self::physical(segment, offset);
        let high = Self::physical(segment, offset.wrapping_add(1));
        u16::from_le_bytes([self.bytes[low as usize], self.bytes[high as usize]])
    }

    /// Write a little-endian word through a `segment:offset` pair.
    ///
    /// The high byte lands at the wrapped offset, exactly where
    /// [`Memory::read_u16`] would read it from.
    pub fn write_u16(&mut self, segment: u16, offset: u16, value: u16) {
        let low = Self::physical(segment, offset);
        let high = Self::physical(segment, offset.wrapping_add(1));
        let [lo, hi] = value.to_le_bytes();
        self.bytes[low as usize] = lo;
        self.bytes[high as usize] = hi;
    }

    /// Bulk load, honouring the same wrap as [`Memory::physical`].
    ///
    /// # Errors
    /// Returns `Err` with the offset of the first byte that would have
    /// wrapped past the top of the address space.
    pub fn load_at(&mut self, physical: u32, data: &[u8]) -> Result<(), u32> {
        let start = physical & BUS_MASK;
        if start as usize + data.len() > ADDR_SPACE {
            return Err(start);
        }
        self.bytes[start as usize..start as usize + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read `len` contiguous physical bytes.
    #[must_use]
    pub fn slice(&self, physical: u32, len: usize) -> &[u8] {
        let start = (physical & BUS_MASK) as usize;
        &self.bytes[start..start + len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_segment_offset_pairs() {
        assert_eq!(Memory::physical(0x0000, 0x1234), 0x01234);
        assert_eq!(Memory::physical(0x0100, 0x0000), 0x01000);
        assert_eq!(Memory::physical(0x0800, 0x0000), 0x08000);
    }

    #[test]
    fn wraps_around_the_top_of_the_bus() {
        // FFFF:0010 overflows 20 bits and wraps to the bottom.
        assert_eq!(Memory::physical(0xFFFF, 0x0010), 0x00000);
        assert_eq!(Memory::physical(0xFFFF, 0xFFFF), 0x0FFEF);
        // A segment base plus an offset can cross the top of the bus and wrap
        // back to the bottom: FFFF:2000 = FFFF0 + 2000 = 101FF0 -> 01FF0.
        assert_eq!(Memory::physical(0xFFFF, 0x2000), 0x01FF0);
        // But a base that stays under 1 MiB does not wrap at all.
        assert_eq!(Memory::physical(0x0100, 0xFFFF), 0x10FFF);
    }

    #[test]
    fn reads_and_writes_through_logical_addresses() {
        let mut m = Memory::new();
        m.write_u8(0x0100, 0x0000, 0xAB);
        assert_eq!(m.read_u8(0x0100, 0x0000), 0xAB);
        // Same byte, different logical address: 0x0200:0000 is not 0x0100:0000.
        assert_eq!(m.read_u8(0x0200, 0x0000), 0x00);
    }

    #[test]
    fn a_word_wraps_the_offset_inside_the_segment() {
        // FFFF:000F is physical 0xFFFFF, the last byte on the bus. The high
        // byte of the word comes from offset 0010h of the same segment, which
        // is physical 0x100000 truncated to 0x00000. The 8086 does not carry
        // from the offset into the segment base.
        let mut m = Memory::new();
        m.write_u16(0xFFFF, 0x000F, 0xBEEF);
        assert_eq!(m.bytes[0xFFFFF], 0xEF, "low byte at the top of the bus");
        assert_eq!(m.read_u8(0x0000, 0x0000), 0xBE, "high byte wrapped to 0");
        assert_eq!(m.read_u16(0xFFFF, 0x000F), 0xBEEF);
    }

    #[test]
    fn the_high_byte_reads_the_segment_base_not_the_next_byte() {
        // SS:FFFFh has physical (SS<<4)+FFFFh. Model A would take the next
        // physical byte; the chip takes offset 0000h of the same segment.
        let mut m = Memory::new();
        let physical = Memory::physical(0x2000, 0xFFFF); // 0x2FFFF
        m.bytes[physical as usize] = 0xEF;
        m.bytes[Memory::physical(0x2000, 0x0000) as usize] = 0xBE; // 0x20000
        m.bytes[((physical + 1) & 0xFFFFF) as usize] = 0x11; // what the wrong model reads

        assert_eq!(
            m.read_u16(0x2000, 0xFFFF),
            0xBEEF,
            "segment base, not physical+1"
        );
        assert_eq!(
            m.bytes[((physical + 1) & 0xFFFFF) as usize],
            0x11,
            "the byte the wrong model would have read is untouched"
        );
    }

    #[test]
    fn load_refuses_to_spill_past_the_bus() {
        let mut m = Memory::new();
        assert!(m.load_at(0, &[1, 2, 3]).is_ok());
        assert_eq!(m.slice(0, 3), &[1, 2, 3]);
        assert!(m.load_at(0xFFFF0, &[0; 32]).is_err());
    }
}
