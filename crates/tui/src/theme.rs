//! The palette: the roles the screen has, and the values they hold.
//!
//! The design's document settles the values (`design/DESIGN.md`): each
//! role has one meaning, and the meaning is the same everywhere on the
//! screen. The colours are the calm ones of that document and not the
//! terminal's own ANSI ones, which are harsh on a black ground.
//!
//! The roles are:
//!
//! | Role | Where |
//! |---|---|
//! | canvas | the screen's ground |
//! | surface | the header's and the bottom band's ground |
//! | rule | the thin rules between the sections |
//! | text primary | the values, the instruction's text |
//! | text secondary | the labels, the addresses |
//! | text muted | what has run, the gutter's addresses |
//! | accent | the focus, and the instruction the machine is at |
//! | warning | a breakpoint |
//! | error | a watchpoint, and a refusal |
//! | success | a value that was written |
//!
//! The research's meanings are kept: cyan is the current `IP`, grey is
//! what has run, yellow is a breakpoint and red is a watchpoint.
//!
//! The guest's own screen has a palette of its own: the sixteen colours
//! of the VGA's text page are the guest's and not the debugger's, and
//! they live in [`VGA`]. The VGA's own 256-colour DAC — the table the
//! graphical mode's pixels name — is the core's own
//! (`emo8067_core::bios::VGA_DAC`); [`VGA_DAC`] is that table in
//! ratatui's type and [`VGA`] is its first sixteen.

use ratatui::style::{Color, Modifier, Style};

/// The screen's ground.
pub const CANVAS: Color = Color::Rgb(0x11, 0x14, 0x1a);

/// The header's and the bottom band's ground: one step above the canvas.
pub const SURFACE: Color = Color::Rgb(0x16, 0x1b, 0x22);

/// The thin rules between the sections.
pub const RULE: Color = Color::Rgb(0x2b, 0x31, 0x38);

/// The values and the instruction's text.
pub const TEXT_PRIMARY: Color = Color::Rgb(0xc3, 0xcc, 0xdc);

/// The labels and the addresses.
pub const TEXT_SECONDARY: Color = Color::Rgb(0x8b, 0x95, 0xa5);

/// What has run, and the gutter's addresses.
pub const TEXT_MUTED: Color = Color::Rgb(0x61, 0x6b, 0x7a);

/// The focus, and the instruction the machine is at.
pub const ACCENT: Color = Color::Rgb(0x56, 0xb6, 0xc2);

/// A breakpoint.
pub const WARNING: Color = Color::Rgb(0xd6, 0xb3, 0x52);

/// A watchpoint, and a refusal.
pub const ERROR: Color = Color::Rgb(0xe0, 0x6c, 0x75);

/// A value that was written.
pub const SUCCESS: Color = Color::Rgb(0x98, 0xc3, 0x79);

/// The VGA's own 256-colour DAC.
///
/// Every entry is an RGB triple, and the table's type says so: the DAC is
/// built from the channels, so there is no other kind of colour in it.
/// The table itself is the core's own (`emo8067_core::bios::VGA_DAC`): the
/// VGA's palette is part of the model of the VGA, and the two front ends
/// read it there and not each in its own.
pub const VGA_DAC: [Color; 256] = {
    let mut table = [Color::Rgb(0, 0, 0); 256];
    let mut index = 0;
    while index < emo8067_core::bios::VGA_DAC.len() {
        let (red, green, blue) = emo8067_core::bios::VGA_DAC[index];
        table[index] = Color::Rgb(red, green, blue);
        index += 1;
    }
    table
};

/// The sixteen colours of the guest's text page.
///
/// The DAC's first sixteen, exactly: the guest's colours are the guest's
/// and not the terminal's ANSI ones. The first eight are the foreground's
/// — black, blue, green, cyan, red, magenta, brown and light grey — and
/// the last eight are the background's, the bright half of the first
/// eight.
pub const VGA: [Color; 16] = [
    VGA_DAC[0],
    VGA_DAC[1],
    VGA_DAC[2],
    VGA_DAC[3],
    VGA_DAC[4],
    VGA_DAC[5],
    VGA_DAC[6],
    VGA_DAC[7],
    VGA_DAC[8],
    VGA_DAC[9],
    VGA_DAC[10],
    VGA_DAC[11],
    VGA_DAC[12],
    VGA_DAC[13],
    VGA_DAC[14],
    VGA_DAC[15],
];

/// The colour of a pixel: the DAC's own entry for its index.
///
/// The DAC holds three channels and nothing else, so the entry is three
/// channels and not a colour the caller has to take apart. The table is
/// the core's own.
#[must_use]
pub fn pixel(index: u8) -> (u8, u8, u8) {
    emo8067_core::bios::VGA_DAC[usize::from(index)]
}

/// The style of the screen: the canvas, and the text on it.
#[must_use]
pub fn canvas() -> Style {
    Style::default().fg(TEXT_PRIMARY).bg(CANVAS)
}

/// The style of the header and the bottom band: the surface, and the text
/// on it.
#[must_use]
pub fn surface() -> Style {
    Style::default().fg(TEXT_PRIMARY).bg(SURFACE)
}

/// The style of a rule: the thin line between two sections.
#[must_use]
pub fn rule() -> Style {
    Style::default().fg(RULE)
}

/// The style of the name of the section that has the focus.
#[must_use]
pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// The style of the name of a section that does not have the focus.
#[must_use]
pub fn muted() -> Style {
    Style::default().fg(TEXT_MUTED)
}

/// The style of a value.
#[must_use]
pub fn value() -> Style {
    Style::default().fg(TEXT_PRIMARY)
}

/// The style of a label: the name of a register, the name of a flag.
#[must_use]
pub fn label() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

/// The style of meta: the file and the line of the source strip.
#[must_use]
pub fn meta() -> Style {
    Style::default()
        .fg(TEXT_SECONDARY)
        .add_modifier(Modifier::ITALIC)
}

/// The style of the accent: the focus, and the instruction the machine is
/// at.
#[must_use]
pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

/// The style of a breakpoint.
#[must_use]
pub fn warning() -> Style {
    Style::default().fg(WARNING)
}

/// The style of a watchpoint, and of a refusal.
#[must_use]
pub fn error() -> Style {
    Style::default().fg(ERROR)
}

/// The style of a value that was written.
#[must_use]
pub fn success() -> Style {
    Style::default().fg(SUCCESS)
}

/// The style of a cell of the guest's screen.
///
/// The colours are the guest's own: the foreground is one of the eight
/// and the background one of the sixteen, and they are the values the
/// hardware holds. A cell that blinks carries the terminal's own blink,
/// so that the state is not colour alone.
#[must_use]
pub fn guest(foreground: u8, background: u8, blink: bool) -> Style {
    let mut style = Style::default()
        .fg(VGA[usize::from(foreground & 0x07)])
        .bg(VGA[usize::from(background & 0x0F)]);
    if blink {
        style = style.add_modifier(Modifier::SLOW_BLINK);
    }
    style
}
