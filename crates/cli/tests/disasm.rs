//! The disassembler, through the binary: a file is listed, one instruction
//! per line.
//!
//! These are the tests that hold the listing's contract: the address, the
//! bytes and the reference's own text, the same rule for a `.asm` and for a
//! `.bin`, and a line that says why the listing stopped.

use std::path::PathBuf;
use std::process::Command;

/// A scratch directory outside the worktree, for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// List a file and say what it printed and how it ended.
fn list(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_emo8067"))
        .args(args)
        .output()
        .expect("the runner runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn a_raw_binary_is_listed_with_its_address_bytes_and_text() {
    let dir = scratch("raw-listing");
    let image = dir.join("program.bin");
    std::fs::write(
        &image,
        [0xB8, 0x34, 0x12, 0x8B, 0x1E, 0x00, 0x02, 0xC3, 0xF4],
    )
    .unwrap();

    let (out, err, code) = list(&["disasm", image.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    // The logical address, the bytes and the reference's own text.
    assert_eq!(
        out,
        "0100:0000  B8 34 12  MOV AX, 01234h\n\
         0100:0003  8B 1E 00 02  MOV BX, [00200h]\n\
         0100:0007  C3  RET\n\
         0100:0008  F4  HLT\n"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_asm_program_is_assembled_and_listed_at_its_origin() {
    let dir = scratch("asm-listing");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\nmov ax, 1234h\nhlt\n").unwrap();

    let (out, err, code) = list(&["disasm", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    // The reference's own model: a program is loaded at its origin in the
    // segment 0000, and the listing starts there.
    assert_eq!(
        out,
        "0000:0100  B8 34 12  MOV AX, 01234h\n\
         0000:0103  F4  HLT\n"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_listing_starts_where_the_address_says() {
    let dir = scratch("at-listing");
    let image = dir.join("program.bin");
    std::fs::write(&image, [0xB8, 0x34, 0x12, 0x8B, 0x1E, 0x00, 0x02]).unwrap();

    let (out, _, code) = list(&["disasm", image.to_str().unwrap(), "--at", "0100:0003"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with("0100:0003  8B 1E 00 02  MOV BX, [00200h]\n"),
        "{out}"
    );

    // An address outside the loaded segment is not in the image.
    let (out, err, code) = list(&["disasm", image.to_str().unwrap(), "--at", "0200:0000"]);
    assert_eq!(code, 2, "{out}");
    assert!(err.contains("not in the loaded segment"), "{err}");

    // And an address past the end of the image is not either.
    let (out, err, code) = list(&["disasm", image.to_str().unwrap(), "--at", "0100:0100"]);
    assert_eq!(code, 2, "{out}");
    assert!(err.contains("past the end of the image"), "{err}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_listing_has_as_many_instructions_as_it_is_asked_for() {
    let dir = scratch("count-listing");
    let image = dir.join("program.bin");
    std::fs::write(&image, [0xB8, 0x34, 0x12, 0x8B, 0x1E, 0x00, 0x02, 0xC3]).unwrap();

    let (out, _, code) = list(&["disasm", image.to_str().unwrap(), "--count", "2"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(
        out.starts_with("0100:0000  B8 34 12  MOV AX, 01234h\n"),
        "{out}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_image_is_loaded_where_the_load_says() {
    let dir = scratch("load-listing");
    let image = dir.join("program.bin");
    std::fs::write(&image, [0xB8, 0x34, 0x12, 0xF4]).unwrap();

    let (out, _, code) = list(&["disasm", image.to_str().unwrap(), "--load", "0500:0020"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        "0500:0020  B8 34 12  MOV AX, 01234h\n\
         0500:0023  F4  HLT\n"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_listing_says_why_it_stopped() {
    let dir = scratch("end-listing");
    let image = dir.join("program.bin");
    // The image ends in the middle of an instruction.
    std::fs::write(&image, [0xB8, 0x34]).unwrap();

    let (out, err, code) = list(&["disasm", image.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, "0100:0000  B8 34  ; truncated instruction\n");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_include_is_looked_for_on_the_include_path() {
    let dir = scratch("include-listing");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(library.join("value.inc"), "VALUE EQU 5\n").unwrap();
    let source = dir.join("program.asm");
    std::fs::write(
        &source,
        "org 100h\ninclude \"value.inc\"\nmov ax, VALUE\nhlt\n",
    )
    .unwrap();

    let (out, err, code) = list(&[
        "disasm",
        source.to_str().unwrap(),
        "--include",
        library.to_str().unwrap(),
    ]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("MOV AX, 00005h"), "{out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_help_lists_both_commands() {
    let (out, _, code) = list(&["--help"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("emo8067 run <file.asm|file.bin>"), "{out}");
    assert!(out.contains("emo8067 disasm <file.asm|file.bin>"), "{out}");
}
