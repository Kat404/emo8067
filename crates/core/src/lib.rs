//! `emo8067-core` — a clean-room Intel 8086/8088 emulator core.
//!
//! This crate has no dependencies and no I/O. Everything it knows about
//! the processor comes from the public Intel 8086/8088 architecture
//! documentation; nothing here is derived from decompiled third-party code.
//!
//! # Layout
//!
//! * [`memory`] — the 20-bit physical address space and `segment:offset` wrap.
//! * [`regs`] — the register file.
//! * [`flags`] — FLAGS, including the non-writable bits.
//! * [`modrm`] — ModR/M and effective-address decoding.
//! * [`exec`] — fetch, decode, execute.

#![forbid(unsafe_code)]

pub mod exec;
pub mod flags;
pub mod memory;
pub mod modrm;
pub mod regs;

use memory::Memory;
use regs::{Registers, Seg};

/// The whole processor state.
#[derive(Clone, Debug, Default)]
pub struct Cpu {
    /// Registers, `CS:IP` and FLAGS.
    pub regs: Registers,
    /// Physical memory.
    pub mem: Memory,
    /// The segment the pending override prefix named, for the instruction that
    /// comes next.
    ///
    /// This is decoder state, not a register: the 8086 has no register a
    /// program can read or write that holds it, and it is dead as soon as the
    /// instruction it covers retires. It lives here rather than in
    /// [`Registers`] because every field of that is part of the architectural
    /// register file, and because the prefix byte and the instruction it
    /// modifies are dispatched from different arms — the one sets this, the
    /// other reads it, ten functions away.
    pub seg_override: Option<Seg>,
    /// Set when an instruction asked to stop: `HLT`, a triple fault, or an
    /// interrupt request that is not yet serviceable.
    pub halted: bool,
    /// The reason execution stopped, for the debugger and for tests.
    pub stop_reason: StopReason,
}

/// Why the CPU stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StopReason {
    /// Running, or stopped because a step budget was exhausted.
    #[default]
    None,
    /// `HLT` executed with `IF=1`. Resumes on an interrupt.
    Halted,
    /// An interrupt vector was taken.
    /// An `INT n` was taken. `n` is the vector number.
    Interrupt(u8),
}

impl Cpu {
    /// A fresh processor. This is spelled out rather than derived: the
    /// derived `Default` starts FLAGS at zero, which leaves bit 1 clear, and
    /// bit 1 is a fixed one on every x86 rather than a storage bit.
    #[must_use]
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            mem: Memory::new(),
            seg_override: None,
            halted: false,
            stop_reason: StopReason::None,
        }
    }

    /// Take an `INT n`, the way an 8086 does.
    ///
    /// Pushes FLAGS, then CS, then IP; clears TF and IF; and vectors
    /// through the interrupt table at physical `0000:0000 + 4 * n`. The
    /// return address is whatever `IP` holds, so a fault raised part way
    /// through an instruction returns after that instruction unless the
    /// decoder advanced `IP` past its operands first.
    pub fn interrupt(&mut self, n: u8) {
        use crate::regs::Seg;

        // The 8086 microcode pushes first and reads the vector afterwards:
        // FLAGS, then CS, then IP, then the bus cycles that fetch the new
        // CS:IP from the table at 0000:0000. Reading the vector before the
        // pushes is observably different when SS:SP overlaps physical zero,
        // because the pushes land on the table entry they just read.
        let pushed = self.regs.flags.pushed();
        crate::exec::push(&mut self.mem, &mut self.regs, pushed);
        self.regs.flags.set(crate::flags::TF, false);
        self.regs.flags.set(crate::flags::IF, false);

        let cs = self.regs.segment(Seg::Cs);
        crate::exec::push(&mut self.mem, &mut self.regs, cs);
        let ip = self.regs.ip;
        crate::exec::push(&mut self.mem, &mut self.regs, ip);

        let at = u16::from(n) * 4;
        let offset = self.mem.read_u16(0x0000, at);
        let segment = self.mem.read_u16(0x0000, at + 2);

        self.regs.set_segment(Seg::Cs, segment);
        self.regs.ip = offset;
        self.stop_reason = StopReason::Interrupt(n);
    }

    /// Execute exactly one instruction.
    pub fn step(&mut self) -> Result<StepInfo, exec::ExecError> {
        self.stop_reason = StopReason::None;
        self.halted = false;
        exec::step(self)
    }
}

/// What one [`Cpu::step`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepInfo {
    /// The `CS:IP` the instruction started at.
    pub at: u16,
    /// The opcode byte that dispatched it.
    pub opcode: u8,
    /// The `CS:IP` of the next instruction.
    pub next_ip: u16,
}
