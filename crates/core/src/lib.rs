//! `emu8086-core` — a clean-room Intel 8086/8088 emulator core.
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
use regs::Registers;

/// The whole processor state.
#[derive(Clone, Debug, Default)]
pub struct Cpu {
    /// Registers, `CS:IP` and FLAGS.
    pub regs: Registers,
    /// Physical memory.
    pub mem: Memory,
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
    Interrupt(u8),
}

impl Cpu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
