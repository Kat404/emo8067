//! The palette: the roles the window has, and the values they hold.
//!
//! The roles are the TUI's own (`design/DESIGN.md`): each role has one
//! meaning, and the meaning is the same everywhere on the screen. The
//! values are the ones the TUI holds, in egui's own type.

use egui::{Color32, Visuals};

/// The screen's ground.
pub const CANVAS: Color32 = Color32::from_rgb(0x11, 0x14, 0x1a);

/// The values and the instruction's text.
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xc3, 0xcc, 0xdc);

/// The labels and the addresses.
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x8b, 0x95, 0xa5);

/// The gutter's addresses.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x61, 0x6b, 0x7a);

/// The focus, and the instruction the machine is at.
pub const ACCENT: Color32 = Color32::from_rgb(0x56, 0xb6, 0xc2);

/// The window's own look: the dark palette and the canvas's ground.
#[must_use]
pub fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = CANVAS;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals
}
