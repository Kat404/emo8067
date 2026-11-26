//! The GUI's own paths: the program it opens, and the ones it refuses.
//!
//! None of this needs a display: the paths are the ones the window takes
//! before it is ever drawn.

use std::path::{Path, PathBuf};

use emo8067_gui::app::App;
use emo8067_gui::ui::{Here, listing};

/// A program to open, written where the test can keep it.
///
/// The directory is removed when the program is dropped, so a test leaves
/// nothing behind.
struct Program {
    /// The directory the program is in.
    directory: PathBuf,
    /// The program itself.
    path: PathBuf,
}

impl Program {
    /// A program of the test's own words, written where the test can keep it.
    fn of(name: &str, source: &str) -> Program {
        let directory = std::env::temp_dir().join(format!("emo8067-gui-{name}"));
        std::fs::create_dir_all(&directory).expect("the directory");
        let path = directory.join("program.asm");
        std::fs::write(&path, source).expect("the program");
        Program { directory, path }
    }

    /// The path, as the binary takes it.
    fn argument(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// The file a path names, for the assertions about the refusals.
fn named(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[test]
fn the_listing_is_anchored_on_the_instruction_pointer() {
    let program = Program::of("anchor", "org 100h\nmov ax, 1234h\nhlt\n");

    let app = App::new(Some(program.argument())).expect("the program loads");

    assert_eq!(
        app.ui.disassembly_at, 0x0100,
        "the listing starts where the machine is"
    );
    assert!(
        app.rows.first().is_some_and(|row| row.here),
        "and the machine's own instruction carries the marker"
    );
    assert_eq!(
        app.rows,
        listing(
            &app.panels,
            Here {
                segment: 0,
                offset: 0x0100
            }
        ),
        "and the rows are the model's own"
    );
}

#[test]
fn an_empty_machine_still_opens_a_window() {
    let app = App::new(None).expect("the machine is empty");

    assert_eq!(app.name, "no program", "there is no program to name");
    assert_eq!(app.cpu.regs.ip, 0, "the machine is at its power-on state");
    assert_eq!(app.cpu.mem.read_u8(0, 0), 0, "and its memory is empty");
    assert_eq!(
        app.rows,
        listing(
            &app.panels,
            Here {
                segment: 0,
                offset: 0
            }
        ),
        "and the listing is the model's own"
    );
}

#[test]
fn a_program_that_does_not_assemble_is_refused() {
    let program = Program::of("broken", "nosuchinstruction ax\n");

    let error = App::new(Some(program.argument())).expect_err("the program does not assemble");

    assert!(
        error.contains(&named(&program.path)),
        "the refusal names the file: {error}"
    );
    assert!(
        error.contains("unknown instruction"),
        "and says what it is: {error}"
    );
}

#[test]
fn a_program_that_does_not_load_is_refused_with_its_file() {
    // The load segment and the load offset put the image at the very end
    // of the memory, so the last bytes of it do not fit.
    let program = Program::of(
        "too-big",
        "#load_segment=FFFF#\n#load_offset=000F#\nmov ax, 1234h\n",
    );

    let error = App::new(Some(program.argument())).expect_err("the program does not load");

    assert!(
        error.contains(&named(&program.path)),
        "the refusal names the file: {error}"
    );
    assert!(error.contains("does not fit"), "and says why: {error}");
}
