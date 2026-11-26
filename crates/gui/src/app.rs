//! The GUI's state, and the frame it draws.
//!
//! The state here is the UI's own: the machine and the state the panels
//! are drawn from. Everything the window shows is drawn from the model in
//! `emo8067-debug`, which is a pure function of the machine and the UI
//! state struct.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_debug::panels::{Panels, Ui};

use crate::ui::window;

/// What the GUI knows.
#[derive(Debug)]
pub struct App {
    /// The machine.
    pub cpu: Cpu,
    /// The name of the program, for the window's title.
    pub name: String,
    /// The UI's state, which the panels are drawn from.
    pub ui: Ui,
    /// The panels, drawn from the machine and the UI's state.
    pub panels: Panels,
}

impl App {
    /// The GUI, with a program loaded.
    ///
    /// Without a file the machine is empty: the listing has nothing to show
    /// and the window still opens.
    pub fn new(path: Option<String>) -> Result<App, String> {
        let mut cpu = Cpu::new();
        let mut name = "no program".to_string();
        if let Some(path) = path {
            let file = PathBuf::from(&path);
            let program = emo8067_asm::program::assemble_file(&file, &[])
                .map_err(|error| format!("{path}: {error}"))?;
            name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            cpu = emo8067_debug::machine::load(&program)?;
        }
        // The listing follows `CS:IP`, the way the TUI's follows it.
        let ui = Ui {
            disassembly_at: cpu.regs.ip,
            ..Ui::default()
        };
        let panels = Panels::of(&cpu, &ui);
        Ok(App {
            cpu,
            name,
            ui,
            panels,
        })
    }
}

impl eframe::App for App {
    /// The state update, once before every frame.
    ///
    /// eframe 0.36 replaced `App::update` with this method and `App::ui`,
    /// and the widely copied form does not compile on 0.36
    /// (`odd/research/documentation-guide.md:834`-`:851`).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The listing follows `CS:IP`, the way the TUI's follows it.
        self.ui.disassembly_at = self.cpu.regs.ip;
        self.panels = Panels::of(&self.cpu, &self.ui);
    }

    /// The window: the disassembly, drawn from the panels.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        window(ui, &self.panels);
    }
}
