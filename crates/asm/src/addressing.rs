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
    /// The text is not a memory reference at all.
    NotAReference { text: String },
}

/// A memory reference, as the source wrote it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reference {
    /// The address, taken apart.
    pub address: Address,
    /// The width the source asked for, when it said: `BYTE PTR` or `WORD PTR`.
    pub width: Option<crate::modrm::Width>,
}

/// Read a memory reference: `[BX+SI+4]`, `WORD PTR [1234h]`, `BYTE PTR DS:[BX]`.
pub fn parse(text: &str) -> Result<Reference, Error> {
    let mut rest = text.trim();
    // The size and the segment override come before the brackets.
    let mut width = None;
    let mut segment = None;
    loop {
        let (word, tail) = match rest.split_once(char::is_whitespace) {
            Some((word, tail)) => (word, tail.trim_start()),
            None => (rest, ""),
        };
        match word.to_ascii_uppercase().as_str() {
            "BYTE" => {
                width = Some(crate::modrm::Width::Byte);
                rest = tail;
                // The size is usually written `BYTE PTR`, and the PTR is
                // only a word.
                if let Some(tail) = rest.strip_prefix("PTR ") {
                    rest = tail;
                }
            }
            "WORD" => {
                width = Some(crate::modrm::Width::Word);
                rest = tail;
                if let Some(tail) = rest.strip_prefix("PTR ") {
                    rest = tail;
                }
            }
            "CS" | "DS" | "ES" | "SS" => {
                segment = Some(word.to_string());
                rest = tail;
            }
            _ => break,
        }
    }
    let _ = segment;
    // A segment override may sit right against the bracket: `DS:[BX]`.
    for name in ["CS", "DS", "ES", "SS"] {
        if let Some(tail) = rest.strip_prefix(name) {
            if let Some(tail) = tail.strip_prefix(':') {
                rest = tail;
            }
        }
    }
    let inner = rest
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| Error::NotAReference {
            text: text.to_string(),
        })?;
    let mut address = Address::default();
    // The terms are separated by a sign, and a sign in front of a number
    // belongs to it: `BX-4` is BX and minus four.
    let mut rest = inner.trim();
    let mut negative = false;
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix('-') {
            negative = true;
            rest = tail.trim_start();
            continue;
        }
        if let Some(tail) = rest.strip_prefix('+') {
            rest = tail.trim_start();
            continue;
        }
        let (term, tail) = match rest.find(['+', '-']) {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, ""),
        };
        let term = term.trim();
        match register(term) {
            Some(reg) => {
                if address.base.is_some() {
                    address.index = Some(reg);
                } else {
                    address.base = Some(reg);
                }
            }
            None => {
                let value = crate::literals::parse(term).map_err(|_| Error::NotAReference {
                    text: text.to_string(),
                })?;
                address.displacement = Some(if negative { -value } else { value });
            }
        }
        rest = tail;
    }
    Ok(Reference { address, width })
}

/// The register a term names, if it names one.
#[must_use]
pub fn register(term: &str) -> Option<Reg> {
    match term.to_ascii_uppercase().as_str() {
        "BX" => Some(Reg::Bx),
        "SP" => Some(Reg::Sp),
        "BP" => Some(Reg::Bp),
        "SI" => Some(Reg::Si),
        "DI" => Some(Reg::Di),
        _ => None,
    }
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
