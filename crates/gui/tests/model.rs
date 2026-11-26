//! The GUI reads the machine in one place only: the panel model's.
//!
//! The panel model is the product (`odd/research/features-and-ux.md` §4.3),
//! and the GUI is a second renderer of it: the model reads the machine, and
//! the GUI draws what the model gives it. So the crate holds no reading of
//! the machine's memory, no second listing of the code and no second
//! evaluator of an expression.

use std::path::Path;

/// The crate's own sources, as one text.
fn sources() -> String {
    let mut out = String::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("the crate's own sources") {
            let path = entry.expect("the entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push_str(&std::fs::read_to_string(&path).expect("the source"));
                out.push('\n');
            }
        }
    }
    out
}

/// The paths the crate's own sources take into the core.
fn core_paths(sources: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = sources;
    while let Some(at) = rest.find("emo8067_core::") {
        rest = &rest[at + "emo8067_core::".len()..];
        let end = rest
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        paths.push(format!("emo8067_core::{}", &rest[..end]));
    }
    paths
}

#[test]
fn the_gui_reads_the_machine_nowhere_else() {
    let sources = sources();
    assert!(
        sources.contains("Panels::of"),
        "the GUI draws the model's own panels"
    );
    for forbidden in [
        "disassemble(",
        "read_u8(",
        "read_u16(",
        "load_at(",
        "evaluate(",
    ] {
        assert!(
            !sources.contains(forbidden),
            "the GUI has no {forbidden} of its own: the machine is read in one place"
        );
    }
}

#[test]
fn the_gui_reaches_the_core_through_the_cpu_and_nothing_else() {
    for path in core_paths(&sources()) {
        assert!(
            path.starts_with("emo8067_core::Cpu"),
            "the core is read through the Cpu's own surface: {path}"
        );
    }
}
