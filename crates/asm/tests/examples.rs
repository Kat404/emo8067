//! The eighty-five examples, assembled.
//!
//! The examples are the reference's own, and they are local gold: they are
//! not in the repository. The gate runs when they are there and stands
//! aside when they are not.

use emo8067_asm::program::assemble_program;
use std::path::Path;

/// Where the examples live, from this crate's directory.
fn examples() -> Option<std::path::PathBuf> {
    let path = Path::new("../../re/extracted/app/examples");
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

#[test]
fn the_examples_assemble() {
    let Some(directory) = examples() else {
        // The examples are local gold, not part of the repository.
        return;
    };
    let mut sources: Vec<_> = std::fs::read_dir(&directory)
        .expect("the examples")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "asm"))
        .collect();
    sources.sort();
    let mut failures: Vec<(String, String)> = Vec::new();
    for source in &sources {
        // Some of the reference's files are not UTF-8; the bytes that are
        // not are in comments, and a lossy read leaves them alone.
        let bytes = std::fs::read(source).expect("the example");
        let text = String::from_utf8_lossy(&bytes);
        if let Err(error) = assemble_program(&text) {
            // The line the source failed on, and the line itself.
            let line = match &error {
                emo8067_asm::program::Error::AtLine { line, .. } => *line,
                _ => 0,
            };
            let text_of = text
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or("")
                .trim();
            failures.push((
                source.file_name().unwrap().to_string_lossy().to_string(),
                format!("{line}: {text_of}  ({error:?})"),
            ));
        }
    }
    println!(
        "{} examples, {} assembled, {} not",
        sources.len(),
        sources.len() - failures.len(),
        failures.len()
    );
    for (name, error) in &failures {
        println!("  {name}: {error}");
    }
    // The gate is a floor for now, not a wall: these are the examples that
    // still do not assemble, and why. Every one of them is the reference's
    // own dialect and not the guide's, and each is listed with what it
    // needs. When one is closed, its line goes and the floor goes up.
    const KNOWN: [(&str, &str); 10] = [
        (
            "PrinterDemo.asm",
            "`mov cx, offset msg_end - offset msg`: a difference of two offsets",
        ),
        (
            "bin2dec.asm",
            "`JCXZ stop_program`: a jump to a label, forward",
        ),
        (
            "calc.asm",
            "`printn '...'`: a macro of the include, and its LOCAL",
        ),
        (
            "datefile.asm",
            "`mov cx, offset seco - offset txt`: a difference of two offsets",
        ),
        (
            "include.asm",
            "the include directive: `include` of emu8086.inc",
        ),
        (
            "int21.asm",
            "`mov dx, offset filename+2`: an offset with an addition",
        ),
        (
            "micro-os_kernel.asm",
            "`je reboot_command`: a jump to a label, forward",
        ),
        (
            "mouse.asm",
            "`print 1,1,...`: a macro of the include, and its LOCAL",
        ),
        (
            "simplest.asm",
            "`print 1,1,...`: a macro of the include, and its LOCAL",
        ),
        (
            "writebin.asm",
            "`call clear_screen`: a call to a label, forward",
        ),
    ];
    let names: Vec<&str> = failures.iter().map(|(name, _)| name.as_str()).collect();
    for (name, _why) in KNOWN {
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
}
