//! The palette: the roles the window has, and the values they hold.
//!
//! The roles are the TUI's own (`design/DESIGN.md`): each role has one
//! meaning, and the meaning is the same everywhere on the screen. The
//! values are the ones the TUI holds, in egui's own type.

use egui::{Color32, Visuals};

/// The screen's ground.
pub const CANVAS: Color32 = Color32::from_rgb(0x11, 0x14, 0x1a);

/// The panels' ground: one step above the canvas.
pub const SURFACE: Color32 = Color32::from_rgb(0x16, 0x1b, 0x22);

/// The values and the instruction's text.
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xc3, 0xcc, 0xdc);

/// The labels and the addresses.
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x8b, 0x95, 0xa5);

/// The gutter's addresses.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x61, 0x6b, 0x7a);

/// The focus, and the instruction the machine is at.
pub const ACCENT: Color32 = Color32::from_rgb(0x56, 0xb6, 0xc2);

/// A breakpoint.
pub const WARNING: Color32 = Color32::from_rgb(0xd6, 0xb3, 0x52);

/// A watchpoint, and a refusal.
pub const ERROR: Color32 = Color32::from_rgb(0xe0, 0x6c, 0x75);

/// The window's own look: the dark palette and the canvas's ground.
#[must_use]
pub fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = CANVAS;
    visuals.window_fill = SURFACE;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals
}

/// The colour of a pixel: the DAC's own entry for its index.
///
/// The DAC is the core's own table (`emo8067_core::bios::VGA_DAC`): the
/// VGA's palette is part of the model of the VGA, so the TUI and the GUI
/// read it there and not each in its own.
#[must_use]
pub fn pixel(index: u8) -> Color32 {
    let (red, green, blue) = emo8067_core::bios::VGA_DAC[usize::from(index)];
    Color32::from_rgb(red, green, blue)
}

/// The colours of a cell of the guest's text page: its character and its
/// ground.
///
/// The foreground is one of the DAC's first eight and the background one of
/// its first sixteen — the TUI's own `guest` mapping, so the two front ends
/// show the guest's colours the same way.
#[must_use]
pub fn guest(foreground: u8, background: u8) -> (Color32, Color32) {
    (pixel(foreground & 0x07), pixel(background & 0x0F))
}
