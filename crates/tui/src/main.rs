//! The terminal debugger, run.

use emo8067_tui::app;

fn main() -> std::process::ExitCode {
    let path = std::env::args().nth(1);
    let mut app = match app::App::new(path) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("emo8067-tui: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    // A terminal that cannot be had is a refusal, not a panic: the TUI is
    // the product's front end, and it says so.
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("emo8067-tui: the terminal could not be had: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let result = app.run(&mut terminal);
    ratatui::restore();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("emo8067-tui: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
