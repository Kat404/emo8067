//! The GUI's state, and the frame it draws.
//!
//! The state here is the UI's own: the machine and the state the panels
//! are drawn from. Everything the window shows is drawn from the model in
//! `emo8067-debug`, which is a pure function of the machine and the UI
//! state struct.
//!
//! **The machine is read once.** The constructor loads it and draws the
//! panels, and nothing in this unit moves the machine, so reading it again
//! would only reproduce what is already there.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_core::regs::Seg;
use emo8067_debug::panels::{Panels, Ui};

use crate::ui::{Here, Row, listing, window};

/// What the GUI knows.
#[derive(Clone, Debug)]
pub struct App {
    /// The machine.
    pub cpu: Cpu,
    /// The name of the program, for the window's title.
    pub name: String,
    /// The UI's state, which the panels are drawn from.
    pub ui: Ui,
    /// The panels, drawn from the machine and the UI's state.
    pub panels: Panels,
    /// The listing's rows, drawn from the panels.
    pub rows: Vec<Row>,
}

impl App {
    /// The GUI, with a program loaded.
    ///
    /// Without a file the machine is empty: the listing has nothing to show
    /// and the window still opens.
    pub fn new(path: Option<String>) -> Result<App, String> {
        let (cpu, name) = match path {
            Some(path) => {
                let file = PathBuf::from(&path);
                let program = emo8067_asm::program::assemble_file(&file, &[])
                    .map_err(|error| format!("{path}: {error}"))?;
                let name = file
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                // The load is refused with the file's name as well: the
                // refusal names the program and not only the reason.
                let cpu = emo8067_debug::machine::load(&program)
                    .map_err(|error| format!("{path}: {error}"))?;
                (cpu, name)
            }
            None => (Cpu::new(), "no program".to_string()),
        };
        // Where the machine is, read once: the listing's start and the
        // marker of the current row both come from it.
        let here = Here {
            segment: cpu.regs.segment(Seg::Cs),
            offset: cpu.regs.ip,
        };
        let ui = Ui {
            // The listing follows `CS:IP`, the way the TUI's follows it.
            disassembly_at: here.offset,
            ..Ui::default()
        };
        let panels = Panels::of(&cpu, &ui);
        let rows = listing(&panels, here);
        Ok(App {
            cpu,
            name,
            ui,
            panels,
            rows,
        })
    }
}

impl eframe::App for App {
    /// The state update, once before every frame.
    ///
    /// eframe 0.36 replaced `App::update` with this method and `App::ui`
    /// (`odd/research/documentation-guide.md:834`-`:851`), and the widely
    /// copied form does not compile there.
    ///
    /// The model is drawn once, in the constructor, and nothing in this
    /// unit moves the machine: a rebuild would only reproduce the panels
    /// that are already there. The actions, when they arrive, rebuild it.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The machine does not move, so there is nothing to update.
    }

    /// The window: the disassembly, drawn from the rows.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        window(ui, &self.rows);
    }
}
