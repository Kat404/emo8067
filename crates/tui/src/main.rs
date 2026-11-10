//! The terminal debugger, run.

use emo8067_tui::app;

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1);
    let mut app = match app::App::new(path) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("emo8067-tui: {message}");
            return Ok(());
        }
    };
    // A terminal that cannot be had is a refusal, not a panic: the TUI is
    // the product's front end, and it says so.
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("emo8067-tui: the terminal could not be had: {error}");
            return Ok(());
        }
    };
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
