//! The journal of the steps the machine can undo.
//!
//! The step back is the reference's: the machine goes back to the state it
//! had before the instruction, which means the bytes that instruction
//! wrote have to go back too. The journal keeps both — the registers, with
//! the flags and `IP`, and the bytes, with the value they held.

use std::collections::VecDeque;

use crate::StopReason;
use crate::regs::Registers;

/// How many steps the journal keeps.
pub const MAXIMUM_STEPS_BACK: usize = 256;

/// How many write entries the journal holds, in total.
///
/// The bound is on the entries and not on the steps: a guest that writes
/// a megabyte in one instruction must not make the debugger hold a
/// megabyte for every step it keeps. Sixty-five thousand entries are
/// about half a megabyte, whatever the guest does.
pub const MAXIMUM_WRITES: usize = 65_536;

/// One step, and everything needed to undo it.
#[derive(Clone, Debug)]
pub struct Undo {
    /// The registers, the segments, the flags and `IP`, as they were.
    pub regs: Registers,
    /// The bytes the instruction wrote, and the value they held.
    ///
    /// The addresses are physical: the instruction wrote through a
    /// segment, and the bytes are where they landed on the bus.
    pub writes: Vec<(u32, u8)>,
    /// Whether the machine was halted.
    pub halted: bool,
    /// Why the machine had stopped.
    pub stop_reason: StopReason,
    /// How deep the machine was in the calls it had made.
    ///
    /// The depth is part of the machine's state: a step back over a call
    /// puts the machine outside it again, the way it was.
    pub call_depth: i32,
}

/// The journal: the steps the machine can undo.
#[derive(Clone, Debug)]
pub struct History {
    /// The steps, oldest first.
    records: VecDeque<Undo>,
    /// How many steps the journal keeps. Zero is a journal that is off.
    limit: usize,
    /// How many write entries the records hold.
    writes: usize,
    /// Whether a step could not be kept whole.
    ///
    /// A step that wrote more than the journal can hold cannot be put
    /// back: the machine's memory has moved past it, and the records that
    /// are left would restore a state that never existed. The journal is
    /// broken from then on.
    broken: bool,
}

impl Default for History {
    /// A journal that is off.
    ///
    /// The headless runner never rewinds the machine, so it keeps no
    /// journal and pays nothing for one.
    fn default() -> Self {
        Self::new(0)
    }
}

impl History {
    /// A journal that keeps `limit` steps.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            records: VecDeque::new(),
            limit,
            writes: 0,
            broken: false,
        }
    }

    /// Whether the journal keeps anything.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.limit > 0
    }

    /// How many steps the journal holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the journal holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// How many write entries the journal holds.
    #[must_use]
    pub fn writes(&self) -> usize {
        self.writes
    }

    /// Whether a step could not be kept whole, and the journal is broken.
    #[must_use]
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// Keep a step, and drop the oldest ones when the journal is full.
    ///
    /// The oldest go first: the records that are left are the most recent
    /// ones, and rewinding through them undoes the steps in order.
    pub fn push(&mut self, undo: Undo) {
        if self.limit == 0 || self.broken {
            return;
        }
        self.writes += undo.writes.len();
        self.records.push_back(undo);
        while self.records.len() > self.limit || self.writes > MAXIMUM_WRITES {
            let Some(oldest) = self.records.pop_front() else {
                break;
            };
            self.writes -= oldest.writes.len();
        }
    }

    /// Take the last step out of the journal.
    ///
    /// A broken journal gives nothing back: the step that broke it
    /// cannot be undone, and the steps before it would restore a state
    /// that never existed.
    pub fn pop(&mut self) -> Option<Undo> {
        if self.broken {
            return None;
        }
        let undo = self.records.pop_back()?;
        self.writes -= undo.writes.len();
        Some(undo)
    }

    /// Break the journal: a step could not be kept whole.
    ///
    /// The records go, because the machine's memory has already moved
    /// past the step that could not be undone.
    pub fn mark_broken(&mut self) {
        self.records.clear();
        self.writes = 0;
        self.broken = true;
    }
}
