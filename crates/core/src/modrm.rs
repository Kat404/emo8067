//! ModR/M and effective-address decoding for the 8086.

use crate::regs::Seg;

/// The addressing mode selected by the low three bits of a ModR/M byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    BxSi,
    BxDi,
    BpSi,
    BpDi,
    Si,
    Di,
    Bp,
    Bx,
    /// `mod=00, rm=110`: a plain `disp16` with no index register.
    Direct,
}

/// One side of an instruction: either a register or a memory reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    /// A register, held as the raw 3-bit ModR/M `rm` field.
    ///
    /// The field alone cannot say whether that is a byte or a word
    /// register: encoding `011` names AL in an 8-bit instruction and AX in a
    /// 16-bit one, and the only thing that knows which is the width bit of
    /// the opcode. Resolving it here would silently misread every 16-bit
    /// register operand as its low byte.
    Reg(u8),
    Seg(Seg),
    /// A memory operand, already resolved to a segment and a 16-bit offset.
    Mem {
        seg: Seg,
        base: Base,
        disp: u16,
    },
}

/// Decoded ModR/M byte: its `reg` field and its `rm` operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModRm {
    pub reg: u8,
    pub rm: Operand,
    /// The raw two-bit `mod` field. An instruction that only exists for
    /// `mod=00` has to test this, because `rm` alone cannot say: every
    /// `mod` value produces an `Operand::Mem` for most `rm` values.
    pub mod_bits: u8,
}

/// Something the decoder could not make sense of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Ran off the end of the available opcode bytes.
    Truncated,
    /// `mod=00, rm=110` is the direct form; every other `rm=110` is the
    /// 8088's invalid `(disp16)` alias, which needs a 186 or later.
    Unsupported,
}

/// Decode one ModR/M byte from `code`, advancing `ip` past any displacement.
///
/// `default_seg` is the segment a memory operand uses when the instruction
/// does not carry a segment-override prefix.
pub fn decode_modrm(code: &[u8], ip: &mut u16, default_seg: Seg) -> Result<ModRm, DecodeError> {
    let byte = *code.get(*ip as usize).ok_or(DecodeError::Truncated)?;
    *ip = ip.wrapping_add(1);

    let mode = byte >> 6;
    let reg = (byte >> 3) & 0b111;
    let rm = byte & 0b111;

    if mode == 0b11 {
        return Ok(ModRm {
            reg,
            mod_bits: mode,
            rm: Operand::Reg(rm),
        });
    }

    let (base, disp) = match (mode, rm) {
        // mod=00: no displacement, except for the direct form.
        (0b00, 0) => (Base::BxSi, 0),
        (0b00, 1) => (Base::BxDi, 0),
        (0b00, 2) => (Base::BpSi, 0),
        (0b00, 3) => (Base::BpDi, 0),
        (0b00, 4) => (Base::Si, 0),
        (0b00, 5) => (Base::Di, 0),
        (0b00, 6) => (Base::Direct, take_u16(code, ip)?),
        (0b00, 7) => (Base::Bx, 0),
        // mod=01: disp8, sign-extended to 16 bits.
        (0b01, r) => (base_of(r), take_disp8(code, ip)?),
        // mod=10: disp16.
        (0b10, r) => (base_of(r), take_u16(code, ip)?),
        _ => unreachable!(),
    };

    Ok(ModRm {
        reg,
        mod_bits: mode,
        rm: Operand::Mem {
            seg: default_seg,
            base,
            disp,
        },
    })
}

const fn base_of(rm: u8) -> Base {
    match rm {
        0 => Base::BxSi,
        1 => Base::BxDi,
        2 => Base::BpSi,
        3 => Base::BpDi,
        4 => Base::Si,
        5 => Base::Di,
        6 => Base::Bp,
        _ => Base::Bx,
    }
}

fn take_u16(code: &[u8], ip: &mut u16) -> Result<u16, DecodeError> {
    let at = *ip as usize;
    let lo = *code.get(at).ok_or(DecodeError::Truncated)?;
    let hi = *code.get(at + 1).ok_or(DecodeError::Truncated)?;
    *ip = ip.wrapping_add(2);
    Ok(u16::from_le_bytes([lo, hi]))
}

fn take_disp8(code: &[u8], ip: &mut u16) -> Result<u16, DecodeError> {
    let raw = *code.get(*ip as usize).ok_or(DecodeError::Truncated)?;
    *ip = ip.wrapping_add(1);
    Ok(i16::from(raw as i8) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode a single ModR/M byte that needs no displacement.
    fn rm(byte: u8) -> Operand {
        let code = [byte, 0x00, 0x00];
        let mut ip = 0;
        decode_modrm(&code, &mut ip, Seg::Ds).unwrap().rm
    }

    #[test]
    fn mod00_covers_all_eight_8086_addressing_modes() {
        // No displacement anywhere except the direct form.
        let expected = [
            Base::BxSi,
            Base::BxDi,
            Base::BpSi,
            Base::BpDi,
            Base::Si,
            Base::Di,
            Base::Direct,
            Base::Bx,
        ];
        for (i, base) in expected.into_iter().enumerate() {
            let Operand::Mem {
                base: got, disp, ..
            } = rm(i as u8)
            else {
                panic!("rm={i} should be a memory operand");
            };
            assert_eq!(got, base, "rm={i}");
            // Only the direct form carries a displacement, and the helper
            // feeds it zeros, so every displacement here reads as zero.
            assert_eq!(disp, 0);
            assert_eq!(base, got);
        }
    }

    #[test]
    fn direct_form_consumes_a_displacement() {
        let code = [0x06, 0x34, 0x12]; // mod=00 rm=110, disp16 = 0x1234
        let mut ip = 0;
        let decoded = decode_modrm(&code, &mut ip, Seg::Ds).unwrap();
        assert_eq!(
            decoded.rm,
            Operand::Mem {
                seg: Seg::Ds,
                base: Base::Direct,
                disp: 0x1234
            }
        );
        assert_eq!(ip, 3);
    }

    #[test]
    fn disp8_is_sign_extended() {
        // mod=01 rm=101 -> [DI+disp8]; disp8 = 0xFE is -2 -> 0xFFFE.
        let code = [0b01_000_101u8, 0xFE];
        let mut ip = 0;
        let decoded = decode_modrm(&code, &mut ip, Seg::Ds).unwrap();
        assert_eq!(
            decoded.rm,
            Operand::Mem {
                seg: Seg::Ds,
                base: Base::Di,
                disp: 0xFFFE
            }
        );
    }

    #[test]
    fn register_mode_keeps_the_raw_field() {
        // The width is not decidable from the ModR/M byte alone, so the
        // decoder must not guess: `011` is AL in one instruction and AX in
        // another, and only the caller knows which.
        assert_eq!(rm(0b11_000_011), Operand::Reg(0b011));
        assert_eq!(rm(0b11_000_100), Operand::Reg(0b100));
        assert_eq!(rm(0b11_000_110), Operand::Reg(0b110));
    }

    #[test]
    fn truncated_input_is_an_error_not_a_panic() {
        let mut ip = 0;
        assert_eq!(
            decode_modrm(&[], &mut ip, Seg::Ds),
            Err(DecodeError::Truncated)
        );
        let code = [0b00_000_110u8, 0x01]; // direct form missing its high byte
        let mut ip = 0;
        assert_eq!(
            decode_modrm(&code, &mut ip, Seg::Ds),
            Err(DecodeError::Truncated)
        );
    }
}
