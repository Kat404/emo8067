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

impl Prefix {
    /// The byte the prefix is written as, out of the guide's own map.
    ///
    /// The map gives the prefixes their bytes: `F3` is `REP/REPE/REPZ`,
    /// `F2` is `REPNE/REPNZ`, `F0` is `LOCK`, and the segment overrides
    /// are the cells that say `SEG CS:` and so on.
    #[must_use]
    pub fn opcode(&self) -> Option<u8> {
        let forms = crate::table::forms();
        let cell = match self {
            Prefix::RepeatEqual => forms.iter().find(|form| form.mnemonic == "REP/REPE/REPZ"),
            Prefix::RepeatNotEqual => forms.iter().find(|form| form.mnemonic == "REPNE/REPNZ"),
            // The map has two LOCKs - `F1` is the same prefix a second
            // time - and the first one is the byte the prefix is written as.
            Prefix::Lock => forms
                .iter()
                .filter(|form| form.mnemonic == "LOCK")
                .min_by_key(|form| form.opcode),
            Prefix::Segment(name) => {
                let cell = format!("{}:", name.to_ascii_uppercase());
                forms
                    .iter()
                    .find(|form| form.mnemonic == "SEG" && form.operands == cell)
            }
        };
        cell.map(|form| form.opcode)
    }
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
    /// The label the line carries, if it carries one: `start:`.
    pub label: Option<String>,
    /// The instruction's name, as the source wrote it.
    pub mnemonic: String,
    /// The operands, as the source wrote them.
    pub operands: Vec<String>,
    /// The qualifier the source wrote before its operand: `far`, `near` or
    /// `short`. The reference's `CALL FAR es:[bx]` has one.
    pub qualifier: Option<String>,
}

impl Line {
    /// The bytes the prefixes are written as, out of the guide's own map.
    ///
    /// The prefixes come before the instruction they cover, in the order
    /// the source wrote them.
    #[must_use]
    pub fn prefix_bytes(&self) -> Vec<u8> {
        self.prefixes
            .iter()
            .filter_map(|prefix| prefix.opcode())
            .collect()
    }
}

/// Take a line apart.
pub fn parse(line: &str) -> Result<Line, Error> {
    // The reference writes its files with Windows line endings, and the
    // carriage return is not part of the line.
    let line = line.strip_suffix('\r').unwrap_or(line);
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
        let upper = bare.to_ascii_uppercase();
        // The reference writes the segment overrides with a dot in front as
        // well: `.cs:`.
        let segment = upper.strip_prefix('.').unwrap_or(upper.as_str());
        let prefix = match upper.as_str() {
            // `REP` is the same prefix as `REPE`: the reference's own
            // examples write `rep movsb`.
            "REP" | "REPE" | "REPZ" => Some(Prefix::RepeatEqual),
            "REPNE" | "REPNZ" => Some(Prefix::RepeatNotEqual),
            "LOCK" => Some(Prefix::Lock),
            "CS" | "DS" | "ES" | "SS" => Some(Prefix::Segment(segment.to_string())),
            ".CS" | ".DS" | ".ES" | ".SS" => Some(Prefix::Segment(segment.to_string())),
            _ => None,
        };
        let Some(prefix) = prefix else {
            break;
        };
        prefixes.push(prefix);
        index += 1;
    }
    // A name with a colon is a label, and the instruction follows it. The
    // colon may stand on its own, as `msg0:db 'x'`.
    let mut label = None;
    if let Some(word) = words.get(index) {
        if let Some(name) = word.strip_suffix(':') {
            if !name.is_empty() {
                label = Some(name.to_string());
                index += 1;
            }
        } else if let Some((name, tail)) = word.split_once(':') {
            if !name.is_empty() && !tail.is_empty() {
                // The rest of the word is the instruction, and the words
                // after it are its operands.
                let mut operands = Vec::new();
                let first_operand = words[index + 1..].join(" ").trim().to_string();
                if !first_operand.is_empty() {
                    operands.push(first_operand);
                }
                operands.extend(parts.drain(1..));
                let (qualifier, operands) = take_qualifier(operands);
                return Ok(Line {
                    prefixes,
                    label: Some(name.to_string()),
                    mnemonic: tail.to_string(),
                    operands,
                    qualifier,
                });
            }
        }
    }
    // A label with no colon, with the instruction after it: `msg0 db 'x'`.
    // The parser does not know which names are instructions, so the caller
    // puts the colon in; until it does, the name stands where the
    // instruction goes.
    let Some(mnemonic) = words.get(index) else {
        return Ok(Line {
            prefixes,
            label,
            mnemonic: String::new(),
            operands: Vec::new(),
            qualifier: None,
        });
    };
    let first_operand = words[index + 1..].join(" ").trim().to_string();
    let mut operands: Vec<String> = Vec::new();
    if !first_operand.is_empty() {
        operands.push(first_operand);
    }
    operands.extend(parts.drain(1..));
    let (qualifier, operands) = take_qualifier(operands);
    Ok(Line {
        prefixes,
        label,
        mnemonic: (*mnemonic).to_string(),
        operands,
        qualifier,
    })
}

/// Take the qualifier off the first operand.
///
/// The reference writes the qualifier before its operand: `CALL FAR
/// es:[bx]`, and `JMP SHORT there`. The guide's own cells carry the same
/// words - `JMP far`, `JMP short (intra)` - and they say which segment the
/// jump stays in.
fn take_qualifier(mut operands: Vec<String>) -> (Option<String>, Vec<String>) {
    let Some(first) = operands.first_mut() else {
        return (None, operands);
    };
    let trimmed = first.trim_start();
    let Some((word, rest)) = trimmed.split_once(char::is_whitespace) else {
        return (None, operands);
    };
    let upper = word.to_ascii_uppercase();
    if !matches!(upper.as_str(), "FAR" | "NEAR" | "SHORT") {
        return (None, operands);
    }
    let rest = rest.trim().to_string();
    if rest.is_empty() {
        operands.remove(0);
    } else {
        *first = rest;
    }
    (Some(upper), operands)
}

/// Split a line on the commas that are not inside brackets or quotes.
///
/// A string in quotes is one operand, commas and all: `db 'a,b'` is one
/// value, not two.
fn split_operands(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for c in code.chars() {
        if let Some(open) = quote {
            current.push(c);
            if c == open {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                current.push(c);
            }
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
