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
    /// The names that are local to one expansion, which the reference
    /// declares with `LOCAL`.
    pub locals: Vec<String>,
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
    let mut locals: Vec<String> = Vec::new();
    let mut body: Vec<String> = Vec::new();
    for raw in source.lines() {
        let line = raw.trim_end_matches('\r').trim();
        // A comment is not code: the reference's own examples talk about
        // macros in their comments.
        if line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(current) = &name {
            if line.eq_ignore_ascii_case("ENDM") {
                macros.push(Macro {
                    name: current.clone(),
                    parameters: parameters.clone(),
                    locals: locals.clone(),
                    body: body.clone(),
                });
                name = None;
                parameters.clear();
                locals.clear();
                body.clear();
            } else if let Some(rest) = strip_local(line) {
                // LOCAL names the labels that belong to one expansion.
                locals = rest
                    .split(',')
                    .map(|local| local.trim().to_string())
                    .filter(|local| !local.is_empty())
                    .collect();
            } else {
                body.push(raw.to_string());
            }
            continue;
        }
        // The reference writes the macro's name first and the word MACRO
        // after it: `PUTC    MACRO   char`. The word may also come first,
        // as `MACRO name params`, and both are read.
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(at) = words
            .iter()
            .position(|word| word.eq_ignore_ascii_case("MACRO"))
        else {
            continue;
        };
        let (name_words, parameter_words) = if at == 0 {
            (&words[1..], &words[2..])
        } else {
            (&words[..at], &words[at + 1..])
        };
        let Some(first) = name_words.first() else {
            continue;
        };
        name = Some((*first).to_string());
        locals.clear();
        parameters = parameter_words
            .join(" ")
            .split(',')
            .map(|parameter| parameter.trim().to_string())
            .filter(|parameter| !parameter.is_empty())
            .collect();
        body.clear();
    }
    macros
}

/// The names a `LOCAL` line declares, if the line is one.
fn strip_local(line: &str) -> Option<&str> {
    let (head, rest) = line.split_once(char::is_whitespace)?;
    if !head.eq_ignore_ascii_case("LOCAL") {
        return None;
    }
    Some(rest.trim())
}

/// The body of a macro with the arguments in place of the parameters.
///
/// The `LOCAL` names are given an ending of their own, so that two
/// expansions of the same macro do not land on the same labels.
#[must_use]
pub fn expand(mac: &Macro, arguments: &[String], id: usize) -> Vec<String> {
    mac.body
        .iter()
        .map(|line| {
            let mut out = line.clone();
            for (parameter, argument) in mac.parameters.iter().zip(arguments) {
                out = replace_word(&out, parameter, argument);
            }
            for local in &mac.locals {
                out = replace_word(&out, local, &format!("{local}${id}"));
            }
            out
        })
        .collect()
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
///
/// The number each line has in the source comes with it, so that a refusal
/// can say where it happened.
pub fn strip(source: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut inside = false;
    for (number, raw) in source.lines().enumerate() {
        let line = raw.trim_end_matches('\r').trim();
        // A comment is not code: the reference's own examples talk about
        // macros in their comments.
        if line.starts_with(';') || line.starts_with('#') {
            out.push((number + 1, raw.to_string()));
            continue;
        }
        if inside {
            if line.eq_ignore_ascii_case("ENDM") {
                inside = false;
            }
            continue;
        }
        if line
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case("MACRO"))
        {
            inside = true;
            continue;
        }
        out.push((number + 1, raw.to_string()));
    }
    out
}
