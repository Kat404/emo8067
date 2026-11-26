//! The windowed debugger, run.
//!
//! The binary takes one argument: the path of the program to open. It
//! assembles it, loads it and opens the window on it.
//!
//! ```text
//! emo8067-gui <program.asm>
//! ```

use emo8067_gui::app;
use emo8067_gui::{install_logger, window};

/// What the binary accepts.
const USAGE: &str = "usage: emo8067-gui <program.asm>";

fn main() -> std::process::ExitCode {
    // The toolkit's diagnostics go here: the adapter, the surface and the
    // device-lost messages decide whether the window can open at all.
    if let Err(error) = install_logger(std::io::stderr) {
        eprintln!("emo8067-gui: the logger could not be installed: {error}");
    }
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    // A second program is not a thing: the binary takes one, and says so.
    if arguments.len() > 1 {
        eprintln!("emo8067-gui: one program at a time");
        eprintln!("{USAGE}");
        return std::process::ExitCode::from(2);
    }
    let app = match app::App::new(arguments.into_iter().next()) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("emo8067-gui: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    let name = app.name.clone();
    // A window that cannot be had is a refusal, not a panic: the GUI is the
    // TUI's sibling, and it says so.
    match window::open(&app, &name) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("emo8067-gui: {message}");
            std::process::ExitCode::from(2)
        }
    }
}
