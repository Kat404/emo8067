//! The drawing.
//!
//! The layout is the one the research draws (`features-and-ux.md` §4.4): the
//! title, the disassembly, the registers and the flags, and the status line.
//! The source strip is drawn under the disassembly, in the band the
//! disassembly is in.
//! The stack and the conditions are windows and not columns: they take the
//! listing's place in the band, and only one of them holds that place at a
//! time.
//! The colours are the research's too: cyan is the current `IP`, grey is
//! what has run, yellow is a breakpoint and red is a watchpoint.

use emo8067_debug::panels::{Cell, Flag, Line as Instruction, Panels};
use emo8067_debug::source::SourceView;
use emo8067_debug::{Access, Entry, Kind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::{App, Edit, FLAGS_COLUMNS, Focus};

/// The registers panel's grid: two columns.
const COLUMNS: usize = 2;

/// The bytes a row of the memory window holds.
const ROW_BYTES: usize = 16;

/// The width of the memory window's gutter: the address, and the space
/// after it.
const GUTTER: usize = 5;

/// The lines of source the strip shows at most.
///
/// The research's own strip is three to five lines, and five is the most
/// that is worth showing.
const SOURCE_LINES: u16 = 5;

/// The rows the listing keeps, so the strip never squeezes it out.
///
/// These are the listing's own rows: the border around it takes two more
/// rows of the band.
const LISTING_ROWS: u16 = 3;

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
    let bytes = line
        .bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{:04X}:{:04X}  {:<12}  {}",
        line.segment, line.offset, bytes, line.text
    )
}

/// Draw the whole screen.
pub fn draw(app: &App, frame: &mut Frame) {
    let panels = app.panels();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(9),
            Constraint::Length(1),
        ])
        .split(frame.area());
    title(app, frame, rows[0]);
    if app.help {
        help(frame, rows[1]);
    } else if app.console_open {
        console(app, frame, rows[1]);
    } else if app.conditions_open {
        conditions(app, frame, rows[1]);
    } else if app.stack_open {
        stack(&panels, app, frame, rows[1]);
    } else {
        listing(&panels, app, frame, rows[1]);
    }
    // The registers and the flags are the two panes the research draws on
    // the sides, and the middle one is where the memory window goes.
    let sides = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(26),
            Constraint::Min(0),
            Constraint::Length(22),
        ])
        .split(rows[2]);
    registers(&panels, app, frame, sides[0]);
    memory(&panels, app, frame, sides[1]);
    flags(&panels, app, frame, sides[2]);
    status(app, frame, rows[3]);
}

/// The title: the program, and whether it is running.
fn title(app: &App, frame: &mut Frame, area: Rect) {
    let state = if app.finished { "HALTED" } else { "RUNNING" };
    let title = format!(" emo8067 — {} — {state} ", app.name);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title).centered());
    frame.render_widget(block, area);
}

/// The listing, and the source strip under it.
///
/// The band is split in two: the listing above, the strip below. The strip
/// takes as many rows as it can without squeezing the listing out - the
/// listing keeps three rows of its own - and a program that has no source
/// leaves the band to the listing alone.
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
        let view = app.source_view(usize::from(rows));
        source(&view, frame, parts[1]);
    }
}

/// The rows the source strip takes of the band.
///
/// The strip shows up to five lines, and only as many as fit after the
/// listing has kept three rows of its own: the band is short on an 80x20
/// terminal, and the strip must not squeeze the listing out. A program
/// with fewer lines than that takes only the rows it has.
fn source_rows(height: u16, available: u16) -> u16 {
    // The listing's border takes two rows of the band, and the listing
    // keeps three rows of its own.
    let listing = LISTING_ROWS + 2;
    height
        .saturating_sub(listing)
        .min(SOURCE_LINES)
        .min(available)
}

/// The source strip: the program's own lines, and the one the machine is
/// stopped at.
///
/// The strip is drawn as plain lines, with no border of its own: it reads
/// as the source and nothing else. Every line carries the number the
/// source gave it and the source's own text.
///
/// The line the machine is stopped at is marked with `►` in the gutter.
/// The research's own drawing marks it with `◄ current` at the end of the
/// line; the arrow here is the other way round, because the marker is in
/// the gutter and it points at the line it marks - and it stays visible
/// however long the line is.
fn source(view: &SourceView, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (index, line) in view.lines.iter().enumerate() {
        let current = view.current == Some(index);
        let marker = if current { "►" } else { " " };
        // The model keeps the truth: the drawing tidies the line, and the
        // space at its end is not part of the source.
        let text = format!("{marker}{:>4}  {}", line.number, line.text.trim_end());
        // The line the machine is at is the current instruction's own
        // line, and cyan is the colour the research gives to the current
        // `IP`.
        let style = if current {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// The disassembly, with the cursor and the colours.
fn disassembly(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let items = panels
        .disassembly
        .iter()
        .map(|line| {
            let text = line_text(line);
            let style = if app
                .breakpoints
                .watchpoints()
                .iter()
                .any(|watch| watch.segment == line.segment && watch.offset == line.offset)
            {
                Style::default().fg(Color::Red)
            } else if app.breakpoints.breakpoints().iter().any(|breakpoint| {
                breakpoint.segment == line.segment && breakpoint.offset == line.offset
            }) {
                Style::default().fg(Color::Yellow)
            } else if line.offset == app.cpu.regs.ip {
                Style::default().fg(Color::Cyan)
            } else if app.executed.contains(&line.offset) {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" disassembly "),
        )
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(
        app.cursor.min(panels.disassembly.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, area, &mut state);
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
/// watches. The colours are the research's: a breakpoint is yellow and a
/// watchpoint is red.
fn conditions(app: &App, frame: &mut Frame, area: Rect) {
    let entries = app.breakpoints.listing();
    let items = entries
        .iter()
        .map(|entry| {
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
            let text = format!(
                "{:04X}:{:04X}  {:<10}  {}",
                entry.segment,
                entry.offset,
                kind_of(entry.kind),
                detail
            );
            let style = match entry.kind {
                Kind::Breakpoint => Style::default().fg(Color::Yellow),
                Kind::Watchpoint(_) => Style::default().fg(Color::Red),
            };
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" conditions "))
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(
        app.condition_cursor.min(entries.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, area, &mut state);
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

/// The stack pane: the words on the stack, and the pointer among them.
///
/// The stack is a pane you open and not a fourth column: it takes the
/// listing's place in the main band. The title carries the segment and the
/// pointer, and every line is a word with the offset it is at.
fn stack(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let view = &panels.stack;
    let at = app.stack_at();
    let mut lines = Vec::new();
    for (index, word) in view.words.iter().enumerate() {
        let offset = at.wrapping_add(2u16.wrapping_mul(index as u16));
        // The word the pointer is at is marked, the way the byte under the
        // memory cursor is.
        let style = if offset == view.sp {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::raw(format!("{offset:04X}  ")),
            Span::styled(format!("{word:04X}"), style),
        ]));
    }
    let title = format!(" stack · {:04X}:{:04X} ", view.segment, view.sp);
    let list = List::new(lines).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);
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
    let block = Block::default().borders(Borders::ALL).title(" console ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let height = usize::from(parts[0].height);
    let lines = app
        .console
        .output
        .iter()
        .skip(app.console.output.len().saturating_sub(height))
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), parts[0]);
    // The command line: the `:` is the key that opens the console.
    let prompt = format!(": {}", app.console.text);
    frame.render_widget(Paragraph::new(prompt), parts[1]);
}

/// The registers panel: the machine's registers, in two columns.
fn registers(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
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
            let text = match app.edit.as_ref() {
                Some(Edit::Register { register, text }) if *register == cell.register => {
                    format!("{name} {text}")
                }
                _ => register_text(cell),
            };
            let style = if index == app.register_cursor && app.focus == Focus::Registers {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }
    let list = List::new(lines).block(Block::default().borders(Borders::ALL).title(" registers "));
    frame.render_widget(list, area);
}

/// The memory window: the bytes around the cursor, and the printable side
/// beside them.
///
/// The window is a window: the model hands over the bytes it holds and
/// where the cursor is in them, and the rows shown are the ones around the
/// cursor.
fn memory(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let view = &panels.memory;
    let inner = usize::from(area.width.saturating_sub(2));
    // What fits in the panel: the gutter, the bytes as hex, and — when
    // there is room for it — the printable side.
    let bytes_per_row = ROW_BYTES.min(inner.saturating_sub(GUTTER) / 3).max(1);
    let printable = inner >= GUTTER + bytes_per_row * 3 + bytes_per_row;
    let rows = usize::from(area.height.saturating_sub(2));
    // The window follows the cursor: the cursor's row is in the middle of
    // what is shown, and the window is never left behind.
    let first_row = (view.cursor / bytes_per_row).saturating_sub(rows / 2);
    let mut lines = Vec::new();
    for row in 0..rows {
        let index = (first_row + row) * bytes_per_row;
        if index >= view.bytes.len() {
            break;
        }
        let address = view.window_at.wrapping_add(index as u16);
        let mut spans = vec![Span::raw(format!("{address:04X} "))];
        for column in 0..bytes_per_row {
            let Some(byte) = view.bytes.get(index + column) else {
                break;
            };
            if column > 0 {
                spans.push(Span::raw(" "));
            }
            let style = if index + column == view.cursor && app.focus == Focus::Memory {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
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
            spans.push(Span::styled(text, style));
        }
        if printable {
            let characters: String = view.ascii.chars().skip(index).take(bytes_per_row).collect();
            spans.push(Span::raw(format!(" {characters}")));
        }
        lines.push(Line::from(spans));
    }
    let title = format!(" memory · {:04X}:{:04X} ", view.segment, view.at);
    let list = List::new(lines).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);
}

/// The flags panel: the nine flags and their values.
///
/// The cursor is drawn the way the registers panel draws the register
/// under it: the flag under the cursor is the marked one, and the flag
/// that is being edited shows what has been typed over it.
fn flags(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (row, chunk) in Flag::ALL.chunks(FLAGS_COLUMNS).enumerate() {
        let mut spans = Vec::new();
        for (column, flag) in chunk.iter().enumerate() {
            if column > 0 {
                spans.push(Span::raw("  "));
            }
            let index = row * FLAGS_COLUMNS + column;
            let text = match app.edit.as_ref() {
                Some(Edit::Flag { flag: edited, text }) if edited == flag => {
                    format!("{} {text}", flag.name())
                }
                _ => flag_text(*flag, panels.registers.flag(*flag)),
            };
            let style = if index == app.flag_cursor && app.focus == Focus::Flags {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }
    let list = List::new(lines).block(Block::default().borders(Borders::ALL).title(" flags "));
    frame.render_widget(list, area);
}

/// The status line: where the machine is, and what is set.
fn status(app: &App, frame: &mut Frame, area: Rect) {
    let text = match (&app.status, &app.edit, &app.goto) {
        (Some(status), _, _) => status.clone(),
        (None, Some(edit), _) => edit_says(edit),
        (None, None, Some(goto)) => format!("goto: {}", goto.text),
        (None, None, None) => format!(
            "IP {:04X}  ·  STEP {}  ·  BREAK {}  ·  WATCH {}  ·  q quits · ? helps",
            app.cpu.regs.ip,
            app.steps,
            app.breakpoints.breakpoints().len(),
            app.breakpoints.watchpoints().len(),
        ),
    };
    frame.render_widget(Paragraph::new(text), area);
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
fn help(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("F5   run to the next breakpoint"),
        Line::from("F8   one instruction"),
        Line::from("F9   a breakpoint at the cursor"),
        Line::from("Tab  the next pane"),
        Line::from("1    the disassembly"),
        Line::from("2    the registers"),
        Line::from("3    the memory"),
        Line::from("4    the flags"),
        Line::from("5    the stack (5 again closes it)"),
        Line::from("b    the conditions (b again closes it)"),
        Line::from(":    the console (: again closes it)"),
        Line::from("e    edit the value under the cursor"),
        Line::from("g    jump the memory to an address, a label or a segment"),
        Line::from("↑ ↓  move the cursor"),
        Line::from("← →  move the cursor (registers, flags)"),
        Line::from("PgUp PgDn  a page of memory"),
        Line::from("Enter commit the edit"),
        Line::from("Esc  cancel the edit, close the stack"),
        Line::from("?    close this"),
        Line::from("q    quit"),
    ];
    let block = Block::default().borders(Borders::ALL).title(" keys ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
