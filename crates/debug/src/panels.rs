//! The panel model.
//!
//! Every panel is a pure function of the machine's state and of a UI state
//! struct, which is what the research settled (`features-and-ux.md` §4.3):
//! the TUI and the GUI are two renderers of the same model, and nothing
//! here draws.
//!
//! The panels themselves are the ones the reference has, and the layout the
//! research draws (`features-and-ux.md` §4.4): the disassembly, the
//! registers with their flags, the memory as hex and characters, and the
//! stack.

use emo8067_core::Cpu;
use emo8067_core::bios::Video;
use emo8067_core::flags::{AF, CF, DF, IF, OF, PF, SF, TF, ZF};
use emo8067_core::regs::{Reg16, Seg};

/// What the panels show, besides the machine's own state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ui {
    /// Where the memory panel's cursor is.
    ///
    /// The panel is a window around this address and not a buffer from it:
    /// the bytes it holds are the ones around it, and the cursor is in the
    /// middle of them.
    pub memory_at: u16,
    /// The segment the memory panel looks through.
    pub memory_segment: u16,
    /// How many bytes the memory panel's window holds.
    pub memory_len: usize,
    /// Where the disassembly starts.
    pub disassembly_at: u16,
    /// How many instructions the disassembly lists.
    pub disassembly_len: usize,
    /// How many words the stack panel shows.
    pub stack_len: usize,
    /// Where the stack window starts.
    ///
    /// The window is a window over the stack and not the stack itself: it
    /// starts at this offset, so the pointer can be inside it or above it,
    /// and the words around the pointer are there to be looked at.
    pub stack_at: u16,
    /// The segment the screen panel reads the text page through.
    ///
    /// The page is at `B800:0000` by default, and the segment is the
    /// UI's own state, the way the memory window's segment is.
    pub screen_segment: u16,
    /// The page of text memory the screen panel shows.
    ///
    /// The hardware has eight pages of text memory and no more; a page
    /// past the last one is the last one.
    pub screen_page: u8,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            memory_at: 0,
            memory_segment: 0x1000,
            memory_len: 256,
            disassembly_at: 0,
            disassembly_len: 16,
            stack_len: 8,
            stack_at: 0,
            screen_segment: 0xB800,
            screen_page: 0,
        }
    }
}

/// One line of the disassembly panel.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Line {
    /// The logical address the instruction is at.
    pub segment: u16,
    /// The offset it is at.
    pub offset: u16,
    /// The instruction's bytes.
    pub bytes: Vec<u8>,
    /// The reference's own text for it.
    pub text: String,
}

/// The registers panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Registers {
    /// `AX`.
    pub ax: u16,
    /// `BX`.
    pub bx: u16,
    /// `CX`.
    pub cx: u16,
    /// `DX`.
    pub dx: u16,
    /// `SP`.
    pub sp: u16,
    /// `BP`.
    pub bp: u16,
    /// `SI`.
    pub si: u16,
    /// `DI`.
    pub di: u16,
    /// `CS`.
    pub cs: u16,
    /// `DS`.
    pub ds: u16,
    /// `ES`.
    pub es: u16,
    /// `SS`.
    pub ss: u16,
    /// `IP`.
    pub ip: u16,
    /// The flags word, as the machine holds it.
    pub flags: u16,
}

/// The registers the panel shows, in the order it shows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Register {
    /// One of the eight general registers.
    Word(Reg16),
    /// One of the four segment registers.
    Segment(Seg),
    /// The instruction pointer.
    Ip,
}

impl Register {
    /// Every register, in the order the panel shows them.
    pub const ALL: [Register; 13] = [
        Register::Word(Reg16::Ax),
        Register::Word(Reg16::Bx),
        Register::Word(Reg16::Cx),
        Register::Word(Reg16::Dx),
        Register::Word(Reg16::Si),
        Register::Word(Reg16::Di),
        Register::Word(Reg16::Bp),
        Register::Word(Reg16::Sp),
        Register::Segment(Seg::Cs),
        Register::Segment(Seg::Ds),
        Register::Segment(Seg::Es),
        Register::Segment(Seg::Ss),
        Register::Ip,
    ];

    /// The name the panel shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Register::Word(Reg16::Ax) => "AX",
            Register::Word(Reg16::Bx) => "BX",
            Register::Word(Reg16::Cx) => "CX",
            Register::Word(Reg16::Dx) => "DX",
            Register::Word(Reg16::Si) => "SI",
            Register::Word(Reg16::Di) => "DI",
            Register::Word(Reg16::Bp) => "BP",
            Register::Word(Reg16::Sp) => "SP",
            Register::Segment(Seg::Cs) => "CS",
            Register::Segment(Seg::Ds) => "DS",
            Register::Segment(Seg::Es) => "ES",
            Register::Segment(Seg::Ss) => "SS",
            Register::Ip => "IP",
        }
    }
}

impl Register {
    /// Write a value into this register.
    ///
    /// The other side of the panel's door: the model reads the machine, and
    /// this writes it.
    pub fn write(self, cpu: &mut Cpu, value: u16) {
        match self {
            Register::Word(reg) => cpu.regs.set_word(reg, value),
            Register::Segment(seg) => cpu.regs.set_segment(seg, value),
            Register::Ip => cpu.regs.ip = value,
        }
    }
}

/// One register of the panel: which one it is, and what it holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    /// Which register it is.
    pub register: Register,
    /// The value the machine holds in it.
    pub value: u16,
}

impl Cell {
    /// A cell of the panel.
    #[must_use]
    pub const fn new(register: Register, value: u16) -> Cell {
        Cell { register, value }
    }
}

/// The nine flags of the 8086.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    /// Carry.
    Cf,
    /// Parity.
    Pf,
    /// Auxiliary carry.
    Af,
    /// Zero.
    Zf,
    /// Sign.
    Sf,
    /// Trap.
    Tf,
    /// Overflow.
    Of,
    /// Interrupt enable — the manual's `IF`.
    If,
    /// Direction.
    Df,
}

impl Flag {
    /// Every flag, in the order the panel shows them.
    pub const ALL: [Flag; 9] = [
        Flag::Cf,
        Flag::Pf,
        Flag::Af,
        Flag::Zf,
        Flag::Sf,
        Flag::Tf,
        Flag::Of,
        Flag::If,
        Flag::Df,
    ];

    /// The name the manual prints.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Flag::Cf => "CF",
            Flag::Pf => "PF",
            Flag::Af => "AF",
            Flag::Zf => "ZF",
            Flag::Sf => "SF",
            Flag::Tf => "TF",
            Flag::Of => "OF",
            Flag::If => "IF",
            Flag::Df => "DF",
        }
    }

    /// The bit the flag occupies in the flags word.
    #[must_use]
    pub fn mask(self) -> u16 {
        match self {
            Flag::Cf => CF,
            Flag::Pf => PF,
            Flag::Af => AF,
            Flag::Zf => ZF,
            Flag::Sf => SF,
            Flag::Tf => TF,
            Flag::Of => OF,
            Flag::If => IF,
            Flag::Df => DF,
        }
    }

    /// Write a value into this flag.
    ///
    /// The other side of the panel's door: the model reads the machine,
    /// and this writes it. The flag is a bit of the flags word, and the
    /// other bits are left as they were.
    pub fn write(self, cpu: &mut Cpu, value: bool) {
        cpu.regs.flags.set(self.mask(), value);
    }
}

impl Registers {
    /// The registers, in the order the panel shows them: which one each is,
    /// and what the machine holds in it.
    #[must_use]
    pub fn cells(&self) -> [Cell; 13] {
        [
            Cell::new(Register::Word(Reg16::Ax), self.ax),
            Cell::new(Register::Word(Reg16::Bx), self.bx),
            Cell::new(Register::Word(Reg16::Cx), self.cx),
            Cell::new(Register::Word(Reg16::Dx), self.dx),
            Cell::new(Register::Word(Reg16::Si), self.si),
            Cell::new(Register::Word(Reg16::Di), self.di),
            Cell::new(Register::Word(Reg16::Bp), self.bp),
            Cell::new(Register::Word(Reg16::Sp), self.sp),
            Cell::new(Register::Segment(Seg::Cs), self.cs),
            Cell::new(Register::Segment(Seg::Ds), self.ds),
            Cell::new(Register::Segment(Seg::Es), self.es),
            Cell::new(Register::Segment(Seg::Ss), self.ss),
            Cell::new(Register::Ip, self.ip),
        ]
    }

    /// The value of one of the nine flags.
    #[must_use]
    pub fn flag(&self, flag: Flag) -> bool {
        self.flags & flag.mask() != 0
    }
}

/// The memory panel: the bytes around the cursor, and the same bytes as
/// characters.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemoryView {
    /// The segment the bytes are read through.
    pub segment: u16,
    /// The address the cursor is at.
    pub at: u16,
    /// The address the window starts at.
    ///
    /// The window begins before the cursor, so the bytes around it are
    /// there to be looked at.
    pub window_at: u16,
    /// Where the cursor is in the window: an index into the bytes.
    pub cursor: usize,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// The same bytes as characters: the printable ones, and a dot for the
    /// rest, which is what a hex panel shows.
    pub ascii: String,
}

/// The stack panel: the words on the stack, from the top down.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackView {
    /// The segment the stack is in.
    pub segment: u16,
    /// The offset the stack pointer is at.
    pub sp: u16,
    /// The words, from the top of the stack down.
    pub words: Vec<u16>,
}

/// The columns the guest's text page has by default.
///
/// The panel shows the geometry the machine has, and the machine's geometry
/// is the mode's: the mode sets it and the data area holds it. This is the
/// geometry a machine starts in.
pub const SCREEN_COLUMNS: usize = emo8067_core::bios::DEFAULT_COLUMNS as usize;

/// The rows the guest's text page has by default.
///
/// The panel shows the geometry the machine has, and the machine's geometry
/// is the mode's: the mode sets it and the data area holds it. This is the
/// geometry a machine starts in.
pub const SCREEN_ROWS: usize = emo8067_core::bios::DEFAULT_ROWS as usize;

/// The pages of text memory the hardware has.
pub const SCREEN_PAGES: u8 = 8;

/// The bytes between two pages of text memory.
const PAGE_STRIDE: u16 = 0x1000;

/// One cell of the guest's screen: what it says and how it is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScreenCell {
    /// The character the guest wrote.
    ///
    /// A byte that cannot be shown is a space and not a control glyph:
    /// the screen shows what the guest wrote, and a control character is
    /// not something a terminal shows.
    pub character: char,
    /// The attribute byte: the foreground, the blink, the background and
    /// the background's own brightness.
    pub attribute: u8,
}

impl ScreenCell {
    /// The foreground colour: the low three bits, one of the eight.
    #[must_use]
    pub fn foreground(&self) -> u8 {
        self.attribute & 0x07
    }

    /// The background colour: the high four bits, one of the sixteen.
    ///
    /// The high bit is the background's own brightness: it is the eighth
    /// background and not a shade of the seventh.
    #[must_use]
    pub fn background(&self) -> u8 {
        (self.attribute >> 4) & 0x0F
    }

    /// Whether the cell blinks.
    #[must_use]
    pub fn blink(&self) -> bool {
        self.attribute & 0x08 != 0
    }
}

/// Where the cursor is on the guest's screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cursor {
    /// The row the cursor is on.
    pub row: usize,
    /// The column the cursor is on.
    pub column: usize,
}

/// The graphics page: the framebuffer of the graphical mode.
///
/// The mode `13h` has a page of its own, at `A000:0000`: 320 pixels of
/// one byte each, and two hundred rows of them. The bytes are the colour
/// indices the VGA's own DAC names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Graphics {
    /// The framebuffer's width, in pixels.
    pub width: usize,
    /// The framebuffer's height, in pixels.
    pub height: usize,
    /// The colour index of every pixel, row by row.
    pub pixels: Vec<u8>,
}

/// The screen panel: the guest's text page.
///
/// The page is the machine's own memory: what the guest wrote to it is
/// what it shows. The geometry is the machine's own too — the mode sets it
/// and the data area holds it — and so is the cursor: the panel reads the
/// machine again every time and holds no copy of any of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScreenView {
    /// The segment the page is read through.
    pub segment: u16,
    /// The page of text memory the panel shows.
    pub page: u8,
    /// The columns the page has.
    pub columns: usize,
    /// The rows the page has.
    pub rows: usize,
    /// The cells of the page, row by row.
    ///
    /// There are as many of them as the geometry claims — `columns *
    /// rows` — in every mode, so that a consumer which maps a cell to
    /// `row * columns + column` finds one. A graphics page has no
    /// characters, so its cells are blank.
    pub cells: Vec<ScreenCell>,
    /// The cursor, where the BIOS says it is.
    ///
    /// `None` when there is no cursor on the page the panel shows: the
    /// guest hid it, or the panel shows another page than the one the
    /// machine is on.
    pub cursor: Option<Cursor>,
    /// The graphics page, when the machine is in the graphical mode.
    ///
    /// `None` in a text mode: the graphics page is a second shape of the
    /// same thing, and the mode names which one the panel shows.
    pub graphics: Option<Graphics>,
}

/// Every panel, drawn from the machine and the UI's state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panels {
    /// The disassembly panel.
    pub disassembly: Vec<Line>,
    /// The registers panel.
    pub registers: Registers,
    /// The memory panel.
    pub memory: MemoryView,
    /// The stack panel.
    pub stack: StackView,
    /// The screen panel: the guest's own text page.
    pub screen: ScreenView,
}

impl Panels {
    /// Draw every panel from the machine and the UI's state.
    ///
    /// Nothing here reads a terminal: the same call serves the TUI and the
    /// GUI.
    #[must_use]
    pub fn of(cpu: &Cpu, ui: &Ui) -> Panels {
        let registers = Registers {
            ax: cpu.regs.word(Reg16::Ax),
            bx: cpu.regs.word(Reg16::Bx),
            cx: cpu.regs.word(Reg16::Cx),
            dx: cpu.regs.word(Reg16::Dx),
            sp: cpu.regs.word(Reg16::Sp),
            bp: cpu.regs.word(Reg16::Bp),
            si: cpu.regs.word(Reg16::Si),
            di: cpu.regs.word(Reg16::Di),
            cs: cpu.regs.segment(Seg::Cs),
            ds: cpu.regs.segment(Seg::Ds),
            es: cpu.regs.segment(Seg::Es),
            ss: cpu.regs.segment(Seg::Ss),
            ip: cpu.regs.ip,
            flags: cpu.regs.flags.0,
        };
        Panels {
            disassembly: disassembly_of(cpu, ui),
            registers,
            memory: memory_of(cpu, ui),
            stack: stack_of(cpu, ui),
            screen: screen_of(cpu, ui),
        }
    }
}

/// The disassembly panel: the instructions from where the UI says.
fn disassembly_of(cpu: &Cpu, ui: &Ui) -> Vec<Line> {
    let segment = cpu.regs.segment(Seg::Cs);
    let mut bytes = Vec::new();
    for offset in 0..=255u16 {
        bytes.push(
            cpu.mem
                .read_u8(segment, ui.disassembly_at.wrapping_add(offset)),
        );
    }
    let mut out = Vec::new();
    let mut at = ui.disassembly_at;
    for _ in 0..ui.disassembly_len {
        let rest = &bytes[usize::from(at.wrapping_sub(ui.disassembly_at))..];
        let Ok(instruction) = emo8067_asm::disasm::disassemble(rest, at) else {
            break;
        };
        let length = instruction.length;
        out.push(Line {
            segment,
            offset: at,
            bytes: rest[..length.min(rest.len())].to_vec(),
            text: instruction.text,
        });
        at = at.wrapping_add(length as u16);
    }
    out
}

/// The memory panel: the bytes around the cursor.
///
/// The window is a window and not a buffer: it holds the bytes around one
/// address, so there is no megabyte of memory in the UI anywhere.
fn memory_of(cpu: &Cpu, ui: &Ui) -> MemoryView {
    let count = ui.memory_len.max(1);
    // The cursor sits in the middle of the window, so there are as many
    // bytes before it as there are after it.
    let window_at = ui.memory_at.saturating_sub((count / 2) as u16);
    let bytes = (0..count)
        .map(|offset| {
            cpu.mem
                .read_u8(ui.memory_segment, window_at.wrapping_add(offset as u16))
        })
        .collect::<Vec<u8>>();
    let ascii = bytes
        .iter()
        .map(|byte| {
            if (0x20..0x7F).contains(byte) {
                *byte as char
            } else {
                '.'
            }
        })
        .collect();
    MemoryView {
        segment: ui.memory_segment,
        at: ui.memory_at,
        window_at,
        cursor: usize::from(ui.memory_at.wrapping_sub(window_at)),
        bytes,
        ascii,
    }
}

/// The stack panel: the words on the stack, from the top down.
///
/// The window starts where the UI says and not at the stack pointer, so
/// the pointer can be anywhere in it — or above it. The pointer itself is
/// still reported, because the panel is about where the stack is and not
/// only about what it holds.
fn stack_of(cpu: &Cpu, ui: &Ui) -> StackView {
    let segment = cpu.regs.segment(Seg::Ss);
    let sp = cpu.regs.word(Reg16::Sp);
    let words = (0..ui.stack_len)
        .map(|index| {
            // The offset wraps inside the segment, the way every other
            // offset in the model does.
            let offset = ui.stack_at.wrapping_add(2u16.wrapping_mul(index as u16));
            cpu.mem.read_u16(segment, offset)
        })
        .collect();
    StackView { segment, sp, words }
}

/// The screen panel: the guest's text page.
///
/// The page is read through the segment the UI says, the way the memory
/// window reads its bytes: the physical address is masked, so the read
/// cannot run off the bus whatever segment the UI names.
///
/// The geometry is the machine's own — the mode sets it and the data area
/// holds it — and so is the cursor: the panel reads the machine's video
/// through the core's door and holds no copy of it.
///
/// The page number is bounded before it is used: the hardware has eight
/// pages of text memory and no more, and a page past the last one is the
/// last one.
fn screen_of(cpu: &Cpu, ui: &Ui) -> ScreenView {
    let video = cpu.video();
    if video.mode == emo8067_core::bios::MODE_GRAPHICS {
        // The graphical mode has a page of its own, and it is the one the
        // panel shows.
        return graphics_of(cpu, &video);
    }
    let page = ui.screen_page.min(SCREEN_PAGES - 1);
    // The pages of text memory are four kilobytes apart.
    let base = u16::from(page) * PAGE_STRIDE;
    let columns = usize::from(video.columns);
    let rows = usize::from(video.rows);
    let cells = (0..columns * rows)
        .map(|index| {
            // Two bytes a cell: the character and the attribute.
            let at = base.wrapping_add(2 * index as u16);
            let character = cpu.mem.read_u8(ui.screen_segment, at);
            let attribute = cpu.mem.read_u8(ui.screen_segment, at.wrapping_add(1));
            ScreenCell {
                character: printable(character),
                attribute,
            }
        })
        .collect();
    // The cursor is on the page the panel shows only when the panel shows
    // the machine's own page, and only when the guest has not hidden it.
    let cursor = (video.page == page && video.cursor_visible).then_some(Cursor {
        row: usize::from(video.cursor_row),
        column: usize::from(video.cursor_column),
    });
    ScreenView {
        segment: ui.screen_segment,
        page,
        columns,
        rows,
        cells,
        cursor,
        graphics: None,
    }
}

/// The graphics page: the framebuffer the graphical mode has.
///
/// The framebuffer is the machine's own memory at `A000:0000`, 320 pixels
/// of one byte each and two hundred rows of them, and the panel reads it
/// like any other memory: what the guest drew is what the panel shows.
fn graphics_of(cpu: &Cpu, video: &Video) -> ScreenView {
    let width = usize::from(emo8067_core::bios::GRAPHICS_WIDTH);
    let height = usize::from(emo8067_core::bios::GRAPHICS_HEIGHT);
    let pixels = (0..width * height)
        .map(|index| {
            cpu.mem
                .read_u8(emo8067_core::bios::GRAPHICS_SEGMENT, index as u16)
        })
        .collect();
    // The machine's own cell grid: forty columns by twenty-five rows,
    // whose cells are eight pixels wide and eight pixels tall.
    let columns = usize::from(video.columns);
    let rows = usize::from(video.rows);
    ScreenView {
        segment: emo8067_core::bios::GRAPHICS_SEGMENT,
        // The graphical mode has one page.
        page: 0,
        columns,
        rows,
        // The graphics page has no characters, so its cells are blank —
        // but there are as many of them as the grid claims, so that a
        // consumer that maps a cell to `row * columns + column` finds one.
        cells: vec![BLANK_CELL; columns * rows],
        // The graphics page has no text cursor.
        cursor: None,
        graphics: Some(Graphics {
            width,
            height,
            pixels,
        }),
    }
}

/// A cell of a page that has no characters in it.
const BLANK_CELL: ScreenCell = ScreenCell {
    character: ' ',
    attribute: 0,
};

/// The character a byte is drawn as.
///
/// A byte that is not printable is a space and not a control glyph, and
/// the range is the memory window's own: the page shows what the guest
/// wrote, and a control character is not something a terminal shows.
fn printable(byte: u8) -> char {
    if (0x20..0x7F).contains(&byte) {
        byte as char
    } else {
        ' '
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The segment the BIOS data area lives in.
    const DATA: u16 = 0x0040;
    /// The offset of the machine's current video mode.
    const MODE: u16 = 0x0049;

    /// The view's shape is the same in both modes: the cells are as many
    /// as the geometry claims.
    ///
    /// A consumer that maps a cell to `row * columns + column` — the
    /// mapping the text path itself uses — must find one, or the view is
    /// not the shape it says it is.
    #[test]
    fn the_view_publishes_as_many_cells_as_its_geometry_claims() {
        let cpu = Cpu::new();
        let text = &Panels::of(&cpu, &Ui::default()).screen;
        assert_eq!(
            text.cells.len(),
            text.columns * text.rows,
            "the text page's cells are the grid's"
        );

        // And the graphics page: the machine's own cell grid, and as many
        // cells as it claims.
        let mut cpu = Cpu::new();
        // The mode byte is the guest's own: the BIOS reads it out of the
        // data area, and it keeps no copy of it.
        cpu.mem.write_u8(DATA, MODE, 0x13);
        let graphics = &Panels::of(&cpu, &Ui::default()).screen;
        assert!(
            graphics.graphics.is_some(),
            "the panel shows the graphics page"
        );
        assert_eq!(
            graphics.cells.len(),
            graphics.columns * graphics.rows,
            "the graphics page's cells are the grid's"
        );
    }
}
