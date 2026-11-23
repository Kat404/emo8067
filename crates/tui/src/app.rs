//! The TUI's state, and the keys that move it.
//!
//! The state here is the UI's own: where the cursor is, which addresses the
//! machine has been at, and the breakpoints. Everything the panels show is
//! drawn from the model in `emo8067-debug`, which is a pure function of the
//! machine and the UI state struct.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::breakpoints::{Breakpoints, Entry};
use emo8067_debug::expr;
use emo8067_debug::machine::Machine;
use emo8067_debug::panels::{Flag, Panels, Register, Ui};
use emo8067_debug::source::SourceView;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Which pane has the cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    /// The disassembly.
    Disassembly,
    /// The registers.
    Registers,
    /// The memory window.
    Memory,
    /// The flags.
    Flags,
}

/// What is being edited, in place.
///
/// The panels write three things: a register, a flag and a byte, and the
/// conditions pane writes a condition. The edit says which one of them it
/// is, because the value that is typed is written into a different place
/// each time.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Edit {
    /// The register under the cursor.
    Register {
        /// The register.
        register: Register,
        /// What has been typed so far.
        text: String,
    },
    /// The flag under the cursor.
    Flag {
        /// The flag.
        flag: Flag,
        /// What has been typed so far.
        text: String,
    },
    /// The byte under the memory cursor.
    Byte {
        /// The segment the byte is at.
        segment: u16,
        /// The offset the byte is at.
        offset: u16,
        /// What has been typed so far.
        text: String,
    },
    /// The condition of the entry under the cursor.
    ///
    /// The condition is a text and not a value: it is read by the
    /// evaluator, and it is written through the model's own door.
    Condition {
        /// The segment the entry is in.
        segment: u16,
        /// The offset the entry is at.
        offset: u16,
        /// What has been typed so far.
        text: String,
    },
}

impl Edit {
    /// What has been typed so far.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Edit::Register { text, .. }
            | Edit::Flag { text, .. }
            | Edit::Byte { text, .. }
            | Edit::Condition { text, .. } => text,
        }
    }

    /// What has been typed so far, to write into.
    pub fn text_mut(&mut self) -> &mut String {
        match self {
            Edit::Register { text, .. }
            | Edit::Flag { text, .. }
            | Edit::Byte { text, .. }
            | Edit::Condition { text, .. } => text,
        }
    }
}

/// The address the memory window is jumped to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Goto {
    /// What has been typed so far.
    pub text: String,
}

/// The bytes a row of the memory window holds.
const ROW: i32 = 16;

/// The bytes a page of the memory window holds.
const PAGE: i32 = 0x100;

/// The flags the panel shows on a row.
///
/// The flags are drawn three to a row, and their cursor moves on that
/// grid: a flag to the left or to the right, and a row up or down.
pub const FLAGS_COLUMNS: usize = 3;

/// What the TUI knows.
#[derive(Debug)]
pub struct App {
    /// The machine.
    pub cpu: Cpu,
    /// The name of the program, for the title.
    pub name: String,
    /// The program that was opened, when there is one.
    ///
    /// The program carries the source: the assembler writes down where
    /// every line landed, and the source strip is drawn from those lines.
    /// Without a program there is no source and no strip.
    pub program: Option<emo8067_asm::program::Program>,
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
    /// Which flag of the panel the cursor is on.
    pub flag_cursor: usize,
    /// Whether the stack pane is open where the listing is.
    ///
    /// The stack is a pane you open and not a fourth column: it takes the
    /// listing's place in the main band.
    pub stack_open: bool,
    /// How far the stack window is from the stack pointer.
    ///
    /// The window follows the pointer, the way the listing follows the
    /// instruction pointer, and this is the scroll the user has made from
    /// it: it is the TUI's own state, like the listing's cursor.
    pub stack_scroll: u16,
    /// Whether the conditions pane is open where the listing is.
    ///
    /// The conditions are a pane you open and not a fifth column: it takes
    /// the listing's place in the main band, the way the stack does, and
    /// only one of the two holds that place at a time.
    pub conditions_open: bool,
    /// Which entry of the conditions listing the cursor is on.
    ///
    /// The cursor is over the entries of the model's own listing, and it
    /// is the TUI's own state, like the listing's cursor.
    pub condition_cursor: usize,
    /// What is being edited, in place.
    pub edit: Option<Edit>,
    /// The address the memory window is being jumped to.
    pub goto: Option<Goto>,
    /// The segment the last goto named.
    ///
    /// The memory window looks through the segment the goto named, and
    /// through the data segment when no goto has named one: the choice is
    /// kept until another goto names another segment.
    pub goto_segment: Option<u16>,
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
            program: None,
            breakpoints: Breakpoints::new(),
            executed: Vec::new(),
            steps: 0,
            cursor: 0,
            focus: Focus::Disassembly,
            register_cursor: 0,
            flag_cursor: 0,
            stack_open: false,
            stack_scroll: 0,
            conditions_open: false,
            condition_cursor: 0,
            edit: None,
            goto: None,
            goto_segment: None,
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
        // The program is kept: the source strip is drawn from the source
        // the program carries.
        app.program = Some(program);
        Ok(app)
    }

    /// The panels, drawn from the machine and the UI's state.
    pub fn panels(&self) -> Panels {
        let mut ui = self.ui;
        // The listing follows `CS:IP`, and the research says it is never
        // asked to disassemble a range.
        ui.disassembly_at = self.cpu.regs.ip;
        // The memory window looks through the segment the goto named, and
        // through the data segment when no goto has named one.
        ui.memory_segment = self.memory_segment();
        // The stack window follows the stack pointer, and the scroll is
        // where the user has moved it to.
        ui.stack_at = self.stack_at();
        Panels::of(&self.cpu, &ui)
    }

    /// The segment the memory window looks through.
    ///
    /// The goto may name a segment, and the choice is kept: the window
    /// looks through it until another goto names another one. Without one
    /// the window looks through the data segment, the way it always has.
    #[must_use]
    pub fn memory_segment(&self) -> u16 {
        self.goto_segment.unwrap_or(self.cpu.regs.segment(Seg::Ds))
    }

    /// Where the stack window starts: the stack pointer and the scroll.
    ///
    /// The window follows the pointer, the way the listing follows the
    /// instruction pointer, and the scroll is how far the user has moved it
    /// from there.
    #[must_use]
    pub fn stack_at(&self) -> u16 {
        self.cpu
            .regs
            .word(Reg16::Sp)
            .wrapping_add(self.stack_scroll)
    }

    /// The source view: the program's own lines, and the machine's place
    /// in them.
    ///
    /// The view is a pure function of the program and of the machine, the
    /// way every other panel is a pure function of the machine and of the
    /// UI's state. `count` is how many lines the strip shows.
    ///
    /// The view is asked again on every draw, so the mark moves with the
    /// machine: a step moves the machine, and the strip follows it.
    #[must_use]
    pub fn source_view(&self, count: usize) -> SourceView {
        let Some(program) = &self.program else {
            return SourceView {
                lines: Vec::new(),
                current: None,
            };
        };
        SourceView::of(program, &self.cpu, count)
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
        // The goto takes every key too, while it is open.
        if self.goto.is_some() {
            self.goto_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('1') => self.take_listing(),
            KeyCode::Char('2') => self.focus = Focus::Registers,
            KeyCode::Char('3') => self.focus = Focus::Memory,
            KeyCode::Char('4') => self.focus = Focus::Flags,
            KeyCode::Char('5') => self.toggle_stack(),
            KeyCode::Char('b') => self.toggle_conditions(),
            KeyCode::Char('e') => self.open_edit(),
            KeyCode::Char('g') => self.open_goto(),
            KeyCode::Esc => self.close_window(),
            KeyCode::Tab => self.focus = self.other_focus(),
            KeyCode::F(8) => self.step(),
            KeyCode::F(5) => self.run_to_the_next_breakpoint(),
            // A breakpoint goes at the cursor, and the cursor is not on the
            // screen while a window holds the listing's place.
            KeyCode::F(9) if !self.window_open() => self.toggle_breakpoint(),
            KeyCode::PageUp if self.focus == Focus::Memory => self.move_memory_page(-1),
            KeyCode::PageDown if self.focus == Focus::Memory => self.move_memory_page(1),
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
            Focus::Registers => Focus::Memory,
            Focus::Memory => Focus::Flags,
            Focus::Flags => Focus::Disassembly,
        }
    }

    /// Move the cursor of the pane that has it.
    ///
    /// A window holds the listing's place while it is open, so the arrows
    /// move the window's own cursor and not the listing's cursor.
    fn move_cursor(&mut self, horizontal: i32, vertical: i32) {
        if self.conditions_open {
            self.move_condition_cursor(vertical);
            return;
        }
        if self.stack_open {
            self.move_stack_window(vertical);
            return;
        }
        match self.focus {
            Focus::Disassembly => self.move_listing_cursor(vertical),
            Focus::Registers => self.move_register_cursor(horizontal, vertical),
            Focus::Memory => self.move_memory_cursor(horizontal, vertical),
            Focus::Flags => self.move_flag_cursor(horizontal, vertical),
        }
    }

    /// The conditions pane's cursor: an entry at a time.
    ///
    /// The cursor stays on the listing: it never leaves the entries, and
    /// the listing's own cursor does not move while the pane holds its
    /// place.
    fn move_condition_cursor(&mut self, vertical: i32) {
        let count = self.breakpoints.listing().len();
        if count == 0 {
            self.condition_cursor = 0;
            return;
        }
        let at = self.condition_cursor as i32 + vertical;
        self.condition_cursor = at.clamp(0, count as i32 - 1) as usize;
    }

    /// The stack window, a word at a time.
    ///
    /// The window moves over the stack and the pointer stays where it is,
    /// so the words above the pointer come into view.
    fn move_stack_window(&mut self, vertical: i32) {
        if vertical < 0 {
            self.stack_scroll = self.stack_scroll.wrapping_sub(2);
        } else if vertical > 0 {
            self.stack_scroll = self.stack_scroll.wrapping_add(2);
        }
    }

    /// Open the stack pane where the listing is, or close it again.
    fn toggle_stack(&mut self) {
        if self.stack_open {
            self.close_window();
            return;
        }
        self.stack_open = true;
        // Only one window holds the listing's place: the conditions pane
        // gives way to the stack.
        self.conditions_open = false;
        // The window starts at the pointer: the scroll is the user's own,
        // and it begins at the top of the stack.
        self.stack_scroll = 0;
    }

    /// Open the conditions pane where the listing is, or close it again.
    ///
    /// The conditions are a window and not a fifth column: they take the
    /// listing's place in the main band, the way the stack does.
    fn toggle_conditions(&mut self) {
        if self.conditions_open {
            self.close_window();
            return;
        }
        self.conditions_open = true;
        // Only one window holds the listing's place: the stack gives way to
        // the conditions.
        self.stack_open = false;
        // The cursor starts at the first entry of the listing.
        self.condition_cursor = 0;
    }

    /// Close the window that holds the listing's place, whichever it is.
    ///
    /// The stack and the conditions are two windows over the same place,
    /// and the listing comes back when they close.
    fn close_window(&mut self) {
        self.stack_open = false;
        self.conditions_open = false;
    }

    /// Whether a window holds the listing's place.
    ///
    /// The listing's cursor is not on the screen while a window holds its
    /// place, and the keys that act on the listing have nothing to act on.
    fn window_open(&self) -> bool {
        self.stack_open || self.conditions_open
    }

    /// Take the listing, and the window that holds its place gives way to
    /// it.
    fn take_listing(&mut self) {
        self.focus = Focus::Disassembly;
        self.close_window();
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

    /// The flags panel's cursor, on its three-column grid.
    ///
    /// The flags are drawn three to a row, so the arrows move the cursor
    /// the way the panel is drawn: a flag to the left or to the right, and
    /// a row of three up or down.
    fn move_flag_cursor(&mut self, horizontal: i32, vertical: i32) {
        let last = Flag::ALL.len() - 1;
        let columns = FLAGS_COLUMNS as i32;
        let row = (self.flag_cursor / FLAGS_COLUMNS) as i32 + vertical;
        let column = ((self.flag_cursor % FLAGS_COLUMNS) as i32 + horizontal).clamp(0, columns - 1);
        if row < 0 {
            return;
        }
        self.flag_cursor = ((row * columns + column) as usize).min(last);
    }

    /// The memory window's cursor: a byte at a time, or a row of sixteen.
    fn move_memory_cursor(&mut self, horizontal: i32, vertical: i32) {
        let at = i32::from(self.ui.memory_at) + horizontal + vertical * ROW;
        self.ui.memory_at = at.clamp(0, 0xFFFF) as u16;
    }

    /// The memory window's cursor, a page at a time.
    fn move_memory_page(&mut self, pages: i32) {
        let at = i32::from(self.ui.memory_at) + pages * PAGE;
        self.ui.memory_at = at.clamp(0, 0xFFFF) as u16;
    }

    /// Open what is under the cursor for editing.
    ///
    /// A register, a flag or a byte: each of them is edited in place, with
    /// the value typed over it. The conditions pane edits the condition
    /// under its own cursor, the same way.
    fn open_edit(&mut self) {
        if self.conditions_open {
            self.open_condition_edit();
            return;
        }
        let edit = match self.focus {
            Focus::Registers => {
                let Some(register) = Register::ALL.get(self.register_cursor).copied() else {
                    return;
                };
                Edit::Register {
                    register,
                    text: String::new(),
                }
            }
            Focus::Flags => {
                let Some(flag) = Flag::ALL.get(self.flag_cursor).copied() else {
                    return;
                };
                Edit::Flag {
                    flag,
                    text: String::new(),
                }
            }
            Focus::Memory => Edit::Byte {
                segment: self.memory_segment(),
                offset: self.ui.memory_at,
                text: String::new(),
            },
            Focus::Disassembly => return,
        };
        self.edit = Some(edit);
        self.status = None;
    }

    /// Open the condition under the cursor for editing.
    ///
    /// The condition is a text and not a value: it is read by the
    /// evaluator, and it is written through the model's own door.
    fn open_condition_edit(&mut self) {
        let Some(entry) = self.condition_entry() else {
            return;
        };
        self.edit = Some(Edit::Condition {
            segment: entry.segment,
            offset: entry.offset,
            text: String::new(),
        });
        self.status = None;
    }

    /// The entry the conditions cursor is on.
    ///
    /// The listing is the model's own: the pane reads the rows out of the
    /// breakpoints and holds none of its own.
    fn condition_entry(&self) -> Option<Entry> {
        self.breakpoints
            .listing()
            .get(self.condition_cursor)
            .cloned()
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
                    edit.text_mut().pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(edit) = &mut self.edit {
                    edit.text_mut().push(character);
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
    ///
    /// What is written depends on what is being edited: a register holds
    /// sixteen bits, a flag holds one bit, and a byte holds eight. A
    /// value that does not fit is refused and the machine is left as it
    /// was. A condition is not a value at all: it is written through the
    /// model's own door.
    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.clone() else {
            return;
        };
        let text = edit.text().trim().to_string();
        // A condition that is typed empty takes the condition away, and
        // the breakpoint then always stops: an empty text is not a
        // condition. A value that is typed empty is nothing at all.
        if text.is_empty() {
            if let Edit::Condition {
                segment, offset, ..
            } = &edit
            {
                self.commit_condition(*segment, *offset, None);
                return;
            }
            self.edit = None;
            self.status = None;
            return;
        }
        let outcome = expr::evaluate(&text, &Machine::new(&self.cpu));
        let value = match outcome {
            Ok(value) => value,
            Err(error) => {
                self.status = Some(error.to_string());
                return;
            }
        };
        match &edit {
            Edit::Register { register, .. } => {
                if !(0..=0xFFFF).contains(&value) {
                    self.status = Some(format!("value out of range: {value}"));
                    return;
                }
                register.write(&mut self.cpu, value as u16);
            }
            Edit::Flag { flag, .. } => {
                if !matches!(value, 0 | 1) {
                    self.status = Some(format!("value out of range: {value}"));
                    return;
                }
                flag.write(&mut self.cpu, value == 1);
            }
            Edit::Byte {
                segment, offset, ..
            } => {
                if !(0..=0xFF).contains(&value) {
                    self.status = Some(format!("value out of range: {value}"));
                    return;
                }
                self.cpu.mem.write_u8(*segment, *offset, value as u8);
            }
            // A condition is not a value: the value was only read to know
            // that the condition reads, and the condition itself is
            // written through the model's own door.
            Edit::Condition {
                segment, offset, ..
            } => {
                self.commit_condition(*segment, *offset, Some(&text));
                return;
            }
        }
        self.edit = None;
        self.status = None;
    }

    /// Write the condition into the breakpoint.
    ///
    /// The condition goes through the model's own door, `set_condition`, so
    /// the pane holds no second opinion about what a condition is: the
    /// evaluator that reads the breakpoint's condition is the one the
    /// breakpoints use.
    ///
    /// The door has the last word: a watchpoint carries no condition, and
    /// the model says so in its own words. A condition that does not read
    /// was refused before the door was reached, and the old condition
    /// stays where it was.
    fn commit_condition(&mut self, segment: u16, offset: u16, condition: Option<&str>) {
        match self.breakpoints.set_condition(segment, offset, condition) {
            Ok(_) => {
                self.edit = None;
                self.status = None;
            }
            Err(error) => {
                self.status = Some(error.to_string());
            }
        }
    }

    /// Open the goto, to jump the memory window to an address.
    fn open_goto(&mut self) {
        if self.focus != Focus::Memory {
            return;
        }
        self.goto = Some(Goto {
            text: String::new(),
        });
        self.status = None;
    }

    /// A key, while the goto is open.
    fn goto_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.goto = None;
                self.status = None;
            }
            KeyCode::Enter => self.commit_goto(),
            KeyCode::Backspace => {
                if let Some(goto) = &mut self.goto {
                    goto.text.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(goto) = &mut self.goto {
                    goto.text.push(character);
                }
            }
            _ => {}
        }
    }

    /// Jump the memory window to the address that was typed.
    ///
    /// The text is read in the reference's own order: a name of the
    /// program's own names, then a memory reference `SSSS:OOOO`, and then
    /// a bare hexadecimal offset. A text that cannot be read is refused
    /// in the evaluator's own words, the prompt stays open, and the window
    /// does not move.
    fn commit_goto(&mut self) {
        let Some(goto) = self.goto.clone() else {
            return;
        };
        let text = goto.text.trim();
        if text.is_empty() {
            self.goto = None;
            self.status = None;
            return;
        }
        match self.target_of(text) {
            Ok(target) => {
                self.ui.memory_at = target.offset;
                // The segment the goto named is kept: the window looks
                // through it until another goto names another one.
                if let Some(segment) = target.segment {
                    self.goto_segment = Some(segment);
                }
                self.goto = None;
                self.status = None;
            }
            Err(error) => {
                self.status = Some(error);
            }
        }
    }

    /// Read the text of a goto, and say where it points.
    ///
    /// The text is read in the reference's own order:
    ///
    /// * a name of the program's own names wins,
    /// * then a memory reference `SSSS:OOOO`,
    /// * and then a bare hexadecimal offset.
    ///
    /// The name comes first because it is the most specific of the three:
    /// the user typed a name and not a number, and the program's own
    /// names are where the program's own things are. The memory reference
    /// comes next because it carries a segment as well as an offset, and
    /// the bare offset comes last because it says nothing but an offset.
    fn target_of(&self, text: &str) -> Result<Target, String> {
        // A name of the program's own names: the address it is at is an
        // offset in the segment the window looks through.
        if let Some(program) = &self.program {
            if let Some((_, at)) = program
                .names
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(text))
            {
                return Ok(Target {
                    offset: *at,
                    segment: None,
                });
            }
        }
        // A memory reference: `SSSS:OOOO`. The left half may be the name
        // of a segment, and then it is the machine's own value of it.
        if let Some((left, right)) = text.split_once(':') {
            return Ok(Target {
                offset: hexadecimal(right)?,
                segment: Some(self.segment_of(left)?),
            });
        }
        // A bare hexadecimal offset, in the segment the window looks
        // through.
        Ok(Target {
            offset: offset_of(text)?,
            segment: None,
        })
    }

    /// The segment a memory reference names.
    ///
    /// The four segment registers have names, and the machine's own value
    /// of the one that is named is the segment: `DS:0100` is the data
    /// segment the machine holds and not the number thirteen.
    fn segment_of(&self, text: &str) -> Result<u16, String> {
        let segment = match text.trim().to_ascii_uppercase().as_str() {
            "CS" => Seg::Cs,
            "DS" => Seg::Ds,
            "ES" => Seg::Es,
            "SS" => Seg::Ss,
            _ => return hexadecimal(text),
        };
        Ok(self.cpu.regs.segment(segment))
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

/// Where a goto's text points: the offset, and the segment when the text
/// named one.
struct Target {
    /// The offset the window goes to.
    offset: u16,
    /// The segment the window looks through, when the text named one.
    segment: Option<u16>,
}

/// Read a hexadecimal offset, the way the reference types one.
///
/// The reference types an address in hexadecimal and needs no suffix for
/// it: `0100` is the same address as `100h`. A text that cannot be read
/// is refused in the evaluator's own words.
fn offset_of(text: &str) -> Result<u16, String> {
    let trimmed = text.trim();
    let digits = trimmed.strip_suffix(['h', 'H']).unwrap_or(trimmed);
    hexadecimal(digits)
}

/// Read a plain hexadecimal number, with no suffix.
///
/// The two halves of a memory reference are written the way the reference
/// writes them: `SSSS:OOOO`, with no suffix at all.
fn hexadecimal(text: &str) -> Result<u16, String> {
    u16::from_str_radix(text.trim(), 16).map_err(|_| bad_syntax())
}

/// The refusal of a text that cannot be read, in the evaluator's own
/// words.
fn bad_syntax() -> String {
    expr::Error::BadSyntax.to_string()
}
