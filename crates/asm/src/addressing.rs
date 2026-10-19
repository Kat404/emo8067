//! The addressing modes, and the bytes they need.
//!
//! Table 4-10 of the manual: the R/M field names the base and the index,
//! and the MOD field says how much displacement follows. The eight forms
//! are the ones the manual lists, and the two the reference refused - a base
//! of BX with an index of BP, and SI with DI - are refused here too, with
//! the reference's complaint.

use crate::modrm::Mod;

/// A register a memory reference can name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reg {
    /// BX.
    Bx,
    /// SP.
    Sp,
    /// BP.
    Bp,
    /// SI.
    Si,
    /// DI.
    Di,
}

impl Reg {
    /// The name the source wrote.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Reg::Bx => "BX",
            Reg::Sp => "SP",
            Reg::Bp => "BP",
            Reg::Si => "SI",
            Reg::Di => "DI",
        }
    }
}

/// A memory reference, taken apart.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Address {
    /// The base register, when the source named one.
    pub base: Option<Reg>,
    /// The index register, when the source named one.
    pub index: Option<Reg>,
    /// The displacement, when the source gave one.
    pub displacement: Option<i64>,
}

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The base and the index cannot go together.
    WrongAddressing {
        base: &'static str,
        index: &'static str,
    },
    /// The displacement does not fit in the byte the form wants.
    DisplacementTooBig { value: i64 },
    /// The reference named a register that cannot be a base.
    NotABase { name: String },
}

/// The segment a reference is in when the source did not say.
///
/// The manual is explicit: "All data references which explicitly or
/// implicitly involve SP or BP are taken by default to be relative to SS".
#[must_use]
pub fn default_segment(address: &Address) -> &'static str {
    match (address.base, address.index) {
        (Some(Reg::Bp), _) | (Some(Reg::Sp), _) => "SS",
        (_, Some(Reg::Bp)) | (_, Some(Reg::Sp)) => "SS",
        _ => "DS",
    }
}

/// The MOD, the R/M and the displacement bytes a reference needs.
pub fn encode(address: &Address) -> Result<(Mod, u8, Vec<u8>), Error> {
    if let (Some(base), Some(index)) = (address.base, address.index) {
        let pair = (base.name(), index.name());
        if matches!(pair, ("BX", "BP") | ("BP", "BX")) {
            return Err(Error::WrongAddressing {
                base: pair.0,
                index: pair.1,
            });
        }
        if matches!(pair, ("SI", "DI") | ("DI", "SI")) {
            return Err(Error::WrongAddressing {
                base: pair.0,
                index: pair.1,
            });
        }
    }
    // The R/M field names the pair, and the MOD says how the displacement
    // arrives: none, a byte, or a word.
    let rm = match (address.base, address.index) {
        (Some(Reg::Bx), Some(Reg::Si)) => 0b000,
        (Some(Reg::Bx), Some(Reg::Di)) => 0b001,
        (Some(Reg::Bp), Some(Reg::Si)) => 0b010,
        (Some(Reg::Bp), Some(Reg::Di)) => 0b011,
        (Some(Reg::Si), None) => 0b100,
        (Some(Reg::Di), None) => 0b101,
        (Some(Reg::Bp), None) => 0b110,
        (Some(Reg::Bx), None) => 0b111,
        (None, Some(Reg::Si)) => 0b100,
        (None, Some(Reg::Di)) => 0b101,
        (None, None) => 0b110,
        _ => 0b110,
    };
    let displacement = address.displacement.unwrap_or(0);
    // The direct form, with no register at all, is the only one that must
    // carry a word.
    let plain = address.base.is_none() && address.index.is_none();
    let (modrm, bytes) = if plain {
        if displacement == 0 {
            return Err(Error::DisplacementTooBig { value: 0 });
        }
        (Mod::Word, displacement_word(displacement)?)
    } else {
        match displacement {
            0 => (Mod::Memory, Vec::new()),
            d if (-128..=127).contains(&d) => (Mod::Byte, vec![d as u8]),
            d => (Mod::Word, displacement_word(d)?),
        }
    };
    Ok((modrm, rm, bytes))
}

/// The two bytes of a word displacement, little endian.
fn displacement_word(value: i64) -> Result<Vec<u8>, Error> {
    if !(0..=0xFFFF).contains(&value) {
        return Err(Error::DisplacementTooBig { value });
    }
    Ok((value as u16).to_le_bytes().to_vec())
}
