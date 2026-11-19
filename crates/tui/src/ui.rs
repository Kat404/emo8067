//! The drawing.
//!
//! The layout is the one the research draws (`features-and-ux.md` §4.4): the
//! title, the disassembly, the registers and the flags, and the status line.
//! The colours are the research's too: cyan is the current `IP`, grey is
//! what has run, yellow is a breakpoint and red is a watchpoint.

use emo8067_debug::panels::{Flag, Panels};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::{App, Focus};

/// The registers panel's grid: two columns.
const COLUMNS: usize = 2;

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
    } else {
        disassembly(&panels, app, frame, rows[1]);
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
    flags(&panels, frame, sides[2]);
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

/// The disassembly, with the cursor and the colours.
fn disassembly(panels: &Panels, app: &App, frame: &mut Frame, area: Rect) {
    let items = panels
        .disassembly
        .iter()
        .map(|line| {
            let bytes = line
                .bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!(
                "{:04X}:{:04X}  {:<12}  {}",
                line.segment, line.offset, bytes, line.text
            );
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
            let text = match app
                .edit
                .as_ref()
                .filter(|edit| edit.register == cell.register)
            {
                Some(edit) => format!("{name} {}", edit.text),
                None => format!("{name} {:04X}", cell.value),
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

/// The flags panel: the nine flags and their values.
fn flags(panels: &Panels, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for chunk in Flag::ALL.chunks(3) {
        let mut spans = Vec::new();
        for (index, flag) in chunk.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            let value = if panels.registers.flag(*flag) {
                "1"
            } else {
                "0"
            };
            spans.push(Span::raw(format!("{} {}", flag.name(), value)));
        }
        lines.push(Line::from(spans));
    }
    let list = List::new(lines).block(Block::default().borders(Borders::ALL).title(" flags "));
    frame.render_widget(list, area);
}

/// The status line: where the machine is, and what is set.
fn status(app: &App, frame: &mut Frame, area: Rect) {
    let text = match (&app.status, &app.edit) {
        (Some(status), _) => status.clone(),
        (None, Some(edit)) => format!("{} = {}", edit.register.name(), edit.text),
        (None, None) => format!(
            "IP {:04X}  ·  STEP {}  ·  BREAK {}  ·  WATCH {}  ·  q quits · ? helps",
            app.cpu.regs.ip,
            app.steps,
            app.breakpoints.breakpoints().len(),
            app.breakpoints.watchpoints().len(),
        ),
    };
    frame.render_widget(Paragraph::new(text), area);
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
        Line::from("e    edit the register under the cursor"),
        Line::from("↑ ↓  move the cursor"),
        Line::from("← →  move the cursor (registers)"),
        Line::from("Enter commit the edit"),
        Line::from("Esc  cancel the edit"),
        Line::from("?    close this"),
        Line::from("q    quit"),
    ];
    let block = Block::default().borders(Borders::ALL).title(" keys ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
