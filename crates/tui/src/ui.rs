//! The drawing.
//!
//! The layout is the one the research draws (`features-and-ux.md` §4.4): the
//! title, the disassembly, and the status line. The colours are the
//! research's too: cyan is the current `IP`, grey is what has run, yellow is
//! a breakpoint and red is a watchpoint.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::App;

/// Draw the whole screen.
pub fn draw(app: &App, frame: &mut Frame) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
    title(app, frame, rows[0]);
    if app.help {
        help(frame, rows[1]);
    } else {
        disassembly(app, frame, rows[1]);
    }
    status(app, frame, rows[2]);
}

/// The title: the program, and whether it is running.
fn title(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let state = if app.finished { "HALTED" } else { "RUNNING" };
    let title = format!(" emo8067 — {} — {state} ", app.name);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title).centered());
    frame.render_widget(block, area);
}

/// The disassembly, with the cursor and the colours.
fn disassembly(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let panels = app.panels();
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

/// The status line: where the machine is, and what is set.
fn status(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let text = match &app.status {
        Some(status) => status.clone(),
        None => format!(
            "IP {:04X}  ·  STEP {}  ·  BREAK {}  ·  WATCH {}  ·  q quits · ? helps",
            app.cpu.regs.ip,
            app.executed.len(),
            app.breakpoints.breakpoints().len(),
            app.breakpoints.watchpoints().len(),
        ),
    };
    frame.render_widget(Paragraph::new(text), area);
}

/// The help overlay.
fn help(frame: &mut Frame, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from("F5   run to the next breakpoint"),
        Line::from("F8   one instruction"),
        Line::from("F9   a breakpoint at the cursor"),
        Line::from("↑ ↓  move the cursor"),
        Line::from("?    close this"),
        Line::from("q    quit"),
    ];
    let block = Block::default().borders(Borders::ALL).title(" keys ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
