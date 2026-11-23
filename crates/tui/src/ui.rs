//! The drawing.
//!
//! The layout is the design's own (`design/DESIGN.md`): the header is one
//! row, the sections are separated by thin rules and not by boxes, and
//! every state is carried by a glyph as well as by a colour.
//!
//! The rules of that document are the rules of this file:
//!
//! * no panel is framed with a box: two sections are separated by a rule
//!   and by nothing else;
//! * a state is never colour alone: the instruction the machine is at
//!   carries `▸`, a breakpoint `◆` and a watchpoint `◇`;
//! * a label is dim and a value is not;
//! * the focus is visible in every section: the header of the section
//!   that has it is the accent, and the headers of the others are muted;
//! * and the memory window's cursor is marked whether the section has the
//!   focus or not.
//!
//! The words stay: the redesign changes the frame and not the vocabulary,
//! so the sections keep their names, the registers and the flags keep
//! their form, and the status bar keeps its hints.

use std::num::NonZeroU16;

use emo8067_debug::panels::{Cell, Flag, Line as Instruction, Panels};
use emo8067_debug::source::SourceView;
use emo8067_debug::{Access, Entry, Kind};
use ratatui::Frame;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

use crate::app::{App, Edit, FLAGS_COLUMNS, Focus};

/// The palette: the roles the screen has, and the values they hold.
#[path = "theme.rs"]
pub mod theme;

/// The registers panel's grid: two columns.
const COLUMNS: usize = 2;

/// The bytes a row of the memory window holds over a wide screen.
const WIDE_BYTES: usize = 16;

/// The bytes a row of the memory window holds under one.
const NARROW_BYTES: usize = 8;

/// The screen's width at which the memory window shows sixteen bytes.
///
/// The window shows sixteen bytes a row over this width, and eight under
/// it: a narrow terminal has no room for the printable side beside them.
const WIDE_SCREEN: u16 = 140;

/// The width of the memory window's gutter: the address, and the space
/// after it.
const GUTTER: usize = 5;

/// The columns the registers take.
const REGISTERS_WIDTH: u16 = 22;

/// The columns the flags take.
const FLAGS_WIDTH: u16 = 20;

/// The lines of source the strip shows at most.
///
/// The research's own strip is three to five lines, and five is the most
/// that is worth showing.
const SOURCE_LINES: u16 = 5;

/// The rows the listing keeps, so the strip never squeezes it out.
///
/// These are the listing's own rows: the header of the section takes one
/// more row of the band.
const LISTING_ROWS: u16 = 3;

/// The glyph that says "here": the instruction the machine is at, and the
/// place the console's cursor is at.
const HERE: &str = "▸";

/// The glyph of a breakpoint.
const BREAKPOINT: &str = "◆";

/// The glyph of a watchpoint.
const WATCHPOINT: &str = "◇";

/// The text of a register cell: the name of the register and its value.
///
/// The console prints the registers as the panel draws them, so the text
/// is made here and used in both places: the project grows one format and
/// not two.
pub(crate) fn register_text(cell: &Cell) -> String {
    format!("{} {:04X}", cell.register.name(), cell.value)
}

/// The text of a flag: the name of the flag and its value.
///
/// The console prints the flags as the panel draws them, so the text is
/// made here and used in both places.
pub(crate) fn flag_text(flag: Flag, value: bool) -> String {
    format!("{} {}", flag.name(), if value { "1" } else { "0" })
}

/// The text of a line of the disassembly: the address, the bytes and the
/// instruction.
///
/// The console says the instruction at `CS:IP` as the listing draws it,
/// so the text is made here and used in both places.
pub(crate) fn line_text(line: &Instruction) -> String {
    format!(
        "{:04X}:{:04X}  {:<12}  {}",
        line.segment,
        line.offset,
        bytes_of(line),
        line.text
    )
}

/// The bytes of an instruction, as hex.
fn bytes_of(line: &Instruction) -> String {
    line.bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Draw the whole screen.
pub fn draw(app: &App, frame: &mut Frame) {
    let panels = app.panels();
    let area = frame.area();
    // The ground of the screen is the canvas, and the header and the
    // bottom band are one step above it.
    frame.render_widget(Paragraph::new("").style(theme::canvas()), area);
    // The bands, top to bottom: the header, the rule under it, the main
    // band, the rule above the bottom band, the bottom band, the rule
    // above the status bar, and the status bar.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(bottom_rows(area.height)),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    header(app, frame, rows[0]);
    rule(frame, rows[1]);
    if app.help {
        help(frame, rows[2]);
    } else if app.console_open {
        console(app, frame, rows[2]);
    } else if app.conditions_open {
        conditions(app, frame, rows[2]);
    } else if app.stack_open {
        stack(&panels, app, frame, rows[2]);
    } else {
        listing(&panels, app, frame, rows[2]);
    }
    rule(frame, rows[3]);
    // The bottom band: the registers, the memory window and the flags,
    // side by side and separated by a rule.
    let sides = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(REGISTERS_WIDTH),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(FLAGS_WIDTH),
        ])
        .split(rows[4]);
    frame.render_widget(Paragraph::new("").style(theme::surface()), rows[4]);
    registers(&panels, app, frame, sides[0]);
    memory(&panels, app, frame, sides[2], area.width);
    flags(&panels, app, frame, sides[4]);
    vertical_rule(frame, sides[1]);
    vertical_rule(frame, sides[3]);
    rule(frame, rows[5]);
    status(app, frame, rows[6]);
}

/// The rows the bottom band takes of the screen.
///
/// The band holds the registers, the memory window and the flags side by
/// side, and the registers are the tallest of the three: a header and
/// seven rows of cells. The band grows with the screen, and slowly, so a
/// big terminal gives the memory window more rows without taking them
/// from the listing.
fn bottom_rows(height: u16) -> u16 {
    let extra = height.saturating_sub(20) / 8;
    (9 + extra).min(16).min(height.saturating_sub(8))
}

/// The header: the product, the program's name, and the state.
///
/// The header is one row and it is not a card: the product is dim, the
/// program's name is a value, and the state carries its glyph.
fn header(app: &App, frame: &mut Frame, area: Rect) {
    let (glyph, word, style) = if app.finished {
        ("○", "HALTED", theme::muted())
    } else {
        ("●", "RUNNING", theme::accent())
    };
    let line = Line::from(vec![
        Span::styled(" emo8067", theme::label()),
        Span::styled(format!("  {}", app.name), theme::value()),
        Span::styled(format!("  {glyph}"), style),
        Span::styled(format!(" {word}"), theme::value()),
    ]);
    frame.render_widget(Paragraph::new(line).style(theme::surface()), area);
}

/// A rule: the thin line between two bands.
fn rule(frame: &mut Frame, area: Rect) {
    let line = Line::from(Span::styled(
        "─".repeat(usize::from(area.width)),
        theme::rule(),
    ));
    frame.render_widget(Paragraph::new(line).style(theme::canvas()), area);
}

/// A rule between two of the bottom band's sections.
///
/// The rule is a line of its own in every row of the band, and not a row
/// of rules: a paragraph would wrap a row of them into one row only.
fn vertical_rule(frame: &mut Frame, area: Rect) {
    let lines = (0..area.height)
        .map(|_| Line::from(Span::styled("│", theme::rule())))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::surface()), area);
}

/// The style of the header of a section.
///
/// The section that has the focus says so with its header: it is the
/// accent and it is bold, and the headers of the other sections are
/// muted.
fn section_style(focused: bool) -> Style {
    if focused {
        theme::title()
    } else {
        theme::muted()
    }
}

/// The listing, and the source strip under it.
///
/// The band is split in two: the listing above, the strip below. The strip
/// has a header of its own, so it does not float, and it takes as many
/// rows as it can without squeezing the listing out - the listing keeps
/// three rows of its own - and a program that has no source leaves the
/// band to the listing alone.
fn listing(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    // The model says how many lines the program has, and the band says how
    // many of them fit under the listing.
    let view = app.source_view(usize::from(SOURCE_LINES));
    let rows = source_rows(area.height, view.lines.len() as u16);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(rows)])
        .split(area);
    disassembly(panels, app, frame, parts[0]);
    if rows > 0 {
        // The view is asked again for the count the strip shows, so the
        // window of lines is the one that fits in the band.
        let view = app.source_view(usize::from(rows - 1));
        source(&view, frame, parts[1]);
    }
}

/// The rows the source strip takes of the band.
///
/// The strip has a header of its own and the lines under it, and the
/// listing keeps three rows of its own: the band is short on an 80x20
/// terminal, and the strip must not squeeze the listing out. A program
/// with fewer lines than that takes only the rows it has.
fn source_rows(height: u16, available: u16) -> u16 {
    let room = height.saturating_sub(LISTING_ROWS + 2);
    let lines = room.min(SOURCE_LINES).min(available);
    if lines == 0 { 0 } else { lines + 1 }
}

/// The disassembly: the instructions, with the cursor and the markers.
fn disassembly(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled(
        "disassembly",
        section_style(app.focus == Focus::Disassembly),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let rows = usize::from(body.height);
    let first = first_line(panels.disassembly.len(), app.cursor, rows);
    let lines = panels
        .disassembly
        .iter()
        .skip(first)
        .take(rows)
        .enumerate()
        .map(|(index, line)| listing_line(app, first + index, line))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), body);
}

/// The first line the listing shows.
///
/// The listing is longer than the band, and the cursor moves over it: the
/// window follows the cursor, so the line it is on is always on the
/// screen.
fn first_line(lines: usize, cursor: usize, rows: usize) -> usize {
    if rows == 0 {
        return 0;
    }
    cursor.saturating_sub(rows - 1).min(lines.saturating_sub(1))
}

/// One line of the listing, with its markers and its colours.
///
/// The address is in the gutter, the bytes are muted, and the
/// instruction's text is a value. The line the machine is at carries `▸`
/// and the accent, a line that carries a breakpoint carries `◆` and a
/// line that carries a watchpoint carries `◇`, and the address of the
/// line the cursor is on is the accent.
fn listing_line(app: &App, at: usize, line: &Instruction) -> Line<'static> {
    let here = is_here(app, line);
    let cursor = at == app.cursor;
    let text = if here {
        theme::accent()
    } else if app.executed.contains(&line.offset) {
        theme::muted()
    } else {
        theme::value()
    };
    let address = format!("{:04X}:{:04X}", line.segment, line.offset);
    let address_style = if cursor {
        theme::accent()
    } else {
        theme::label()
    };
    let mut spans = vec![Span::styled(
        if here { HERE } else { " " },
        if here {
            theme::accent()
        } else {
            theme::canvas()
        },
    )];
    match state_marker(app, line) {
        Some((glyph, style)) => spans.push(Span::styled(glyph, style)),
        None => spans.push(Span::raw(" ")),
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(address, address_style));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(bytes_of(line), theme::muted()));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(line.text.clone(), text));
    Line::from(spans)
}

/// Whether a line is the instruction the machine is at.
///
/// The marker says where the machine is, and it needs a program to mean
/// anything: a machine with no program is not at an instruction.
fn is_here(app: &App, line: &Instruction) -> bool {
    app.program.is_some() && line.offset == app.cpu.regs.ip
}

/// The marker of a breakpoint or a watchpoint at a line.
///
/// A breakpoint carries `◆` and a watchpoint carries `◇`, and a line that
/// carries neither carries nothing.
fn state_marker(app: &App, line: &Instruction) -> Option<(&'static str, Style)> {
    if app
        .breakpoints
        .watchpoints()
        .iter()
        .any(|watch| watch.segment == line.segment && watch.offset == line.offset)
    {
        return Some((WATCHPOINT, theme::error()));
    }
    if app
        .breakpoints
        .breakpoints()
        .iter()
        .any(|breakpoint| breakpoint.segment == line.segment && breakpoint.offset == line.offset)
    {
        return Some((BREAKPOINT, theme::warning()));
    }
    None
}

/// The source strip: the program's own lines, and the one the machine is
/// stopped at.
///
/// The strip has a header of its own, so it does not float under the
/// listing. Every line carries the number the source gave it and the
/// source's own text, and the line the machine is stopped at carries `▸`
/// and the accent: the listing's own glyph, and the listing's own
/// meaning.
fn source(view: &SourceView, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled("source", theme::muted()));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let lines = view
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let current = view.current == Some(index);
            let text = if current {
                theme::accent()
            } else {
                theme::value()
            };
            Line::from(vec![
                Span::styled(
                    if current { HERE } else { " " },
                    if current {
                        theme::accent()
                    } else {
                        theme::canvas()
                    },
                ),
                Span::styled(format!("{:>4}", line.number), theme::meta()),
                Span::raw("  "),
                // The model keeps the truth: the drawing tidies the line,
                // and the space at its end is not part of the source.
                Span::styled(line.text.trim_end().to_string(), text),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), body);
}

/// The stack pane: the words on the stack, and the pointer among them.
///
/// The stack is a pane you open and not a fourth column: it takes the
/// listing's place in the main band. The header carries the segment and
/// the pointer, and every line is a word with the offset it is at.
fn stack(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let view = &panels.stack;
    let at = app.stack_at();
    let header = Line::from(Span::styled(
        format!("stack · {:04X}:{:04X}", view.segment, view.sp),
        section_style(app.focus == Focus::Disassembly),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let mut lines = Vec::new();
    for (index, word) in view.words.iter().enumerate() {
        let offset = at.wrapping_add(2u16.wrapping_mul(index as u16));
        // The word the pointer is at is marked, the way the byte under the
        // memory cursor is.
        let style = if offset == view.sp {
            theme::value().add_modifier(Modifier::REVERSED)
        } else {
            theme::value()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{offset:04X}"), theme::label()),
            Span::raw("  "),
            Span::styled(format!("{word:04X}"), style),
        ]));
    }
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), body);
}

/// The conditions pane: the breakpoints and the watchpoints, and what
/// each one carries.
///
/// The conditions are a window and not a fifth column: they take the
/// listing's place in the main band, the way the stack does. The rows are
/// the model's own listing, so the pane holds no copy of the
/// breakpoints.
///
/// Every row says where the entry is, what it is, and what it carries: a
/// breakpoint says the condition it has, and a watchpoint says what it
/// watches. The entry under the cursor is marked, the way the entry
/// under the cursor of every other pane is.
fn conditions(app: &App, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled(
        "conditions",
        section_style(app.focus == Focus::Disassembly),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let entries = app.breakpoints.listing();
    let lines = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            // The entry under the cursor is edited in place, and it shows
            // what has been typed over it, the way the register under the
            // cursor does.
            let detail = match app.edit.as_ref() {
                Some(Edit::Condition {
                    segment,
                    offset,
                    text,
                }) if *segment == entry.segment && *offset == entry.offset => text.clone(),
                _ => detail_of(entry),
            };
            let (glyph, kind_style) = match entry.kind {
                Kind::Breakpoint => (BREAKPOINT, theme::warning()),
                Kind::Watchpoint(_) => (WATCHPOINT, theme::error()),
            };
            let cursor = index == app.condition_cursor;
            let address_style = if cursor {
                theme::label().add_modifier(Modifier::REVERSED)
            } else {
                theme::label()
            };
            Line::from(vec![
                Span::styled(glyph, kind_style),
                Span::raw(" "),
                Span::styled(
                    format!("{:04X}:{:04X}", entry.segment, entry.offset),
                    address_style,
                ),
                Span::raw("  "),
                Span::styled(format!("{:<10}", kind_of(entry.kind)), kind_style),
                Span::raw("  "),
                Span::styled(detail, theme::value()),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), body);
}

/// What an entry of the listing is called.
///
/// The kind is the model's own: a breakpoint is a breakpoint and a
/// watchpoint is a watchpoint, and the pane says which is which.
fn kind_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Breakpoint => "breakpoint",
        Kind::Watchpoint(_) => "watchpoint",
    }
}

/// What an entry of the listing carries.
///
/// A breakpoint carries a condition, or none at all; a watchpoint carries
/// no condition and says what it watches instead.
fn detail_of(entry: &Entry) -> String {
    match entry.kind {
        Kind::Breakpoint => match entry.condition.as_deref() {
            Some(text) if !text.is_empty() => text.to_string(),
            _ => "no condition".to_string(),
        },
        Kind::Watchpoint(access) => access_of(access).to_string(),
    }
}

/// What a watchpoint watches, in words.
///
/// The access says what it watches: a write, a read, or both.
fn access_of(access: Access) -> &'static str {
    match access {
        Access::Write => "write",
        Access::Read => "read",
        Access::Both => "read and write",
    }
}

/// The console window: the log of what the commands said, and the command
/// line at the bottom.
///
/// The console is a window and not a column: it takes the listing's place
/// in the main band, the way the stack and the conditions do. The log is
/// above and the command line is at the bottom, the way the reference's
/// own debug window has a log and a box to type in.
///
/// The log grows with the commands and the window does not, so the last
/// lines that fit are the ones that are shown.
fn console(app: &App, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled(
        "console",
        section_style(app.focus == Focus::Disassembly),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(body);
    let height = usize::from(parts[0].height);
    let lines = app
        .console
        .output
        .iter()
        .skip(app.console.output.len().saturating_sub(height))
        .map(|line| Line::from(Span::styled(line.clone(), theme::value())))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), parts[0]);
    // The command line: the `:` is the key that opens the console, and
    // the `▸` is the cursor of the line: the place where the next
    // character goes.
    let line = Line::from(vec![
        Span::styled(": ", theme::value()),
        Span::styled(app.console.text.clone(), theme::value()),
        Span::styled(HERE, theme::accent()),
    ]);
    frame.render_widget(Paragraph::new(line).style(theme::canvas()), parts[1]);
}

/// The registers panel: the machine's registers, in two columns.
///
/// The name of a register is a label and its value is a value, and the
/// register under the cursor is the accent when the panel has the focus.
fn registers(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled(
        "registers",
        section_style(app.focus == Focus::Registers),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::surface()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let cells = panels.registers.cells();
    let mut lines = Vec::new();
    for row in 0..cells.len().div_ceil(COLUMNS) {
        let mut spans = Vec::new();
        for column in 0..COLUMNS {
            let index = row * COLUMNS + column;
            let Some(cell) = cells.get(index) else {
                continue;
            };
            if column > 0 {
                spans.push(Span::raw("  "));
            }
            let name = cell.register.name();
            // The value that is being typed over the register is shown
            // in place of the one the machine holds.
            let text = match app.edit.as_ref() {
                Some(Edit::Register { register, text }) if *register == cell.register => {
                    text.clone()
                }
                _ => format!("{:04X}", cell.value),
            };
            let cursor = index == app.register_cursor && app.focus == Focus::Registers;
            let name_style = if cursor {
                theme::accent()
            } else {
                theme::label()
            };
            let value_style = if cursor {
                theme::accent()
            } else {
                theme::value()
            };
            spans.push(Span::styled(name, name_style));
            spans.push(Span::styled(format!(" {text}"), value_style));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).style(theme::surface()), body);
}

/// The memory window: the bytes around the cursor, and the printable side
/// beside them.
///
/// The window is a window: the model hands over the bytes it holds and
/// where the cursor is in them, and the rows shown are the ones around the
/// cursor.
///
/// The byte under the cursor is marked whether the section has the focus
/// or not: with the accent when it has it, and with the muted colour when
/// it does not.
fn memory(panels: &Panels, app: &App, frame: &mut Frame, area: Rect, screen: u16) {
    let view = &panels.memory;
    let header = Line::from(Span::styled(
        format!("memory · {:04X}:{:04X}", view.segment, view.at),
        section_style(app.focus == Focus::Memory),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::surface()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let width = usize::from(body.width);
    let per_row = bytes_per_row(screen, width);
    // What fits in the panel: the gutter, the bytes as hex, and — when
    // there is room for it — the printable side.
    let printable = width >= GUTTER + per_row * 4;
    let rows = usize::from(body.height);
    // The window follows the cursor: the cursor's row is in the middle of
    // what is shown, and the window is never left behind.
    let first_row = (view.cursor / per_row).saturating_sub(rows / 2);
    let mut lines = Vec::new();
    let mut cursor = None;
    for row in 0..rows {
        let index = (first_row + row) * per_row;
        if index >= view.bytes.len() {
            break;
        }
        let address = view.window_at.wrapping_add(index as u16);
        let mut spans = vec![Span::styled(format!("{address:04X} "), theme::muted())];
        for column in 0..per_row {
            let Some(byte) = view.bytes.get(index + column) else {
                break;
            };
            if column > 0 {
                spans.push(Span::raw(" "));
            }
            // The byte that is being edited shows what has been typed
            // over it, the way the register under the cursor does.
            let at = view.window_at.wrapping_add((index + column) as u16);
            let text = match app.edit.as_ref() {
                Some(Edit::Byte {
                    segment,
                    offset,
                    text,
                }) if *segment == view.segment && *offset == at => text.clone(),
                _ => format!("{byte:02X}"),
            };
            let marked = index + column == view.cursor;
            let style = if marked {
                if app.focus == Focus::Memory {
                    theme::accent()
                } else {
                    theme::muted()
                }
            } else {
                theme::value()
            };
            if marked {
                // The byte under the cursor is one cell, so that the mark
                // covers the whole byte.
                cursor = Some(Cursor {
                    column: area.x + GUTTER as u16 + 3 * column as u16,
                    row: body.y + row as u16,
                    text: text.clone(),
                });
            }
            spans.push(Span::styled(text, style));
        }
        if printable {
            let characters: String = view.ascii.chars().skip(index).take(per_row).collect();
            spans.push(Span::styled(format!(" {characters}"), theme::muted()));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(MemoryRows { lines, cursor }, body);
}

/// The bytes a row of the memory window holds.
///
/// The window shows sixteen bytes over a wide screen, and eight under one:
/// a narrow terminal has no room for the printable side beside them.
fn bytes_per_row(screen: u16, width: usize) -> usize {
    let wanted = if screen >= WIDE_SCREEN {
        WIDE_BYTES
    } else {
        NARROW_BYTES
    };
    wanted.min(width.saturating_sub(GUTTER) / 3).max(1)
}

/// The rows of the memory window.
///
/// The rows are drawn as a paragraph, and the byte under the cursor is
/// made one cell afterwards: the mark is the byte and not half of it.
/// The cells that the byte takes are left empty, so that its two
/// characters keep the two columns they take on the screen.
struct MemoryRows<'a> {
    /// The rows of the window.
    lines: Vec<Line<'a>>,
    /// The byte under the cursor, and where it is.
    cursor: Option<Cursor>,
}

/// The byte under the cursor, and where it is on the screen.
struct Cursor {
    /// The column the byte is at.
    column: u16,
    /// The row the byte is on.
    row: u16,
    /// The text the byte is drawn with.
    text: String,
}

impl Widget for MemoryRows<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines)
            .style(theme::surface())
            .render(area, buf);
        let Some(cursor) = self.cursor else {
            return;
        };
        let width = cursor.text.chars().count() as u16;
        if let Some(cell) = buf.cell_mut((cursor.column, cursor.row)) {
            cell.set_symbol(&cursor.text);
            // The cell holds two characters and one column. The cell that
            // follows it is left empty, so the two characters keep the two
            // columns they take on the screen, and the cell is told that it
            // is one column wide, so that the cell it covers is drawn as
            // well.
            cell.set_diff_option(CellDiffOption::ForcedWidth(
                NonZeroU16::new(1).expect("one is not zero"),
            ));
        }
        for offset in 1..width {
            if let Some(cell) = buf.cell_mut((cursor.column + offset, cursor.row)) {
                cell.set_symbol("");
            }
        }
    }
}

/// The flags panel: the nine flags and their values.
///
/// The name of a flag is a label and its value is a value, and the flag
/// under the cursor is the accent when the panel has the focus.
fn flags(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let header = Line::from(Span::styled(
        "flags",
        section_style(app.focus == Focus::Flags),
    ));
    frame.render_widget(
        Paragraph::new(header).style(theme::surface()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let mut lines = Vec::new();
    for (row, chunk) in Flag::ALL.chunks(FLAGS_COLUMNS).enumerate() {
        let mut spans = Vec::new();
        for (column, flag) in chunk.iter().enumerate() {
            if column > 0 {
                spans.push(Span::raw("  "));
            }
            let index = row * FLAGS_COLUMNS + column;
            // The value that is being typed over the flag is shown in
            // place of the one the machine holds.
            let text = match app.edit.as_ref() {
                Some(Edit::Flag { flag: edited, text }) if edited == flag => text.clone(),
                _ => {
                    if panels.registers.flag(*flag) {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    }
                }
            };
            let cursor = index == app.flag_cursor && app.focus == Focus::Flags;
            let name_style = if cursor {
                theme::accent()
            } else {
                theme::label()
            };
            let value_style = if cursor {
                theme::accent()
            } else {
                theme::value()
            };
            spans.push(Span::styled(flag.name(), name_style));
            spans.push(Span::styled(format!(" {text}"), value_style));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).style(theme::surface()), body);
}

/// The status bar: the hints on the left, and the volatile data on the
/// right.
///
/// The hints are what the user needs once, and the numbers are what the
/// user needs always: the hints are on the left and the numbers are on
/// the right. What the user is doing is said on its own, and it is said
/// instead of the bar.
fn status(app: &App, frame: &mut Frame, area: Rect) {
    let message = match (&app.status, &app.edit, &app.goto) {
        (Some(status), _, _) => Some(status.clone()),
        (None, Some(edit), _) => Some(edit_says(edit)),
        (None, None, Some(goto)) => Some(format!("goto: {}", goto.text)),
        (None, None, None) => None,
    };
    if let Some(message) = message {
        frame.render_widget(
            Paragraph::new(Span::styled(message, theme::value())).style(theme::canvas()),
            area,
        );
        return;
    }
    let hints = "q quits · ? helps";
    let numbers = format!(
        "IP {:04X} · STEP {} · BREAK {} · WATCH {}",
        app.cpu.regs.ip,
        app.steps,
        app.breakpoints.breakpoints().len(),
        app.breakpoints.watchpoints().len(),
    );
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(hints.chars().count() as u16),
            Constraint::Min(0),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(hints, theme::muted())).style(theme::canvas()),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(numbers, theme::label()))
            .alignment(Alignment::Right)
            .style(theme::canvas()),
        parts[1],
    );
}

/// What the status line says about the thing that is being edited.
///
/// The register is named with the value that is being typed, the flag is
/// named, and the byte is named by its address: each of them says what
/// the value will be written into. The condition is named by its address
/// too, because that is where it will be written.
fn edit_says(edit: &Edit) -> String {
    match edit {
        Edit::Register { register, text } => format!("{} = {}", register.name(), text),
        Edit::Flag { flag, .. } => flag.name().to_string(),
        Edit::Byte {
            segment, offset, ..
        } => format!("{segment:04X}:{offset:04X}"),
        Edit::Condition {
            segment,
            offset,
            text,
        } => format!("{segment:04X}:{offset:04X} = {text}"),
    }
}

/// The help overlay.
///
/// The overlay takes the main band's place, and it is not a card: the
/// ground is cleared and the keys are listed under a header of their own.
fn help(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new("").style(theme::canvas()), area);
    let header = Line::from(Span::styled("keys", theme::muted()));
    frame.render_widget(
        Paragraph::new(header).style(theme::canvas()),
        Rect { height: 1, ..area },
    );
    let body = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let keys = [
        "F5   run to the next breakpoint",
        "F8   one instruction",
        "F9   a breakpoint at the cursor",
        "Tab  the next pane",
        "1    the disassembly",
        "2    the registers",
        "3    the memory",
        "4    the flags",
        "5    the stack (5 again closes it)",
        "b    the conditions (b again closes it)",
        ":    the console (: again closes it)",
        "e    edit the value under the cursor",
        "g    jump the memory to an address, a label or a segment",
        "↑ ↓  move the cursor",
        "← →  move the cursor (registers, flags)",
        "PgUp PgDn  a page of memory",
        "Enter commit the edit",
        "Esc  cancel the edit, close the stack",
        "?    close this",
        "q    quit",
    ];
    let lines = keys
        .iter()
        .map(|key| Line::from(Span::styled(*key, theme::value())))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).style(theme::canvas()), body);
}
