//! The TUI's state, and the keys that move it.
//!
//! The state here is the UI's own: where the cursor is, which addresses the
//! machine has been at, and the breakpoints. Everything the panels show is
//! drawn from the model in `emo8067-debug`, which is a pure function of the
//! machine and the UI state struct.

use std::path::PathBuf;

use emo8067_core::Cpu;
use emo8067_core::bios::{
    SCAN_BACKSPACE, SCAN_DELETE, SCAN_DOWN, SCAN_END, SCAN_ENTER, SCAN_ESCAPE, SCAN_FUNCTION_FIRST,
    SCAN_HOME, SCAN_INSERT, SCAN_LEFT, SCAN_PAGE_DOWN, SCAN_PAGE_UP, SCAN_RIGHT, SCAN_TAB, SCAN_UP,
    scan_code,
};
use emo8067_core::regs::{Reg16, Seg};
use emo8067_debug::Access;
use emo8067_debug::breakpoints::{Breakpoints, Entry};
use emo8067_debug::expr;
use emo8067_debug::machine::Machine;
use emo8067_debug::panels::{Flag, Panels, Register, Ui};
use emo8067_debug::source::SourceView;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::console::{self, Console};
use crate::ui;

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

/// What the console says when the machine has stopped for good.
const HALTED: &str = "the machine is halted";

/// The keys the help must name.
///
/// The handler binds these, and the help names them: a key that is bound
/// and not named is a key the user does not know about. The quit shortcut
/// `Ctrl-C` is bound and not named: it is a shortcut and not a key of
/// the screen.
pub const BOUND_KEYS: [&str; 28] = [
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "Tab",
    "1",
    "2",
    "3",
    "4",
    "5",
    "b",
    ":",
    "e",
    "g",
    "w",
    "x",
    "↑",
    "↓",
    "←",
    "→",
    "PgUp",
    "PgDn",
    "Enter",
    "Backspace",
    "Esc",
    "?",
    "q",
];
/// How many instructions the machine may run before the budget is out.
///
/// The budget bounds the run to the next breakpoint and the step over:
/// both of them may run the machine for a long while, and neither may run
/// it for ever.
const BUDGET: u64 = 100_000;

/// The message the budget's end is said in.
///
/// The run and the step over say it the same way.
fn out_of_budget() -> String {
    format!("the budget of {BUDGET} instructions ran out")
}

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
    /// The address of the instruction that ran last.
    ///
    /// The step back takes it off the list of what has run: the
    /// instruction is not executed any more. It is the address of the
    /// instruction that ran, and not the address the machine was at when
    /// the step back was pressed — that is the next instruction, which
    /// never ran.
    pub last_ran: Option<u16>,
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
    /// Whether the console is open where the listing is.
    ///
    /// The console is a window and not a sixth column: it takes the
    /// listing's place in the main band, the way the stack and the
    /// conditions do, and only one of the three holds that place at a
    /// time.
    pub console_open: bool,
    /// Whether the guest's screen takes the terminal over.
    ///
    /// The screen is a takeover and not a window in the listing's place:
    /// it takes every row of the screen but the status bar, and it shows
    /// the page the guest wrote to and not a copy of it.
    pub screen_open: bool,
    /// The console: its command line, its log and where its next dump
    /// begins.
    ///
    /// The console is the TUI's own: the commands it takes are the
    /// reference's own, and the words it says are this project's.
    pub console: Console,
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
            last_ran: None,
            steps: 0,
            cursor: 0,
            focus: Focus::Disassembly,
            register_cursor: 0,
            flag_cursor: 0,
            stack_open: false,
            stack_scroll: 0,
            conditions_open: false,
            condition_cursor: 0,
            console_open: false,
            screen_open: false,
            console: Console::default(),
            edit: None,
            goto: None,
            goto_segment: None,
            ui: Ui::default(),
            help: false,
            finished: false,
            quit: false,
            status: None,
        };
        if let Some(path) = path {
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
        }
        // The journal is on: the step back is one of the keys, and the
        // machine keeps the steps it can undo.
        app.cpu.enable_history();
        Ok(app)
    }

    /// The panels, drawn from the machine and the UI's state.
    pub fn panels(&self) -> Panels {
        self.panels_with(self.ui.disassembly_len)
    }

    /// The panels, with the listing as long as the band that holds it.
    ///
    /// The listing asks the model for as many instructions as the band
    /// holds: the model reads a window of bytes and gives the
    /// instructions it finds in it, and the band says how many of them
    /// are shown.
    pub fn panels_with(&self, disassembly_len: usize) -> Panels {
        let mut ui = self.ui;
        // The listing follows `CS:IP`, and the research says it is never
        // asked to disassemble a range.
        ui.disassembly_at = self.cpu.regs.ip;
        // The listing is as long as the band that holds it.
        ui.disassembly_len = disassembly_len;
        // The memory window looks through the segment the goto named, and
        // through the data segment when no goto has named one.
        ui.memory_segment = self.memory_segment();
        // The stack window follows the stack pointer, and the scroll is
        // where the user has moved it to.
        ui.stack_at = self.stack_at();
        // The screen follows the machine's active video page: the guest
        // draws on the page the BIOS has active, and that is the page the
        // panel shows.
        ui.screen_page = self.cpu.video().page;
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
        // The console takes every key too, while it is open: it is a
        // window with a command line in it, and the keys go there.
        if self.console_open {
            self.console_key(key);
            return;
        }
        // The quit keys are the TUI's own, in every mode: they quit, and
        // they never reach the guest.
        let quitting = match key.code {
            KeyCode::Char('q') => true,
            KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
            _ => false,
        };
        // And the takeover has the focus: the keyboard is the guest's.
        let takeover = self.screen_open;
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
            KeyCode::Char(':') => self.toggle_console(),
            KeyCode::Char('x') => self.toggle_screen(),
            KeyCode::Char('e') => self.open_edit(),
            KeyCode::Char('g') => self.open_goto(),
            KeyCode::Char('w') => self.toggle_watchpoint(),
            KeyCode::Esc => self.close_window(),
            KeyCode::Tab => self.focus = self.other_focus(),
            KeyCode::F(6) => self.step_back(),
            KeyCode::F(7) => self.step_over(),
            KeyCode::F(8) => {
                self.step();
            }
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
            // A key the TUI does not bind is the guest's — and in the
            // takeover the key below is the one that reaches it.
            _ if !takeover => self.forward_to_guest(key),
            _ => {}
        }
        // And in the takeover the keys go to the guest, all of them but the
        // quit keys: in that mode the user is typing to the guest, and the
        // arrows are the guest's most wanted key.
        if takeover && !quitting {
            self.forward_to_guest(key);
        }
        // The cursor never leaves the listing.
        let lines = self.panels().disassembly.len();
        if lines == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(lines - 1);
        }
    }

    /// Put the key into the guest's keyboard.
    ///
    /// The scan code is the IBM PC's set 1, and the ASCII character is what
    /// the guest reads. A character that is not ASCII is not a keystroke:
    /// it is skipped.
    fn forward_to_guest(&mut self, key: KeyEvent) {
        let Some((scan_code, ascii)) = key_of(key.code) else {
            return;
        };
        self.cpu.push_key(scan_code, ascii);
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
        // The takeover has no cursor of its own: the listing's cursor is
        // not on the screen while the guest's screen holds it, and the
        // keys that act on the listing have nothing to act on.
        if self.screen_open {
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
        // Only one window holds the listing's place: the conditions pane,
        // the console and the guest's screen give way to the stack.
        self.conditions_open = false;
        self.console_open = false;
        self.screen_open = false;
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
        // Only one window holds the listing's place: the stack, the console
        // and the guest's screen give way to the conditions.
        self.stack_open = false;
        self.console_open = false;
        self.screen_open = false;
        // The cursor starts at the first entry of the listing.
        self.condition_cursor = 0;
    }

    /// Open the console where the listing is, or close it again.
    ///
    /// The console is a window and not a sixth column: it takes the
    /// listing's place in the main band, the way the stack and the
    /// conditions do.
    fn toggle_console(&mut self) {
        if self.console_open {
            self.close_console();
            return;
        }
        self.console_open = true;
        // Only one window holds the listing's place: the stack, the
        // conditions and the guest's screen give way to the console.
        self.stack_open = false;
        self.conditions_open = false;
        self.screen_open = false;
    }

    /// Open the guest's screen, or close it again.
    ///
    /// The screen is a takeover and not a window in the listing's place:
    /// it takes every row of the screen but the status bar, and it shows
    /// the guest's own text page.
    fn toggle_screen(&mut self) {
        if self.screen_open {
            self.close_window();
            return;
        }
        self.screen_open = true;
        // Only one window holds the screen: the stack, the conditions and
        // the console give way to the guest's screen.
        self.stack_open = false;
        self.conditions_open = false;
        self.console_open = false;
    }

    /// Close the console.
    ///
    /// The command line goes with it: a command that was half typed is
    /// abandoned, and the log stays where it is.
    fn close_console(&mut self) {
        self.console_open = false;
        self.console.text.clear();
    }

    /// Close the window that holds the listing's place, whichever it is.
    ///
    /// The stack, the conditions, the console and the guest's screen are
    /// four windows over the same screen, and the listing comes back when
    /// they close.
    fn close_window(&mut self) {
        self.stack_open = false;
        self.conditions_open = false;
        self.screen_open = false;
        self.close_console();
    }

    /// Whether a window holds the listing's place.
    ///
    /// The listing's cursor is not on the screen while a window holds its
    /// place, and the keys that act on the listing have nothing to act on.
    ///
    /// The guest's screen holds the screen itself, and it holds the
    /// listing's place with it: the takeover takes the listing's place
    /// and every other row of the screen with it.
    fn window_open(&self) -> bool {
        self.stack_open || self.conditions_open || self.console_open || self.screen_open
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

    /// A key, while the console is open.
    ///
    /// The console takes every key, the way the edit and the goto do: the
    /// characters go on the command line and `Enter` runs what is there.
    ///
    /// The `:` closes the console when the command line is empty; with
    /// text on the line it is a character, because a memory reference is
    /// written `SSSS:OOOO`.
    fn console_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.close_console(),
            KeyCode::Enter => {
                let text = std::mem::take(&mut self.console.text);
                self.run_console_command(&text);
            }
            KeyCode::Backspace => {
                self.console.text.pop();
            }
            KeyCode::Char(':') if self.console.text.is_empty() => self.close_console(),
            KeyCode::Char(character) => self.console.text.push(character),
            _ => {}
        }
    }

    /// Run the command that was typed, and put what it says in the log.
    ///
    /// The command is read first, and a command that cannot be read is
    /// refused before anything is touched: the machine, the breakpoints and
    /// the counters are left exactly as they were.
    fn run_console_command(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        // The console's own message is the last one in the status line.
        self.status = None;
        // The log says what was run, the way a console's own log does, and
        // it says it before the answer: a command that is refused is in
        // the log too.
        self.console.output.push(format!("> {text}"));
        let command = match console::parse(text, &self.cpu) {
            Ok(command) => command,
            Err(refusal) => {
                self.refuse(refusal);
                return;
            }
        };
        match self.run_command(command) {
            Ok(lines) => {
                self.console.output.extend(lines);
                self.trim_console_output();
            }
            Err(refusal) => self.refuse(refusal),
        }
    }

    /// Say that the console cannot do what it was asked.
    ///
    /// The refusal is in the console's own words, and it is in the log and
    /// in the status line. Nothing else is touched: the machine, the
    /// breakpoints and the counters are left as they were.
    fn refuse(&mut self, refusal: console::Refusal) {
        let text = refusal.to_string();
        self.console.output.push(text.clone());
        self.trim_console_output();
        self.status = Some(text);
    }

    /// Keep the log from growing without end.
    ///
    /// The log holds the last thousand lines: the oldest are the ones that
    /// go, because a long session must not grow the UI's memory.
    fn trim_console_output(&mut self) {
        let extra = self
            .console
            .output
            .len()
            .saturating_sub(console::OUTPUT_LINES);
        if extra > 0 {
            self.console.output.drain(0..extra);
        }
    }

    /// Run a command against the machine, and say what it says.
    ///
    /// The commands act on the machine: they read it, they write it and
    /// they step it. The registers and the flags are said as the panels
    /// draw them, so the project grows one format and not two.
    fn run_command(&mut self, command: console::Command) -> Result<Vec<String>, console::Refusal> {
        match command {
            console::Command::Registers => {
                let panels = self.panels();
                let cells = panels.registers.cells();
                let mut lines: Vec<String> = cells.iter().map(ui::register_text).collect();
                // The instruction at `CS:IP` is the listing's own first
                // line: the listing follows the instruction pointer.
                if let Some(line) = panels.disassembly.first() {
                    lines.push(ui::line_text(line));
                }
                Ok(lines)
            }
            console::Command::ReadRegister(register) => {
                let panels = self.panels();
                let cells = panels.registers.cells();
                Ok(cells
                    .iter()
                    .filter(|cell| cell.register == register)
                    .map(ui::register_text)
                    .collect())
            }
            console::Command::WriteRegister { register, value } => {
                // The write goes through the model's own door.
                register.write(&mut self.cpu, value);
                let panels = self.panels();
                let cells = panels.registers.cells();
                Ok(cells
                    .iter()
                    .filter(|cell| cell.register == register)
                    .map(ui::register_text)
                    .collect())
            }
            console::Command::Flags => {
                let panels = self.panels();
                Ok(Flag::ALL
                    .iter()
                    .map(|flag| ui::flag_text(*flag, panels.registers.flag(*flag)))
                    .collect())
            }
            console::Command::ReadFlag(flag) => {
                let panels = self.panels();
                Ok(vec![ui::flag_text(flag, panels.registers.flag(flag))])
            }
            console::Command::WriteFlag { flag, value } => {
                // The write goes through the model's own door.
                flag.write(&mut self.cpu, value);
                let panels = self.panels();
                Ok(vec![ui::flag_text(flag, panels.registers.flag(flag))])
            }
            console::Command::Step => {
                if self.finished {
                    return Ok(vec![HALTED.to_string()]);
                }
                self.step();
                if self.finished {
                    return Ok(vec![
                        self.status.clone().unwrap_or_else(|| HALTED.to_string()),
                    ]);
                }
                let panels = self.panels();
                Ok(panels
                    .disassembly
                    .first()
                    .map(|line| vec![ui::line_text(line)])
                    .unwrap_or_default())
            }
            console::Command::StepOver => {
                if self.finished {
                    return Ok(vec![HALTED.to_string()]);
                }
                self.step_over();
                if self.finished {
                    return Ok(vec![
                        self.status.clone().unwrap_or_else(|| HALTED.to_string()),
                    ]);
                }
                let panels = self.panels();
                Ok(panels
                    .disassembly
                    .first()
                    .map(|line| vec![ui::line_text(line)])
                    .unwrap_or_default())
            }
            console::Command::Dump { address, count } => {
                let (segment, offset) = match address {
                    Some(address) => (self.segment_of_address(address.segment), address.offset),
                    None => match self.console.dump {
                        Some(dump) => (dump.segment, dump.next()),
                        None => (self.cpu.regs.segment(Seg::Cs), self.cpu.regs.ip),
                    },
                };
                let bytes: Vec<u8> = (0..count)
                    .map(|index| {
                        self.cpu
                            .mem
                            .read_u8(segment, offset.wrapping_add(index as u16))
                    })
                    .collect();
                let mut lines = Vec::new();
                for (row, chunk) in bytes.chunks(16).enumerate() {
                    let at = offset.wrapping_add((row * 16) as u16);
                    lines.push(console::dump_row(segment, at, chunk));
                }
                // The console keeps where its last dump was, so that a
                // bare `d` shows the next block.
                self.console.dump = Some(console::Dump {
                    segment,
                    offset,
                    count,
                });
                Ok(lines)
            }
            console::Command::ReadPort { port, word } => {
                let mut bus = self.cpu.bus.borrow_mut();
                Ok(vec![if word {
                    let value = bus.read_word(port);
                    format!("port {port:04X} = {value:04X}")
                } else {
                    let value = bus.read(port);
                    format!("port {port:04X} = {value:02X}")
                }])
            }
            console::Command::WritePort { port, value, word } => {
                {
                    let mut bus = self.cpu.bus.borrow_mut();
                    if word {
                        bus.write_word(port, value);
                    } else {
                        bus.write(port, value as u8);
                    }
                }
                Ok(vec![if word {
                    format!("wrote {value:04X} to port {port:04X}")
                } else {
                    format!("wrote {value:02X} to port {port:04X}")
                }])
            }
            console::Command::Compare {
                first,
                count,
                second,
            } => {
                let first_segment = self.segment_of_address(first.segment);
                let second_segment = self.segment_of_address(second.segment);
                let mut lines = Vec::new();
                for index in 0..count {
                    let at = index as u16;
                    let one = self
                        .cpu
                        .mem
                        .read_u8(first_segment, first.offset.wrapping_add(at));
                    let other = self
                        .cpu
                        .mem
                        .read_u8(second_segment, second.offset.wrapping_add(at));
                    // Only the bytes that differ are said: "The command
                    // outputs only those addresses with values that are
                    // different. Equal bytes are not printed out."
                    if one != other {
                        lines.push(console::compare_row(
                            first_segment,
                            first.offset.wrapping_add(at),
                            one,
                            second_segment,
                            second.offset.wrapping_add(at),
                            other,
                        ));
                    }
                }
                Ok(lines)
            }
        }
    }

    /// The segment an address of the console is in.
    ///
    /// The segment is the one the address named, and the code segment when
    /// it named none: "If no segment is specified CS is used by default."
    fn segment_of_address(&self, segment: Option<console::Segment>) -> u16 {
        match segment {
            Some(console::Segment::Register(seg)) => self.cpu.regs.segment(seg),
            Some(console::Segment::Number(number)) => number,
            None => self.cpu.regs.segment(Seg::Cs),
        }
    }

    /// One instruction.
    ///
    /// It is `true` when the machine stopped at a watchpoint: the write
    /// the instruction left hit one, and the machine waits there. Every
    /// path a step goes through — the `F8` key, the `F5` run, the `F7`
    /// step over and the console's `t` and `p` — goes through here, so
    /// they all see it.
    pub fn step(&mut self) -> bool {
        if self.finished {
            return false;
        }
        let at = self.cpu.regs.ip;
        match self.cpu.step() {
            Ok(info) => {
                self.steps += 1;
                if !self.executed.contains(&at) {
                    self.executed.push(at);
                }
                self.last_ran = Some(at);
                self.cursor = 0;
                if info.opcode == 0xF4 {
                    self.finished = true;
                }
                self.watchpoint_hit()
            }
            Err(error) => {
                self.finished = true;
                self.status = Some(format!("the machine stopped: {error:?}"));
                // The instruction was attempted, and the journal holds a
                // record for it, so its address is the one the step back
                // takes off the list.
                self.last_ran = Some(at);
                false
            }
        }
    }

    /// Whether the write the last instruction left hit a watchpoint.
    ///
    /// The watchpoint is the model's own, and the model says whether the
    /// write hit it. The machine stops where it fired, and the status
    /// names the address that was written, the way the console prints its
    /// addresses. An instruction that wrote nothing fires nothing: the
    /// core clears the last write at the start of every instruction, so
    /// the write of the step before is not this step's.
    fn watchpoint_hit(&mut self) -> bool {
        let write = self.cpu.mem.last_write;
        let hit = write.is_some_and(|write| !self.breakpoints.written(Some(write)).hit.is_empty());
        match (hit, write) {
            (true, Some((segment, offset, _))) => {
                self.status = Some(format!("the machine wrote {segment:04X}:{offset:04X}"));
                true
            }
            _ => {
                self.status = None;
                false
            }
        }
    }

    /// Run until a breakpoint, a halt or the end of the budget.
    fn run_to_the_next_breakpoint(&mut self) {
        let budget = BUDGET;
        for _ in 0..budget {
            if self.at_breakpoint() {
                self.cursor = 0;
                return;
            }
            let stopped = self.step();
            if stopped || self.finished {
                return;
            }
        }
        self.status = Some(out_of_budget());
    }

    /// Rewind the machine one instruction.
    ///
    /// The core keeps the journal — the registers, the flags and the bytes
    /// the instruction wrote — and this puts them back, so the machine is
    /// where it was before that instruction. The listing's cursor goes
    /// back with it: the listing is anchored at `CS:IP`.
    pub fn step_back(&mut self) {
        if !self.cpu.step_back() {
            self.status = Some(if self.cpu.history.is_broken() {
                // A step that wrote too much cannot be undone, and the
                // machine's memory has moved past it.
                "the step was too big to undo".to_string()
            } else if self.cpu.history.is_discarded() {
                // A step that wrote too much threw the recorded steps
                // away: they cannot be undone either.
                "the history was discarded by a step that wrote too much".to_string()
            } else {
                "There are no recorded back steps ....".to_string()
            });
            return;
        }
        self.cursor = 0;
        // The address that is not executed any more goes off the list: it
        // is the address of the instruction that ran last.
        if let Some(ran) = self.last_ran.take() {
            self.executed.retain(|address| *address != ran);
        }
        self.steps = self.steps.saturating_sub(1);
        // The debugger follows the machine: the machine is not halted any
        // more, so the debugger is not finished either.
        self.finished = self.cpu.halted;
        self.status = None;
    }

    /// Run a procedure or an interrupt call to its end.
    ///
    /// The core keeps the depth of the calls, and this asks it how deep the
    /// machine is: an instruction that is not a call is stepped once, the
    /// way `F8` does, and a call is run until the depth is back where it
    /// was — which is where the call returns.
    ///
    /// A breakpoint inside the procedure stops it, the way the run does:
    /// the step over may not run past a breakpoint. The place the user
    /// started from is not reached by the step over, and the instruction
    /// there runs as it does under `F8`.
    pub fn step_over(&mut self) {
        let depth = self.cpu.call_depth;
        for _ in 0..BUDGET {
            let stopped = self.step();
            if stopped || self.finished {
                return;
            }
            if self.cpu.call_depth <= depth {
                return;
            }
            if self.at_breakpoint() {
                self.status = Some("the machine stopped at a breakpoint".to_string());
                return;
            }
        }
        self.status = Some(out_of_budget());
    }

    /// Whether the machine is at an address a breakpoint sits on.
    ///
    /// The run asks the same question before every instruction it runs,
    /// and the step over asks it too.
    fn at_breakpoint(&self) -> bool {
        self.breakpoints.stops_at(
            &Machine::new(&self.cpu),
            self.cpu.regs.segment(Seg::Cs),
            self.cpu.regs.ip,
        )
    }

    /// Put a watchpoint on the byte under the memory cursor, or take it
    /// away.
    ///
    /// The watchpoint is the model's own: the conditions pane draws the
    /// model's listing, and the TUI holds no copy of it. It watches the
    /// writes, because a write of the value that was already there is
    /// still a write.
    fn toggle_watchpoint(&mut self) {
        let segment = self.memory_segment();
        let offset = self.ui.memory_at;
        if self.breakpoints.unwatch(segment, offset).is_some() {
            return;
        }
        self.breakpoints.watch(segment, offset, Access::Write);
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

/// The function keys the set names: the ten of the original keyboard and
/// the two the enhanced one added. A key past them is not a keystroke.
const FUNCTION_KEYS: u8 = 12;

/// The scan code and the ASCII character of a key.
///
/// The scan code is the IBM PC's set 1: the corpus is the evidence inside
/// the repository — `re/extracted/app/examples/snake.asm:37`-`:41` names
/// the four arrow codes "bios key codes" and compares the returned `AH`
/// against them.
///
/// A character that is not ASCII is not a keystroke: there is none.
fn key_of(code: KeyCode) -> Option<(u8, u8)> {
    match code {
        KeyCode::Char(character) => {
            if !character.is_ascii() {
                return None;
            }
            let ascii = u8::try_from(character).ok()?;
            Some((scan_code(ascii), ascii))
        }
        KeyCode::Enter => Some((SCAN_ENTER, b'\r')),
        KeyCode::Esc => Some((SCAN_ESCAPE, 0x1B)),
        KeyCode::Backspace => Some((SCAN_BACKSPACE, 0x08)),
        KeyCode::Tab => Some((SCAN_TAB, b'\t')),
        KeyCode::Up => Some((SCAN_UP, 0)),
        KeyCode::Down => Some((SCAN_DOWN, 0)),
        KeyCode::Left => Some((SCAN_LEFT, 0)),
        KeyCode::Right => Some((SCAN_RIGHT, 0)),
        KeyCode::Home => Some((SCAN_HOME, 0)),
        KeyCode::End => Some((SCAN_END, 0)),
        KeyCode::PageUp => Some((SCAN_PAGE_UP, 0)),
        KeyCode::PageDown => Some((SCAN_PAGE_DOWN, 0)),
        KeyCode::Insert => Some((SCAN_INSERT, 0)),
        KeyCode::Delete => Some((SCAN_DELETE, 0)),
        KeyCode::F(number) if number <= FUNCTION_KEYS => Some((function_scan_code(number), 0)),
        // A key the set has no code for is not a keystroke: it would reach
        // the guest as a NUL, and the guest cannot tell it from a real one.
        _ => None,
    }
}

/// The scan code of a function key: the ten the IBM's keyboard has, and
/// the two the enhanced one added.
fn function_scan_code(number: u8) -> u8 {
    match number {
        1..=10 => SCAN_FUNCTION_FIRST + number - 1,
        11 => 0x57,
        12 => 0x58,
        _ => 0,
    }
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
