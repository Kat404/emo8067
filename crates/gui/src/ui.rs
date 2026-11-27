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
use emo8067_debug::panels::{Edit, Flag, Panels, ScreenView, Ui as State};
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

/// One cell of the guest's text page, as the window draws it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextCell {
    /// The character the guest wrote.
    pub character: char,
    /// The colour of the character: the DAC's own.
    pub foreground: egui::Color32,
    /// The colour of the cell's ground: the DAC's own.
    pub background: egui::Color32,
    /// Whether the cursor is on this cell.
    pub cursor: bool,
}

/// The guest's screen, as the window draws it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Screen {
    /// The graphical mode: the framebuffer's pixels, one to a pixel.
    Graphics {
        /// The framebuffer's width, in pixels.
        width: usize,
        /// The framebuffer's height, in pixels.
        height: usize,
        /// The colour of every pixel, row by row: the DAC's own.
        pixels: Vec<egui::Color32>,
    },
    /// A text mode: the page's character grid.
    Text {
        /// The columns the page has.
        columns: usize,
        /// The rows the page has.
        rows: usize,
        /// The cells of the page, row by row.
        cells: Vec<TextCell>,
    },
}

/// The guest's screen, drawn from the model.
///
/// The model is the product: the framebuffer's bytes are the pixels and
/// the page's cells are the characters. The GUI reads the model and
/// nothing else — not the machine's memory, and not the framebuffer
/// again.
#[must_use]
pub fn screen(view: &ScreenView) -> Screen {
    match &view.graphics {
        Some(graphics) => Screen::Graphics {
            width: graphics.width,
            height: graphics.height,
            // Every byte of the framebuffer is a colour the DAC names.
            pixels: graphics
                .pixels
                .iter()
                .map(|index| theme::pixel(*index))
                .collect(),
        },
        None => Screen::Text {
            columns: view.columns,
            rows: view.rows,
            cells: text_cells(view),
        },
    }
}

/// The text page's cells, as the panel draws them.
///
/// The character is the guest's own, the colours are the DAC's, and the
/// cursor is where the BIOS says it is.
#[must_use]
pub fn text_cells(view: &ScreenView) -> Vec<TextCell> {
    view.cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let (foreground, background) = theme::guest(cell.foreground(), cell.background());
            TextCell {
                character: cell.character,
                foreground,
                background,
                cursor: view
                    .cursor
                    .is_some_and(|cursor| cursor.row * view.columns + cursor.column == index),
            }
        })
        .collect()
}

/// The guest's screen, in its own window.
///
/// The window is the GUI's mirror of the TUI's takeover: it draws the page
/// over the debugger's panels. It is open by default and it can be closed;
/// the action that opens it again is T5's.
pub fn screen_window(ctx: &egui::Context, screen: &Screen) {
    let id = egui::Id::new("guest screen");
    let mut open = ctx.memory_mut(|memory| *memory.data.get_temp_mut_or_insert_with(id, || true));
    egui::Window::new("guest screen")
        .open(&mut open)
        .show(ctx, |ui| {
            draw_screen(ui, screen);
        });
    // The window's own close button has the say, and the flag is the
    // window's state: it lives between the frames.
    ctx.memory_mut(|memory| memory.data.insert_temp(id, open));
}

/// The screen, as it is drawn: the framebuffer's pixels, or the character
/// grid.
fn draw_screen(ui: &mut egui::Ui, screen: &Screen) {
    match screen {
        Screen::Graphics {
            width,
            height,
            pixels,
        } => {
            let texture = screen_texture(ui.ctx(), *width, *height, pixels);
            // The image is the framebuffer's own size — a guest pixel to a
            // point — and the nearest of neighbours, so that a scaled
            // picture keeps the DAC's own colours and never a blend.
            ui.add(egui::Image::new(&texture).texture_options(egui::TextureOptions::NEAREST));
        }
        Screen::Text { columns, cells, .. } => {
            let galley = text_galley(ui, *columns, cells);
            ui.label(galley);
        }
    }
}

/// The texture of the guest's screen.
///
/// The framebuffer is uploaded once and kept: the picture changes only
/// when the machine moves, and the pixels are the picture, so the two are
/// compared to know whether the texture is still the framebuffer.
fn screen_texture(
    ctx: &egui::Context,
    width: usize,
    height: usize,
    pixels: &[egui::Color32],
) -> egui::TextureHandle {
    let id = egui::Id::new("guest screen texture");
    let cached = ctx.memory(|memory| {
        memory
            .data
            .get_temp::<(Vec<egui::Color32>, egui::TextureHandle)>(id)
    });
    if let Some((cached, texture)) = cached
        && cached == pixels
    {
        return texture;
    }
    let image = egui::ColorImage::new([width, height], pixels.to_vec());
    let texture = ctx.load_texture("guest screen", image, egui::TextureOptions::NEAREST);
    ctx.memory_mut(|memory| {
        memory
            .data
            .insert_temp(id, (pixels.to_vec(), texture.clone()));
    });
    texture
}

/// The character grid, laid out as one galley.
///
/// Every cell is a section of its own, so the character keeps its own
/// colour and its own ground, and the cursor's cell is underlined — the
/// TUI's own drawing, in egui's own way.
fn text_galley(
    ui: &egui::Ui,
    columns: usize,
    cells: &[TextCell],
) -> std::sync::Arc<egui::text::Galley> {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    // The grid is a grid: the rows are not wrapped, and the spaces among
    // the cells keep their ground.
    job.wrap.max_width = f32::INFINITY;
    job.keep_trailing_whitespace = true;
    let columns = columns.max(1);
    for (index, cell) in cells.iter().enumerate() {
        let format = egui::text::TextFormat {
            font_id: font.clone(),
            color: cell.foreground,
            background: cell.background,
            underline: if cell.cursor {
                egui::Stroke::new(1.0, cell.foreground)
            } else {
                egui::Stroke::NONE
            },
            ..egui::text::TextFormat::default()
        };
        if index > 0 && index % columns == 0 {
            // The end of the row: the next cell is on the next line.
            job.append("\n", 0.0, format.clone());
        }
        let mut bytes = [0u8; 4];
        job.append(cell.character.encode_utf8(&mut bytes), 0.0, format);
    }
    ui.painter().fonts_mut(|fonts| fonts.layout_job(job))
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
    // The guest's screen is a window of its own, over the panels: the
    // GUI's mirror of the TUI's takeover.
    screen_window(ui.ctx(), &screen(&app.panels.screen));
}

/// The registers and the flags.
fn registers_panel(ui: &mut egui::Ui, app: &App) {
    section(ui, "registers");
    egui::Grid::new("registers").num_columns(2).show(ui, |ui| {
        for row in registers_rows(&app.panels, &app.ui) {
            ui.label(RichText::new(row.name).color(label_colour(row.cursor)));
            ui.label(RichText::new(row.value).color(value_colour(row.cursor)));
            // A grid row ends only when the user ends it.
            ui.end_row();
        }
    });
    ui.separator();
    section(ui, "flags");
    egui::Grid::new("flags").num_columns(2).show(ui, |ui| {
        for row in flags_rows(&app.panels, &app.ui) {
            ui.label(RichText::new(row.name).color(label_colour(row.cursor)));
            ui.label(RichText::new(row.value).color(value_colour(row.cursor)));
            ui.end_row();
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
