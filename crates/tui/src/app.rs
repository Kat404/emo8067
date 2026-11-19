//! The TUI's state, and the keys that move it.
//!
//! The state here is the UI's own: where the cursor is, which addresses the
//! machine has been at, and the breakpoints. Everything the panels show is
//! drawn from the model in `emo8067-debug`, which is a pure function of the
//! machine and the UI state struct.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_debug::breakpoints::Breakpoints;
use emo8067_debug::expr;
use emo8067_debug::machine::Machine;
use emo8067_debug::panels::{Panels, Register, Ui};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Which pane has the cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    /// The disassembly.
    Disassembly,
    /// The registers.
    Registers,
}

/// What is being edited, in place.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Edit {
    /// The register under the cursor.
    pub register: Register,
    /// What has been typed so far.
    pub text: String,
}

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
    /// How many instructions have run.
    pub steps: u64,
    /// Which line of the listing the cursor is on.
    pub cursor: usize,
    /// Which pane has the cursor.
    pub focus: Focus,
    /// Which register of the panel the cursor is on.
    pub register_cursor: usize,
    /// What is being edited, in place.
    pub edit: Option<Edit>,
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
            steps: 0,
            cursor: 0,
            focus: Focus::Disassembly,
            register_cursor: 0,
            edit: None,
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
            .map_err(|error| format!("{path}: {error}"))?;
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
        // An open edit takes every key but the ones that end it.
        if self.edit.is_some() {
            self.edit_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('1') => self.focus = Focus::Disassembly,
            KeyCode::Char('2') => self.focus = Focus::Registers,
            KeyCode::Char('e') => self.open_edit(),
            KeyCode::Tab => self.focus = self.other_focus(),
            KeyCode::F(8) => self.step(),
            KeyCode::F(5) => self.run_to_the_next_breakpoint(),
            KeyCode::F(9) => self.toggle_breakpoint(),
            KeyCode::Up => self.move_cursor(0, -1),
            KeyCode::Down => self.move_cursor(0, 1),
            KeyCode::Left => self.move_cursor(-1, 0),
            KeyCode::Right => self.move_cursor(1, 0),
            _ => {}
        }
        // The cursor never leaves the listing.
        let lines = self.panels().disassembly.len();
        if lines == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(lines - 1);
        }
    }

    /// The other pane, for `Tab`.
    fn other_focus(&self) -> Focus {
        match self.focus {
            Focus::Disassembly => Focus::Registers,
            Focus::Registers => Focus::Disassembly,
        }
    }

    /// Move the cursor of the pane that has it.
    fn move_cursor(&mut self, horizontal: i32, vertical: i32) {
        match self.focus {
            Focus::Disassembly => self.move_listing_cursor(vertical),
            Focus::Registers => self.move_register_cursor(horizontal, vertical),
        }
    }

    /// The listing's cursor, one line up or down.
    fn move_listing_cursor(&mut self, vertical: i32) {
        if vertical < 0 {
            self.cursor = self.cursor.saturating_sub(1);
        } else if vertical > 0 {
            self.cursor += 1;
        }
    }

    /// The registers panel's cursor, on its two-column grid.
    fn move_register_cursor(&mut self, horizontal: i32, vertical: i32) {
        let last = Register::ALL.len() - 1;
        let row = (self.register_cursor / 2) as i32 + vertical;
        let column = ((self.register_cursor % 2) as i32 + horizontal).clamp(0, 1);
        if row < 0 {
            return;
        }
        self.register_cursor = ((row * 2 + column) as usize).min(last);
    }

    /// Open the register under the cursor for editing.
    fn open_edit(&mut self) {
        if self.focus != Focus::Registers {
            return;
        }
        let Some(register) = Register::ALL.get(self.register_cursor).copied() else {
            return;
        };
        self.edit = Some(Edit {
            register,
            text: String::new(),
        });
        self.status = None;
    }

    /// A key, while an edit is open.
    fn edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.edit = None;
                self.status = None;
            }
            KeyCode::Enter => self.commit_edit(),
            KeyCode::Backspace => {
                if let Some(edit) = &mut self.edit {
                    edit.text.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(edit) = &mut self.edit {
                    edit.text.push(character);
                }
            }
            _ => {}
        }
    }

    /// Write the value that was typed into the machine.
    ///
    /// The value is in the reference's own expression language, so the
    /// evaluator that reads the breakpoint conditions reads this one too:
    /// `1234h`, `4660` and `101b` are the same value, and `BX+1` is an
    /// expression.
    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.clone() else {
            return;
        };
        let text = edit.text.trim();
        if text.is_empty() {
            self.edit = None;
            self.status = None;
            return;
        }
        let outcome = expr::evaluate(text, &Machine::new(&self.cpu));
        let value = match outcome {
            Ok(value) => value,
            Err(error) => {
                self.status = Some(error.to_string());
                return;
            }
        };
        if !(0..=0xFFFF).contains(&value) {
            self.status = Some(format!("value out of range: {value}"));
            return;
        }
        edit.register.write(&mut self.cpu, value as u16);
        self.edit = None;
        self.status = None;
    }

    /// One instruction.
    pub fn step(&mut self) {
        if self.finished {
            return;
        }
        let at = self.cpu.regs.ip;
        match self.cpu.step() {
            Ok(info) => {
                self.steps += 1;
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
        let budget = 100_000u64;
        for _ in 0..budget {
            let at = self.cpu.regs.ip;
            if self.breakpoints.stops_at(
                &Machine::new(&self.cpu),
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
        self.status = Some(format!("the budget of {budget} instructions ran out"));
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
