//! The window's drawing.
//!
//! The drawing is a pure function of the panel model: the rows come from
//! `Panels`, and the widget only draws them. A window that needs a display
//! is not a test, so the part that can be tested is the part that is pure.

use egui::{CentralPanel, RichText, ScrollArea, Ui};
use emo8067_debug::panels::{Line, Panels};

use crate::theme;

/// One row of the listing: what the window shows of one instruction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    /// The address the instruction is at.
    pub address: String,
    /// The instruction's bytes, in hex.
    pub bytes: String,
    /// The instruction's text.
    pub text: String,
    /// Whether the machine is at this instruction.
    pub here: bool,
}

/// The listing: the panel model's own disassembly, one row per instruction.
///
/// The rows are the model's own: the GUI reads the machine nowhere else,
/// and it reads the model for everything it shows.
#[must_use]
pub fn listing(panels: &Panels) -> Vec<Row> {
    panels
        .disassembly
        .iter()
        .map(|line| Row {
            address: format!("{:04X}:{:04X}", line.segment, line.offset),
            bytes: bytes_of(line),
            text: line.text.clone(),
            here: line.offset == panels.registers.ip && line.segment == panels.registers.cs,
        })
        .collect()
}

/// The bytes of an instruction, as hex.
fn bytes_of(line: &Line) -> String {
    line.bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The window: the disassembly, in the panel the TUI draws it in.
pub fn window(ui: &mut Ui, panels: &Panels) {
    CentralPanel::default().show(ui, |ui| {
        ui.label(
            RichText::new("disassembly")
                .strong()
                .color(theme::TEXT_SECONDARY),
        );
        ScrollArea::vertical().show(ui, |ui| {
            for row in listing(panels) {
                row_ui(ui, &row);
            }
        });
    });
}

/// One row of the listing, drawn.
fn row_ui(ui: &mut Ui, row: &Row) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(if row.here { "▸" } else { " " }).color(theme::ACCENT));
        ui.label(
            RichText::new(row.address.as_str())
                .monospace()
                .color(theme::TEXT_SECONDARY),
        );
        ui.label(
            RichText::new(row.bytes.as_str())
                .monospace()
                .color(theme::TEXT_MUTED),
        );
        ui.label(RichText::new(row.text.as_str()).color(if row.here {
            theme::ACCENT
        } else {
            theme::TEXT_PRIMARY
        }));
    });
}
