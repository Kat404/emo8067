//! Numeric literals, and the rules the reference applied to them.
//!
//! Four ways to write a number, all of them from the reference's own
//! diagnostics:
//!
//! * `&HFF` — a hex value with the `&` prefix and the `H` suffix,
//! * `1234h` — a hex value with the `h` suffix,
//! * `0123456789$` — MASM's radix: the digits before the `$` are the base,
//! * `1234` — decimal.
//!
//! And two complaints the reference made about them: a hex value needs a
//! zero prefix when it would otherwise look decimal ("zero prefix must be
//! added to a HEX value"), and a value that will not fit in sixteen bits is
//! refused ("cannot convert to 16 bit value").

/// What went wrong with a literal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The text is not a number in any of the four ways.
    NotANumber { text: String },
    /// The value does not fit in sixteen bits.
    TooBig { text: String, value: i64 },
    /// A hex value that would be read as decimal without a zero prefix.
    NeedsZeroPrefix { text: String },
}

/// Read a numeric literal.
pub fn parse(text: &str) -> Result<i64, Error> {
    let text = text.trim();
    let bad = || Error::NotANumber {
        text: text.to_string(),
    };
    // A character: `'H'`.
    if text.len() >= 3 && text.starts_with('\'') && text.ends_with('\'') {
        let inner = &text[1..text.len() - 1];
        let ch = inner.chars().next().ok_or_else(bad)?;
        return Ok(ch as i64);
    }
    // MASM's radix: the base, then the `$`, then the digits: `16$FF`.
    if let Some((base, digits)) = text.split_once('$') {
        let base = base.trim().parse::<u32>().map_err(|_| bad())?;
        if !(2..=36).contains(&base) {
            return Err(bad());
        }
        let digits = digits.trim();
        if digits.is_empty() {
            return Err(bad());
        }
        return i64::from_str_radix(digits, base).map_err(|_| bad());
    }
    // `&HFF`.
    if let Some(hex) = text.strip_prefix('&').or_else(|| text.strip_prefix('^')) {
        let hex = hex
            .strip_prefix('H')
            .or_else(|| hex.strip_prefix('h'))
            .ok_or_else(bad)?;
        return i64::from_str_radix(hex, 16).map_err(|_| bad());
    }
    // `1234h`.
    if let Some(hex) = text.strip_suffix('h').or_else(|| text.strip_suffix('H')) {
        // A hex value has to begin with a digit, or it reads as a name: the
        // reference wanted `0FFh` and not `FFh`.
        if hex.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Err(Error::NeedsZeroPrefix {
                text: text.to_string(),
            });
        }
        return i64::from_str_radix(hex, 16).map_err(|_| bad());
    }
    // A binary value: `11101100b`.
    if let Some(bits) = text.strip_suffix('b').or_else(|| text.strip_suffix('B')) {
        return i64::from_str_radix(bits, 2).map_err(|_| bad());
    }
    // Decimal.
    text.parse::<i64>().map_err(|_| bad())
}

/// Read a literal that has to fit in sixteen bits.
pub fn parse_word(text: &str) -> Result<u16, Error> {
    let value = parse(text)?;
    if !(0..=0xFFFF).contains(&value) {
        return Err(Error::TooBig {
            text: text.to_string(),
            value,
        });
    }
    Ok(value as u16)
}
