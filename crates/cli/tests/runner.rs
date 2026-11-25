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

/// `PUSHA` pushes eight words and `POPA` pops eight, so the stack comes back
/// where it was.
///
/// The word in the middle is the `SP` the reference pops and ignores, and it
/// has to land somewhere: the register that is written last takes it, and
/// the last pop puts the register back.
#[test]
fn pusha_and_popa_leave_the_stack_where_it_was() {
    let dir = scratch("pusha");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\npusha\npopa\nhlt\n").unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    assert!(
        out.contains("SS:SP  0000:FFFE"),
        "the stack is where it started: {out}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The program's own register initialisers are the state it starts in.
///
/// The reference writes them as `#cs=1234#` and the rest, and the load
/// segment and the load offset are the first two words of the `.binf` file
/// it writes them into. Before this they were dropped as if they were
/// comments, and the program ran somewhere else.
#[test]
fn a_program_runs_where_its_own_initialisers_say() {
    let dir = scratch("initialisers");
    let source = dir.join("program.asm");
    std::fs::write(
        &source,
        "#cs=1234#\n#ip=0010#\n#ss=3000#\n#sp=fff0#\n#ds=4000#\n#es=5000#\n\
         #si=0100#\n#di=0200#\n#bp=0300#\n#dl=f0#\norg 0010h\nmov al, dl\nhlt\n",
    )
    .unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    // The program ran and halted, so `IP` is past the `HLT`; the segment
    // and the load address are what the initialisers said.
    assert!(out.contains("CS:IP  1234:"), "{out}");
    assert!(out.contains("SS:SP  3000:FFF0"), "{out}");
    assert!(out.contains("DS 4000  ES 5000"), "{out}");
    assert!(out.contains("AX 00F0"), "DL went into AL: {out}");
    assert!(out.contains("SI 0100  DI 0200  BP 0300"), "{out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The load address and the state the program starts in are two things.
///
/// The reference's `.binf` file has a line for the load segment and the
/// load offset and a line for `CS` and `IP`, and each falls back to the
/// other only when the other is not given: "if LOAD_SEGMENT and LOAD_OFFSET
/// are not defined, then CS and IP values are used and vice-versa". A
/// program that declares both keeps both.
#[test]
fn the_load_address_and_the_starting_state_are_two_things() {
    let dir = scratch("load-and-state");
    let source = dir.join("program.asm");
    std::fs::write(
        &source,
        "#load_segment=1000#\n#load_offset=0000#\n#cs=2000#\n#ip=0010#\norg 0010h\nhlt\n",
    )
    .unwrap();

    let (out, err, _) = run(&["run", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert!(out.contains("CS:IP  2000:"), "{out}");

    // And the image is where the load address says.
    let (golden, _, _) = run(&["run", source.to_str().unwrap(), "--golden"]);
    assert!(golden.contains("page 10000"), "{golden}");

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The flags win over the program's own initialisers: they are what the
/// person running it asked for.
#[test]
fn the_flags_win_over_the_programmes_initialisers() {
    let dir = scratch("initialisers-flag");
    let source = dir.join("program.asm");
    std::fs::write(&source, "#cs=1234#\n#ip=0010#\norg 0010h\nhlt\n").unwrap();

    let (out, err, code) = run(&[
        "run",
        source.to_str().unwrap(),
        "--org",
        "5000:0020",
        "--ss",
        "6000",
        "--sp",
        "0100",
        "--ds",
        "7000",
    ]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    assert!(out.contains("CS:IP  5000:"), "{out}");
    assert!(out.contains("SS:SP  6000:0100"), "{out}");
    assert!(out.contains("DS 7000"), "{out}");

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

/// The runner takes the keys from a script, and the guest's keyboard gets
/// them.
///
/// The script is fed as the guest consumes it, so a script longer than the
/// buffer works.
#[test]
fn the_runner_takes_the_keys_from_a_script() {
    let dir = scratch("keys");
    let source = dir.join("program.asm");
    // The program asks for a key, keeps the pair it got, and asks for
    // another one.
    std::fs::write(
        &source,
        "org 100h\n\
         mov ah, 0\n\
         int 16h\n\
         mov bx, ax\n\
         mov ah, 0\n\
         int 16h\n\
         hlt\n",
    )
    .unwrap();

    let (out, err, code) = run(&["run", source.to_str().unwrap(), "--keys", "AB"]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    // 'A' is 1Eh and 'B' is 30h in the IBM's set 1.
    assert!(
        out.contains("BX 1E41"),
        "the first key reached the guest: {out}"
    );
    assert!(out.contains("AX 3042"), "and so did the second one: {out}");

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The script's escapes name the keys a character cannot.
///
/// `\n` and `\r` are the enter key — and so is a literal newline, the DOS
/// way — and the arrows are `\up`, `\down`, `\left` and `\right`.
#[test]
fn the_script_escapes_name_the_keys() {
    let dir = scratch("escapes");
    let source = dir.join("program.asm");
    // The program asks for three keys and keeps the characters it got.
    std::fs::write(
        &source,
        "org 100h\n\
         mov ah, 0\n\
         int 16h\n\
         mov [0200h], al\n\
         mov ah, 0\n\
         int 16h\n\
         mov [0201h], al\n\
         mov ah, 0\n\
         int 16h\n\
         mov [0202h], al\n\
         hlt\n",
    )
    .unwrap();

    let (golden, err, code) = run(&[
        "run",
        source.to_str().unwrap(),
        "--keys",
        "a\\n\\up",
        "--golden",
    ]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {golden}");
    // The character, the enter key — 0Dh, the way the corpus's programs
    // compare it — and the arrow, which has no character of its own.
    assert!(
        golden.contains("00200  61 0D"),
        "the character, the enter key and the arrow: {golden}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A script longer than the buffer works: the script is fed as the guest
/// consumes it, and not all at once.
#[test]
fn a_script_longer_than_the_buffer_works() {
    let dir = scratch("long-keys");
    let source = dir.join("program.asm");
    // The program asks for twenty keys, more than the buffer holds.
    std::fs::write(
        &source,
        "org 100h\n\
         mov cx, 20\n\
         again:\n\
         mov ah, 0\n\
         int 16h\n\
         dec cx\n\
         jnz again\n\
         hlt\n",
    )
    .unwrap();

    let (out, err, code) = run(&[
        "run",
        source.to_str().unwrap(),
        "--keys",
        "abcdefghijklmnopqrst",
    ]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    assert!(
        !out.contains("step budget exhausted"),
        "the guest did not wait for a key that never came: {out}"
    );
    // The last key of the script is the 't', whose set 1 code is 14h.
    assert!(
        out.contains("1474"),
        "the guest read all twenty keys: {out}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The CLI's own help names the `--keys` option.
#[test]
fn the_help_names_the_keys_option() {
    let (out, err, code) = run(&["--help"]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("--keys"), "the help names the option: {out}");
}

/// A script that does not fit in the guest's keyboard buffer is said out
/// loud: the run is not a failure, but the guest did not get the script.
#[test]
fn a_script_that_does_not_fit_is_said_out_loud() {
    let dir = scratch("too-many-keys");
    let source = dir.join("program.asm");
    // A program that never polls the keyboard, and a script of ten keys
    // for a buffer that holds eight.
    std::fs::write(&source, "org 100h\nmov ax, 1234h\nhlt\n").unwrap();
    let (out, err, code) = run(&["run", "--keys", "abcdefghij", source.to_str().unwrap()]);
    assert_eq!(code, 0, "the program halted: {out}");
    assert!(
        err.contains("2 of the script's keys never reached the guest"),
        "{err}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The option does not change a run that asks for no key: the program runs
/// as it always did.
#[test]
fn the_keys_option_leaves_a_program_that_asks_for_no_key_alone() {
    let dir = scratch("no-keys");
    let source = dir.join("program.asm");
    std::fs::write(&source, "org 100h\nmov ax, 1234h\nhlt\n").unwrap();

    let (out, err, code) = run(&["run", "--keys", "xy", source.to_str().unwrap()]);
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, 0, "the program halted: {out}");
    assert!(out.contains("AX 1234"), "{out}");

    std::fs::remove_dir_all(&dir).unwrap();
}
