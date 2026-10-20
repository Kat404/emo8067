//! The include directive.

use emo8067_asm::program::{self, Error};
use std::path::PathBuf;

/// A scratch directory outside the worktree, for one test.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("emo8067-asm-include-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The library a source includes.
const LIBRARY: &str = "PUTC MACRO char\n  mov al, char\nPUTC ENDM\n";

#[test]
fn an_include_is_spliced_in() {
    let dir = scratch("beside");
    std::fs::write(dir.join("lib.inc"), LIBRARY).unwrap();
    let source = dir.join("main.asm");
    std::fs::write(&source, "include \"lib.inc\"\nPUTC 'A'\n").unwrap();

    let program = program::assemble_file(&source, &[]).unwrap();
    assert_eq!(program.bytes, vec![0xB0, b'A']);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_include_is_found_in_the_include_path() {
    let dir = scratch("path");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(library.join("lib.inc"), LIBRARY).unwrap();
    let source = dir.join("main.asm");
    std::fs::write(&source, "include \"lib.inc\"\nPUTC 'A'\n").unwrap();

    let program = program::assemble_file(&source, &[library]).unwrap();
    assert_eq!(program.bytes, vec![0xB0, b'A']);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_include_that_is_not_there_is_refused() {
    let dir = scratch("missing");
    let source = dir.join("main.asm");
    std::fs::write(&source, "include \"nope.inc\"\n").unwrap();

    let error = program::assemble_file(&source, &[]).unwrap_err();
    match error {
        Error::Include { message } => assert_eq!(message, "file not found: nope.inc"),
        other => panic!("expected the refusal, got {other:?}"),
    }

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_include_that_includes_itself_is_refused() {
    let dir = scratch("recursive");
    let source = dir.join("main.asm");
    std::fs::write(&source, "include \"main.asm\"\n").unwrap();

    let error = program::assemble_file(&source, &[]).unwrap_err();
    match error {
        Error::Include { message } => assert_eq!(message, "recursive include file?"),
        other => panic!("expected the refusal, got {other:?}"),
    }

    std::fs::remove_dir_all(&dir).unwrap();
}
