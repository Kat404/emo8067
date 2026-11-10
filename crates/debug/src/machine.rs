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
            "AX" => i32::from(cpu.regs.word(Reg16::Ax)),
            "BX" => i32::from(cpu.regs.word(Reg16::Bx)),
            "CX" => i32::from(cpu.regs.word(Reg16::Cx)),
            "DX" => i32::from(cpu.regs.word(Reg16::Dx)),
            "SP" => i32::from(cpu.regs.word(Reg16::Sp)),
            "BP" => i32::from(cpu.regs.word(Reg16::Bp)),
            "SI" => i32::from(cpu.regs.word(Reg16::Si)),
            "DI" => i32::from(cpu.regs.word(Reg16::Di)),
            "CS" => i32::from(cpu.regs.segment(Seg::Cs)),
            "DS" => i32::from(cpu.regs.segment(Seg::Ds)),
            "ES" => i32::from(cpu.regs.segment(Seg::Es)),
            "SS" => i32::from(cpu.regs.segment(Seg::Ss)),
            "IP" => i32::from(cpu.regs.ip),
            "AL" => i32::from(cpu.regs.byte(Reg8::Al)),
            "AH" => i32::from(cpu.regs.byte(Reg8::Ah)),
            "BL" => i32::from(cpu.regs.byte(Reg8::Bl)),
            "BH" => i32::from(cpu.regs.byte(Reg8::Bh)),
            "CL" => i32::from(cpu.regs.byte(Reg8::Cl)),
            "CH" => i32::from(cpu.regs.byte(Reg8::Ch)),
            "DL" => i32::from(cpu.regs.byte(Reg8::Dl)),
            "DH" => i32::from(cpu.regs.byte(Reg8::Dh)),
            _ => return None,
        })
    }

    fn word(&self, segment: u16, offset: u16) -> Option<Value> {
        Some(i32::from(self.cpu.mem.read_u16(segment, offset)))
    }
}
