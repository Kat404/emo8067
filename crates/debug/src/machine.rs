//! The machine behind an expression.
//!
//! The evaluator reads the registers and the memory of a machine, and this
//! is the bridge: the debugger's own words, over the core's own state.

use crate::expr::{Context, Value};
use emo8067_asm::program::Register;
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

/// Load a program into a machine, the way the reference's own model does.
///
/// The reference's boot template is the record: "load segment: 0000, load
/// offset: 7c00, same values are set to CS and IP registers accordingly".
/// So a program is loaded at its origin in the segment 0000, and the
/// segments follow it.
///
/// **And what the program declares wins over the template.** The
/// reference's `.binf` file gives the load segment and the load offset
/// their own lines, apart from `CS` and `IP`, and each falls back to the
/// other when the other is not given: "if LOAD_SEGMENT and LOAD_OFFSET are
/// not defined, then CS and IP values are used and vice-versa". The
/// register initialisers the program writes - `#cs=`, `#ss=`, `#sp=` and the
/// rest - say the same thing, and a program that declares them starts in
/// that state.
pub fn load(program: &emo8067_asm::program::Program) -> Result<Cpu, String> {
    let declared = |register: Register| {
        program
            .initial
            .iter()
            .rev()
            .find(|(named, _)| *named == register)
            .map(|(_, value)| *value)
    };
    let load_segment = declared(Register::LoadSegment)
        .or_else(|| declared(Register::Cs))
        .unwrap_or(0x0000);
    let load_offset = declared(Register::LoadOffset)
        .or_else(|| declared(Register::Ip))
        .unwrap_or(program.origin);
    let cs = declared(Register::Cs).unwrap_or(load_segment);
    let ip = declared(Register::Ip).unwrap_or(load_offset);
    let ss = declared(Register::Ss).unwrap_or(load_segment);
    let sp = declared(Register::Sp).unwrap_or(0xFFFE);
    let ds = declared(Register::Ds).unwrap_or(load_segment);
    let es = declared(Register::Es).unwrap_or(ds);

    let mut cpu = Cpu::new();
    let physical = u32::from(load_segment) * 16 + u32::from(load_offset);
    cpu.mem
        .load_at(physical, &program.bytes)
        .map_err(|offset| format!("the image does not fit: {offset:#x}"))?;
    cpu.regs.set_segment(Seg::Cs, cs);
    cpu.regs.ip = ip;
    cpu.regs.set_segment(Seg::Ss, ss);
    cpu.regs.set_segment(Seg::Ds, ds);
    cpu.regs.set_segment(Seg::Es, es);
    cpu.regs.set_word(Reg16::Sp, sp);
    // The registers the reference's initialisers name besides the segments:
    // the eight byte registers, and the three index registers. Its `.binf`
    // file lists them all: "EF12 ; SI", "3456 ; DI", "7890 ; BP".
    for (register, value) in &program.initial {
        let byte = *value as u8;
        match register {
            Register::Al => cpu.regs.set_byte(Reg8::Al, byte),
            Register::Ah => cpu.regs.set_byte(Reg8::Ah, byte),
            Register::Bl => cpu.regs.set_byte(Reg8::Bl, byte),
            Register::Bh => cpu.regs.set_byte(Reg8::Bh, byte),
            Register::Cl => cpu.regs.set_byte(Reg8::Cl, byte),
            Register::Ch => cpu.regs.set_byte(Reg8::Ch, byte),
            Register::Dl => cpu.regs.set_byte(Reg8::Dl, byte),
            Register::Dh => cpu.regs.set_byte(Reg8::Dh, byte),
            Register::Si => cpu.regs.set_word(Reg16::Si, *value),
            Register::Di => cpu.regs.set_word(Reg16::Di, *value),
            Register::Bp => cpu.regs.set_word(Reg16::Bp, *value),
            _ => {}
        }
    }
    Ok(cpu)
}
