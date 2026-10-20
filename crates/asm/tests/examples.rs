//! The eighty-five examples, assembled.
//!
//! The examples are the reference's own, and they are local gold: they are
//! not in the repository. The gate runs when they are there and stands
//! aside when they are not.

use emo8067_asm::program::{self, Error};
use std::path::{Path, PathBuf};

/// Where the examples live, from this crate's directory.
fn examples() -> Option<PathBuf> {
    let path = Path::new("../../re/extracted/app/examples");
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Where the reference's macro library lives, next to the examples.
fn library() -> Option<PathBuf> {
    let path = Path::new("../../re/extracted/app/inc");
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// The reference's own words for a conditional jump that is out of range.
const OUT_OF_RANGE: &str = "conditional jump is out of range: allowed values are from -128 to 127";

/// The refusal of a jump that is out of range, if the error is one.
fn out_of_range(error: &Error) -> Option<&str> {
    match error {
        Error::AtLine { error, .. } => out_of_range(error),
        Error::JumpOutOfRange { message } => Some(message),
        _ => None,
    }
}

/// An example that did not assemble: the line, the line itself, and why.
struct Failure {
    /// The file's name.
    name: String,
    /// The line the source failed on.
    line: usize,
    /// The line itself, as the text that was assembled says it.
    text: String,
    /// Why it did not assemble.
    why: String,
}

#[test]
fn the_examples_assemble() {
    let (Some(directory), Some(library)) = (examples(), library()) else {
        // The examples and the library are local gold, not part of the
        // repository.
        return;
    };
    let mut sources: Vec<_> = std::fs::read_dir(&directory)
        .expect("the examples")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "asm"))
        .collect();
    sources.sort();
    let mut failures: Vec<Failure> = Vec::new();
    for source in &sources {
        // The library's macros must be visible to the whole program, so the
        // include is spliced in before the program is assembled.
        let text =
            program::splice_includes(source, std::slice::from_ref(&library)).expect("the include");
        if let Err(error) = program::assemble_program(&text) {
            // The line the source failed on, and the line itself - the line
            // of the text that was assembled, not of the file on disk.
            let line = match &error {
                Error::AtLine { line, .. } => *line,
                _ => 0,
            };
            let source_line = text
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or("")
                .trim()
                .to_string();
            let why = match out_of_range(&error) {
                Some(message) => message.to_string(),
                None => format!("{error:?}"),
            };
            failures.push(Failure {
                name: source.file_name().unwrap().to_string_lossy().to_string(),
                line,
                text: source_line,
                why,
            });
        }
    }
    println!(
        "{} examples, {} assembled, {} not",
        sources.len(),
        sources.len() - failures.len(),
        failures.len()
    );
    for failure in &failures {
        println!(
            "  {}: {}: {}  ({})",
            failure.name, failure.line, failure.text, failure.why
        );
    }
    // The gate is a floor for now, not a wall: these are the examples that
    // still do not assemble, and why. When one is closed, its line goes and
    // the floor goes up.
    const KNOWN: [&str; 4] = [
        "bin2dec.asm",
        "micro-os_kernel.asm",
        "mouse.asm",
        "writebin.asm",
    ];
    let names: Vec<&str> = failures
        .iter()
        .map(|failure| failure.name.as_str())
        .collect();
    for name in KNOWN {
        assert!(
            names.contains(&name),
            "{name} assembles now; remove it from the list"
        );
    }
    assert_eq!(
        names.len(),
        KNOWN.len(),
        "the examples that do not assemble are: {names:?}"
    );
    // Every one of them is refused with the reference's own words.
    for failure in &failures {
        assert_eq!(
            failure.why, OUT_OF_RANGE,
            "{} is refused with another reason",
            failure.name
        );
    }
}
