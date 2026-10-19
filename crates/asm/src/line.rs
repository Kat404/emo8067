//! A source line, taken apart.
//!
//! The reference accepted a handful of prefixes before the instruction, and
//! its diagnostics name them: the repeat prefixes, `LOCK`, and the segment
//! overrides. An operand may hold a comma inside brackets, so the line is
//! split on the commas that are not inside any.

/// A prefix that came before the instruction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Prefix {
    /// `REPE` or `REPZ`.
    RepeatEqual,
    /// `REPNE` or `REPNZ`.
    RepeatNotEqual,
    /// `LOCK`.
    Lock,
    /// A segment override: `CS`, `DS`, `ES` or `SS`.
    Segment(String),
}

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The line has no instruction on it.
    Empty,
    /// A prefix the assembler does not know.
    UnknownPrefix { word: String },
}

/// One source line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Line {
    /// The prefixes, in the order they were written.
    pub prefixes: Vec<Prefix>,
    /// The instruction's name, as the source wrote it.
    pub mnemonic: String,
    /// The operands, as the source wrote them.
    pub operands: Vec<String>,
}

/// Take a line apart.
pub fn parse(line: &str) -> Result<Line, Error> {
    // A semicolon starts a comment, and nothing after it is code.
    let code = line.split(';').next().unwrap_or("").trim();
    if code.is_empty() {
        return Err(Error::Empty);
    }
    let mut parts = split_operands(code);
    let head = parts.first().cloned().unwrap_or_default();
    let words: Vec<&str> = head.split_whitespace().collect();
    // The prefixes come before the instruction, and each of them may be
    // followed by a colon, as the segment overrides are.
    let mut prefixes = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let bare = words[index].trim_end_matches(':');
        let prefix = match bare.to_ascii_uppercase().as_str() {
            "REPE" | "REPZ" => Some(Prefix::RepeatEqual),
            "REPNE" | "REPNZ" => Some(Prefix::RepeatNotEqual),
            "LOCK" => Some(Prefix::Lock),
            "CS" | "DS" | "ES" | "SS" => Some(Prefix::Segment(bare.to_string())),
            _ => None,
        };
        let Some(prefix) = prefix else {
            break;
        };
        prefixes.push(prefix);
        index += 1;
    }
    let Some(mnemonic) = words.get(index) else {
        return Err(Error::Empty);
    };
    let first_operand = words[index + 1..].join(" ").trim().to_string();
    let mut operands: Vec<String> = Vec::new();
    if !first_operand.is_empty() {
        operands.push(first_operand);
    }
    operands.extend(parts.drain(1..));
    Ok(Line {
        prefixes,
        mnemonic: (*mnemonic).to_string(),
        operands,
    })
}

/// Split a line on the commas that are not inside brackets.
fn split_operands(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for c in code.chars() {
        match c {
            '[' | '(' => {
                depth += 1;
                current.push(c);
            }
            ']' | ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}
