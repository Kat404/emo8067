//! The runner, through the binary: a program is assembled, run and pinned.
//!
//! These are the tests that hold the runner's contract: a `.asm` is
//! assembled and run where it says it starts, an include is looked for
//! beside the source first, and an interrupt is taken when there is a
//! handler for it.

use std::path::PathBuf;
use std::process::Command;

/// A scratch directory outside the worktree, for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emo8067-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the runner and say what it printed and how it ended.
fn run(args: &[&str]) -> (String, String, i32) {
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
fn an_asm_program_is_assembled_and_run() {
    let dir = scratch("assembled");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\nmov ax, 1234h\nhlt\n").unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("AX 1234"), "{out}");
    assert_eq!(code, 0, "the program halted: {out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_program_starts_where_its_origin_says() {
    let dir = scratch("origin");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 7c00h\nmov ax, 1234h\nhlt\n").unwrap();

    let (out, _, code) = run(&["run", source.to_str().unwrap(), "--trace"]);
    assert_eq!(code, 0, "{out}");
    // The reference's own model, in the words of its boot template: "load
    // segment: 0000, load offset: 7c00, same values are set to CS and IP
    // registers accordingly". The segments follow the load segment.
    assert!(
        out.starts_with("0000:7C00  B8"),
        "the program starts at its origin: {out}"
    );
    assert!(out.contains("SS:SP  0000:FFFE   DS 0000  ES 0000"), "{out}");

    // The image is loaded where the BIOS puts a boot record.
    let (golden, _, _) = run(&["run", source.to_str().unwrap(), "--golden"]);
    assert!(golden.contains("page 07000"), "{golden}");
    assert!(golden.contains("07C00  B8 34 12 F4"), "{golden}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_load_segment_can_be_overridden() {
    let dir = scratch("loadseg");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\nmov ax, 1234h\nhlt\n").unwrap();

    let (out, _, code) = run(&["run", source.to_str().unwrap(), "--load-seg", "0500"]);
    assert_eq!(code, 0, "{out}");
    // The program is loaded in the segment it was told to be loaded in,
    // and the segments follow it.
    assert!(
        out.contains("CS:IP  0500:0104   SS:SP  0500:FFFE   DS 0500  ES 0500"),
        "{out}"
    );

    // The image goes where the load segment and the origin say.
    let (golden, _, _) = run(&[
        "run",
        source.to_str().unwrap(),
        "--load-seg",
        "0500",
        "--golden",
    ]);
    assert!(golden.contains("page 05000"), "{golden}");
    assert!(golden.contains("05100  B8 34 12 F4"), "{golden}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_program_with_data_reads_its_data_back() {
    let dir = scratch("data");
    let source = dir.join("program.asm");
    // The string is behind the jump, and the program counts it.
    std::fs::write(
        &source,
        "org 100h\n\
         jmp start\n\
         str1 db 'hello', 0\n\
         start:\n\
         lea bx, str1\n\
         mov ax, 0\n\
         again:\n\
         cmp [bx], 0\n\
         je done\n\
         inc ax\n\
         inc bx\n\
         jmp again\n\
         done:\n\
         hlt\n",
    )
    .unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    // A label is the address of the thing it names, so the program reads
    // the string it wrote.
    assert!(out.contains("AX 0005"), "five characters: {out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_include_is_looked_for_beside_the_source_first() {
    let dir = scratch("include");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    // The same name in two places: the one beside the source wins, and the
    // one on the include path is only looked at when there is none.
    std::fs::write(dir.join("value.inc"), "VALUE EQU 5\n").unwrap();
    std::fs::write(library.join("value.inc"), "VALUE EQU 9\n").unwrap();
    let source = dir.join("program.asm");
    std::fs::write(
        &source,
        "org 100h\ninclude \"value.inc\"\nmov ax, VALUE\nhlt\n",
    )
    .unwrap();

    let (out, err, _) = run(&[
        "run",
        source.to_str().unwrap(),
        "--include",
        library.to_str().unwrap(),
    ]);
    assert!(err.is_empty(), "{err}");
    assert!(
        out.contains("AX 0005"),
        "the include beside the source wins: {out}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_golden_text_says_what_the_program_wrote() {
    let dir = scratch("golden");
    let source = dir.join("program.asm");
    // The program writes a byte to the video memory and halts.
    std::fs::write(
        &source,
        "org 100h\nmov ax, 0b800h\nmov ds, ax\nmov [0000h], 41h\nhlt\n",
    )
    .unwrap();

    let (out, _, _) = run(&["run", source.to_str().unwrap(), "--golden"]);
    assert!(out.contains("--- memory ---"), "{out}");
    assert!(out.contains("page B8000"), "{out}");
    assert!(out.contains("B8000  41"), "{out}");

    // Without the flag, the memory is not printed: the state is the same
    // as it has always been.
    let (plain, _, _) = run(&["run", source.to_str().unwrap()]);
    assert!(!plain.contains("--- memory ---"), "{plain}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_interrupt_with_a_handler_is_taken_and_the_program_continues() {
    let dir = scratch("handled");
    let source = dir.join("program.asm");
    // The program installs its own handler for INT 21h and then asks for
    // the service. The handler is in the segment the program was loaded
    // in, which is the segment 0000.
    std::fs::write(
        &source,
        "org 100h\n\
         mov ax, 0000h\n\
         mov ds, ax\n\
         mov bx, offset handler\n\
         mov [0084h], bx\n\
         mov [0086h], 0000h\n\
         mov ax, 0000h\n\
         mov ds, ax\n\
         int 21h\n\
         hlt\n\
         handler:\n\
         mov ax, 5678h\n\
         iret\n",
    )
    .unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program ran to the end: {out}");
    assert!(out.contains("AX 5678"), "the handler ran: {out}");
    assert!(!out.contains("STOP"), "the run was not cut short: {out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_interrupt_without_a_handler_stops_the_run() {
    let dir = scratch("unhandled");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\nmov ax, 1111h\nint 21h\nhlt\n").unwrap();

    let (out, _, code) = run(&["run", source.to_str().unwrap()]);
    assert_eq!(code, 1, "the run ended at the interrupt: {out}");
    assert!(out.contains("STOP  took INT 0x21"), "{out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_raw_binary_is_still_loaded_raw() {
    let dir = scratch("raw");
    let image = dir.join("program.bin");
    std::fs::write(&image, [0xB8, 0x34, 0x12, 0xF4]).unwrap();

    let (out, _, code) = run(&["run", image.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    // The defaults of a raw image: 0100:0000, SS 0100, SP FFFE, DS 0100.
    assert!(
        out.contains("CS:IP  0100:0004   SS:SP  0100:FFFE   DS 0100  ES 0100"),
        "{out}"
    );
    assert!(out.contains("AX 1234"), "{out}");

    std::fs::remove_dir_all(&dir).unwrap();
}
