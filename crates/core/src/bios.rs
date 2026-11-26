//! The BIOS: the video services of `INT 10h` and the keyboard services of
//! `INT 16h`.
//!
//! The BIOS keeps **no state of its own**. Everything it knows lives in the
//! BIOS data area at `0040:0000`, at the offsets the reference's own memory
//! map gives, so the guest can read it and every service is a function of
//! the machine's memory and its registers.
//!
//! The BIOS is a **handler at an address**, and the machine's vector table
//! points at it: the reference's own map gives the video functions' entry
//! as `F400:0190`, so the machine installs the vector for `10h` when it
//! starts. The `INT` is then the 8086's `INT` — it pushes the frame, it
//! clears TF and IF, and it loads `CS:IP` from the table — and the service
//! runs when the machine reaches the entry, and returns the way a handler
//! returns.
//!
//! **The ROM is this project's own.** The reference's `BIOS_ROM` is its
//! code and is never copied: the services are native, and the ROM's own
//! byte at the entry is the `IRET` that comes out of them.
//!
//! The text pages are the machine's own memory at `B800:0000`, four
//! kilobytes apart: a page is read and written like any other memory, and
//! what the guest wrote to it is what the panel shows.
//!
//! The graphical mode `13h` has a page of its own, at `A000:0000`: 320
//! pixels of one byte each, two hundred rows of them. The two pixel
//! services write and read it, and they answer only in that mode.
//!
//! The keyboard's entry is the mirror of the video's, `F400:01C0`, and the
//! reference's own map gives it too: "16 16x4 = 58 F400:01C0 - keyboard
//! functions" (`memory.txt:66`). Its buffer is the BIOS data area as well:
//! the **shape** is the IBM PC's — a head, a tail, a start and a size — and
//! the **offsets** are this project's own, for the reference's own map
//! documents none of them.

use crate::Cpu;
use crate::regs::{Reg8, Reg16, Seg};

/// The segment the BIOS data area lives in.
pub const DATA_SEGMENT: u16 = 0x0040;

/// The segment the text pages live in.
pub const VIDEO_SEGMENT: u16 = 0xB800;

/// The segment the graphics page lives in.
///
/// The published mode `13h` framebuffer is at `A000:0000`: "the screen is
/// an array of 64000 contiguous bytes, arranged as 200 rows of 320 bytes,
/// with one byte per pixel, located in memory at absolute address
/// `$A000:$0000`" (Dr Bill Verts, *Notes on VGA 320x200 256-Color Mode*,
/// `https://people.cs.umass.edu/~verts/cs32/vga_320.html`).
pub const GRAPHICS_SEGMENT: u16 = 0xA000;

/// The pixels a row of the graphics page has.
pub const GRAPHICS_WIDTH: u16 = 320;

/// The rows the graphics page has.
pub const GRAPHICS_HEIGHT: u16 = 200;

/// The mode `00h`: the text mode, forty columns by twenty-five rows.
///
/// "00h - text mode. 40x25. 16 colors. 8 pages."
pub const MODE_TEXT_40: u8 = 0x00;

/// The mode `03h`: the text mode, eighty columns by twenty-five rows.
///
/// "03h - text mode. 80x25. 16 colors. 8 pages."
pub const MODE_TEXT_80: u8 = 0x03;

/// The mode `13h`: the graphical mode, 320x200 pixels in 256 colours.
///
/// "13h - graphical mode. 40x25. 256 colors. 320x200 pixels. 1 page."
/// (`8086_bios_and_dos_interrupts.txt:76`).
pub const MODE_GRAPHICS: u8 = 0x13;

/// The bytes between two pages of text memory.
pub const PAGE_STRIDE: u16 = 0x1000;

/// The pages of text memory the hardware has.
pub const PAGES: u8 = 8;

/// The segment the BIOS's ROM lives in.
///
/// The reference's own map gives the video functions' entry as `F400:0190`
/// and the BIOS area at `F4000`.
pub const ENTRY_SEGMENT: u16 = 0xF400;

/// The offset the video functions' entry is at.
pub const ENTRY_OFFSET: u16 = 0x0190;

/// The offset the keyboard functions' entry is at.
///
/// The mirror of the video's entry, and the reference's own map gives it:
/// "16 16x4 = 58 F400:01C0 - keyboard functions" (`memory.txt:66`).
pub const KEYBOARD_ENTRY_OFFSET: u16 = 0x01C0;

/// The vector the BIOS's entry is installed at: `INT 10h`.
const VECTOR: u8 = 0x10;

/// The vector the keyboard's entry is installed at: `INT 16h`.
const KEYBOARD_VECTOR: u8 = 0x16;

/// The ROM's own byte at the entry: the `IRET` that comes out of the
/// service.
///
/// The services are native, so the ROM is one byte of this project's own
/// code: the return that ends the handler.
pub const ENTRY_BYTE: u8 = 0xCF;

/// The columns the machine starts in: the equipment list's "80x25 CGA color
/// (emulator default)".
pub const DEFAULT_COLUMNS: u16 = 80;

/// The rows the machine starts in.
pub const DEFAULT_ROWS: u16 = 25;

/// The columns of the graphical mode's cell grid.
///
/// The mode `13h` is forty by twenty-five cells, and a cell is eight
/// pixels wide and eight pixels tall — "13h = G 40x25 8x8 320x200
/// 256/256k A000" (`https://pdos.csail.mit.edu/6.828/2008/readings/hardware/vgadoc/VGABIOS.TXT`).
const GRAPHICS_COLUMNS: u16 = 40;

/// The widest text mode the hardware has: eighty columns.
///
/// The text modes are forty columns and eighty columns, and the widest of
/// them is the bound a guest's column count cannot pass.
const MAX_COLUMNS: u16 = 80;

/// The tallest text mode the hardware has: twenty-five rows.
///
/// Both text modes are twenty-five rows tall, and that is the bound a
/// guest's row count cannot pass.
const MAX_ROWS: u16 = 25;

/// The cursor's start line the machine starts with: the standard text
/// cursor, the lines 6 and 7 of the cell.
pub const DEFAULT_CURSOR_START: u8 = 6;

/// The cursor's bottom line the machine starts with.
pub const DEFAULT_CURSOR_BOTTOM: u8 = 7;

/// The offset of the machine's current video mode.
///
/// "40:49 byte current CRT mode (hex value)"
/// (`https://pic.hallikainen.org/techref/bios/dataarea.htm`; the same in
/// Ralf Brown's Interrupt List's data-area map and in `VGABIOS.TXT`'s
/// "0:449h BYTE Video current mode"). The reference's own map documents
/// none of it, so the offset is this project's own assumption, the way the
/// cursor's shape at `0040:0080` already is.
const MODE_OFFSET: u16 = 0x0049;
/// The offset of the number of columns on screen.
const COLUMNS: u16 = 0x004A;
/// The offset of the current video page's start in video memory.
const PAGE_START: u16 = 0x004E;
/// The offset of the eight cursors, one pair a page.
const CURSORS: u16 = 0x0050;
/// The offset of the current video page number.
const PAGE: u16 = 0x0062;
/// The offset of the cursor's shape: the start line and the bottom line.
const SHAPE: u16 = 0x0080;
/// The offset of the rows on screen, minus one.
const ROWS: u16 = 0x0084;

/// The offset of the keyboard buffer's head.
///
/// The **shape** is the IBM PC's — a head, a tail, a start and a size in
/// the BIOS data area — and the **offsets** are this project's own: the
/// reference's own map documents none of them, for it gives the area
/// `0040:0000`-`:0084` seven entries, all of video.
const KEYBOARD_HEAD: u16 = 0x001A;
/// The offset of the keyboard buffer's tail.
const KEYBOARD_TAIL: u16 = 0x001C;
/// The offset of the keyboard buffer's first entry: the ring's start.
const KEYBOARD_BUFFER: u16 = 0x001E;
/// The offset of the keyboard buffer's size, in bytes.
const KEYBOARD_SIZE: u16 = 0x0030;

/// The bytes the keyboard buffer's ring holds: nine two-byte entries.
///
/// The size word holds it, and the ring's end is derived from it. The
/// word is outside the body, so the ring can never clobber it.
const KEYBOARD_RING: u16 = 0x0012;

/// The bytes one entry of the keyboard buffer takes: the ASCII character
/// and the scan code.
const KEYBOARD_ENTRY: u16 = 2;

/// The keystrokes the keyboard buffer holds.
///
/// The ring holds nine two-byte entries and **one of them stays free**, so
/// that "the head equals the tail" is unambiguously empty: eight
/// keystrokes fit, and the ninth is dropped.
pub const KEYBOARD_CAPACITY: usize = (KEYBOARD_RING / KEYBOARD_ENTRY - 1) as usize;

/// The IBM PC's set 1 scan codes.
///
/// The set is the IBM's, and the corpus is the evidence inside the
/// repository: `re/extracted/app/examples/snake.asm:37`-`:41` names the
/// four arrow codes "bios key codes" (`4bh`, `4dh`, `48h`, `50h`) and
/// compares the returned `AH` against them (`:143`, `:204`-`:211`).
pub const SCAN_ESCAPE: u8 = 0x01;
/// The scan code of the backspace key.
pub const SCAN_BACKSPACE: u8 = 0x0E;
/// The scan code of the tab key.
pub const SCAN_TAB: u8 = 0x0F;
/// The scan code of the enter key.
pub const SCAN_ENTER: u8 = 0x1C;
/// The scan code of the space bar.
pub const SCAN_SPACE: u8 = 0x39;
/// The scan code of the home key.
pub const SCAN_HOME: u8 = 0x47;
/// The scan code of the up arrow.
pub const SCAN_UP: u8 = 0x48;
/// The scan code of the page up key.
pub const SCAN_PAGE_UP: u8 = 0x49;
/// The scan code of the left arrow.
pub const SCAN_LEFT: u8 = 0x4B;
/// The scan code of the right arrow.
pub const SCAN_RIGHT: u8 = 0x4D;
/// The scan code of the end key.
pub const SCAN_END: u8 = 0x4F;
/// The scan code of the down arrow.
pub const SCAN_DOWN: u8 = 0x50;
/// The scan code of the page down key.
pub const SCAN_PAGE_DOWN: u8 = 0x51;
/// The scan code of the insert key.
pub const SCAN_INSERT: u8 = 0x52;
/// The scan code of the delete key.
pub const SCAN_DELETE: u8 = 0x53;

/// The scan code of a function key: `F1` is the first of the ten.
pub const SCAN_FUNCTION_FIRST: u8 = 0x3B;

/// The attribute the emulator's own text carries: light grey on black.
///
/// The reference's contract for the teletype names no attribute, and a
/// blank page's attribute is zero: black on black, and a guest's text would
/// be there and invisible.
const TEXT_ATTRIBUTE: u8 = 0x07;

/// The bit of the cursor's start line that hides the cursor.
const CURSOR_HIDDEN: u8 = 0x20;

/// What the machine's video is: the page it is on, the mode it is in, its
/// geometry and its cursor.
///
/// This is the door the panel reads the machine's video through. The panel
/// holds no copy of any of it: it reads the machine again every time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Video {
    /// The active video page.
    pub page: u8,
    /// The machine's current video mode.
    ///
    /// The mode is the byte at `0040:0049`, and it names the machine's
    /// own state: the graphical mode is [`MODE_GRAPHICS`], and every
    /// other mode is a text mode.
    pub mode: u8,
    /// The columns on screen.
    pub columns: u16,
    /// The rows on screen.
    pub rows: u16,
    /// The cursor's row on the active page.
    pub cursor_row: u8,
    /// The cursor's column on the active page.
    pub cursor_column: u8,
    /// Whether the cursor is visible.
    ///
    /// The shape's options hold it: the cursor's start line carries bit 5,
    /// and the bit set is the cursor that is not visible.
    pub cursor_visible: bool,
}

impl Cpu {
    /// The machine's video: the page it is on, its geometry and its cursor.
    #[must_use]
    pub fn video(&self) -> Video {
        let page = self.active_page();
        let (cursor_row, cursor_column) = self.cursor(page);
        Video {
            page,
            mode: self.data_byte(MODE_OFFSET),
            columns: self.columns(),
            rows: self.rows(),
            cursor_row,
            cursor_column,
            cursor_visible: self.data_byte(SHAPE) & CURSOR_HIDDEN == 0,
        }
    }

    /// Whether the machine is in the graphical mode.
    ///
    /// The mode is the machine's own memory — the byte at `0040:0049` —
    /// so the guest can write it and the answer is what it says.
    fn in_graphics_mode(&self) -> bool {
        self.data_byte(MODE_OFFSET) == MODE_GRAPHICS
    }

    /// The active video page.
    ///
    /// A page past the last one is the last one: the hardware has eight
    /// pages of text memory and no more.
    fn active_page(&self) -> u8 {
        self.data_byte(PAGE).min(PAGES - 1)
    }

    /// The columns on screen.
    ///
    /// The data area is ordinary memory and the guest can write it, so the
    /// count is bounded before anything computes with it: the widest text
    /// mode the hardware has is eighty columns, and a count above it is
    /// that mode's count. The word in the data area is the guest's own,
    /// and the guest reads it back.
    fn columns(&self) -> u16 {
        self.data_word(COLUMNS).min(MAX_COLUMNS)
    }

    /// The rows on screen.
    ///
    /// The data area holds the rows minus one, so a byte of `0FFh` is 256
    /// rows: the count is bounded the way the columns are, at the
    /// twenty-five rows of the tallest text mode.
    fn rows(&self) -> u16 {
        (u16::from(self.data_byte(ROWS)) + 1).min(MAX_ROWS)
    }

    /// The cursor's row and column on a page.
    ///
    /// The column is in the low byte and the row in the high byte, the IBM
    /// way: every other offset the reference's map names is the IBM one, and
    /// the IBM BIOS loads `DX` from that word with the column in the low
    /// byte.
    fn cursor(&self, page: u8) -> (u8, u8) {
        let word = self.data_word(CURSORS + 2 * u16::from(page.min(PAGES - 1)));
        ((word >> 8) as u8, (word & 0xFF) as u8)
    }

    /// Put the cursor at a row and a column on a page.
    fn set_cursor(&mut self, page: u8, row: u8, column: u8) {
        let word = (u16::from(row) << 8) | u16::from(column);
        self.set_data_word(CURSORS + 2 * u16::from(page.min(PAGES - 1)), word);
    }

    /// A byte of the BIOS data area.
    fn data_byte(&self, offset: u16) -> u8 {
        self.mem.read_u8(DATA_SEGMENT, offset)
    }

    /// Write a byte of the BIOS data area.
    fn set_data_byte(&mut self, offset: u16, value: u8) {
        self.mem.write_u8(DATA_SEGMENT, offset, value);
    }

    /// A word of the BIOS data area.
    fn data_word(&self, offset: u16) -> u16 {
        self.mem.read_u16(DATA_SEGMENT, offset)
    }

    /// Write a word of the BIOS data area.
    fn set_data_word(&mut self, offset: u16, value: u16) {
        self.mem.write_u16(DATA_SEGMENT, offset, value);
    }

    /// Where the keyboard buffer's ring ends.
    ///
    /// The size is the size word's, and the end is derived from it. The
    /// data area is ordinary guest-writable memory, so the size is
    /// validated before it is trusted: a size that is not even, or one
    /// whose ring would reach the size word, leaves the buffer empty.
    fn keyboard_end(&self) -> Option<u16> {
        let size = self.data_word(KEYBOARD_SIZE);
        // The size is the guest's own word, so the addition is checked: a
        // size that would leave the segment leaves the buffer empty, and
        // the emulator lives.
        let end = KEYBOARD_BUFFER.checked_add(size)?;
        if size == 0 || !size.is_multiple_of(2) || end > KEYBOARD_SIZE {
            return None;
        }
        Some(end)
    }

    /// The head, the tail and the end of the keyboard buffer, when they
    /// are offsets of the ring.
    ///
    /// The head is the entry to read and the tail the entry to write, and
    /// both hold **absolute** offsets, the way the IBM's own buffer holds
    /// them: a guest that reads `0040:001A` sees where the buffer is. The
    /// end comes with them, because the ring's end is the size word's and
    /// the size is validated once, here.
    fn keyboard_ends(&self) -> Option<(u16, u16, u16)> {
        let end = self.keyboard_end()?;
        let head = self.data_word(KEYBOARD_HEAD);
        let tail = self.data_word(KEYBOARD_TAIL);
        let offset_of_the_ring =
            |offset: u16| offset >= KEYBOARD_BUFFER && offset < end && offset.is_multiple_of(2);
        (offset_of_the_ring(head) && offset_of_the_ring(tail)).then_some((head, tail, end))
    }

    /// Put a keystroke into the keyboard buffer.
    ///
    /// This is the door the host writes through: the BIOS module owns the
    /// buffer's layout, and the door it offers is the pair of the scan code
    /// and the ASCII character.
    ///
    /// The answer says whether the keystroke was taken: a ring that is full
    /// — one of its nine slots stays free, so that "the head equals the
    /// tail" is unambiguously empty — drops the keystroke, and so does a
    /// buffer whose ends the guest scribbled.
    pub fn push_key(&mut self, scan_code: u8, ascii: u8) -> bool {
        let Some((head, tail, end)) = self.keyboard_ends() else {
            // The guest scribbled where the head and the tail live: the
            // buffer is empty, and the keystroke has nowhere to go.
            return false;
        };
        let next = next_entry(tail, end);
        if next == head {
            // The ring is full: the keystroke is dropped.
            return false;
        }
        self.set_data_byte(tail, ascii);
        self.set_data_byte(tail + 1, scan_code);
        self.set_data_word(KEYBOARD_TAIL, next);
        true
    }

    /// How many keystrokes are waiting in the keyboard buffer.
    ///
    /// The other half of the door: what the host is told about the keys
    /// the guest has not taken yet.
    #[must_use]
    pub fn keys_waiting(&self) -> usize {
        let Some((head, tail, end)) = self.keyboard_ends() else {
            return 0;
        };
        let size = end - KEYBOARD_BUFFER;
        usize::from((tail + size - head) % size) / 2
    }
}

/// The offset of the entry after the one at `offset`.
///
/// The ring wraps: the entry after the last one is the first one again.
fn next_entry(offset: u16, end: u16) -> u16 {
    if offset + KEYBOARD_ENTRY >= end {
        KEYBOARD_BUFFER
    } else {
        offset + KEYBOARD_ENTRY
    }
}

/// The IBM PC's set 1 scan code of a character.
///
/// The table is the published one
/// (`https://www.marjorie.de/ps2/scancode-set1.htm`, the table
/// Wikipedia's "Scancode" article cites): the letters are **not**
/// contiguous in it — the rows break at the bracket keys and at the `` ` ``
/// and `\` keys — so `Q = 10h`, `E = 12h`, `I = 17h`, `Z = 2Ch` and
/// `M = 32h`.
///
/// A character the shift made — `?`, `:`, `|`, the rest — has the code of
/// the **key** it is on, the way the IBM's own BIOS answers: the shift
/// itself lives in the `AH = 02h` service this unit does not have. A
/// character the set has no code for has none either: the scan code is
/// zero, and the ASCII character is what the guest reads.
#[must_use]
pub fn scan_code(character: u8) -> u8 {
    match character {
        // The number row: 1h to 0h, and the two keys after it. A
        // character the shift made has the code of the key it is on.
        b'1' | b'!' => 0x02,
        b'2' | b'@' => 0x03,
        b'3' | b'#' => 0x04,
        b'4' | b'$' => 0x05,
        b'5' | b'%' => 0x06,
        b'6' | b'^' => 0x07,
        b'7' | b'&' => 0x08,
        b'8' | b'*' => 0x09,
        b'9' | b'(' => 0x0A,
        b'0' | b')' => 0x0B,
        b'-' | b'_' => 0x0C,
        b'=' | b'+' => 0x0D,
        // The top letter row.
        b'q' | b'Q' => 0x10,
        b'w' | b'W' => 0x11,
        b'e' | b'E' => 0x12,
        b'r' | b'R' => 0x13,
        b't' | b'T' => 0x14,
        b'y' | b'Y' => 0x15,
        b'u' | b'U' => 0x16,
        b'i' | b'I' => 0x17,
        b'o' | b'O' => 0x18,
        b'p' | b'P' => 0x19,
        b'[' | b'{' => 0x1A,
        b']' | b'}' => 0x1B,
        // The home row.
        b'a' | b'A' => 0x1E,
        b's' | b'S' => 0x1F,
        b'd' | b'D' => 0x20,
        b'f' | b'F' => 0x21,
        b'g' | b'G' => 0x22,
        b'h' | b'H' => 0x23,
        b'j' | b'J' => 0x24,
        b'k' | b'K' => 0x25,
        b'l' | b'L' => 0x26,
        b';' | b':' => 0x27,
        b'\'' | b'"' => 0x28,
        b'`' | b'~' => 0x29,
        // The key between the two lower rows, and the bottom row.
        b'\\' | b'|' => 0x2B,
        b'z' | b'Z' => 0x2C,
        b'x' | b'X' => 0x2D,
        b'c' | b'C' => 0x2E,
        b'v' | b'V' => 0x2F,
        b'b' | b'B' => 0x30,
        b'n' | b'N' => 0x31,
        b'm' | b'M' => 0x32,
        b',' | b'<' => 0x33,
        b'.' | b'>' => 0x34,
        b'/' | b'?' => 0x35,
        // And the keys that are not characters of their own.
        b' ' => SCAN_SPACE,
        b'\t' => SCAN_TAB,
        b'\r' | b'\n' => SCAN_ENTER,
        0x08 => SCAN_BACKSPACE,
        0x1B => SCAN_ESCAPE,
        _ => 0,
    }
}

/// The state the machine starts with, and the BIOS it starts with.
///
/// The emulator's own default video mode: eighty columns by twenty-five
/// rows, every page's cursor at the origin, the current page zero and its
/// start at zero.
///
/// The machine also installs the BIOS: the vector for `10h` points at the
/// BIOS's entry, and the ROM's own byte is at that entry.
pub fn init(cpu: &mut Cpu) {
    cpu.set_data_word(COLUMNS, DEFAULT_COLUMNS);
    cpu.set_data_word(PAGE_START, 0);
    cpu.set_data_byte(PAGE, 0);
    cpu.set_data_byte(ROWS, (DEFAULT_ROWS - 1) as u8);
    cpu.set_data_byte(MODE_OFFSET, MODE_TEXT_80);
    cpu.set_data_byte(SHAPE, DEFAULT_CURSOR_START);
    cpu.set_data_byte(SHAPE + 1, DEFAULT_CURSOR_BOTTOM);
    for page in 0..PAGES {
        cpu.set_cursor(page, 0, 0);
    }
    // The machine installs the video BIOS's vector, so that the `INT` is
    // the chip's `INT` and the service runs where the ROM is.
    let at = u16::from(VECTOR) * 4;
    cpu.mem.write_u16(0x0000, at, ENTRY_OFFSET);
    cpu.mem.write_u16(0x0000, at + 2, ENTRY_SEGMENT);
    // And the ROM's own byte at the entry.
    cpu.mem.write_u8(ENTRY_SEGMENT, ENTRY_OFFSET, ENTRY_BYTE);
    // The keyboard's entry is the mirror of the video's, and the machine
    // installs its vector the same way.
    let at = u16::from(KEYBOARD_VECTOR) * 4;
    cpu.mem.write_u16(0x0000, at, KEYBOARD_ENTRY_OFFSET);
    cpu.mem.write_u16(0x0000, at + 2, ENTRY_SEGMENT);
    cpu.mem
        .write_u8(ENTRY_SEGMENT, KEYBOARD_ENTRY_OFFSET, ENTRY_BYTE);
    // And the keyboard buffer: empty, and the layout this project chose.
    // The head and the tail hold absolute offsets, so they point at the
    // ring's first entry, and the size word holds the ring's size.
    cpu.set_data_word(KEYBOARD_HEAD, KEYBOARD_BUFFER);
    cpu.set_data_word(KEYBOARD_TAIL, KEYBOARD_BUFFER);
    cpu.set_data_word(KEYBOARD_SIZE, KEYBOARD_RING);
}

/// The offset of a cell in the video memory.
fn offset_of(page: u8, index: u16) -> u16 {
    u16::from(page.min(PAGES - 1)) * PAGE_STRIDE + index * 2
}

/// The character and the attribute of a cell.
fn cell(cpu: &Cpu, page: u8, index: u16) -> (u8, u8) {
    let at = offset_of(page, index);
    (
        cpu.mem.read_u8(VIDEO_SEGMENT, at),
        cpu.mem.read_u8(VIDEO_SEGMENT, at + 1),
    )
}

/// Write a character and an attribute into a cell.
fn put_cell(cpu: &mut Cpu, page: u8, index: u16, character: u8, attribute: u8) {
    let at = offset_of(page, index);
    cpu.mem.write_u8(VIDEO_SEGMENT, at, character);
    cpu.mem.write_u8(VIDEO_SEGMENT, at + 1, attribute);
}

/// Run the video services of `INT 10h`.
///
/// The machine reached the BIOS's entry through the interrupt vector, and
/// the service runs here: this project's own code, at the address the
/// vector names. A function the BIOS has no service for does nothing, and
/// the machine returns the way a handler returns.
pub fn service(cpu: &mut Cpu) {
    match cpu.regs.byte(Reg8::Ah) {
        0x00 => set_mode(cpu),
        0x01 => set_cursor_shape(cpu),
        0x02 => set_cursor_position(cpu),
        0x03 => get_cursor_position(cpu),
        0x05 => select_page(cpu),
        0x06 => scroll(cpu, false),
        0x07 => scroll(cpu, true),
        0x08 => read_character(cpu),
        0x09 => write_character(cpu, true),
        0x0A => write_character(cpu, false),
        0x0C => write_pixel(cpu),
        0x0D => read_pixel(cpu),
        0x0E => teletype(cpu),
        0x10 => toggle_intensity(cpu),
        0x13 => write_string(cpu),
        _ => {}
    }
}

/// `AH = 00h` — set video mode. `AL` = desired video mode.
///
/// `00h` is text mode, forty columns by twenty-five rows; `03h` is text
/// mode, eighty columns by twenty-five rows; and `13h` is the graphical
/// mode, forty columns by twenty-five rows with a 320x200 framebuffer.
/// A mode the machine does not have leaves the machine as it was.
fn set_mode(cpu: &mut Cpu) {
    let mode = cpu.regs.byte(Reg8::Al);
    let (columns, rows) = match mode {
        MODE_TEXT_40 => (40, DEFAULT_ROWS),
        MODE_TEXT_80 => (DEFAULT_COLUMNS, DEFAULT_ROWS),
        MODE_GRAPHICS => (GRAPHICS_COLUMNS, DEFAULT_ROWS),
        _ => return,
    };
    cpu.set_data_word(COLUMNS, columns);
    cpu.set_data_byte(ROWS, (rows - 1) as u8);
    // The machine's mode is one byte in the data area, and the mode the
    // guest asked for is the mode the machine is in.
    cpu.set_data_byte(MODE_OFFSET, mode);
    if mode == MODE_GRAPHICS {
        // The graphical mode has one page: the active page is the first
        // one, and its start in video memory is zero.
        cpu.set_data_byte(PAGE, 0);
        cpu.set_data_word(PAGE_START, 0);
    }
}

/// `AH = 01h` — set the text-mode cursor shape.
///
/// `CH` = cursor start line (bits 0-4) and options (bits 5-7), `CL` =
/// bottom cursor line (bits 0-4). The two bytes are kept as they are, so
/// the options come back with the shape.
fn set_cursor_shape(cpu: &mut Cpu) {
    cpu.set_data_byte(SHAPE, cpu.regs.byte(Reg8::Ch));
    cpu.set_data_byte(SHAPE + 1, cpu.regs.byte(Reg8::Cl));
}

/// `AH = 02h` — set the cursor position.
///
/// `DH` = row, `DL` = column, `BH` = page number (0..7).
fn set_cursor_position(cpu: &mut Cpu) {
    let page = cpu.regs.byte(Reg8::Bh);
    let row = cpu.regs.byte(Reg8::Dh);
    let column = cpu.regs.byte(Reg8::Dl);
    cpu.set_cursor(page, row, column);
}

/// `AH = 03h` — get the cursor position and size.
///
/// `BH` = page number. Returns `DH` = row, `DL` = column, `CH` = cursor
/// start line, `CL` = cursor bottom line.
fn get_cursor_position(cpu: &mut Cpu) {
    let page = cpu.regs.byte(Reg8::Bh);
    let (row, column) = cpu.cursor(page);
    cpu.regs.set_byte(Reg8::Dh, row);
    cpu.regs.set_byte(Reg8::Dl, column);
    cpu.regs.set_byte(Reg8::Ch, cpu.data_byte(SHAPE));
    cpu.regs.set_byte(Reg8::Cl, cpu.data_byte(SHAPE + 1));
}

/// `AH = 05h` — select the active video page. `AL` = new page number
/// (0..7).
fn select_page(cpu: &mut Cpu) {
    let page = cpu.regs.byte(Reg8::Al).min(PAGES - 1);
    cpu.set_data_byte(PAGE, page);
    cpu.set_data_word(PAGE_START, u16::from(page) * PAGE_STRIDE);
}

/// `AH = 06h` — scroll up window. `AH = 07h` — scroll down window.
///
/// `AL` = number of lines by which to scroll (00h = clear entire window),
/// `BH` = attribute used to write blank lines, `CH, CL` = row, column of
/// the window's upper left corner, `DH, DL` = row, column of the window's
/// lower right corner. The window is on the active page.
fn scroll(cpu: &mut Cpu, down: bool) {
    let lines = cpu.regs.byte(Reg8::Al);
    let attribute = cpu.regs.byte(Reg8::Bh);
    let top = u16::from(cpu.regs.byte(Reg8::Ch));
    let left = u16::from(cpu.regs.byte(Reg8::Cl));
    let bottom = u16::from(cpu.regs.byte(Reg8::Dh));
    let right = u16::from(cpu.regs.byte(Reg8::Dl));

    let columns = cpu.columns();
    let rows = cpu.rows();
    if bottom >= rows || right >= columns || bottom < top || right < left {
        return;
    }
    let page = cpu.active_page();
    let height = bottom - top + 1;
    let width = right - left + 1;
    // Zero lines means the whole window is cleared.
    let lines = if lines == 0 {
        height
    } else {
        u16::from(lines).min(height)
    };

    // The window is read into a buffer and written back where it belongs:
    // a scroll moves the cells of the window, and a cell is two bytes in a
    // row the geometry lays out.
    let mut window = Vec::with_capacity((height * width) as usize);
    for row in top..=bottom {
        for column in left..=right {
            window.push(cell(cpu, page, row * columns + column));
        }
    }
    for row in 0..height {
        let source = if down {
            row.checked_sub(lines)
        } else {
            row.checked_add(lines)
        };
        for column in 0..width {
            let (character, attribute) = match source {
                Some(source) if source < height => window[(source * width + column) as usize],
                _ => (b' ', attribute),
            };
            put_cell(
                cpu,
                page,
                (top + row) * columns + left + column,
                character,
                attribute,
            );
        }
    }
}

/// `AH = 08h` — read the character and the attribute at the cursor
/// position.
///
/// `BH` = page number. Returns `AH` = attribute, `AL` = character.
fn read_character(cpu: &mut Cpu) {
    let page = cpu.regs.byte(Reg8::Bh);
    let (row, column) = cpu.cursor(page);
    let columns = cpu.columns();
    let rows = cpu.rows();
    let cells = columns * rows;
    if cells == 0 {
        // A machine with no geometry has no page to read from.
        return;
    }
    // The cursor is the guest's own: the row is brought into the screen,
    // the way it always was, and the column is the guest's own, because a
    // cell is addressed by its index in the page: the row times the
    // columns plus the column.
    let row = u16::from(row).min(rows - 1);
    let column = u16::from(column);
    let index = row * columns + column;
    let (character, attribute) = if index < cells {
        cell(cpu, page, index)
    } else {
        // The cursor names no cell of the screen: the service reads a
        // blank one.
        (0, 0)
    };
    cpu.regs.set_byte(Reg8::Ah, attribute);
    cpu.regs.set_byte(Reg8::Al, character);
}

/// `AH = 09h` — write the character and the attribute at the cursor
/// position. `AH = 0Ah` — write the character only at the cursor position.
///
/// `AL` = character to display, `BH` = page number, `BL` = attribute,
/// `CX` = number of times to write the character. The cursor stays where
/// it is.
fn write_character(cpu: &mut Cpu, with_attribute: bool) {
    let page = cpu.regs.byte(Reg8::Bh);
    let character = cpu.regs.byte(Reg8::Al);
    let attribute = cpu.regs.byte(Reg8::Bl);
    let count = cpu.regs.word(Reg16::Cx);
    let (row, column) = cpu.cursor(page);
    let columns = cpu.columns();
    if columns == 0 {
        // A machine with no geometry has no page to write on.
        return;
    }
    let index = u16::from(row) * columns + u16::from(column);
    let cells = columns * cpu.rows();

    for step in 0..count {
        let Some(at) = index.checked_add(step) else {
            break;
        };
        if at >= cells {
            break;
        }
        // The character only keeps the attribute the cell had.
        let attribute = if with_attribute {
            attribute
        } else {
            cell(cpu, page, at).1
        };
        put_cell(cpu, page, at, character, attribute);
    }
}

/// `AH = 0Ch` — write a pixel.
///
/// "INT 10h / AH = 0Ch - change color for a single pixel. input: AL =
/// pixel color. CX = column. DX = row."
/// (`8086_bios_and_dos_interrupts.txt:150`-`:154`).
///
/// **There is no page register**: the reference names none, and the mode
/// has one page. The service answers only in the graphical mode, and a
/// pixel outside the framebuffer is not written.
fn write_pixel(cpu: &mut Cpu) {
    if !cpu.in_graphics_mode() {
        return;
    }
    let colour = cpu.regs.byte(Reg8::Al);
    let column = cpu.regs.word(Reg16::Cx);
    let row = cpu.regs.word(Reg16::Dx);
    let Some(at) = pixel_offset(column, row) else {
        // The pixel is outside the framebuffer: the IBM ignores a pixel
        // that is outside the screen, and this project does the same
        // rather than wrapping the address into the page.
        return;
    };
    cpu.mem.write_u8(GRAPHICS_SEGMENT, at, colour);
}

/// `AH = 0Dh` — read a pixel.
///
/// "INT 10h / AH = 0Dh - get color of a single pixel. input: CX = column.
/// DX = row. output: AL = pixel color"
/// (`8086_bios_and_dos_interrupts.txt:164`-`:169`).
///
/// The byte comes from the same offset the write used, and a pixel
/// outside the framebuffer reads zero.
fn read_pixel(cpu: &mut Cpu) {
    if !cpu.in_graphics_mode() {
        // In a text mode the service does not answer at all.
        return;
    }
    let column = cpu.regs.word(Reg16::Cx);
    let row = cpu.regs.word(Reg16::Dx);
    let colour = match pixel_offset(column, row) {
        Some(at) => cpu.mem.read_u8(GRAPHICS_SEGMENT, at),
        None => 0,
    };
    cpu.regs.set_byte(Reg8::Al, colour);
}

/// The offset of a pixel in the graphics page.
///
/// The pixel at the column and the row is at the offset `row * 320 +
/// column`, and a pixel outside the framebuffer has no offset at all.
fn pixel_offset(column: u16, row: u16) -> Option<u16> {
    if column >= GRAPHICS_WIDTH || row >= GRAPHICS_HEIGHT {
        return None;
    }
    Some(row * GRAPHICS_WIDTH + column)
}

/// `AH = 0Eh` — teletype output. `AL` = character to write.
///
/// "this functions displays a character on the screen, advancing the cursor
/// and scrolling the screen as necessary. the printing is always done to
/// current active page."
fn teletype(cpu: &mut Cpu) {
    let character = cpu.regs.byte(Reg8::Al);
    let page = cpu.active_page();
    let columns = cpu.columns();
    let rows = cpu.rows();
    if columns == 0 {
        // A machine with no geometry has no page to write on.
        return;
    }
    let (row, column) = cpu.cursor(page);
    let cells = columns * rows;
    // The cursor is the guest's own: the row is brought into the screen,
    // the way it always was, and the column is the guest's own, because a
    // cell is addressed by its index in the page: the row times the
    // columns plus the column. The arithmetic is wide enough that it
    // cannot overflow.
    let row = u16::from(row).min(rows - 1);
    let column = u16::from(column);
    let index = row * columns + column;
    if index >= cells {
        // The cursor names no cell of the screen: there is nothing to
        // print on, and the cursor does not move.
        return;
    }

    put_cell(cpu, page, index, character, TEXT_ATTRIBUTE);

    // The cursor advances, and the column and the row are counted in
    // sixteen bits, so that neither of them can be incremented past what
    // the type holds.
    let mut column = column + 1;
    let mut row = row;
    if column >= columns {
        column = 0;
        row += 1;
        if row >= rows {
            // The cursor reached the bottom: the page scrolls up.
            scroll_line(cpu, page, columns, rows);
            row = rows - 1;
        }
    }
    cpu.set_cursor(page, row as u8, column as u8);
}

/// Scroll the whole page up by one line, and fill the last line with
/// blanks.
fn scroll_line(cpu: &mut Cpu, page: u8, columns: u16, rows: u16) {
    for row in 0..rows - 1 {
        for column in 0..columns {
            let (character, attribute) = cell(cpu, page, (row + 1) * columns + column);
            put_cell(cpu, page, row * columns + column, character, attribute);
        }
    }
    let last = (rows - 1) * columns;
    for column in 0..columns {
        put_cell(cpu, page, last + column, b' ', TEXT_ATTRIBUTE);
    }
}

/// `AH = 13h` — write string.
///
/// `AL` = write mode: bit 0: update cursor after writing; bit 1: string
/// contains attributes. `BH` = page number, `BL` = attribute if the string
/// contains only characters, `CX` = number of characters in the string
/// (attributes are not counted), `DL, DH` = column, row at which to start
/// writing, `ES:BP` = the string to be printed.
fn write_string(cpu: &mut Cpu) {
    let mode = cpu.regs.byte(Reg8::Al);
    let update_cursor = mode & 0x01 != 0;
    let with_attributes = mode & 0x02 != 0;
    let page = cpu.regs.byte(Reg8::Bh);
    let attribute = cpu.regs.byte(Reg8::Bl);
    let count = cpu.regs.word(Reg16::Cx);
    let column = u16::from(cpu.regs.byte(Reg8::Dl));
    let row = u16::from(cpu.regs.byte(Reg8::Dh));
    let segment = cpu.regs.segment(Seg::Es);
    let mut at = cpu.regs.word(Reg16::Bp);

    let columns = cpu.columns();
    if columns == 0 {
        // A machine with no geometry has no page to write on.
        return;
    }
    let cells = columns * cpu.rows();
    let mut index = row * columns + column;

    for _ in 0..count {
        if index >= cells {
            break;
        }
        let character = cpu.mem.read_u8(segment, at);
        // A string with attributes carries the attribute byte after every
        // character, and the attributes are not counted.
        let attribute = if with_attributes {
            cpu.mem.read_u8(segment, at.wrapping_add(1))
        } else {
            attribute
        };
        put_cell(cpu, page, index, character, attribute);
        index += 1;
        at = at.wrapping_add(if with_attributes { 2 } else { 1 });
    }

    if update_cursor {
        // The cursor goes to the cell after the last one written.
        let last = index.min(cells.saturating_sub(1));
        cpu.set_cursor(page, (last / columns) as u8, (last % columns) as u8);
    }
}

/// `AX = 1003h` — toggle intensity/blinking.
///
/// `BL` = write mode: 0: enable intensive colors. 1: enable blinking.
/// `BH` = 0.
///
/// The emulator's text page is the CGA's: the attribute's third bit is the
/// blink and its high nibble the background, so the mode the toggle asks
/// for is the one the page is already in. The service runs and the page is
/// left as it is.
fn toggle_intensity(_cpu: &mut Cpu) {}

/// Run the keyboard services of `INT 16h`.
///
/// The machine reached the BIOS's entry through the interrupt vector, the
/// way it reaches any handler, and the service runs here: this project's
/// own code, at the address the vector names.
///
/// The answer says whether the machine may return: a service that waits
/// for a keystroke keeps the machine at the entry, and the machine comes
/// back to it on the next step, so the step budget still bounds the wait.
pub fn keyboard_service(cpu: &mut Cpu) -> bool {
    match cpu.regs.byte(Reg8::Ah) {
        0x00 => get_keystroke(cpu),
        0x01 => check_keystroke(cpu),
        _ => true,
    }
}

/// `AH = 00h` — get keystroke from keyboard (no echo).
///
/// "return: AH = BIOS scan code. AL = ASCII character. (if a keystroke is
/// present, it is removed from the keyboard buffer)."
///
/// With an empty buffer the service waits: the machine stays at the
/// entry, and it comes back to it on the next step.
fn get_keystroke(cpu: &mut Cpu) -> bool {
    let Some((scan_code, ascii)) = take_keystroke(cpu) else {
        return false;
    };
    cpu.regs.set_byte(Reg8::Ah, scan_code);
    cpu.regs.set_byte(Reg8::Al, ascii);
    true
}

/// `AH = 01h` — check for keystroke in the keyboard buffer.
///
/// "return: ZF = 1 if keystroke is not available. ZF = 0 if keystroke
/// available. AH = BIOS scan code. AL = ASCII character. (if a keystroke
/// is present, it is not removed from the keyboard buffer)."
///
/// The service never waits: an empty buffer is an answer, not a wait.
fn check_keystroke(cpu: &mut Cpu) -> bool {
    match peek_keystroke(cpu) {
        Some((scan_code, ascii)) => {
            cpu.regs.set_byte(Reg8::Ah, scan_code);
            cpu.regs.set_byte(Reg8::Al, ascii);
            set_zero_flag(cpu, false);
        }
        None => set_zero_flag(cpu, true),
    }
    true
}

/// The keystroke at the head of the buffer, and the head moved past it.
fn take_keystroke(cpu: &mut Cpu) -> Option<(u8, u8)> {
    let keystroke = peek_keystroke(cpu)?;
    let head = cpu.data_word(KEYBOARD_HEAD);
    let end = cpu.keyboard_end()?;
    cpu.set_data_word(KEYBOARD_HEAD, next_entry(head, end));
    Some(keystroke)
}

/// The keystroke at the head of the buffer, and the buffer left as it is.
fn peek_keystroke(cpu: &mut Cpu) -> Option<(u8, u8)> {
    let (head, tail, _) = cpu.keyboard_ends()?;
    if head == tail {
        // The buffer is empty.
        return None;
    }
    let ascii = cpu.data_byte(head);
    let scan_code = cpu.data_byte(head + 1);
    Some((scan_code, ascii))
}

/// Set the zero flag in the frame the `INT` pushed.
///
/// The `IRET` that comes out of the service restores the flags the `INT`
/// pushed, so the flag the service answers with is set there: the frame's
/// flags word is the one the `IRET` pops.
///
/// The address **wraps**: the frame's flags word is where the push put it,
/// and a stack pointer near the segment's end puts it across the end.
fn set_zero_flag(cpu: &mut Cpu, value: bool) {
    let segment = cpu.regs.segment(Seg::Ss);
    let at = cpu.regs.word(Reg16::Sp).wrapping_add(4);
    let flags = cpu.mem.read_u16(segment, at);
    let flags = if value {
        flags | crate::flags::ZF
    } else {
        flags & !crate::flags::ZF
    };
    cpu.mem.write_u16(segment, at, flags);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine whose guest has written a geometry of its own.
    fn cpu_with_the_guests_geometry(columns: u16, rows: u8) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.set_data_word(COLUMNS, columns);
        cpu.set_data_byte(ROWS, rows);
        cpu
    }

    /// The character at a cell of a page.
    fn character(cpu: &Cpu, page: u16, index: u16) -> u8 {
        cpu.mem
            .read_u8(VIDEO_SEGMENT, page * PAGE_STRIDE + index * 2)
    }

    /// The cursor of a page, the way the data area keeps it.
    fn cursor(cpu: &Cpu, page: u8) -> (u8, u8) {
        let word = cpu.data_word(CURSORS + 2 * u16::from(page));
        ((word >> 8) as u8, (word & 0xFF) as u8)
    }

    /// The geometry the guest writes is bounded by the hardware's modes.
    ///
    /// The data area is ordinary memory and the guest can write it: a
    /// column count or a row count the hardware cannot hold is bounded
    /// before anything computes with it, and the widest text mode and the
    /// tallest one are the bounds.
    #[test]
    fn the_geometry_the_guest_writes_is_bounded_by_the_hardware() {
        let cpu = cpu_with_the_guests_geometry(0xFFFF, 0xFF);
        let video = cpu.video();
        assert_eq!(
            video.columns, 80,
            "the widest text mode the hardware has is eighty columns"
        );
        assert_eq!(
            video.rows, 25,
            "and the tallest text mode is twenty-five rows"
        );
        // The data area keeps the guest's own words: the BIOS has no state
        // of its own, and the guest reads them back.
        assert_eq!(
            cpu.data_word(COLUMNS),
            0xFFFF,
            "the guest's column count is where the guest put it"
        );
        assert_eq!(cpu.data_byte(ROWS), 0xFF, "and so is its row count");
    }

    /// The teletype prints at the cell the cursor names.
    ///
    /// A cell is addressed by its index in the page: the row times the
    /// columns plus the column. The column is the guest's own, and the
    /// index is the one the cursor names.
    #[test]
    fn the_teletype_prints_at_the_cell_the_cursor_names() {
        let mut cpu = cpu_with_the_guests_geometry(0xFFFF, 0xFF);
        // The cursor of the page, at the row 0 and the column 255.
        cpu.set_cursor(0, 0, 255);
        cpu.regs.set_byte(Reg8::Al, b'X');
        teletype(&mut cpu);
        assert_eq!(
            character(&cpu, 0, 255),
            b'X',
            "the character is at the cell the cursor names"
        );
        assert_eq!(cursor(&cpu, 0), (1, 0), "and the cursor advanced");
    }

    /// A cell that is not a cell of the screen is not written.
    ///
    /// The page is four kilobytes and a write must not leave it: a cursor
    /// that names no cell of the screen has nothing to print on.
    #[test]
    fn a_cell_that_is_not_a_cell_of_the_screen_is_not_written() {
        let mut cpu = cpu_with_the_guests_geometry(0xFFFF, 0xFF);
        // The cursor of the page, at the last row and a column past it.
        cpu.set_cursor(0, 24, 255);
        cpu.regs.set_byte(Reg8::Al, b'X');
        teletype(&mut cpu);
        assert_eq!(
            character(&cpu, 0, 24 * MAX_COLUMNS + 255),
            0,
            "nothing was written: the cell is not a cell of the screen"
        );
        assert_eq!(
            character(&cpu, 0, 24 * MAX_COLUMNS + 79),
            0,
            "and nothing was written on the screen either"
        );
        assert_eq!(cursor(&cpu, 0), (24, 255), "and the cursor did not move");
    }

    /// Every service writes inside the page.
    ///
    /// The geometry the guest wrote is a count the hardware cannot hold,
    /// and every service that writes computes with the bounded one: the
    /// page's own cells are the only ones they touch.
    #[test]
    fn every_service_writes_inside_the_page() {
        let mut cpu = cpu_with_the_guests_geometry(0xFFFF, 0xFF);
        // The page the services write on, and the one after it, filled with
        // a pattern of their own.
        let base = 7 * PAGE_STRIDE;
        for offset in base..base + 2 * PAGE_STRIDE {
            cpu.mem.write_u8(VIDEO_SEGMENT, offset, 0xAA);
        }
        // The cursor of the page, at the last cell of the screen.
        cpu.set_cursor(7, 24, 79);
        for ah in [
            0x00u8, 0x01, 0x02, 0x03, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0C, 0x0D, 0x0E, 0x10,
            0x13,
        ] {
            cpu.regs.set_byte(Reg8::Ah, ah);
            cpu.regs.set_byte(Reg8::Al, 0xFF);
            cpu.regs.set_byte(Reg8::Bh, 7);
            cpu.regs.set_byte(Reg8::Bl, 0x1E);
            cpu.regs.set_byte(Reg8::Ch, 0);
            cpu.regs.set_byte(Reg8::Cl, 0);
            cpu.regs.set_byte(Reg8::Dh, 24);
            cpu.regs.set_byte(Reg8::Dl, 79);
            cpu.regs.set_word(Reg16::Cx, 0xFFFF);
            cpu.regs.set_word(Reg16::Bp, 0x0200);
            service(&mut cpu);
        }
        // The cells of the page past the screen are untouched, and so is
        // the page after the one the services wrote on.
        for cell in MAX_COLUMNS * MAX_ROWS..PAGE_STRIDE / 2 {
            assert_eq!(
                cpu.mem.read_u8(VIDEO_SEGMENT, base + cell * 2),
                0xAA,
                "the page's cells past the screen are untouched"
            );
        }
        for offset in base + PAGE_STRIDE..base + 2 * PAGE_STRIDE {
            assert_eq!(
                cpu.mem.read_u8(VIDEO_SEGMENT, offset),
                0xAA,
                "the page after the one the services wrote on is untouched"
            );
        }
    }
}
