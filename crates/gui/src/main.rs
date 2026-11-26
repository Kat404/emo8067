//! The windowed debugger, run.

use egui::ViewportBuilder;
use emo8067_gui::app;
use emo8067_gui::theme;

fn main() -> std::process::ExitCode {
    let path = std::env::args().nth(1);
    let app = match app::App::new(path) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("emo8067-gui: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    let name = app.name.clone();
    // A window that cannot be had is a refusal, not a panic: the GUI is the
    // TUI's sibling, and it says so.
    match eframe::run_native(
        &name,
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(theme::visuals());
            Ok(Box::new(app))
        }),
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("emo8067-gui: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
