//! The breakpoints and the watchpoints.
//!
//! The reference's own breakpoint syntax is not recovered anywhere: the word
//! "breakpoint" appears in none of its thirty-four documentation files,
//! although `frmTrapping` proves the mechanism exists
//! (`odd/research/features-and-ux.md`). So this part is a design choice, and
//! the research made it:
//!
//! > `b AX==0x1234 && CS==0x0200` stops at exactly that address; `w
//! > 0100:0004` fires on write and on read-only access reports why it did
//! > not.
//!
//! The condition of a breakpoint is an expression in the reference's own
//! language, with the comparisons this project added
//! (`odd/research/features-and-ux.md` §6).

use crate::expr::{self, Context};

/// A breakpoint: an address, and a condition that may hold there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Breakpoint {
    /// The segment the breakpoint is in.
    pub segment: u16,
    /// The offset the breakpoint is at.
    pub offset: u16,
    /// The condition it carries, if it has one.
    pub condition: Option<String>,
}

/// What a watchpoint watches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    /// A write to the address.
    Write,
    /// A read from the address.
    Read,
    /// A write or a read.
    Both,
}

impl Access {
    /// Whether the access includes a write.
    fn writes(self) -> bool {
        matches!(self, Access::Write | Access::Both)
    }

    /// Whether the access includes a read.
    fn reads(self) -> bool {
        matches!(self, Access::Read | Access::Both)
    }
}

/// A watchpoint: an address, and what it watches there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Watchpoint {
    /// The segment it watches.
    pub segment: u16,
    /// The offset it watches.
    pub offset: u16,
    /// What it watches.
    pub access: Access,
}

/// Why a watchpoint did not fire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Missed {
    /// The watchpoint watches writes, and the access was a read.
    NotAWrite,
    /// The watchpoint watches reads, and the access was a write.
    NotARead,
}

/// What an access did, and what the watchpoints said about it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Report {
    /// The watchpoints the access hit.
    pub hit: Vec<u16>,
    /// The watchpoints it did not hit, and why.
    pub missed: Vec<(u16, Missed)>,
}

impl Report {
    /// The report of an access that changed nothing.
    fn empty() -> Report {
        Report {
            hit: Vec::new(),
            missed: Vec::new(),
        }
    }
}

/// The breakpoints and the watchpoints.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Breakpoints {
    breakpoints: Vec<Breakpoint>,
    watchpoints: Vec<Watchpoint>,
}

impl Breakpoints {
    /// Nothing set yet.
    #[must_use]
    pub fn new() -> Breakpoints {
        Breakpoints::default()
    }

    /// The breakpoints, in the order they were set.
    #[must_use]
    pub fn breakpoints(&self) -> &[Breakpoint] {
        &self.breakpoints
    }

    /// The watchpoints, in the order they were set.
    #[must_use]
    pub fn watchpoints(&self) -> &[Watchpoint] {
        &self.watchpoints
    }

    /// Set a breakpoint at an address, with a condition.
    ///
    /// The condition is checked when the machine arrives, and it is parsed
    /// here, so a condition that cannot be read is refused at the point
    /// where it is written.
    ///
    /// # Errors
    /// Returns the expression's own refusal when the condition does not
    /// parse.
    pub fn add(
        &mut self,
        segment: u16,
        offset: u16,
        condition: Option<&str>,
    ) -> Result<&mut Breakpoints, expr::Error> {
        if let Some(text) = condition {
            expr::check(text)?;
        }
        self.breakpoints.push(Breakpoint {
            segment,
            offset,
            condition: condition.map(str::to_string),
        });
        Ok(self)
    }

    /// Take the breakpoint at an address away.
    pub fn remove(&mut self, segment: u16, offset: u16) -> Option<Breakpoint> {
        let at = self
            .breakpoints
            .iter()
            .position(|b| b.segment == segment && b.offset == offset)?;
        Some(self.breakpoints.remove(at))
    }

    /// Watch an address.
    pub fn watch(&mut self, segment: u16, offset: u16, access: Access) -> &mut Breakpoints {
        self.watchpoints.push(Watchpoint {
            segment,
            offset,
            access,
        });
        self
    }

    /// Stop watching an address.
    pub fn unwatch(&mut self, segment: u16, offset: u16) -> Option<Watchpoint> {
        let at = self
            .watchpoints
            .iter()
            .position(|w| w.segment == segment && w.offset == offset)?;
        Some(self.watchpoints.remove(at))
    }

    /// Whether the machine stops at an address.
    ///
    /// A breakpoint with no condition always stops. One with a condition
    /// stops when the condition comes out true, and a condition that cannot
    /// be read does not stop: the machine carries on, and the debugger says
    /// why.
    #[must_use]
    pub fn stops_at(&self, context: &impl Context, segment: u16, offset: u16) -> bool {
        self.breakpoints
            .iter()
            .filter(|breakpoint| breakpoint.segment == segment && breakpoint.offset == offset)
            .any(|breakpoint| match &breakpoint.condition {
                None => true,
                Some(text) => expr::evaluate(text, context).is_ok_and(|value| value != 0),
            })
    }

    /// What a write did to the watchpoints.
    ///
    /// The write is where the memory says it landed: the segment, the offset
    /// and how many bytes it took.
    #[must_use]
    pub fn written(&self, write: Option<(u16, u16, u8)>) -> Report {
        let Some((segment, offset, width)) = write else {
            return Report::empty();
        };
        let mut report = Report::empty();
        for watchpoint in &self.watchpoints {
            let covered = watchpoint.segment == segment
                && watchpoint.offset >= offset
                && watchpoint.offset < offset + u16::from(width);
            if covered && watchpoint.access.writes() {
                report.hit.push(watchpoint.offset);
            } else {
                report.missed.push((watchpoint.offset, Missed::NotARead));
            }
        }
        report
    }

    /// What a read did to the watchpoints.
    ///
    /// A watchpoint on writes does not fire on a read, and says so.
    #[must_use]
    pub fn read(&self, segment: u16, offset: u16) -> Report {
        let mut report = Report::empty();
        for watchpoint in &self.watchpoints {
            let covered = watchpoint.segment == segment && watchpoint.offset == offset;
            if covered && watchpoint.access.reads() {
                report.hit.push(watchpoint.offset);
            } else {
                report.missed.push((watchpoint.offset, Missed::NotAWrite));
            }
        }
        report
    }
}
