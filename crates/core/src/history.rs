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

    /// Keep a step, and drop the oldest one when the journal is full.
    pub fn push(&mut self, undo: Undo) {
        if self.limit == 0 {
            return;
        }
        while self.records.len() >= self.limit {
            self.records.pop_front();
        }
        self.records.push_back(undo);
    }

    /// Take the last step out of the journal.
    pub fn pop(&mut self) -> Option<Undo> {
        self.records.pop_back()
    }
}
