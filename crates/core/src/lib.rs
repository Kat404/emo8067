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

pub mod bios;
pub mod devices;
pub mod exec;
pub mod flags;
pub mod history;
pub mod memory;
pub mod modrm;
pub mod regs;
pub mod transport;

use history::{History, MAXIMUM_STEPS_BACK};
use memory::Memory;
use regs::{Registers, Seg};

/// Which repeat prefix a `F2`/`F3` byte set on the REP latch.
///
/// `F3` is `REPE`/`REPZ` and `F2` is `REPNE`/`REPNZ`. Like
/// [`Cpu::seg_override`] this is decoder state rather than a register: the
/// 8086 holds it in a latch, no program can read it, and it is dead as soon as
/// the instruction it covers retires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rep {
    /// `F3`: repeat while the comparison says equal, i.e. while `ZF` is set.
    Rep,
    /// `F2`: repeat while the comparison says unequal, i.e. while `ZF` is clear.
    RepNe,
}

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
    /// The repeat prefix the pending instruction carries, for the instruction
    /// that comes next.
    ///
    /// Same reasoning as [`Cpu::seg_override`], and the same reason it is a
    /// separate field rather than something `seg_override` could encode: the
    /// 8086 sets the two from independent columns of the opcode decode, so
    /// neither order of the two prefix bytes can matter.
    pub rep: Option<Rep>,
    /// Set when an instruction asked to stop: `HLT`, a triple fault, or an
    /// interrupt request that is not yet serviceable.
    pub halted: bool,
    /// The reason execution stopped, for the debugger and for tests.
    pub stop_reason: StopReason,
    /// The I/O bus: the devices a port reaches.
    ///
    /// It is shared, so a device can be watched from outside the emulator
    /// while the emulator runs. An empty bus reads every port as all ones,
    /// which is what a bare chip with nothing plugged into it does.
    pub bus: devices::SharedBus,
    /// The journal of the steps the machine can undo.
    ///
    /// It is off until [`Cpu::enable_history`] turns it on: the headless
    /// runner never rewinds the machine, and it pays nothing for a
    /// journal it does not keep.
    pub history: History,
    /// How deep the machine is in the calls it made.
    ///
    /// A call goes in and a return comes out, and the step over asks how
    /// deep the machine is to know where the call returns. A return
    /// without a call takes the count below zero: the guest's business,
    /// and not a panic.
    pub call_depth: i32,
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
        let mut cpu = Self {
            regs: Registers::new(),
            mem: Memory::new(),
            seg_override: None,
            rep: None,
            halted: false,
            stop_reason: StopReason::None,
            bus: devices::empty_bus(),
            history: History::default(),
            call_depth: 0,
        };
        // The machine starts in the emulator's own default video mode: the
        // BIOS data area holds the geometry from the moment the machine
        // starts.
        bios::init(&mut cpu);
        cpu
    }

    /// Turn the journal on, so that the machine can be rewound.
    ///
    /// The journal keeps [`MAXIMUM_STEPS_BACK`] steps: the debugger's
    /// step back is one instruction, and a long run must not grow the
    /// machine's memory without end.
    pub fn enable_history(&mut self) {
        self.history = History::new(MAXIMUM_STEPS_BACK);
        self.mem.set_logging(true);
    }

    /// Turn the journal off, and let go of the steps it holds.
    pub fn disable_history(&mut self) {
        self.history = History::default();
        self.mem.set_logging(false);
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
        // The interrupt is a call: the `IRET` that comes back out of it
        // takes the depth down again.
        self.call_depth = self.call_depth.saturating_add(1);
    }

    /// Execute exactly one instruction.
    pub fn step(&mut self) -> Result<StepInfo, exec::ExecError> {
        // The journal keeps the state the machine had before the step, so
        // it is taken before the step changes anything — and only when the
        // journal is on: a machine that never rewinds pays nothing for it.
        let undo = self.history.is_enabled().then(|| history::Undo {
            regs: self.regs,
            writes: Vec::new(),
            halted: self.halted,
            stop_reason: self.stop_reason,
            call_depth: self.call_depth,
        });
        // The log holds the writes of this instruction alone.
        self.mem.clear_writes();
        self.stop_reason = StopReason::None;
        self.halted = false;
        let outcome = exec::step(self);
        if let Some(mut undo) = undo {
            if self.mem.is_truncated() {
                // The step wrote more than the journal can hold: the
                // machine's memory has moved past it and cannot be put
                // back, so the journal breaks — and the log keeps
                // nothing, because the step's writes are gone.
                self.history.mark_broken();
                self.mem.take_writes();
            } else {
                undo.writes = self.mem.take_writes();
                self.history.push(undo);
            }
        } else {
            // The journal is off: the log keeps nothing, and the machine
            // pays nothing for it.
            self.mem.clear_writes();
        }
        outcome
    }

    /// Rewind the machine one instruction.
    ///
    /// The journal puts back the registers, the flags, `IP`, the depth of
    /// the calls and the bytes the instruction wrote, so the machine is
    /// where it was before it.
    ///
    /// It is `false` when there is nothing to rewind — the journal is
    /// empty or off — and nothing changes when it is.
    pub fn step_back(&mut self) -> bool {
        let Some(undo) = self.history.pop() else {
            return false;
        };
        self.regs = undo.regs;
        self.halted = undo.halted;
        self.stop_reason = undo.stop_reason;
        self.call_depth = undo.call_depth;
        self.mem.restore(&undo.writes);
        true
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
