//! The machine behind an expression.
//!
//! The evaluator reads the registers and the memory of a machine, and this
//! is the bridge: the debugger's own words, over the core's own state.

use crate::expr::{Context, Value};
use emo8067_core::Cpu;
use emo8067_core::regs::{Reg8, Reg16, Seg};

/// A machine an expression can read.
#[derive(Debug)]
pub struct Machine<'a> {
    cpu: &'a Cpu,
}

impl<'a> Machine<'a> {
    /// The machine the expressions are read from.
    #[must_use]
    pub fn new(cpu: &'a Cpu) -> Machine<'a> {
        Machine { cpu }
    }
}

impl Context for Machine<'_> {
    fn register(&self, name: &str) -> Option<Value> {
        let cpu = self.cpu;
        Some(match name {
            "AX" => u32::from(cpu.regs.word(Reg16::Ax)),
            "BX" => u32::from(cpu.regs.word(Reg16::Bx)),
            "CX" => u32::from(cpu.regs.word(Reg16::Cx)),
            "DX" => u32::from(cpu.regs.word(Reg16::Dx)),
            "SP" => u32::from(cpu.regs.word(Reg16::Sp)),
            "BP" => u32::from(cpu.regs.word(Reg16::Bp)),
            "SI" => u32::from(cpu.regs.word(Reg16::Si)),
            "DI" => u32::from(cpu.regs.word(Reg16::Di)),
            "CS" => u32::from(cpu.regs.segment(Seg::Cs)),
            "DS" => u32::from(cpu.regs.segment(Seg::Ds)),
            "ES" => u32::from(cpu.regs.segment(Seg::Es)),
            "SS" => u32::from(cpu.regs.segment(Seg::Ss)),
            "IP" => u32::from(cpu.regs.ip),
            "AL" => u32::from(cpu.regs.byte(Reg8::Al)),
            "AH" => u32::from(cpu.regs.byte(Reg8::Ah)),
            "BL" => u32::from(cpu.regs.byte(Reg8::Bl)),
            "BH" => u32::from(cpu.regs.byte(Reg8::Bh)),
            "CL" => u32::from(cpu.regs.byte(Reg8::Cl)),
            "CH" => u32::from(cpu.regs.byte(Reg8::Ch)),
            "DL" => u32::from(cpu.regs.byte(Reg8::Dl)),
            "DH" => u32::from(cpu.regs.byte(Reg8::Dh)),
            _ => return None,
        })
    }

    fn word(&self, segment: u16, offset: u16) -> Option<Value> {
        Some(u32::from(self.cpu.mem.read_u16(segment, offset)))
    }
}
