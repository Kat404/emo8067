//! The macros.
//!
//! The reference's own list has `MACRO` and `ENDM`, and its own diagnostic
//! for a macro that calls itself: "recursive MACRO definition is over " -
//! it counts the passes, so a macro that never stops is refused rather
//! than left to run for ever.
//!
//! A macro's body is the source between `MACRO` and `ENDM`, and the names
//! that follow the macro's name are its parameters. A call replaces them
//! with the arguments of the call, word by word.

/// A macro: its name, its parameters, and its body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Macro {
    /// The name the source calls it by.
    pub name: String,
    /// The names of its parameters.
    pub parameters: Vec<String>,
    /// Its body, as the source wrote it.
    pub body: Vec<String>,
}

/// How deep a macro may nest before the assembler gives up.
pub const MAX_DEPTH: usize = 64;

/// The macros a source defines.
#[must_use]
pub fn read(source: &str) -> Vec<Macro> {
    let mut macros: Vec<Macro> = Vec::new();
    let mut name: Option<String> = None;
    let mut parameters: Vec<String> = Vec::new();
    let mut body: Vec<String> = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(current) = &name {
            if line.eq_ignore_ascii_case("ENDM") {
                macros.push(Macro {
                    name: current.clone(),
                    parameters: parameters.clone(),
                    body: body.clone(),
                });
                name = None;
                parameters.clear();
                body.clear();
            } else {
                body.push(raw.to_string());
            }
            continue;
        }
        let Some(rest) = strip_word(line, "MACRO") else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let Some(first) = words.first() else {
            continue;
        };
        name = Some((*first).to_string());
        parameters = words[1..]
            .join(" ")
            .split(',')
            .map(|parameter| parameter.trim().to_string())
            .filter(|parameter| !parameter.is_empty())
            .collect();
        body.clear();
    }
    macros
}

/// The body of a macro with the arguments in place of the parameters.
#[must_use]
pub fn expand(mac: &Macro, arguments: &[String]) -> Vec<String> {
    mac.body
        .iter()
        .map(|line| {
            let mut out = line.clone();
            for (parameter, argument) in mac.parameters.iter().zip(arguments) {
                out = replace_word(&out, parameter, argument);
            }
            out
        })
        .collect()
}

/// The part of a line after a word, if the line starts with that word.
fn strip_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    if line.len() < word.len() {
        return None;
    }
    let (head, rest) = line.split_at(word.len());
    if !head.eq_ignore_ascii_case(word) {
        return None;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None;
    }
    Some(rest)
}

/// Replace a whole word in a line, and not a part of a longer name.
fn replace_word(line: &str, name: &str, value: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(name) {
        let before = &rest[..at];
        let after = &rest[at + name.len()..];
        if is_boundary(before) && is_boundary(after) {
            out.push_str(before);
            out.push_str(value);
        } else {
            out.push_str(&rest[..at + name.len()]);
        }
        rest = &rest[at + name.len()..];
    }
    out.push_str(rest);
    out
}

/// Whether an edge of a word is a boundary and not part of a name.
fn is_boundary(edge: &str) -> bool {
    match edge.chars().last() {
        None => true,
        Some(character) => !character.is_alphanumeric() && character != '_',
    }
}

/// The source with the macro definitions taken out.
///
/// A definition is not code: it stands for the lines the calls will put in
/// its place.
#[must_use]
pub fn strip(source: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    for raw in source.lines() {
        let line = raw.trim();
        if inside {
            if line.eq_ignore_ascii_case("ENDM") {
                inside = false;
            }
            continue;
        }
        if strip_word(line, "MACRO").is_some() {
            inside = true;
            continue;
        }
        out.push(raw.to_string());
    }
    out
}
