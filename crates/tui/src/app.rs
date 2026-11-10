//! The TUI's state, and the keys that move it.
//!
//! The state here is the UI's own: where the cursor is, which addresses the
//! machine has been at, and the breakpoints. Everything the panels show is
//! drawn from the model in `emo8067-debug`, which is a pure function of the
//! machine and of the UI state struct.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_debug::breakpoints::Breakpoints;
use emo8067_debug::panels::{Panels, Ui};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What the TUI knows.
#[derive(Debug)]
pub struct App {
    /// The machine.
    pub cpu: Cpu,
    /// The name of the program, for the title.
    pub name: String,
    /// The breakpoints and the watchpoints.
    pub breakpoints: Breakpoints,
    /// The addresses the machine has been at, in order.
    pub executed: Vec<u16>,
    /// Which line of the listing the cursor is on.
    pub cursor: usize,
    /// The UI's state, which the panels are drawn from.
    pub ui: Ui,
    /// Whether the help overlay is up.
    pub help: bool,
    /// Whether the program has stopped for good.
    pub finished: bool,
    /// Whether the user asked to quit.
    pub quit: bool,
    /// What the status line says, when there is something to say.
    pub status: Option<String>,
}

impl App {
    /// The TUI, with a program loaded.
    ///
    /// Without a file the machine is empty: the listing has nothing to show
    /// and the keys still work.
    pub fn new(path: Option<String>) -> Result<App, String> {
        let mut app = App {
            cpu: Cpu::new(),
            name: "no program".to_string(),
            breakpoints: Breakpoints::new(),
            executed: Vec::new(),
            cursor: 0,
            ui: Ui::default(),
            help: false,
            finished: false,
            quit: false,
            status: None,
        };
        let Some(path) = path else {
            return Ok(app);
        };
        let file = PathBuf::from(&path);
        let program = emo8067_asm::program::assemble_file(&file, &[])
            .map_err(|error| format!("{path}: {error:?}"))?;
        app.name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        app.cpu = emo8067_debug::machine::load(&program)?;
        Ok(app)
    }

    /// The panels, drawn from the machine and the UI's state.
    pub fn panels(&self) -> Panels {
        let mut ui = self.ui;
        // The listing follows `CS:IP`, and the research says it is never
        // asked to disassemble a range.
        ui.disassembly_at = self.cpu.regs.ip;
        Panels::of(&self.cpu, &ui)
    }

    /// The loop: draw, then wait for a key.
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
        while !self.quit {
            terminal.draw(|frame| {
                crate::ui::draw(self, frame);
            })?;
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.key(key);
                    }
                }
            }
        }
        Ok(())
    }

    /// What a key does.
    pub fn key(&mut self, key: KeyEvent) {
        // The help overlay takes every key but the one that closes it.
        if self.help {
            if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc {
                self.help = false;
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::F(8) => self.step(),
            KeyCode::F(5) => self.run_to_the_next_breakpoint(),
            KeyCode::F(9) => self.toggle_breakpoint(),
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => self.cursor += 1,
            _ => {}
        }
    }

    /// One instruction.
    pub fn step(&mut self) {
        if self.finished {
            return;
        }
        let at = self.cpu.regs.ip;
        match self.cpu.step() {
            Ok(info) => {
                if !self.executed.contains(&at) {
                    self.executed.push(at);
                }
                self.cursor = 0;
                if info.opcode == 0xF4 {
                    self.finished = true;
                }
            }
            Err(error) => {
                self.finished = true;
                self.status = Some(format!("the machine stopped: {error:?}"));
            }
        }
    }

    /// Run until a breakpoint, a halt or the end of the budget.
    fn run_to_the_next_breakpoint(&mut self) {
        let budget = 100_000u32;
        for _ in 0..budget {
            let at = self.cpu.regs.ip;
            if self.breakpoints.stops_at(
                &emo8067_debug::machine::Machine::new(&self.cpu),
                self.cpu.regs.segment(emo8067_core::regs::Seg::Cs),
                at,
            ) {
                self.cursor = 0;
                return;
            }
            self.step();
            if self.finished {
                return;
            }
        }
    }

    /// Set a breakpoint at the cursor, or take it away.
    fn toggle_breakpoint(&mut self) {
        let panels = self.panels();
        let Some(line) = panels.disassembly.get(self.cursor) else {
            return;
        };
        let address = line.offset;
        let segment = line.segment;
        if self.breakpoints.remove(segment, address).is_some() {
            return;
        }
        let _ = self.breakpoints.add(segment, address, None);
    }
}
