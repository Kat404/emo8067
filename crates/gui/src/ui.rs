//! The window's drawing.
//!
//! The drawing is a pure function of the panel model: the rows come from
//! `Panels`, and the widget only draws them. A window that needs a display
//! is not a test, so the part that can be tested is the part that is pure.
//!
//! **Every panel is in the window and none of them is behind a key.** The
//! GUI's own argument for existing is that a panel is one click away, and a
//! panel that is always on the screen is nearer than that.

use egui::containers::Panel;
use egui::{CentralPanel, RichText, ScrollArea};
use emo8067_core::Cpu;
use emo8067_debug::breakpoints::{Entry, Kind, detail_of, kind_of};
use emo8067_debug::console::Console;
use emo8067_debug::panels::{Edit, Flag, Panels, Ui as State};
use emo8067_debug::source::SourceView;

use crate::app::App;
use crate::theme;

/// The marker of a breakpoint.
const BREAKPOINT: &str = "◆";

/// The marker of a watchpoint.
const WATCHPOINT: &str = "◇";

/// The bytes a row of the memory window holds.
const BYTES_PER_ROW: usize = 8;

/// Where the machine is: the address the listing is anchored on, and the
/// instruction that carries the marker.
///
/// The machine's `CS:IP` is read once, and both the listing's start and
/// the marker of the current row come from it, so the two cannot disagree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Here {
    /// The segment the machine is in.
    pub segment: u16,
    /// The offset the instruction pointer holds.
    pub offset: u16,
}

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

/// One register, as the panel shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RegisterRow {
    /// The name of the register.
    pub name: &'static str,
    /// The value the machine holds in it, or what is being typed over it.
    pub value: String,
    /// Whether the cursor is on it.
    pub cursor: bool,
}

/// One flag, as the panel shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlagRow {
    /// The name of the flag.
    pub name: &'static str,
    /// The value the flag holds, or what is being typed over it.
    pub value: String,
    /// Whether the cursor is on it.
    pub cursor: bool,
}

/// One row of the memory window.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemoryRow {
    /// The address the row is at.
    pub address: String,
    /// The bytes of the row, as hex.
    pub bytes: Vec<String>,
    /// The same bytes as characters.
    pub printable: String,
    /// The byte the cursor is on, if it is in this row.
    pub cursor: Option<usize>,
}

/// One row of the stack panel.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackRow {
    /// The offset the word is at.
    pub address: String,
    /// The value of the word.
    pub value: String,
    /// Whether the stack pointer is at it.
    pub pointer: bool,
}

/// One row of the conditions listing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConditionRow {
    /// The marker of the kind: a breakpoint or a watchpoint.
    pub glyph: &'static str,
    /// The address the entry is at.
    pub address: String,
    /// What the entry is called.
    pub kind: &'static str,
    /// What the entry carries.
    pub detail: String,
    /// Whether the cursor is on it.
    pub cursor: bool,
}

/// One row of the source strip.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceRow {
    /// The number the source gave the line.
    pub number: usize,
    /// The line's own text.
    pub text: String,
    /// Whether the machine is stopped at it.
    pub current: bool,
}

/// The console's panel: the log of what the commands said, and the command
/// line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConsoleView {
    /// The log: one line at a time, the oldest first.
    pub log: Vec<String>,
    /// The command line: what has been typed.
    pub line: String,
}

/// The listing: the panel model's own disassembly, one row per instruction.
///
/// The rows are the model's own: the GUI reads the machine nowhere else,
/// and it reads the model for everything it shows.
#[must_use]
pub fn listing(panels: &Panels, here: Here) -> Vec<Row> {
    panels
        .disassembly
        .iter()
        .map(|line| Row {
            address: format!("{:04X}:{:04X}", line.segment, line.offset),
            bytes: bytes_of(line),
            text: line.text.clone(),
            here: line.offset == here.offset && line.segment == here.segment,
        })
        .collect()
}

/// The registers panel's rows: the machine's registers, in the order the
/// panel shows them.
#[must_use]
pub fn registers_rows(panels: &Panels, state: &State) -> Vec<RegisterRow> {
    panels
        .registers
        .cells()
        .iter()
        .enumerate()
        .map(|(index, cell)| RegisterRow {
            name: cell.register.name(),
            // The value that is being typed over the register is shown in
            // place of the one the machine holds.
            value: match state.edit.as_ref() {
                Some(Edit::Register { register, text }) if *register == cell.register => {
                    text.clone()
                }
                _ => format!("{:04X}", cell.value),
            },
            cursor: index == state.register_cursor,
        })
        .collect()
}

/// The flags panel's rows: the nine flags and their values.
#[must_use]
pub fn flags_rows(panels: &Panels, state: &State) -> Vec<FlagRow> {
    Flag::ALL
        .iter()
        .enumerate()
        .map(|(index, flag)| FlagRow {
            name: flag.name(),
            value: match state.edit.as_ref() {
                Some(Edit::Flag { flag: edited, text }) if edited == flag => text.clone(),
                _ => {
                    if panels.registers.flag(*flag) {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    }
                }
            },
            cursor: index == state.flag_cursor,
        })
        .collect()
}

/// The memory window's rows: the bytes around the cursor, and the printable
/// side beside them.
#[must_use]
pub fn memory_rows(panels: &Panels, state: &State, per_row: usize) -> Vec<MemoryRow> {
    let view = &panels.memory;
    let per_row = per_row.max(1);
    let mut rows = Vec::new();
    for index in (0..view.bytes.len()).step_by(per_row) {
        let end = (index + per_row).min(view.bytes.len());
        let bytes = view.bytes[index..end]
            .iter()
            .enumerate()
            .map(|(offset, byte)| {
                // The byte that is being edited shows what has been typed
                // over it, the way the register under the cursor does.
                let at = view.window_at.wrapping_add((index + offset) as u16);
                match state.edit.as_ref() {
                    Some(Edit::Byte {
                        segment,
                        offset: edited,
                        text,
                    }) if *segment == view.segment && *edited == at => text.clone(),
                    _ => format!("{byte:02X}"),
                }
            })
            .collect::<Vec<String>>();
        let printable: String = view.ascii.chars().skip(index).take(per_row).collect();
        rows.push(MemoryRow {
            address: format!("{:04X}", view.window_at.wrapping_add(index as u16)),
            bytes,
            printable,
            cursor: (index..end).find(|at| *at == view.cursor),
        });
    }
    rows
}

/// The stack panel's rows: the words on the stack, and the pointer among
/// them.
#[must_use]
pub fn stack_rows(panels: &Panels, state: &State, cpu: &Cpu) -> Vec<StackRow> {
    let view = &panels.stack;
    let at = state.stack_at(cpu);
    view.words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let offset = at.wrapping_add(2u16.wrapping_mul(index as u16));
            StackRow {
                address: format!("{offset:04X}"),
                value: format!("{word:04X}"),
                pointer: offset == view.sp,
            }
        })
        .collect()
}

/// The conditions listing's rows: the breakpoints and the watchpoints, and
/// what each one carries.
#[must_use]
pub fn condition_rows(entries: &[Entry], state: &State) -> Vec<ConditionRow> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            // The entry under the cursor is edited in place, and it shows
            // what has been typed over it, the way the register under the
            // cursor does.
            let detail = match state.edit.as_ref() {
                Some(Edit::Condition {
                    segment,
                    offset,
                    text,
                }) if *segment == entry.segment && *offset == entry.offset => text.clone(),
                _ => detail_of(entry),
            };
            ConditionRow {
                glyph: match entry.kind {
                    Kind::Breakpoint => BREAKPOINT,
                    Kind::Watchpoint(_) => WATCHPOINT,
                },
                address: format!("{:04X}:{:04X}", entry.segment, entry.offset),
                kind: kind_of(entry.kind),
                detail,
                cursor: index == state.condition_cursor,
            }
        })
        .collect()
}

/// The source strip's rows: the program's own lines, and the one the
/// machine is stopped at.
#[must_use]
pub fn source_rows(view: &SourceView) -> Vec<SourceRow> {
    view.lines
        .iter()
        .enumerate()
        .map(|(index, line)| SourceRow {
            number: line.number,
            // The model keeps the truth: the drawing tidies the line, and
            // the space at its end is not part of the source.
            text: line.text.trim_end().to_string(),
            current: view.current == Some(index),
        })
        .collect()
}

/// The console's panel: the log and the command line.
#[must_use]
pub fn console_view(console: &Console) -> ConsoleView {
    ConsoleView {
        log: console.output.clone(),
        line: console.text.clone(),
    }
}

/// The bytes of an instruction, as hex.
fn bytes_of(line: &emo8067_debug::panels::Line) -> String {
    line.bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The window: the debugger's panels, all of them at once.
pub fn window(ui: &mut egui::Ui, app: &App) {
    Panel::top("header").show(ui, |ui| {
        ui.label(
            RichText::new(&app.name)
                .strong()
                .color(theme::TEXT_SECONDARY),
        );
    });
    Panel::left("registers").show(ui, |ui| {
        registers_panel(ui, app);
    });
    Panel::right("memory").show(ui, |ui| {
        memory_panel(ui, app);
    });
    Panel::bottom("console").show(ui, |ui| {
        console_panel(ui, app);
    });
    CentralPanel::default().show(ui, |ui| {
        disassembly_panel(ui, app);
    });
}

/// The registers and the flags.
fn registers_panel(ui: &mut egui::Ui, app: &App) {
    section(ui, "registers");
    egui::Grid::new("registers").num_columns(2).show(ui, |ui| {
        for row in registers_rows(&app.panels, &app.ui) {
            ui.label(RichText::new(row.name).color(label_colour(row.cursor)));
            ui.label(RichText::new(row.value).color(value_colour(row.cursor)));
        }
    });
    ui.separator();
    section(ui, "flags");
    egui::Grid::new("flags").num_columns(3).show(ui, |ui| {
        for row in flags_rows(&app.panels, &app.ui) {
            ui.label(RichText::new(row.name).color(label_colour(row.cursor)));
            ui.label(RichText::new(row.value).color(value_colour(row.cursor)));
        }
    });
}

/// The memory window, the stack and the conditions.
fn memory_panel(ui: &mut egui::Ui, app: &App) {
    section(
        ui,
        &format!(
            "memory · {:04X}:{:04X}",
            app.panels.memory.segment, app.panels.memory.at
        ),
    );
    for row in memory_rows(&app.panels, &app.ui, BYTES_PER_ROW) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&row.address)
                    .monospace()
                    .color(theme::TEXT_MUTED),
            );
            for (index, byte) in row.bytes.iter().enumerate() {
                let marked = row.cursor == Some(index);
                ui.label(RichText::new(byte).monospace().color(if marked {
                    theme::ACCENT
                } else {
                    theme::TEXT_PRIMARY
                }));
            }
            ui.label(
                RichText::new(&row.printable)
                    .monospace()
                    .color(theme::TEXT_MUTED),
            );
        });
    }
    ui.separator();
    section(
        ui,
        &format!(
            "stack · {:04X}:{:04X}",
            app.panels.stack.segment, app.panels.stack.sp
        ),
    );
    for row in stack_rows(&app.panels, &app.ui, &app.cpu) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&row.address)
                    .monospace()
                    .color(theme::TEXT_SECONDARY),
            );
            ui.label(RichText::new(&row.value).monospace().color(if row.pointer {
                theme::ACCENT
            } else {
                theme::TEXT_PRIMARY
            }));
        });
    }
    ui.separator();
    section(ui, "conditions");
    for row in condition_rows(&app.breakpoints.listing(), &app.ui) {
        ui.horizontal(|ui| {
            let colour = if row.kind == "watchpoint" {
                theme::ERROR
            } else {
                theme::WARNING
            };
            ui.label(RichText::new(row.glyph).color(colour));
            ui.label(
                RichText::new(&row.address)
                    .monospace()
                    .color(theme::TEXT_SECONDARY),
            );
            ui.label(RichText::new(row.kind).color(colour));
            ui.label(RichText::new(&row.detail).color(theme::TEXT_PRIMARY));
        });
    }
}

/// The console: the log of what the commands said, and the command line.
fn console_panel(ui: &mut egui::Ui, app: &App) {
    section(ui, "console");
    let view = console_view(&app.console);
    ScrollArea::vertical().show(ui, |ui| {
        for line in &view.log {
            ui.label(RichText::new(line).monospace().color(theme::TEXT_PRIMARY));
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new(":").color(theme::TEXT_SECONDARY));
        ui.label(
            RichText::new(&view.line)
                .monospace()
                .color(theme::TEXT_PRIMARY),
        );
    });
}

/// The disassembly, and the source strip under it.
fn disassembly_panel(ui: &mut egui::Ui, app: &App) {
    section(ui, "disassembly");
    ScrollArea::vertical().show(ui, |ui| {
        for row in &app.rows {
            ui.horizontal(|ui| {
                ui.label(RichText::new(if row.here { "▸" } else { " " }).color(theme::ACCENT));
                ui.label(
                    RichText::new(&row.address)
                        .monospace()
                        .color(theme::TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(&row.bytes)
                        .monospace()
                        .color(theme::TEXT_MUTED),
                );
                ui.label(RichText::new(&row.text).color(if row.here {
                    theme::ACCENT
                } else {
                    theme::TEXT_PRIMARY
                }));
            });
        }
    });
    ui.separator();
    section(ui, "source");
    for row in source_rows(&app.source_view(SOURCE_LINES)) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(if row.current { "▸" } else { " " }).color(theme::ACCENT));
            ui.label(
                RichText::new(format!("{:>4}", row.number))
                    .monospace()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(RichText::new(&row.text).monospace().color(if row.current {
                theme::ACCENT
            } else {
                theme::TEXT_PRIMARY
            }));
        });
    }
}

/// The lines the source strip shows.
const SOURCE_LINES: usize = 8;

/// The name of a panel.
fn section(ui: &mut egui::Ui, name: &str) {
    ui.label(RichText::new(name).strong().color(theme::TEXT_SECONDARY));
}

/// The colour of a name under the cursor.
fn label_colour(cursor: bool) -> egui::Color32 {
    if cursor {
        theme::ACCENT
    } else {
        theme::TEXT_SECONDARY
    }
}

/// The colour of a value under the cursor.
fn value_colour(cursor: bool) -> egui::Color32 {
    if cursor {
        theme::ACCENT
    } else {
        theme::TEXT_PRIMARY
    }
}
