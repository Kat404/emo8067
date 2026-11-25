//! The source panel.
//!
//! The panel is the program's own source and the place the machine is
//! stopped in it. It is a pure function of the program and of the machine,
//! the way every other panel is a pure function of the machine and of the
//! UI's state, and it reads nothing else.

use emo8067_asm::program::{Program, SourceLine};
use emo8067_core::Cpu;

/// The source panel: the program's own lines, and the one the machine is
/// stopped at.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceView {
    /// The lines the strip shows.
    pub lines: Vec<SourceLine>,
    /// Which of them the machine is stopped at, if it is stopped at one.
    ///
    /// The index is into [`SourceView::lines`]: the line the machine is at
    /// is in the strip itself, and not somewhere else in the program.
    pub current: Option<usize>,
}

impl SourceView {
    /// The view of the program's source, with the machine's place in it.
    ///
    /// The strip shows `count` lines: the one the machine is stopped at in
    /// the middle, and the lines before and after it. A machine that is
    /// not stopped in any line shows the first lines of the program.
    #[must_use]
    pub fn of(program: &Program, cpu: &Cpu, count: usize) -> SourceView {
        let total = program.lines.len();
        // The strip shows as many lines as it was asked for, and no more
        // than the program has.
        let count = count.min(total);
        if count == 0 {
            return SourceView {
                lines: Vec::new(),
                current: None,
            };
        }
        let ip = cpu.regs.ip;
        let stopped = program.lines.iter().position(|line| holds(line, ip));
        let start = match stopped {
            // The current line is in the middle of the strip, with the lines
            // before it and the lines after it.
            Some(at) => {
                let start = at.saturating_sub(count / 2);
                // Near the end of the program there are not enough lines
                // after the current one, so the strip moves up instead: it
                // still shows `count` lines, and the current one is as near
                // the middle as it can be.
                start.min(total - count)
            }
            // A machine that is not stopped in any line shows the first
            // lines of the program.
            None => 0,
        };
        let lines = program.lines[start..start + count].to_vec();
        let current = stopped.map(|at| at - start);
        SourceView { lines, current }
    }
}

/// Whether the machine's instruction pointer is inside a line's bytes.
///
/// The program's addresses are offsets in the load segment, and the loader
/// sets `IP` to the load offset, so the offsets are the ones to compare.
fn holds(line: &SourceLine, ip: u16) -> bool {
    let start = u32::from(line.address);
    let end = start + line.length as u32;
    let ip = u32::from(ip);
    ip >= start && ip < end
}
