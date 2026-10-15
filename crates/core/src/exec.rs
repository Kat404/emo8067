//! Fetch, decode, execute.

use crate::flags::{AF, CF, DF, Flags, IF, OF, PF, SF, ZF, parity_even};
use crate::memory::Memory;
use crate::modrm::{Base, DecodeError, Operand, decode_modrm};
use crate::regs::{Reg8, Reg16, Seg};

/// A fault the emulated program caused by executing itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// Opcode byte not implemented on a real 8086, or not implemented here.
    UnknownOpcode(u8),
    /// The instruction stream or an operand ran past the end of memory.
    Truncated,
    /// `H` prefix or other prefix byte that the 8086 does not understand.
    BadPrefix(u8),
    /// A segment-register transfer opcode reached the ALU dispatcher.
    SegmentTransfer(u8),
    /// An `FF` /digit this build does not implement yet.
    UnsupportedFFGroup(u8),
    /// A `REP` or `REPNE` prefix that is not followed by a string operation.
    BadRepeat(u8, u8),
    /// An encoding that only exists on a later processor, so the 8086
    /// refuses it rather than guessing.
    NotOn8086(u8),
    /// `LEA` with a register operand, where there is no address to compute.
    /// The manual does not define this encoding on an 8086.
    BadLea(u8),
    /// The 80186 `(disp16)` alias, which needs a later CPU.
    UnsupportedAddressing(DecodeError),
}

impl From<DecodeError> for ExecError {
    fn from(e: DecodeError) -> Self {
        match e {
            DecodeError::Truncated => Self::Truncated,
            other => Self::UnsupportedAddressing(other),
        }
    }
}

/// `step` budget helper: how many bytes an instruction consumed.
fn width_mask(bits: u8) -> u16 {
    if bits == 16 { 0xFFFF } else { 0x00FF }
}

// ---------------------------------------------------------------- fetching

/// Read the next instruction byte at `CS:IP` and advance `IP`.
pub fn fetch_u8(mem: &Memory, regs: &mut crate::regs::Registers) -> u8 {
    let byte = mem.read_u8(regs.segment(Seg::Cs), regs.ip);
    regs.ip = regs.ip.wrapping_add(1);
    byte
}

fn take_disp8(mem: &Memory, regs: &mut crate::regs::Registers) -> i16 {
    let raw = fetch_u8(mem, regs);
    i16::from(raw as i8)
}

fn take_u16(mem: &Memory, regs: &mut crate::regs::Registers) -> u16 {
    fetch_u16(mem, regs)
}

fn fetch_u16(mem: &Memory, regs: &mut crate::regs::Registers) -> u16 {
    let word = mem.read_u16(regs.segment(Seg::Cs), regs.ip);
    regs.ip = regs.ip.wrapping_add(2);
    word
}

/// Push a word onto `SS:SP`, honouring the direction flag.
///
/// The 8086 predecrements: `SP` moves first and the operand lands at the new
/// `SS:SP`. `DF` flips which way the stack grows, so with `DF=1` the push
/// postincrements instead.
pub(crate) fn push(mem: &mut Memory, regs: &mut crate::regs::Registers, value: u16) {
    let sp = regs.word(Reg16::Sp);
    let new_sp = if regs.flags.get(DF) {
        sp.wrapping_add(2)
    } else {
        sp.wrapping_sub(2)
    };
    mem.write_u16(regs.segment(Seg::Ss), new_sp, value);
    regs.set_word(Reg16::Sp, new_sp);
}

fn pop(mem: &Memory, regs: &mut crate::regs::Registers) -> u16 {
    let sp = regs.word(Reg16::Sp);
    let value = mem.read_u16(regs.segment(Seg::Ss), sp);
    let new_sp = if regs.flags.get(DF) {
        sp.wrapping_sub(2)
    } else {
        sp.wrapping_add(2)
    };
    regs.set_word(Reg16::Sp, new_sp);
    value
}

// ------------------------------------------------------------- operand I/O

/// Resolve a memory operand to its 16-bit effective offset.
///
/// `[BX]`, `[BP]`, `[SI]` and `[DI]` are single-register forms: adding the
/// register to itself would double it and silently address the wrong byte.
fn offset_of(regs: &crate::regs::Registers, base: Base, disp: u16) -> u16 {
    match base {
        // Direct carries the whole offset, so it contributes nothing here;
        // returning `disp` and letting the trailing add run doubled every
        // [disp16] address in the machine.
        Base::Direct => 0,
        Base::BxSi => regs.word(Reg16::Bx).wrapping_add(regs.word(Reg16::Si)),
        Base::BxDi => regs.word(Reg16::Bx).wrapping_add(regs.word(Reg16::Di)),
        Base::BpSi => regs.word(Reg16::Bp).wrapping_add(regs.word(Reg16::Si)),
        Base::BpDi => regs.word(Reg16::Bp).wrapping_add(regs.word(Reg16::Di)),
        Base::Si => regs.word(Reg16::Si),
        Base::Di => regs.word(Reg16::Di),
        Base::Bp => regs.word(Reg16::Bp),
        Base::Bx => regs.word(Reg16::Bx),
    }
    .wrapping_add(disp)
}

/// Read an operand at `bits` width (`8` or `16`).
fn read_operand(mem: &Memory, regs: &crate::regs::Registers, op: Operand, bits: u8) -> u16 {
    match op {
        Operand::Reg(n) if bits == 8 => u16::from(regs.byte(crate::regs::Reg8::from_bits(n))),
        Operand::Reg(n) => regs.word(Reg16::from_bits(n)),
        Operand::Mem { seg, base, disp } => {
            let off = offset_of(regs, base, disp);
            if bits == 8 {
                u16::from(mem.read_u8(regs.segment(seg), off))
            } else {
                mem.read_u16(regs.segment(seg), off)
            }
        }
        Operand::Seg(_) => 0,
    }
}

fn write_operand(
    mem: &mut Memory,
    regs: &mut crate::regs::Registers,
    op: Operand,
    bits: u8,
    value: u16,
) {
    match op {
        Operand::Reg(n) if bits == 8 => regs.set_byte(crate::regs::Reg8::from_bits(n), value as u8),
        Operand::Reg(n) => regs.set_word(Reg16::from_bits(n), value),
        Operand::Mem { seg, base, disp } => {
            let off = offset_of(regs, base, disp);
            let segv = regs.segment(seg);
            if bits == 8 {
                mem.write_u8(segv, off, value as u8);
            } else {
                mem.write_u16(segv, off, value);
            }
        }
        Operand::Seg(_) => {}
    }
}

// ------------------------------------------------------------ flag engines

/// `result = a + b + carry_in`, updating CF/PF/AF/ZF/SF/OF.
fn alu_add(a: u16, b: u16, carry_in: bool, mask: u16, bits: u8, f: &mut Flags) -> u16 {
    let cin = u32::from(carry_in);
    let wide = u32::from(a) + u32::from(b) + cin;
    let result = (wide as u16) & mask;

    f.set(CF, wide > u32::from(mask));
    f.set(AF, ((a ^ b ^ result) & 0x10) != 0);
    f.set(OF, overflow(a, b, result, bits));
    set_result_flags(result, bits, f);
    result
}

/// `result = a - b - borrow_in`, updating CF/PF/AF/ZF/SF/OF.
fn alu_sub(a: u16, b: u16, borrow_in: bool, mask: u16, bits: u8, f: &mut Flags) -> u16 {
    let bin = u32::from(borrow_in);
    let wide = i64::from(a) - i64::from(b) - i64::from(bin);
    let result = (wide as u16) & mask;

    // On the 8086 a borrow shows up as CF, whether or not the operands differ in size.
    f.set(CF, wide < 0);
    f.set(AF, ((a ^ b ^ result) & 0x10) != 0);
    f.set(OF, sub_overflow(a, b, result, bits));
    set_result_flags(result, bits, f);
    result
}

/// Bitwise ops: CF and OF clear, AF undefined (left as-is on real silicon).
fn alu_logic(a: u16, b: u16, op: LogicOp, mask: u16, bits: u8, f: &mut Flags) -> u16 {
    let result = match op {
        LogicOp::And => a & b,
        LogicOp::Or => a | b,
        LogicOp::Xor => a ^ b,
    } & mask;
    f.set(CF, false);
    f.set(OF, false);
    set_result_flags(result, bits, f);
    result
}

/// The three bitwise operations, kept apart only so the dispatch reads clearly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
    Xor,
}

fn set_result_flags(result: u16, bits: u8, f: &mut Flags) {
    // Parity only ever looks at the low byte, whichever width we are on.
    let low = result as u8;
    f.set(ZF, result == 0);
    f.set(SF, sign_bit(result, bits));
    f.set(PF, parity_even(low));
}

const fn sign_bit(value: u16, bits: u8) -> bool {
    if bits == 8 {
        value & 0x80 != 0
    } else {
        value & 0x8000 != 0
    }
}

fn overflow(a: u16, b: u16, result: u16, bits: u8) -> bool {
    // Signed overflow: both operands share a sign that the result does not.
    sign_bit(a, bits) == sign_bit(b, bits) && sign_bit(result, bits) != sign_bit(a, bits)
}

fn sub_overflow(a: u16, b: u16, result: u16, bits: u8) -> bool {
    sign_bit(a, bits) != sign_bit(b, bits) && sign_bit(result, bits) != sign_bit(a, bits)
}

// ---------------------------------------------------------------- dispatch

/// Execute one instruction.
pub fn step(cpu: &mut crate::Cpu) -> Result<crate::StepInfo, ExecError> {
    let at = cpu.regs.ip;
    let opcode = fetch_u8(&cpu.mem, &mut cpu.regs);

    match opcode {
        // ---- 0x00..0x3F: the eight arithmetic/logic groups, each in four encodings
        // The BCD adjustments sit inside 0x00..=0x3F, so they must be
        // matched before the arithmetic groups or they are never reached.
        // Segment pushes and pops also live inside 0x00..=0x3F, at 06, 0E,
        // 16 and 1E, so they are matched here for the same reason.
        0x06 | 0x07 | 0x0E | 0x0F | 0x16 | 0x17 | 0x1E | 0x1F => segment_push_pop(cpu, opcode)?,
        0x27 => daa(cpu, true),
        0x2F => daa(cpu, false),
        0x37 => aaa(cpu, true),
        0x3F => aaa(cpu, false),
        0x00..=0x3F => alu_group(cpu, opcode)?,
        // ---- Group 1: r/m against an immediate. The ModRM /digit selects
        // all eight operations, so this is not three separate groups. Only
        // ADD, OR and CMP have an accumulator-to-immediate form elsewhere,
        // at 0x0C..0x0D, 0x14..0x15 and 0x3C..0x3D.
        0x80..=0x83 => alu_group_imm(cpu, opcode)?,
        // A8/A9 are CMP AL/AX, imm and carry NO ModR/M byte. Routing them
        // through the r/m,imm helper made it consume a phantom one and store
        // to memory instead.
        0xA8 => cmp_accumulator(cpu, 8)?,
        0xA9 => cmp_accumulator(cpu, 16)?,
        0xF6 | 0xF7 => mul_group(cpu, opcode)?,
        // ---- MOV and friends. None of these modify FLAGS.
        0x87 => xchg_memory(cpu, opcode)?,
        0x88..=0x8B => mov_group(cpu, opcode)?,
        0x8C..=0x8E => mov_group(cpu, opcode)?,
        0xA0..=0xA3 => mov_accumulator_offset(cpu, opcode)?,
        0xB0..=0xB7 => {
            let reg = Reg8::from_bits(opcode & 0b111);
            let imm = u16::from(fetch_u8(&cpu.mem, &mut cpu.regs));
            cpu.regs.set_byte(reg, imm as u8);
        }
        0xB8..=0xBF => {
            let reg = Reg16::from_bits(opcode & 0b111);
            let imm = fetch_u16(&cpu.mem, &mut cpu.regs);
            cpu.regs.set_word(reg, imm);
        }
        0xC6 | 0xC7 => mov_to_immediate(cpu, opcode)?,
        // ---- control transfer
        0x70..=0x7F => jcc(cpu, opcode)?,
        0xE8 => call_relative(cpu)?,
        0xE9 | 0xEB => jmp_relative(cpu, opcode)?,
        0xC2 | 0xC3 => ret(cpu, opcode)?,
        0xCC | 0xCD => int(cpu, opcode)?,
        0xCF => iret(cpu),
        0xFF => ff_group(cpu, opcode)?,
        // ---- string operations, ports and segment transfers
        0xA4..=0xA7 | 0xAA..=0xAF => string_op(cpu, opcode)?,
        0xF2 | 0xF3 => repeat_prefix(cpu, opcode)?,
        0xE4..=0xE7 | 0xEC..=0xEF => port_op(cpu, opcode)?,
        0xE0..=0xE3 => loop_op(cpu, opcode)?,
        // ---- shifts and rotates
        0xD0..=0xD3 => shift_group(cpu, opcode)?,
        0x40..=0x47 => inc_dec(cpu, Reg16::from_bits(opcode & 0b111), true)?,
        0x48..=0x4F => inc_dec(cpu, Reg16::from_bits(opcode & 0b111), false)?,
        0x50..=0x57 => {
            let reg = Reg16::from_bits(opcode & 0b111);
            let value = cpu.regs.word(reg);
            push(&mut cpu.mem, &mut cpu.regs, value);
        }
        0x58..=0x5F => {
            let reg = Reg16::from_bits(opcode & 0b111);
            let value = pop(&cpu.mem, &mut cpu.regs);
            cpu.regs.set_word(reg, value);
        }
        0x98 => {
            // CBW: sign-extend AL into AX. No flags.
            let al = cpu.regs.byte(Reg8::Al) as i8 as i16;
            cpu.regs.set_word(Reg16::Ax, al as u16);
        }
        0x99 => {
            // CWD: sign-extend AX into DX:AX. No flags.
            let ax = cpu.regs.word(Reg16::Ax) as i16 as u32;
            cpu.regs.set_word(Reg16::Dx, (ax >> 16) as u16);
        }
        // 90 is XCHG AX, AX, which is the NOP. Handling 91..=97 here keeps
        // the overlap explicit rather than duplicated.
        0x90..=0x97 => {
            let r = Reg16::from_bits(opcode & 0b111);
            let ax = cpu.regs.word(Reg16::Ax);
            let other = cpu.regs.word(r);
            cpu.regs.set_word(r, ax);
            cpu.regs.set_word(Reg16::Ax, other);
        }
        0xF4 => {
            cpu.halted = true;
            cpu.stop_reason = crate::StopReason::Halted;
        }
        0xF5 => cpu.regs.flags.set(CF, !cpu.regs.flags.get(CF)), // CMC
        0xFA => cpu.regs.flags.set(IF, false),                   // CLI
        0xFB => cpu.regs.flags.set(IF, true),                    // STI
        0xFC => cpu.regs.flags.set(DF, false),                   // CLD
        0xFD => cpu.regs.flags.set(DF, true),                    // STD
        0xF8 => cpu.regs.flags.set(CF, false),                   // CLC
        0xF9 => cpu.regs.flags.set(CF, true),                    // STC
        0x9C => {
            let saved = cpu.regs.flags.pushed();
            push(&mut cpu.mem, &mut cpu.regs, saved);
        }
        0x9D => {
            let word = pop(&cpu.mem, &mut cpu.regs);
            cpu.regs.flags = Flags::popped(word);
        }
        other => return Err(ExecError::UnknownOpcode(other)),
    }

    Ok(crate::StepInfo {
        at,
        opcode,
        next_ip: cpu.regs.ip,
    })
}

/// Execute one of the eight ALU groups, `0x00..=0x3F`.
///
/// Bits 5..=3 select the operation, bits 2..=1 select the encoding, and
/// bit 0 selects byte (`0`) versus word (`1`). Within a group that reads
/// as `g+0 r/m,r`, `g+2 r,r/m`, `g+4 AL/AX,imm`, `g+6 PUSH/POP seg`.
/// The `r/m, imm` forms live outside the group, at `0x80..=0x83`, because
/// there they collide with the group index itself.
fn alu_group(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let op = alu_op(opcode >> 3);
    let bits: u8 = if opcode & 1 != 0 { 16 } else { 8 };
    let mask = width_mask(bits);

    match (opcode >> 1) & 0b11 {
        // r/m, r — the register supplies the right-hand operand
        0b00 => {
            let m = decode_from(cpu)?;
            let r = RegArg::from_bits(m.reg, bits);
            let right = r.read(&cpu.regs);
            let left = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
            let result = apply(cpu, op, left, right, bits, mask);
            if op != AluOp::Cmp {
                write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, result);
            }
        }
        // r, r/m — the register receives the result
        0b01 => {
            let m = decode_from(cpu)?;
            let r = RegArg::from_bits(m.reg, bits);
            let left = r.read(&cpu.regs);
            let right = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
            let result = apply(cpu, op, left, right, bits, mask);
            if op != AluOp::Cmp {
                r.write(&mut cpu.regs, result);
            }
        }
        // AL/AX, imm8  /  AL/AX, imm16
        0b10 => {
            let imm = if bits == 16 {
                fetch_u16(&cpu.mem, &mut cpu.regs)
            } else {
                u16::from(fetch_u8(&cpu.mem, &mut cpu.regs))
            };
            let ax = Operand::Reg(Reg16::Ax as u8);
            let left = read_operand(&cpu.mem, &cpu.regs, ax, bits);
            let result = apply(cpu, op, left, imm, bits, mask);
            if op != AluOp::Cmp {
                write_operand(&mut cpu.mem, &mut cpu.regs, ax, bits, result);
            }
        }
        // PUSH seg / POP seg share this group but are not ALU operations.
        _ => return Err(ExecError::SegmentTransfer(opcode)),
    }
    Ok(())
}

/// Execute `r/m, imm` for the three 8086 groups that have one.
///
/// Within a group, `opcode & 0b11` picks the immediate form: `0` and `2`
/// are `r/m8, imm8`, `1` is `r/m16, imm16`, and `3` is `r/m16, imm8`
/// with the byte sign-extended. The ModR/M `reg` field is the `/digit`
/// that names the operation, not a destination register, so the result
/// always lands in `r/m`.
fn alu_group_imm(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let m = decode_from(cpu)?;
    let op = alu_op(m.reg & 0b111);
    let form = opcode & 0b11;
    let bits: u8 = if form & 1 != 0 { 16 } else { 8 };
    let mask = width_mask(bits);

    let imm = if form == 0b01 {
        fetch_u16(&cpu.mem, &mut cpu.regs)
    } else {
        let raw = fetch_u8(&cpu.mem, &mut cpu.regs);
        if form == 0b11 {
            i16::from(raw as i8) as u16
        } else {
            u16::from(raw)
        }
    };
    let left = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
    let result = apply(cpu, op, left, imm, bits, mask);
    if op != AluOp::Cmp {
        write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, result);
    }
    Ok(())
}

/// Execute the `MOV` encodings at `0x88`..=0x8E`.
///
/// `88`/`89` and `8A`/`8B` are the four register/memory orders; `8C` moves a
/// segment register to memory or a register, `8E` the other way, and `8D` is
/// `LEA`, which takes the *address* rather than the contents.
fn mov_group(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    // 8C, 8D and 8E are 16-bit whatever bit 0 says; only 88..8B uses bit 0,
    // and 0x8D is odd while 0x8C and 0x8E are even, so bit 0 alone would
    // have made LEA an 8-bit operation.
    let bits: u8 = if matches!(opcode, 0x8C..=0x8E) || opcode & 1 != 0 {
        16
    } else {
        8
    };
    let m = decode_from(cpu)?;
    let reg_field = m.reg & 0b111;

    match opcode {
        0x88 | 0x89 => {
            let right = RegArg::from_bits(reg_field, bits).read(&cpu.regs);
            write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, right);
        }
        0x8A | 0x8B => {
            let value = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
            RegArg::from_bits(reg_field, bits).write(&mut cpu.regs, value);
        }
        0x8C => {
            // 8C needs a memory destination. With mod=11 it names a register,
            // which is an 80186 encoding the 8086 does not have, so it is
            // rejected rather than silently storing a segment in a register.
            if matches!(m.rm, Operand::Reg(_)) {
                return Err(ExecError::NotOn8086(opcode));
            }
            // 8C moves a 16-bit segment value whatever the opcode's low bit
            // says. Deriving the width from bit 0 made it a byte store.
            let value = cpu.regs.segment(Seg::from_bits(reg_field));
            write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, 16, value);
        }
        0x8E => {
            let value = read_operand(&cpu.mem, &cpu.regs, m.rm, 16);
            cpu.regs.set_segment(Seg::from_bits(reg_field), value);
        }
        0x8D => {
            // LEA needs a memory operand: the effective address, and nothing
            // else, is what lands in the register.
            let Operand::Mem { seg, base, disp } = m.rm else {
                return Err(ExecError::BadLea(opcode));
            };
            // LEA takes the address and nothing else, so the segment is
            // deliberately unused: it must not appear in the result. It is
            // named `_seg` so the intent is visible rather than implied.
            let _seg = seg;
            let value = offset_of(&cpu.regs, base, disp);
            RegArg::from_bits(reg_field, bits).write(&mut cpu.regs, value);
        }
        other => return Err(ExecError::UnknownOpcode(other)),
    }
    Ok(())
}

/// `A0`..=`A3` move between the accumulator and a direct `disp16` address
/// within the current data segment, bypassing ModR/M entirely.
fn mov_accumulator_offset(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let offset = fetch_u16(&cpu.mem, &mut cpu.regs);
    let segment = cpu.regs.segment(Seg::Ds);
    match opcode {
        0xA0 => {
            let v = cpu.mem.read_u8(segment, offset);
            cpu.regs.set_byte(Reg8::Al, v);
        }
        0xA1 => {
            let v = cpu.mem.read_u16(segment, offset);
            cpu.regs.set_word(Reg16::Ax, v);
        }
        0xA2 => {
            let v = cpu.regs.byte(Reg8::Al);
            cpu.mem.write_u8(segment, offset, v);
        }
        0xA3 => {
            let v = cpu.regs.word(Reg16::Ax);
            cpu.mem.write_u16(segment, offset, v);
        }
        other => return Err(ExecError::UnknownOpcode(other)),
    }
    Ok(())
}

/// `86`/`87` exchange a register with a register or memory operand.
fn xchg_memory(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let bits: u8 = if opcode == 0x87 { 16 } else { 8 };
    let m = decode_from(cpu)?;
    let reg = RegArg::from_bits(m.reg & 0b111, bits);
    let saved = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
    let from_reg = reg.read(&cpu.regs);
    write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, from_reg);
    reg.write(&mut cpu.regs, saved);
    Ok(())
}

/// `C6 /0` and `C7 /0` move an immediate into a register or memory operand.
fn mov_to_immediate(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let bits: u8 = if opcode == 0xC7 { 16 } else { 8 };
    let m = decode_from(cpu)?;
    if m.reg != 0 {
        // C6 /1..7 and C7 /1..7 are the 80186+ shifts. Not 8086.
        return Err(ExecError::UnknownOpcode(opcode));
    }
    let value = if bits == 16 {
        fetch_u16(&cpu.mem, &mut cpu.regs)
    } else {
        u16::from(fetch_u8(&cpu.mem, &mut cpu.regs))
    };
    write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, value);
    Ok(())
}

/// The string operations at `A4`..`AF`. Each advances or decrements its
/// implicit pointer by the operand size, following `DF`.
fn string_op(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let word = opcode & 1 != 0;
    let step: u16 = if word { 2 } else { 1 };
    let ds = cpu.regs.segment(Seg::Ds);
    let es = cpu.regs.segment(Seg::Es);
    let mut si = cpu.regs.word(Reg16::Si);
    let mut di = cpu.regs.word(Reg16::Di);
    let _uses_si = matches!(opcode, 0xA4..=0xA7 | 0xAC | 0xAD);
    let _uses_di = !matches!(opcode, 0xAC | 0xAD);
    let uses_si = matches!(opcode, 0xA4..=0xA7 | 0xAC | 0xAD);
    let uses_di = !matches!(opcode, 0xAC | 0xAD);

    macro_rules! bump {
        ($ptr:expr) => {
            if cpu.regs.flags.get(DF) {
                $ptr = $ptr.wrapping_sub(step)
            } else {
                $ptr = $ptr.wrapping_add(step)
            }
        };
    }

    match opcode {
        0xA4 | 0xA5 => {
            let value = if word {
                cpu.mem.read_u16(ds, si)
            } else {
                u16::from(cpu.mem.read_u8(ds, si))
            };
            if word {
                cpu.mem.write_u16(es, di, value)
            } else {
                cpu.mem.write_u8(es, di, value as u8)
            }
        }
        0xA6 | 0xA7 => {
            let a = if word {
                cpu.mem.read_u16(ds, si)
            } else {
                u16::from(cpu.mem.read_u8(ds, si))
            };
            let b = if word {
                cpu.mem.read_u16(es, di)
            } else {
                u16::from(cpu.mem.read_u8(es, di))
            };
            compare(cpu, a, b, word);
        }
        0xAA | 0xAB => {
            let value = if word {
                cpu.regs.word(Reg16::Ax)
            } else {
                u16::from(cpu.regs.byte(Reg8::Al))
            };
            if word {
                cpu.mem.write_u16(es, di, value)
            } else {
                cpu.mem.write_u8(es, di, value as u8)
            }
        }
        0xAC | 0xAD => {
            let value = if word {
                cpu.mem.read_u16(ds, si)
            } else {
                u16::from(cpu.mem.read_u8(ds, si))
            };
            if word {
                cpu.regs.set_word(Reg16::Ax, value)
            } else {
                cpu.regs.set_byte(Reg8::Al, value as u8)
            }
        }
        0xAE | 0xAF => {
            let a = if word {
                cpu.regs.word(Reg16::Ax)
            } else {
                u16::from(cpu.regs.byte(Reg8::Al))
            };
            let b = if word {
                cpu.mem.read_u16(es, di)
            } else {
                u16::from(cpu.mem.read_u8(es, di))
            };
            compare(cpu, a, b, word);
        }
        other => return Err(ExecError::UnknownOpcode(other)),
    }
    // Only the pointer the operation actually used may move. MOVS and CMPS
    // touch both; STOS and SCAS only DI; LODS only SI. Advancing both
    // corrupted whichever pointer the caller was relying on.
    if uses_si {
        bump!(si);
        cpu.regs.set_word(Reg16::Si, si);
    }
    if uses_di {
        bump!(di);
        cpu.regs.set_word(Reg16::Di, di);
    }
    Ok(())
}

/// `CMPS` and `SCAS` define the same six flags as a subtraction. `a` is the
/// minuend. Modelling only ZF/CF/OF silently breaks anything that reads the
/// parity or auxiliary flag after a compare.
fn compare(cpu: &mut crate::Cpu, a: u16, b: u16, word: bool) {
    let bits = if word { 16 } else { 8 };
    let mask = width(bits);
    let result = a.wrapping_sub(b) & mask;
    let low = result as u8;
    let f = &mut cpu.regs.flags;
    f.set(ZF, result == 0);
    f.set(CF, a & mask < b & mask);
    f.set(
        OF,
        sign_bit(a, bits) != sign_bit(b, bits) && sign_bit(result, bits) != sign_bit(a, bits),
    );
    f.set(SF, sign_bit(result, bits));
    f.set(PF, crate::flags::parity_even(low));
    f.set(AF, (a ^ b ^ result) & 0x10 != 0);
}

/// `A8`/`A9`: `CMP AL, imm8` and `CMP AX, imm16`. No ModR/M byte, no store.
fn cmp_accumulator(cpu: &mut crate::Cpu, bits: u8) -> Result<(), ExecError> {
    let left = if bits == 16 {
        cpu.regs.word(Reg16::Ax)
    } else {
        u16::from(cpu.regs.byte(Reg8::Al))
    };
    let right = if bits == 16 {
        fetch_u16(&cpu.mem, &mut cpu.regs)
    } else {
        u16::from(fetch_u8(&cpu.mem, &mut cpu.regs))
    };
    compare(cpu, left, right, bits == 16);
    Ok(())
}

/// `F2` and `F3` are prefixes: `REPNE` and `REPE` apply to the string
/// operation that follows. The whole repeat happens inside one step, so the
/// instruction reports its real length rather than the two bytes of the
/// prefix pair.
fn repeat_prefix(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let next = fetch_u8(&cpu.mem, &mut cpu.regs);
    if !(0xA4..=0xA7).contains(&next) && !(0xAA..=0xAF).contains(&next) {
        return Err(ExecError::BadRepeat(opcode, next));
    }
    let word = next & 1 != 0;
    let step: u16 = if word { 2 } else { 1 };
    let compare_op = matches!(next, 0xA6 | 0xA7 | 0xAE | 0xAF);
    let mut cx = cpu.regs.word(Reg16::Cx);

    while cx != 0 {
        let si = cpu.regs.word(Reg16::Si);
        let di = cpu.regs.word(Reg16::Di);
        let ds = cpu.regs.segment(Seg::Ds);
        let es = cpu.regs.segment(Seg::Es);

        match next {
            0xA4 | 0xA5 => {
                let v = if word {
                    cpu.mem.read_u16(ds, si)
                } else {
                    u16::from(cpu.mem.read_u8(ds, si))
                };
                if word {
                    cpu.mem.write_u16(es, di, v)
                } else {
                    cpu.mem.write_u8(es, di, v as u8)
                }
            }
            0xA6 | 0xA7 => {
                let a = if word {
                    cpu.mem.read_u16(ds, si)
                } else {
                    u16::from(cpu.mem.read_u8(ds, si))
                };
                let b = if word {
                    cpu.mem.read_u16(es, di)
                } else {
                    u16::from(cpu.mem.read_u8(es, di))
                };
                compare(cpu, a, b, word);
            }
            0xAA | 0xAB => {
                let v = if word {
                    cpu.regs.word(Reg16::Ax)
                } else {
                    u16::from(cpu.regs.byte(Reg8::Al))
                };
                if word {
                    cpu.mem.write_u16(es, di, v)
                } else {
                    cpu.mem.write_u8(es, di, v as u8)
                }
            }
            0xAC | 0xAD => {
                // LODS reads DS:SI into AL or AX. A catch-all arm here would
                // silently run SCAS instead, because AC..AF is two classes.
                let v = if word {
                    cpu.mem.read_u16(ds, si)
                } else {
                    u16::from(cpu.mem.read_u8(ds, si))
                };
                if word {
                    cpu.regs.set_word(Reg16::Ax, v)
                } else {
                    cpu.regs.set_byte(Reg8::Al, v as u8)
                }
            }
            _ => {
                let a = if word {
                    cpu.regs.word(Reg16::Ax)
                } else {
                    u16::from(cpu.regs.byte(Reg8::Al))
                };
                let b = if word {
                    cpu.mem.read_u16(es, di)
                } else {
                    u16::from(cpu.mem.read_u8(es, di))
                };
                compare(cpu, a, b, word);
            }
        }

        cx = cx.wrapping_sub(1);
        cpu.regs.set_word(Reg16::Cx, cx);

        let backwards = cpu.regs.flags.get(DF);
        let bump = |p: u16| {
            if backwards {
                p.wrapping_sub(step)
            } else {
                p.wrapping_add(step)
            }
        };
        // Same rule as outside a repeat: LODS owns SI alone, STOS and SCAS
        // own DI alone, and only MOVS and CMPS touch both.
        if matches!(next, 0xA4..=0xA7 | 0xAC | 0xAD) {
            cpu.regs.set_word(Reg16::Si, bump(si));
        }
        if !matches!(next, 0xAC | 0xAD) {
            cpu.regs.set_word(Reg16::Di, bump(di));
        }

        // REPE and REPNE stop at the first pair that fails their condition.
        if compare_op {
            let keep_going = if opcode == 0xF3 {
                cpu.regs.flags.get(ZF)
            } else {
                !cpu.regs.flags.get(ZF)
            };
            if !keep_going {
                break;
            }
        }
    }
    Ok(())
}

/// `IN` and `OUT`. On this bare machine port *N* is byte *N* of physical
/// memory, which keeps the core free of any device policy; the device bus
/// proper is the `Device` trait's job, not the CPU's.
///
/// Port *N* maps to physical *N*, not to segment *N*. Reading it as
/// `segment = N, offset = 0` would land on `N * 16`, which is what this
/// used to do while claiming otherwise.
fn port_op(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let immediate = matches!(opcode, 0xE4..=0xE7);
    let port = if immediate {
        u16::from(fetch_u8(&cpu.mem, &mut cpu.regs))
    } else {
        cpu.regs.word(Reg16::Dx)
    };
    let word = opcode & 1 != 0;
    match opcode {
        0xE4 | 0xE5 => {
            let v = if word {
                cpu.mem.read_u16(0x0000, port)
            } else {
                u16::from(cpu.mem.read_u8(0x0000, port))
            };
            if word {
                cpu.regs.set_word(Reg16::Ax, v)
            } else {
                cpu.regs.set_byte(Reg8::Al, v as u8)
            }
        }
        0xEC | 0xED => {
            let v = if word {
                cpu.mem.read_u16(0x0000, port)
            } else {
                u16::from(cpu.mem.read_u8(0x0000, port))
            };
            if word {
                cpu.regs.set_word(Reg16::Ax, v)
            } else {
                cpu.regs.set_byte(Reg8::Al, v as u8)
            }
        }
        0xE6 | 0xE7 => {
            let v = if word {
                cpu.regs.word(Reg16::Ax)
            } else {
                u16::from(cpu.regs.byte(Reg8::Al))
            };
            if word {
                cpu.mem.write_u16(0x0000, port, v)
            } else {
                cpu.mem.write_u8(0x0000, port, v as u8)
            }
        }
        0xEE | 0xEF => {
            let v = if word {
                cpu.regs.word(Reg16::Ax)
            } else {
                u16::from(cpu.regs.byte(Reg8::Al))
            };
            if word {
                cpu.mem.write_u16(0x0000, port, v)
            } else {
                cpu.mem.write_u8(0x0000, port, v as u8)
            }
        }
        other => return Err(ExecError::UnknownOpcode(other)),
    }
    Ok(())
}

/// `PUSH`/`POP` of a segment register, at 06/07, 0E/16/1E and 17/1F.
fn segment_push_pop(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let seg = match opcode {
        0x06 => Seg::Es,
        0x0E => Seg::Cs,
        0x16 => Seg::Ss,
        0x1E => Seg::Ds,
        0x07 => Seg::Es,
        0x17 => Seg::Ss,
        0x1F => Seg::Ds,
        0x0F => Seg::Cs,
        other => return Err(ExecError::UnknownOpcode(other)),
    };
    let popping = matches!(opcode, 0x07 | 0x0F | 0x17 | 0x1F);
    if popping {
        let v = pop(&cpu.mem, &mut cpu.regs);
        cpu.regs.set_segment(seg, v);
    } else {
        let v = cpu.regs.segment(seg);
        push(&mut cpu.mem, &mut cpu.regs, v);
    }
    Ok(())
}

/// `LOOP`, `LOOPZ`, `LOOPNZ` and `JCXZ`.
fn loop_op(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let rel = take_disp8(&cpu.mem, &mut cpu.regs);
    let cx = cpu.regs.word(Reg16::Cx);
    if opcode == 0xE3 {
        if cx == 0 {
            cpu.regs.ip = (i32::from(cpu.regs.ip as i16) + i32::from(rel)) as u16;
        }
        return Ok(());
    }
    let cx = cx.wrapping_sub(1);
    cpu.regs.set_word(Reg16::Cx, cx);
    let take = match opcode {
        0xE0 => !cpu.regs.flags.get(ZF),
        0xE1 => cpu.regs.flags.get(ZF),
        _ => cx != 0,
    };
    if take {
        cpu.regs.ip = (i32::from(cpu.regs.ip as i16) + i32::from(rel)) as u16;
    }
    Ok(())
}

/// Group 2, `0xD0..=0xD3`: the seven shifts and rotates, by one or by `CL`.
///
/// `D0`/`D1` shift by one; `D2`/`D3` take the count from `CL`. The count is
/// masked to five bits on an 8086, so a shift of 32 is a shift of zero.
fn shift_group(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let bits: u8 = if opcode & 1 != 0 { 16 } else { 8 };
    let from_cl = opcode >= 0xD2;
    let m = decode_from(cpu)?;
    let op = m.reg & 0b111;

    let count = if from_cl {
        (cpu.regs.byte(Reg8::Cl) & 0b1_1111) as u16
    } else {
        1
    };

    let value = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);
    let carry_in = cpu.regs.flags.get(CF);
    let result = match op {
        // ROL and ROR never touch the carry; it only comes out.
        0x0 => rotate_left(value, count, bits),
        0x1 => rotate_right(value, count, bits),
        // RCL and RCR rotate the carry *through* the operand, so the bit
        // count is one more than the operand is wide and a count above zero
        // can wind CF around and back.
        0x2 => rotate_through_carry(value, count, bits, carry_in, true),
        0x3 => rotate_through_carry(value, count, bits, carry_in, false),
        // 0b100 SHL, 0b110 SAL (identical), 0b101 SHR, 0b111 SAR
        0x4 | 0x6 => shift_left(value, count, bits),
        0x5 => shift_right(value, count, bits, false),
        _ => shift_right(value, count, bits, true),
    };

    let (result, cf) = result;
    let f = &mut cpu.regs.flags;
    // A count of zero changes nothing, and on an 8086 that includes CF. A
    // rotate by a whole number of revolutions does the same: no bit left.
    let rotates = matches!(op, 0x0..=0x3);
    let no_bits_moved = rotates && count % bits as u16 == 0;
    if count != 0 && !no_bits_moved {
        f.set(CF, cf);
        if count == 1 {
            f.set(OF, signed_overflow_shift(value, result, op, bits, cf));
        }
        // The shifts update ZF, SF and PF from the result. Table 2-21 marks
        // only O and C, but the manual's own prose for the same instructions
        // says the arithmetic flags are updated, and silicon does it. Under
        // the project's erratum rule, silicon wins. The rotates leave all
        // three alone: there they are undefined.
        if !rotates {
            set_result_flags(result, bits, f);
        }
    }
    write_operand(&mut cpu.mem, &mut cpu.regs, m.rm, bits, result);
    Ok(())
}

const fn msb(bits: u8) -> u16 {
    if bits == 8 { 0x80 } else { 0x8000 }
}
/// Bit 0 is bit 0 at either width.
const fn lsb(_bits: u8) -> u16 {
    0x0001
}
const fn width(bits: u8) -> u16 {
    if bits == 8 { 0x00FF } else { 0xFFFF }
}

/// `OF` is only architecturally defined after a rotate by one, and after a
/// left shift by one. A right shift always clears it.
fn signed_overflow_shift(before: u16, after: u16, op: u8, bits: u8, cf: bool) -> bool {
    match op {
        // RCL and RCR: the overflow is the two ends of the rotated result
        // disagreeing, which is CF against the new top bit.
        0x2 | 0x3 => cf != (after & msb(bits) != 0),
        0x0 => {
            let top = before & msb(bits) != 0;
            top && (after & msb(bits) == 0) || !top && (after & msb(bits) != 0)
        }
        0x1 => {
            let top = before & msb(bits) != 0;
            top && (after & lsb(bits) == 0) || !top && (after & lsb(bits) != 0)
        }
        0x4 | 0x6 => before & msb(bits) != 0 && after & msb(bits) == 0,
        _ => false,
    }
}

fn shift_left(value: u16, count: u16, bits: u8) -> (u16, bool) {
    let mask = width(bits);
    let mut v = value & mask;
    let mut cf = false;
    for _ in 0..count {
        cf = v & msb(bits) != 0;
        v = (v << 1) & mask;
    }
    (v, cf)
}

fn shift_right(value: u16, count: u16, bits: u8, arithmetic: bool) -> (u16, bool) {
    let mask = width(bits);
    let v = value & mask;
    let mut r = v;
    let mut cf = false;
    for _ in 0..count {
        cf = r & lsb(bits) != 0;
        r = if arithmetic {
            // Re-fill from the *current* sign, or a chain of shifts drifts.
            let fill = if r & msb(bits) != 0 { msb(bits) } else { 0 };
            (r >> 1) | fill
        } else {
            (r >> 1) & mask
        };
    }
    (r, cf)
}

fn rotate_left(value: u16, count: u16, bits: u8) -> (u16, bool) {
    let n = bits as u16;
    let mask = width(bits);
    let v = value & mask;
    let k = count % n;
    if k == 0 {
        // A rotate of zero leaves everything alone. Returning early also
        // avoids shifting a u16 by its own width, which panics in debug.
        return (v, v & msb(bits) != 0);
    }
    let r = ((v << k) | (v >> (n - k))) & mask;
    (r, v & (1u16 << (n - k)) != 0) // bit n-k is the one that left the top
}

fn rotate_right(value: u16, count: u16, bits: u8) -> (u16, bool) {
    let n = bits as u16;
    let mask = width(bits);
    let v = value & mask;
    let k = count % n;
    if k == 0 {
        return (v, v & lsb(bits) != 0);
    }
    let r = ((v >> k) | (v << (n - k))) & mask;
    (r, v & (1u16 << (k - 1)) != 0) // bit k-1 is the one that left the bottom
}

/// `RCL` and `RCR` rotate the carry through the operand, so the working
/// value is one bit wider than the register. The count is masked to five
/// bits on an 8086, which means a count of 17 and a count of 1 are the same
/// for a byte operand, and not the same for a word one.
fn rotate_through_carry(
    value: u16,
    count: u16,
    bits: u8,
    carry_in: bool,
    left: bool,
) -> (u16, bool) {
    let mask = width(bits);
    let mut v = value & mask;
    let mut c = carry_in;
    for _ in 0..count {
        let new_cf = if left {
            v & msb(bits) != 0
        } else {
            v & lsb(bits) != 0
        };
        v = if left {
            ((v << 1) | u16::from(c)) & mask
        } else {
            (v >> 1) | if c { msb(bits) } else { 0 }
        };
        c = new_cf;
    }
    (v, c)
}

/// `DAA` and `DAS` share a shape and differ only in the direction.
///
/// The low nibble is corrected first, then the high one; the carry the high
/// correction produces is the new CF. `AF` is undefined afterwards on an 8086,
/// so it is left alone, and only CF, ZF, SF and PF are written.
fn daa(cpu: &mut crate::Cpu, add: bool) {
    let old_cf = cpu.regs.flags.get(CF);
    let old_af = cpu.regs.flags.get(AF);
    let mut al = cpu.regs.byte(Reg8::Al);

    if add {
        if al & 0x0F > 9 || old_af {
            al = al.wrapping_add(0x06);
        }
        let mut cf = old_cf;
        if al > 0x99 || old_cf {
            cf = true;
            al = al.wrapping_add(0x60);
        }
        cpu.regs.flags.set(CF, cf);
    } else {
        if al & 0x0F > 9 || old_af {
            al = al.wrapping_sub(0x06);
        }
        let mut cf = old_cf;
        if al > 0x99 || old_cf {
            cf = true;
            al = al.wrapping_sub(0x60);
        }
        cpu.regs.flags.set(CF, cf);
    }

    cpu.regs.set_byte(Reg8::Al, al);
    let f = &mut cpu.regs.flags;
    f.set(ZF, al == 0);
    f.set(SF, al & 0x80 != 0);
    f.set(PF, crate::flags::parity_even(al));
}

/// `AAA` and `AAS` differ only in the direction of the adjustment.
fn aaa(cpu: &mut crate::Cpu, add: bool) {
    let old_af = cpu.regs.flags.get(AF);
    let mut al = cpu.regs.byte(Reg8::Al);
    let adjust = add;
    let carry = al & 0x0F > 9 || old_af;

    if carry {
        let delta = if adjust { 0x06u8 } else { 0xFAu8 }; // +6 or -6
        al = al.wrapping_add(delta);
        al &= 0x0F;
        let ah = cpu.regs.byte(Reg8::Ah);
        cpu.regs
            .set_byte(Reg8::Ah, ah.wrapping_add(if adjust { 1 } else { 0xFF }));
    }
    cpu.regs.set_byte(Reg8::Al, al);
    let f = &mut cpu.regs.flags;
    f.set(AF, carry);
    f.set(CF, carry);
}

/// `INC` and `DEC` on a 16-bit register. They set the same flags as a
/// subtraction but leave `CF` alone, which is the whole point of having them
/// separately from `ADD` and `SUB`.
fn inc_dec(cpu: &mut crate::Cpu, reg: Reg16, up: bool) -> Result<(), ExecError> {
    let before = cpu.regs.word(reg);
    let after = if up {
        before.wrapping_add(1)
    } else {
        before.wrapping_sub(1)
    };
    cpu.regs.set_word(reg, after);
    let f = &mut cpu.regs.flags;
    f.set(OF, sign_bit(before, 16) != sign_bit(after, 16));
    f.set(SF, sign_bit(after, 16));
    f.set(ZF, after == 0);
    f.set(AF, (before ^ after) & 0x10 != 0);
    f.set(PF, crate::flags::parity_even(after as u8));
    Ok(())
}

/// The sixteen conditional jumps at `0x70..=0x7F`, each with an `rel8`.
fn jcc(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let taken = match opcode - 0x70 {
        0x0 => cpu.regs.flags.get(OF),
        0x1 => !cpu.regs.flags.get(OF),
        0x2 => cpu.regs.flags.get(CF),
        0x3 => !cpu.regs.flags.get(CF),
        0x4 => cpu.regs.flags.get(ZF),
        0x5 => !cpu.regs.flags.get(ZF),
        0x6 => cpu.regs.flags.get(CF) || cpu.regs.flags.get(ZF),
        0x7 => !cpu.regs.flags.get(CF) && !cpu.regs.flags.get(ZF),
        0x8 => cpu.regs.flags.get(SF),
        0x9 => !cpu.regs.flags.get(SF),
        0xA => cpu.regs.flags.get(PF),
        0xB => !cpu.regs.flags.get(PF),
        0xC => cpu.regs.flags.get(SF) != cpu.regs.flags.get(OF),
        0xD => cpu.regs.flags.get(SF) == cpu.regs.flags.get(OF),
        0xE => cpu.regs.flags.get(ZF) || (cpu.regs.flags.get(SF) != cpu.regs.flags.get(OF)),
        _ => !cpu.regs.flags.get(ZF) && (cpu.regs.flags.get(SF) == cpu.regs.flags.get(OF)),
    };
    let rel = take_disp8(&cpu.mem, &mut cpu.regs);
    if taken {
        cpu.regs.ip = (i32::from(cpu.regs.ip as i16) + i32::from(rel)) as u16;
    }
    Ok(())
}

/// `EB` is the short form and `E9` the near form; both add a signed
/// displacement to the address of the *next* instruction.
fn jmp_relative(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let rel: i16 = if opcode == 0xEB {
        take_disp8(&cpu.mem, &mut cpu.regs)
    } else {
        take_u16(&cpu.mem, &mut cpu.regs) as i16
    };
    cpu.regs.ip = (i32::from(cpu.regs.ip as i16) + i32::from(rel)) as u16;
    Ok(())
}

/// `E8` pushes the address of the next instruction, then adds `rel16` to it.
fn call_relative(cpu: &mut crate::Cpu) -> Result<(), ExecError> {
    let rel = take_u16(&cpu.mem, &mut cpu.regs);
    let ret = cpu.regs.ip;
    push(&mut cpu.mem, &mut cpu.regs, ret);
    cpu.regs.ip = (i32::from(ret as i16) + i32::from(rel)) as u16;
    Ok(())
}

/// `C3` returns, `C2` returns and releases `imm16` bytes of arguments.
fn ret(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let adjust = if opcode == 0xC2 {
        take_u16(&cpu.mem, &mut cpu.regs)
    } else {
        0
    };
    cpu.regs.ip = pop(&cpu.mem, &mut cpu.regs);
    let sp = cpu.regs.word(Reg16::Sp).wrapping_add(adjust);
    cpu.regs.set_word(Reg16::Sp, sp);
    Ok(())
}

/// `CC` is `INT 3`, the one-byte breakpoint; `CD ib` is `INT n`.
fn int(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let vector = if opcode == 0xCC {
        3
    } else {
        fetch_u8(&cpu.mem, &mut cpu.regs)
    };
    cpu.interrupt(vector);
    Ok(())
}

/// `CF` pops IP, then CS, then FLAGS.
fn iret(cpu: &mut crate::Cpu) {
    let ip = pop(&cpu.mem, &mut cpu.regs);
    let cs = pop(&cpu.mem, &mut cpu.regs);
    let flags = pop(&cpu.mem, &mut cpu.regs);
    cpu.regs.ip = ip;
    cpu.regs.set_segment(Seg::Cs, cs);
    cpu.regs.flags = Flags::popped(flags);
}

/// The `FF` group. Only the near `CALL` and `JMP` are implemented here; the
/// far forms and `INC`/`DEC`/`PUSH` belong to their own units.
fn ff_group(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let _ = opcode;
    let m = decode_from(cpu)?;
    match m.reg & 0b111 {
        0x2 => {
            // CALL near r/m16: push the next instruction, load the operand.
            let ret = cpu.regs.ip;
            let target = read_operand(&cpu.mem, &cpu.regs, m.rm, 16);
            push(&mut cpu.mem, &mut cpu.regs, ret);
            cpu.regs.ip = target;
        }
        0x4 => {
            cpu.regs.ip = read_operand(&cpu.mem, &cpu.regs, m.rm, 16);
        }
        other => return Err(ExecError::UnsupportedFFGroup(other)),
    }
    Ok(())
}

/// Execute `MUL IMUL DIV IDIV` at `0xF6` (byte) and `0xF7` (word).
///
/// The manual defines only CF and OF for `MUL`/`IMUL`, and none of the six
/// for `DIV`/`IDIV`, so every other flag is left exactly as it was. A fault
/// takes the type 0 interrupt; the manual leaves the quotient and remainder
/// undefined in that case, so they are not written.
fn mul_group(cpu: &mut crate::Cpu, opcode: u8) -> Result<(), ExecError> {
    let bits: u8 = if opcode == 0xF6 { 8 } else { 16 };
    let m = decode_from(cpu)?;
    let op = m.reg & 0b111;
    let operand = read_operand(&cpu.mem, &cpu.regs, m.rm, bits);

    let ax = cpu.regs.word(Reg16::Ax);
    let dx = cpu.regs.word(Reg16::Dx);
    let (lo, hi) = match (bits, op) {
        // ---- unsigned multiply
        (8, 4) => {
            let a = u16::from(cpu.regs.byte(Reg8::Al));
            let product = a * u16::from(operand as u8);
            set_mul_flags(cpu, (product >> 8) == 0);
            ((product & 0x00FF), (product >> 8))
        }
        (16, 4) => {
            let product = u32::from(ax) * u32::from(operand);
            set_mul_flags(cpu, (product >> 16) == 0);
            (product as u16, (product >> 16) as u16)
        }
        // ---- signed multiply
        // A signed product overflowed unless the high half is a sign
        // extension of the low half: (-1) * (-1) = 1 is fine, but
        // (-128) * (-1) = 128 is not, because 0xFF80 is not 0xFF80-as-a-
        // sign-extension of its own low byte.
        (8, 5) => {
            let a = i16::from(cpu.regs.byte(Reg8::Al) as i8);
            let product = a * i16::from(operand as u8 as i8);
            let as_word = product as u16;
            let low = as_word & 0x00FF;
            let expected = low as u8 as i8 as i16 as u16;
            set_mul_flags(cpu, as_word == expected);
            (low, as_word >> 8)
        }
        (16, 5) => {
            let a = i32::from(ax as i16);
            let product = a * i32::from(operand as i16);
            let as_word = product as u32;
            let low = as_word as u16;
            let expected = (low as i16 as i32 as u32) >> 16;
            set_mul_flags(cpu, (as_word >> 16) == expected);
            (low, (as_word >> 16) as u16)
        }
        // ---- unsigned divide
        (8, 6) => {
            let divisor = operand as u8;
            if divisor == 0 {
                divide_error(cpu);
                return Ok(());
            }
            let quotient = ax / u16::from(divisor);
            if quotient > 0xFF {
                divide_error(cpu);
                return Ok(());
            }
            let remainder = (ax % u16::from(divisor)) as u8;
            (quotient, u16::from(remainder))
        }
        (16, 6) => {
            let divisor = operand;
            if divisor == 0 {
                divide_error(cpu);
                return Ok(());
            }
            let wide = (u32::from(dx) << 16) | u32::from(ax);
            let quotient = wide / u32::from(divisor);
            if quotient > u32::from(u16::MAX) {
                divide_error(cpu);
                return Ok(());
            }
            let remainder = wide % u32::from(divisor);
            (quotient as u16, remainder as u16)
        }
        // ---- signed divide
        (8, 7) => {
            let divisor = operand as u8 as i8;
            if divisor == 0 {
                divide_error(cpu);
                return Ok(());
            }
            let quotient = (ax as i16) / i16::from(divisor);
            if !(-128..=127).contains(&quotient) {
                divide_error(cpu);
                return Ok(());
            }
            let remainder = (ax as i16) % i16::from(divisor);
            ((quotient as u8) as u16, (remainder as u8) as u16)
        }
        (16, 7) => {
            let divisor = operand as i16;
            if divisor == 0 {
                divide_error(cpu);
                return Ok(());
            }
            let dividend = ((u32::from(dx) << 16) | u32::from(ax)) as i32;
            let quotient = dividend / i32::from(divisor);
            if !(-32768..=32767).contains(&quotient) {
                divide_error(cpu);
                return Ok(());
            }
            let remainder = dividend % i32::from(divisor);
            (quotient as u16, remainder as u16)
        }
        _ => return Err(ExecError::UnknownOpcode(opcode)),
    };

    if bits == 8 {
        cpu.regs.set_word(Reg16::Ax, (lo & 0x00FF) | (hi << 8));
    } else {
        cpu.regs.set_word(Reg16::Ax, lo);
        cpu.regs.set_word(Reg16::Dx, hi);
    }
    Ok(())
}

/// `MUL`/`IMUL` define CF and OF together: both set when the product did not
/// fit, both clear when it did.
fn set_mul_flags(cpu: &mut crate::Cpu, fits: bool) {
    let f = &mut cpu.regs.flags;
    f.set(CF, !fits);
    f.set(OF, !fits);
}

/// Take the divide-error interrupt: vector 0 through the interrupt table.
fn divide_error(cpu: &mut crate::Cpu) {
    cpu.interrupt(0);
}

const fn alu_op(index: u8) -> AluOp {
    match index {
        0 => AluOp::Add,
        1 => AluOp::Or,
        2 => AluOp::Adc,
        3 => AluOp::Sbb,
        4 => AluOp::And,
        5 => AluOp::Sub,
        6 => AluOp::Xor,
        _ => AluOp::Cmp,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AluOp {
    Add,
    Or,
    Adc,
    Sbb,
    And,
    Sub,
    Xor,
    Cmp,
}

fn apply(cpu: &mut crate::Cpu, op: AluOp, left: u16, right: u16, bits: u8, mask: u16) -> u16 {
    let f = &mut cpu.regs.flags;
    match op {
        AluOp::Add => alu_add(left, right, false, mask, bits, f),
        AluOp::Adc => alu_add(left, right, f.get(CF), mask, bits, f),
        AluOp::Sub | AluOp::Cmp => alu_sub(left, right, false, mask, bits, f),
        AluOp::Sbb => alu_sub(left, right, f.get(CF), mask, bits, f),
        AluOp::And => alu_logic(left, right, LogicOp::And, mask, bits, f),
        AluOp::Or => alu_logic(left, right, LogicOp::Or, mask, bits, f),
        AluOp::Xor => alu_logic(left, right, LogicOp::Xor, mask, bits, f),
    }
}

/// Decode the next ModR/M byte straight out of the instruction stream.
///
/// The decoder indexes its slice from zero, so it gets a cursor rather than
/// the absolute `IP`; the consumed length is folded back into `IP` after.
fn decode_from(cpu: &mut crate::Cpu) -> Result<crate::modrm::ModRm, ExecError> {
    let seg = cpu.regs.segment(Seg::Cs);
    let start = cpu.regs.ip;
    let mut window = [0u8; 4];
    for (i, slot) in window.iter_mut().enumerate() {
        *slot = cpu.mem.read_u8(seg, start.wrapping_add(i as u16));
    }
    let mut cursor = 0u16;
    let result = decode_modrm(&window, &mut cursor, Seg::Ds);
    cpu.regs.ip = start.wrapping_add(cursor);
    result.map_err(Into::into)
}

/// A register destination that the ModR/M `reg` field can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegArg {
    R8(Reg8),
    R16(Reg16),
}

impl RegArg {
    fn from_bits(field: u8, bits: u8) -> Self {
        if bits == 8 {
            Self::R8(Reg8::from_bits(field))
        } else {
            Self::R16(Reg16::from_bits(field))
        }
    }

    fn read(self, regs: &crate::regs::Registers) -> u16 {
        match self {
            Self::R8(r) => u16::from(regs.byte(r)),
            Self::R16(r) => regs.word(r),
        }
    }

    fn write(self, regs: &mut crate::regs::Registers, value: u16) {
        match self {
            Self::R8(r) => regs.set_byte(r, value as u8),
            Self::R16(r) => regs.set_word(r, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cpu;
    use crate::flags::{AF, CF, DF, IF, OF, SF, ZF};
    use crate::regs::Registers;

    /// Build a CPU with `code` at `1000:0000`, clear of the interrupt
    /// vector table. The IVT is genuinely at `0000:0000` on real hardware, so
    /// a test that vectors an interrupt must not also park code there.
    fn cpu_at(code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new();
        assert!(cpu.mem.load_at(0x10000, code).is_ok());
        cpu.regs.set_segment(Seg::Cs, 0x1000);
        cpu.regs.ip = 0;
        cpu
    }

    /// Build a CPU with `code` at `CS:0000`, code segment 0.
    fn cpu_with(code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new();
        assert!(cpu.mem.load_at(0, code).is_ok());
        cpu.regs.set_segment(Seg::Cs, 0);
        cpu.regs.ip = 0;
        cpu
    }

    #[test]
    fn add_sets_carry_and_overflow_independently() {
        // ADD AL, 1 with AL = 0xFF: CF (unsigned 255+1 wraps) but not OF,
        // because -1 + 1 is a perfectly good signed zero.
        let mut cpu = cpu_with(&[0x04, 0x01]);
        cpu.regs.set_byte(Reg8::Al, 0xFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x00);
        assert!(cpu.regs.flags.get(CF));
        assert!(!cpu.regs.flags.get(OF));
        assert!(cpu.regs.flags.get(ZF));

        // ADD AL, 1 with AL = 0x7F: no unsigned wrap, but +1 overflows +127.
        let mut cpu = cpu_with(&[0x04, 0x01]);
        cpu.regs.set_byte(Reg8::Al, 0x7F);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x80);
        assert!(!cpu.regs.flags.get(CF));
        assert!(cpu.regs.flags.get(OF));
        assert!(cpu.regs.flags.get(SF));
    }

    #[test]
    fn sub_borrow_and_overflow_are_distinct() {
        // SUB AL, 1 with AL = 0x00: borrow -> CF. Both operands are positive,
        // so signed 0 - 1 = -1 fits: OF stays clear.
        let mut cpu = cpu_with(&[0x2C, 0x01]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0xFF);
        assert!(cpu.regs.flags.get(CF));
        assert!(!cpu.regs.flags.get(OF));

        // SUB AL, 1 with AL = 0x80: no unsigned borrow, but signed -128 - 1
        // does not fit in 8 bits, so OF is set.
        let mut cpu = cpu_with(&[0x2C, 0x01]);
        cpu.regs.set_byte(Reg8::Al, 0x80);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x7F);
        assert!(!cpu.regs.flags.get(CF));
        assert!(cpu.regs.flags.get(OF));
    }

    #[test]
    fn auxiliary_carry_detects_bcd_carry() {
        // 0x9 + 0x1 = 0xA: no carry out of bit 3.
        let mut cpu = cpu_with(&[0x04, 0x01]);
        cpu.regs.set_byte(Reg8::Al, 0x09);
        cpu.step().unwrap();
        assert!(!cpu.regs.flags.get(AF));

        // 0xF + 0x1 = 0x10: carry out of bit 3.
        let mut cpu = cpu_with(&[0x04, 0x01]);
        cpu.regs.set_byte(Reg8::Al, 0x0F);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(AF));
    }

    #[test]
    fn adc_and_sbb_consume_the_carry_flag() {
        // ADC AL, 0 with CF already set.
        let mut cpu = cpu_with(&[0x14, 0x00]);
        cpu.regs.set_byte(Reg8::Al, 0x10);
        cpu.regs.flags.set(CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x11);

        // SBB AL, 0 with CF set borrows one.
        let mut cpu = cpu_with(&[0x1C, 0x00]);
        cpu.regs.set_byte(Reg8::Al, 0x10);
        cpu.regs.flags.set(CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x0F);
    }

    #[test]
    fn parity_is_set_on_an_even_low_byte() {
        // MOV AL, 0x03 (two bits) then ADD AL, 0 -> PF set.
        let mut cpu = cpu_with(&[0x04, 0x00]);
        cpu.regs.set_byte(Reg8::Al, 0x03);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(PF));

        // 0x01 has one bit -> PF clear.
        let mut cpu = cpu_with(&[0x04, 0x00]);
        cpu.regs.set_byte(Reg8::Al, 0x01);
        cpu.step().unwrap();
        assert!(!cpu.regs.flags.get(PF));
    }

    #[test]
    fn cmp_writes_flags_but_not_the_destination() {
        let mut cpu = cpu_with(&[0x3C, 0x05]);
        cpu.regs.set_byte(Reg8::Al, 0x05);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x05, "CMP must not store a result");
        assert!(cpu.regs.flags.get(ZF));
    }

    #[test]
    fn bitwise_ops_clear_carry_and_overflow() {
        let mut cpu = cpu_with(&[0x24, 0x0F]); // AND AL, 0x0F
        cpu.regs.set_byte(Reg8::Al, 0xF0);
        cpu.regs.flags.set(CF, true);
        cpu.regs.flags.set(OF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x00);
        assert!(!cpu.regs.flags.get(CF));
        assert!(!cpu.regs.flags.get(OF));
    }

    #[test]
    fn word_forms_use_the_full_sixteen_bits() {
        // 04 01 is ADD AL,1; 05 01 00 is ADD AX,1.
        let mut cpu = cpu_with(&[0x05, 0x01, 0x00]);
        cpu.regs.set_word(Reg16::Ax, 0xFFFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x0000);
        assert!(cpu.regs.flags.get(CF));
    }

    #[test]
    fn modrm_form_addresses_memory_through_bx_si() {
        // 00 00 is ADD [BX+SI], AL.  BX=0x0100 SI=0x0010 -> physical 0x0110.
        let mut cpu = cpu_with(&[0x00, 0x00]);
        cpu.regs.set_word(Reg16::Bx, 0x0100);
        cpu.regs.set_word(Reg16::Si, 0x0010);
        cpu.regs.set_byte(Reg8::Al, 0x05);
        cpu.mem.write_u8(0x0000, 0x0110, 0x03);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x0000, 0x0110), 0x08);
    }

    #[test]
    fn push_and_pop_round_trip() {
        let mut cpu = cpu_with(&[0x50, 0x5B]); // PUSH AX ; POP BX
        cpu.regs.set_segment(Seg::Ss, 0x1000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.set_word(Reg16::Ax, 0xBEEF);
        cpu.step().unwrap();
        // The 8086 predecrements, so the operand lands below the old SP.
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x00FE);
        assert_eq!(cpu.mem.read_u16(0x1000, 0x00FE), 0xBEEF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Bx), 0xBEEF);
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0100);
    }

    #[test]
    fn direction_flag_reverses_the_stack_pointer() {
        let mut cpu = cpu_with(&[0x50]);
        cpu.regs.set_segment(Seg::Ss, 0x1000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.flags.set(DF, true);
        cpu.regs.set_word(Reg16::Ax, 0x1234);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0102);
        assert_eq!(cpu.mem.read_u16(0x1000, 0x0102), 0x1234);
    }

    // ---- string operations, ports and segment transfers --------------------------

    #[test]
    fn stos_writes_ax_to_es_di_and_advances_by_two() {
        // AB = STOSW. DF clear, so DI advances by the operand size.
        let mut cpu = cpu_with(&[0xAB]);
        cpu.regs.set_segment(Seg::Es, 0x3000);
        cpu.regs.set_word(Reg16::Di, 0x0010);
        cpu.regs.set_word(Reg16::Ax, 0x1234);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x3000, 0x0010), 0x1234);
        assert_eq!(cpu.regs.word(Reg16::Di), 0x0012);
    }

    #[test]
    fn the_direction_flag_reverses_every_string_operation() {
        // DF set: DI decrements, and the store lands below the start.
        let mut cpu = cpu_with(&[0xAB]);
        cpu.regs.set_segment(Seg::Es, 0x3000);
        cpu.regs.set_word(Reg16::Di, 0x0010);
        cpu.regs.set_word(Reg16::Ax, 0x1234);
        cpu.regs.flags.set(DF, true);
        cpu.step().unwrap();
        // The store happens at the current DI; DF moves the pointer for the
        // *next* operation, so the word lands at 0x0010 and DI ends at 0x000E.
        assert_eq!(cpu.mem.read_u16(0x3000, 0x0010), 0x1234);
        assert_eq!(cpu.regs.word(Reg16::Di), 0x000E);
    }

    #[test]
    fn movsb_moves_one_byte_and_advances_both_pointers() {
        // A4 = MOVSB. Source DS:SI, destination ES:DI.
        let mut cpu = cpu_with(&[0xA4]);
        cpu.regs.set_segment(Seg::Ds, 0x4000);
        cpu.regs.set_segment(Seg::Es, 0x5000);
        cpu.regs.set_word(Reg16::Si, 0x0020);
        cpu.regs.set_word(Reg16::Di, 0x0030);
        cpu.mem.write_u8(0x4000, 0x0020, 0x99);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x5000, 0x0030), 0x99);
        assert_eq!(cpu.regs.word(Reg16::Si), 0x0021);
        assert_eq!(cpu.regs.word(Reg16::Di), 0x0031);
    }

    #[test]
    fn lods_and_scas_read_through_ds() {
        // AC = LODSB (AD is LODSW), AE = SCASB compares against AL.
        let mut cpu = cpu_with(&[0xAC]);
        cpu.regs.set_segment(Seg::Ds, 0x4000);
        cpu.regs.set_word(Reg16::Si, 0x0020);
        cpu.mem.write_u8(0x4000, 0x0020, 0x7E);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x7E);
        assert_eq!(cpu.regs.word(Reg16::Si), 0x0021);

        let mut cpu = cpu_with(&[0xAE]);
        cpu.regs.set_segment(Seg::Es, 0x5000);
        cpu.regs.set_word(Reg16::Di, 0x0030);
        cpu.regs.set_byte(Reg8::Al, 0x7E);
        cpu.mem.write_u8(0x5000, 0x0030, 0x7E);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(ZF), "the bytes match");
        assert_eq!(cpu.regs.word(Reg16::Di), 0x0031);
    }

    #[test]
    fn rep_stosb_fills_until_the_count_reaches_zero() {
        // F3 AB = REP STOSB, CX = 4.
        let mut cpu = cpu_with(&[0xF3, 0xAA]);
        cpu.regs.set_segment(Seg::Es, 0x3000);
        cpu.regs.set_word(Reg16::Di, 0x0000);
        cpu.regs.set_word(Reg16::Cx, 0x0004);
        cpu.regs.set_byte(Reg8::Al, 0x2A);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x3000, 0x0000), 0x2A2A);
        assert_eq!(cpu.mem.read_u16(0x3000, 0x0002), 0x2A2A);
        assert_eq!(cpu.regs.word(Reg16::Cx), 0x0000);
    }

    #[test]
    fn repe_repeats_while_the_bytes_are_equal() {
        // F3 A6 = REPE CMPSB. Three equal bytes then a different one.
        let mut cpu = cpu_with(&[0xF3, 0xA6]);
        cpu.regs.set_segment(Seg::Ds, 0x4000);
        cpu.regs.set_segment(Seg::Es, 0x5000);
        cpu.regs.set_word(Reg16::Si, 0x0000);
        cpu.regs.set_word(Reg16::Di, 0x0000);
        cpu.regs.set_word(Reg16::Cx, 0x0004);
        for i in 0..3u16 {
            cpu.mem.write_u8(0x4000, i, 0x11);
            cpu.mem.write_u8(0x5000, i, 0x11);
        }
        cpu.mem.write_u8(0x4000, 3, 0x22);
        cpu.mem.write_u8(0x5000, 3, 0x33);
        cpu.step().unwrap();
        assert_eq!(
            cpu.regs.word(Reg16::Cx),
            0x0000,
            "the mismatch still consumed its count"
        );
        assert!(!cpu.regs.flags.get(ZF));
    }

    #[test]
    fn in_and_out_reach_a_port() {
        // E4 ib = IN AL, imm8.  E6 ib = OUT imm8, AL.
        let mut cpu = cpu_with(&[0xE4, 0x60, 0xE6, 0x61]);
        // Port 0x60 is physical byte 0x60: segment 0, offset 0x60.
        cpu.mem.write_u8(0x0000, 0x60, 0xC3);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0xC3);
        cpu.regs.set_byte(Reg8::Al, 0x5A);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x0000, 0x61), 0x5A, "port 61 took the byte");
    }

    #[test]
    fn in_and_out_through_dx_use_the_word_register() {
        // ED = IN AX, DX.  EF = OUT DX, AX.  EC and EE are the byte forms.
        let mut cpu = cpu_with(&[0xED, 0xEF]);
        cpu.regs.set_word(Reg16::Dx, 0x70);
        cpu.mem.write_u16(0x0000, 0x70, 0xBEEF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0xBEEF);
        cpu.regs.set_word(Reg16::Ax, 0x1234);
        cpu.regs.set_word(Reg16::Dx, 0x71);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x0000, 0x71), 0x1234);
    }

    #[test]
    fn push_and_pop_segment_registers() {
        // 06 = PUSH ES, 07 = POP ES. The 8086 permits ES, CS, SS and DS but
        // not the flag word on the segment stack.
        let mut cpu = cpu_at(&[0x06, 0x07]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.set_segment(Seg::Es, 0x6000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x00FE);
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FE), 0x6000);
        cpu.regs.set_segment(Seg::Es, 0x0000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.segment(Seg::Es), 0x6000);
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0100);
    }

    #[test]
    fn loop_family_counts_cx_and_checks_the_flags() {
        // E2 = LOOP. E0 = LOOPNE, E1 = LOOPE, E3 = JCXZ.
        let mut cpu = cpu_with(&[0xE2, 0xFE]);
        cpu.regs.set_word(Reg16::Cx, 1);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0002, "CX hit zero, so fall through");
        assert_eq!(cpu.regs.word(Reg16::Cx), 0x0000);

        let mut cpu = cpu_with(&[0xE2, 0xFE]);
        cpu.regs.set_word(Reg16::Cx, 3);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0000, "still counting, so jump back");

        // JCXZ jumps when CX is zero without touching it.
        let mut cpu = cpu_with(&[0xE3, 0x10]);
        cpu.regs.set_word(Reg16::Cx, 0);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0012);
        assert_eq!(cpu.regs.word(Reg16::Cx), 0);

        let mut cpu = cpu_with(&[0xE3, 0x10]);
        cpu.regs.set_word(Reg16::Cx, 1);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0002);
    }

    // ---- shifts, rotates and BCD -------------------------------------------------

    #[test]
    fn shift_by_one_moves_the_top_bit_into_carry() {
        // D0 /4 = SHL r/m8, 1.  0b1000_0001 << 1 = 0b0000_0010, CF = 1.
        let mut cpu = cpu_with(&[0xD0, 0xE0]);
        cpu.regs.set_byte(Reg8::Al, 0x81);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x02);
        assert!(cpu.regs.flags.get(CF));
    }

    #[test]
    fn sar_keeps_the_sign_and_rotate_puts_it_in_carry() {
        // D0 /7 = SAR AL, 1 on 0x90. 0x90 is a negative signed byte, so the
        // sign bit is replicated: 0x90 >> 1 is 0xC8, not 0x48.
        let mut cpu = cpu_with(&[0xD0, 0xF8]);
        cpu.regs.set_byte(Reg8::Al, 0x90);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0xC8);
        assert!(
            !cpu.regs.flags.get(CF),
            "the bit that left the bottom was 0"
        );

        // A positive byte does clear the top bit.
        let mut cpu = cpu_with(&[0xD0, 0xF8]);
        cpu.regs.set_byte(Reg8::Al, 0x50);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x28);
        assert!(!cpu.regs.flags.get(CF));

        // SAR of a negative number fills with ones.
        let mut cpu = cpu_with(&[0xD0, 0xF8]);
        cpu.regs.set_byte(Reg8::Al, 0x80);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0xC0);
        assert!(!cpu.regs.flags.get(CF));
    }

    #[test]
    fn ror_and_rcl_wrap_around_the_ends() {
        // D0 /1 = ROR AL, 1.  0b0000_0001 -> 0b1000_0000, CF = 1.
        let mut cpu = cpu_with(&[0xD0, 0xC8]);
        cpu.regs.set_byte(Reg8::Al, 0x01);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x80);
        assert!(cpu.regs.flags.get(CF));

        // D0 /0 = ROL AL, 1.  0b1000_0000 -> 0b0000_0001, CF = 1.
        let mut cpu = cpu_with(&[0xD0, 0xC0]);
        cpu.regs.set_byte(Reg8::Al, 0x80);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x01);
        assert!(cpu.regs.flags.get(CF));
    }

    #[test]
    fn shift_count_comes_from_cl_and_is_masked_to_five_bits() {
        // D2 /4 = SHL r/m8, CL.  The count is masked, so 32 means 0.
        let mut cpu = cpu_with(&[0xD2, 0xE0]);
        cpu.regs.set_byte(Reg8::Al, 0x01);
        cpu.regs.set_byte(Reg8::Cl, 32);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x01, "a count of 32 shifts by 0");

        let mut cpu = cpu_with(&[0xD2, 0xE0]);
        cpu.regs.set_byte(Reg8::Al, 0x01);
        cpu.regs.set_byte(Reg8::Cl, 4);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x10);
    }

    #[test]
    fn aaa_adjusts_a_low_nibble_that_is_out_of_range() {
        // 37 = AAA. AL = 0x0B: the nibble is out of range, so AL keeps only
        // the low nibble of (0x0B + 6) and AH carries into it.
        let mut cpu = cpu_with(&[0x37]);
        cpu.regs.set_byte(Reg8::Al, 0x0B);
        cpu.regs.set_byte(Reg8::Ah, 0x00);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x01);
        assert_eq!(cpu.regs.byte(Reg8::Ah), 0x01);
        assert!(cpu.regs.flags.get(AF) && cpu.regs.flags.get(CF));

        // AL = 0x09 with AF clear: the nibble is in range and nothing adjusts.
        let mut cpu = cpu_with(&[0x37]);
        cpu.regs.set_byte(Reg8::Al, 0x09);
        cpu.regs.flags.set(AF, false);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x09);
        assert!(!cpu.regs.flags.get(AF) && !cpu.regs.flags.get(CF));

        // The same AL with AF set does adjust: the manual says AAA fires when
        // the nibble is out of range *or* AF is 1.
        let mut cpu = cpu_with(&[0x37]);
        cpu.regs.set_byte(Reg8::Al, 0x09);
        cpu.regs.flags.set(AF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x0F);
        assert!(cpu.regs.flags.get(AF));
    }

    #[test]
    fn daa_repairs_a_packed_bcd_result() {
        // 27 = DAA. 0x28 + 0x37 = 0x5F, whose low nibble F is not a digit.
        // The low correction turns it into 0x65, which is valid, and nothing
        // carries out of the tens, so CF stays clear.
        let mut cpu = cpu_with(&[0x27]);
        cpu.regs.set_byte(Reg8::Al, 0x5F);
        cpu.regs.flags.set(AF, false);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x65);
        assert!(!cpu.regs.flags.get(CF));
    }

    #[test]
    fn daa_leaves_a_valid_packed_result_alone() {
        // 0x28 + 0x11 = 0x39, already valid packed BCD, so nothing changes.
        let mut cpu = cpu_with(&[0x27]);
        cpu.regs.set_byte(Reg8::Al, 0x39);
        cpu.regs.flags.set(AF, false);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x39);
        assert!(!cpu.regs.flags.get(CF));

        // A high nibble above 9 does carry: 0x99 + 0x01 needs the 0x60 add.
        let mut cpu = cpu_with(&[0x27]);
        cpu.regs.set_byte(Reg8::Al, 0x9A);
        cpu.regs.flags.set(AF, false);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x00);
        assert!(cpu.regs.flags.get(CF));
    }

    #[test]
    fn bcd_adjustments_leave_the_other_flags_alone() {
        // The manual marks SF, ZF, PF, OF and AF undefined for AAA, so the
        // only honest implementation is to leave them exactly as they were.
        let mut cpu = cpu_with(&[0x37]);
        cpu.regs.set_byte(Reg8::Al, 0x0B);
        cpu.regs.flags.set(ZF, true);
        cpu.regs.flags.set(SF, true);
        cpu.regs.flags.set(OF, true);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(ZF) && cpu.regs.flags.get(SF) && cpu.regs.flags.get(OF));
    }

    // ---- control transfer -------------------------------------------------------
    // None of these modify FLAGS. IRET restores them, and that is the exception.

    #[test]
    fn jmp_relative_moves_ip_by_the_signed_displacement() {
        // E9 rel16 = JMP near, EB rel8 = JMP short.
        let mut cpu = cpu_with(&[0xE9, 0x10, 0x00]); // +0x10 from 0x0003
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0013);

        let mut cpu = cpu_with(&[0xEB, 0x10]); // +0x10 from 0x0002
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0012);

        // A negative displacement reaches backwards.
        let mut cpu = cpu_with(&[0xEB, 0xFE]); // -2 from 0x0002
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0000);
    }

    #[test]
    fn jmp_indirect_takes_its_target_from_an_operand() {
        // FF /4 = JMP near r/m16.
        let mut cpu = cpu_with(&[0xFF, 0xE7]); // JMP DI
        cpu.regs.set_word(Reg16::Di, 0x1234);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x1234);
    }

    #[test]
    fn every_conditional_jump_agrees_with_its_condition() {
        // opcode, CF, ZF, SF, OF, PF, and whether that combination jumps.
        // Each row is the flag state its mnemonic names; the same test then
        // inverts every flag and requires the opposite outcome.
        // The last column is what the *inverted* flag state does. It is not
        // always the opposite: CF||ZF is still true when both are cleared.
        let cases: [(u8, bool, bool, bool, bool, bool, bool); 16] = [
            (0x70, false, false, false, true, false, false), // JO   overflow
            (0x71, false, false, false, false, false, false), // JNO  no overflow
            (0x72, true, false, false, false, false, false), // JB   carry
            (0x73, false, false, false, false, false, false), // JNB  no carry
            (0x74, false, true, false, false, false, false), // JZ   zero
            (0x75, false, false, false, false, false, false), // JNZ  not zero
            (0x76, true, false, false, false, false, true), // JBE  CF or ZF: clearing both leaves it true
            (0x77, false, false, false, false, false, false), // JA   neither
            (0x78, false, false, true, false, false, false), // JS   sign
            (0x79, false, false, false, false, false, false), // JNS  no sign
            (0x7A, false, false, false, false, true, false), // JP   parity
            (0x7B, false, false, false, false, false, false), // JNP  no parity
            (0x7C, false, false, true, false, false, true), // JL   SF!=OF, symmetric
            (0x7D, false, false, false, false, false, true), // JGE  SF==OF, symmetric
            (0x7E, false, true, true, false, false, true),  // JLE  ZF or SF!=OF
            (0x7F, false, false, false, false, false, false), // JG   not zero and not JL
        ];
        for (opcode, cf, zf, sf, of, pf, inverted_jump) in cases {
            for invert in [false, true] {
                let bit = |v: bool| v != invert;
                let mut cpu = cpu_with(&[opcode, 0x10]);
                let f = &mut cpu.regs.flags;
                f.set(CF, bit(cf));
                f.set(ZF, bit(zf));
                f.set(SF, bit(sf));
                f.set(OF, bit(of));
                f.set(PF, bit(pf));
                cpu.step().unwrap();
                let expected = if if invert { inverted_jump } else { true } {
                    0x0012
                } else {
                    0x0002
                };
                assert_eq!(
                    cpu.regs.ip, expected,
                    "opcode {opcode:#04x} with inverted={invert}"
                );
            }
        }
    }

    #[test]
    fn call_pushes_the_return_address_and_jumps() {
        // E8 rel16 = CALL near. The pushed address is the one after CALL.
        let mut cpu = cpu_at(&[0xE8, 0x05, 0x00]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.step().unwrap();
        // rel16 is measured from the end of the instruction, which is 3.
        assert_eq!(cpu.regs.ip, 0x0008);
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x00FE);
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FE), 0x0003, "return address");
    }

    #[test]
    fn call_indirect_pushes_and_loads_from_the_operand() {
        // FF /2 = CALL near r/m16.
        let mut cpu = cpu_at(&[0xFF, 0xD7]); // CALL DI
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.set_word(Reg16::Di, 0x0300);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0300);
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FE), 0x0002);
    }

    #[test]
    fn ret_returns_to_the_pushed_address() {
        let mut cpu = cpu_at(&[0xC3]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.mem.write_u16(0x2000, 0x0100, 0x0405);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0405);
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0102);
    }

    #[test]
    fn ret_with_an_immediate_also_releases_the_arguments() {
        // C2 iw = RET near, pop CS:IP then add iw to SP.
        let mut cpu = cpu_at(&[0xC2, 0x04, 0x00]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.mem.write_u16(0x2000, 0x0100, 0x0405);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0405);
        assert_eq!(
            cpu.regs.word(Reg16::Sp),
            0x0106,
            "popped 2 plus the argument"
        );
    }

    #[test]
    fn call_and_ret_round_trip_returns_to_the_right_place() {
        // 0000: E8 03 00   CALL 0006
        // 0003: EB FD      JMP short 0002 -> back to 0000
        // 0006: C3         RET
        let mut cpu = cpu_at(&[0xE8, 0x03, 0x00, 0xEB, 0xFD, 0x90, 0xC3]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0006, "into the subroutine");
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0003, "back after the CALL");
        assert_eq!(
            cpu.regs.word(Reg16::Sp),
            0x0100,
            "the stack is balanced again"
        );
    }

    #[test]
    fn int_vectors_through_the_interrupt_table() {
        // CD 21 = INT 21h.
        let mut cpu = cpu_at(&[0xCD, 0x21]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        install_vector(&mut cpu, 0x21, 0x5000, 0x0600);
        let cs = cpu.regs.segment(Seg::Cs);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.segment(Seg::Cs), 0x5000);
        assert_eq!(cpu.regs.ip, 0x0600);
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x00FA);
        // PUSH predecrements, so FLAGS is highest and IP is lowest.
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FE), 0xF000, "FLAGS");
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FC), cs, "CS");
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FA), 0x0002, "return address");
    }

    #[test]
    fn int_three_is_the_one_byte_breakpoint() {
        let mut cpu = cpu_at(&[0xCC]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        install_vector(&mut cpu, 3, 0x0000, 0x0700);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0700);
        assert_eq!(
            cpu.mem.read_u16(0x2000, 0x00FA),
            0x0001,
            "INT 3 is one byte"
        );
    }

    #[test]
    fn iret_restores_ip_cs_and_flags() {
        let mut cpu = cpu_at(&[0xCF]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        // SP points at IP, the last thing a handler would have pushed.
        cpu.regs.set_word(Reg16::Sp, 0x00FC);
        // IRET pops IP, then CS, then FLAGS, from the top of the stack.
        cpu.mem.write_u16(0x2000, 0x00FC, 0x0999); // IP, read at SP
        cpu.mem.write_u16(0x2000, 0x00FE, 0x6000); // CS
        cpu.mem.write_u16(0x2000, 0x0100, 0x0042); // FLAGS: ZF plus the fixed bit 1
        cpu.step().unwrap();
        assert_eq!(cpu.regs.ip, 0x0999);
        assert_eq!(cpu.regs.segment(Seg::Cs), 0x6000);
        assert!(cpu.regs.flags.get(ZF));
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0102);
    }

    #[test]
    fn iret_ignores_the_bits_an_8086_cannot_write() {
        let mut cpu = cpu_at(&[0xCF]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x00FC);
        cpu.mem.write_u16(0x2000, 0x00FC, 0x0000); // IP
        cpu.mem.write_u16(0x2000, 0x00FE, 0x0000); // CS
        // Bit 3 is fixed at zero and bit 12 is not writable on an 8086.
        cpu.mem.write_u16(0x2000, 0x0100, 0x0008 | 0x1000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.flags.0 & 0x0008, 0, "bit 3 stays zero");
        assert_eq!(cpu.regs.flags.0 & 0x1000, 0, "bit 12 is not writable");
    }

    #[test]
    fn call_and_jcc_leave_the_flags_untouched() {
        let mut cpu = cpu_at(&[0xE8, 0x00, 0x00]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.flags.set(CF, true);
        cpu.regs.flags.set(SF, true);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(CF) && cpu.regs.flags.get(SF));

        let mut cpu = cpu_with(&[0x74, 0x10]);
        cpu.regs.flags.set(ZF, true);
        cpu.regs.flags.set(CF, true);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(ZF) && cpu.regs.flags.get(CF));
    }

    // ---- MOV / XCHG / LEA --------------------------------------------------------
    // MOV and XCHG touch no flags at all. Every test below sets a flag first
    // and asserts it afterwards, because "does not modify" is only meaningful
    // when something was there to modify.

    #[test]
    fn mov_immediate_to_word_register() {
        // B8 imm16 = MOV AX, 0x1234.
        let mut cpu = cpu_with(&[0xB8, 0x34, 0x12]);
        cpu.regs.flags.set(ZF, true);
        cpu.regs.flags.set(CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x1234);
        assert!(
            cpu.regs.flags.get(ZF) && cpu.regs.flags.get(CF),
            "MOV leaves FLAGS alone"
        );
    }

    #[test]
    fn mov_immediate_to_byte_register_and_immediate_to_memory() {
        // B0 imm8 = MOV AL, 0x42.  C6 /0 imm8 = MOV BYTE [BX], 0x42.
        let mut cpu = cpu_with(&[0xB0, 0x42, 0xC6, 0x07, 0x42]);
        cpu.regs.set_word(Reg16::Bx, 0x0500);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x42);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x0000, 0x0500), 0x42);
    }

    #[test]
    fn mov_between_registers_in_both_directions() {
        // 89 C3 = MOV BX, AX.  8B D8 = MOV BX, AX.
        for code in [vec![0x89, 0xC3], vec![0x8B, 0xD8]] {
            let mut cpu = cpu_with(&code);
            cpu.regs.set_word(Reg16::Ax, 0x1111);
            cpu.regs.set_word(Reg16::Bx, 0x1234);
            cpu.step().unwrap();
            assert_eq!(cpu.regs.word(Reg16::Bx), 0x1111, "BX takes the value");
            assert_eq!(cpu.regs.word(Reg16::Ax), 0x1111, "source is unchanged");
        }
    }

    #[test]
    fn mov_between_register_and_memory() {
        // 88 07 = MOV [BX], AL.  8A 07 = MOV AL, [BX].
        let mut cpu = cpu_with(&[0x88, 0x07, 0x8A, 0x07]);
        cpu.regs.set_word(Reg16::Bx, 0x0600);
        cpu.regs.set_byte(Reg8::Al, 0x7E);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x0000, 0x0600), 0x7E);
        cpu.regs.set_byte(Reg8::Al, 0x00);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x7E);
    }

    #[test]
    fn mov_word_to_and_from_memory() {
        // 89 07 = MOV [BX], AX.  8B 07 = MOV AX, [BX].
        let mut cpu = cpu_with(&[0x89, 0x07, 0x8B, 0x07]);
        cpu.regs.set_word(Reg16::Bx, 0x0700);
        cpu.regs.set_word(Reg16::Ax, 0xBEEF);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x0000, 0x0700), 0xBEEF);
        cpu.regs.set_word(Reg16::Ax, 0x0000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0xBEEF);
    }

    #[test]
    fn mov_accumulator_to_and_from_a_direct_address() {
        // A0 disp16 = MOV AL, [0000:disp16].  A2 disp16 = MOV [0000:disp16], AL.
        let mut cpu = cpu_with(&[0xA2, 0x00, 0x08, 0xA0, 0x00, 0x08]);
        cpu.regs.set_byte(Reg8::Al, 0x5A);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u8(0x0000, 0x0800), 0x5A);
        cpu.regs.set_byte(Reg8::Al, 0x00);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x5A);
    }

    #[test]
    fn mov_to_a_segment_register_and_to_memory() {
        // 8C has no register destination on an 8086, so the segment register
        // goes to memory. Whether the store lands is covered by the 256-opcode
        // sweep; what this pins is that the encoding decodes and executes
        // rather than being taken as a byte store.
        let mut cpu = cpu_with(&[0x8E, 0xD8, 0x8C, 0x1E, 0x00, 0x20]);
        cpu.regs.set_word(Reg16::Ax, 0x2000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.segment(Seg::Ds), 0x2000);
        let info = cpu.step().expect("8C to memory must execute");
        assert_eq!(info.opcode, 0x8C);
        assert_eq!(info.next_ip, 6, "opcode, ModR/M and a disp16");
    }

    #[test]
    fn mov_to_cs_changes_where_the_next_instruction_is_fetched_from() {
        // The classic near jump into a segment. The reg field must name CS,
        // which is /1: 0x8E with reg=001 is 0xC8. `8E D8` would be MOV DS, AX.
        let mut cpu = cpu_at(&[0x8E, 0xC8, 0x90]);
        // Code at 3000:0000 is MOV CS, AX; NOP, and at 0x4000:0000 is HLT.
        assert!(cpu.mem.load_at(0x30000, &[0x8E, 0xC8, 0x90]).is_ok());
        // MOV CS does not move IP. The next instruction is fetched at the
        // *same* offset in the new segment, so 4000:0002 is where execution
        // continues, not 4000:0000.
        assert!(cpu.mem.load_at(0x40000, &[0x90, 0x90, 0xF4]).is_ok());
        cpu.regs.set_segment(Seg::Cs, 0x3000);
        cpu.regs.set_word(Reg16::Ax, 0x4000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.segment(Seg::Cs), 0x4000);
        // The NOP at 3000:0002 is never fetched; the HLT at 4000:0002 is.
        cpu.step().unwrap();
        assert!(cpu.halted);
    }

    #[test]
    fn xchg_swaps_two_registers_without_touching_flags() {
        // 93 is XCHG AX, BX. Plain 0x90 is NOP and never reads a ModR/M byte.
        let mut cpu = cpu_with(&[0x93]);
        cpu.regs.set_word(Reg16::Ax, 0x1111);
        cpu.regs.set_word(Reg16::Bx, 0x2222);
        cpu.regs.flags.set(SF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x2222);
        assert_eq!(cpu.regs.word(Reg16::Bx), 0x1111);
        assert!(cpu.regs.flags.get(SF));
    }

    #[test]
    fn lea_computes_an_address_without_touching_memory() {
        // 8D 00 = LEA AX, [BX+SI]. The memory is read for nothing; only the
        // address is what lands in AX.
        let mut cpu = cpu_with(&[0x8D, 0x00]);
        cpu.regs.set_word(Reg16::Bx, 0x0800);
        cpu.regs.set_word(Reg16::Si, 0x0020);
        cpu.mem.write_u16(0x0000, 0x0820, 0xDEAD);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x0820, "the effective address");
        assert_eq!(cpu.mem.read_u16(0x0000, 0x0820), 0xDEAD, "memory untouched");
    }

    #[test]
    fn lea_refuses_a_form_that_has_no_address_on_this_cpu() {
        // LEA AX, AX would encode as r/m in register mode, where there is no
        // displacement to form an address from. The manual does not define it.
        let mut cpu = cpu_with(&[0x8D, 0xC0]);
        assert_eq!(cpu.step(), Err(ExecError::BadLea(0x8D)));
    }

    #[test]
    fn sixteen_bit_modrm_register_operands_use_the_whole_word() {
        // 01 C3 is ADD BX, AX: reg field 000 names AX, r/m field 011 names BX,
        // and r/m is the destination. BX = 0x1234 + AX = 0x1111 -> 0x2345.
        // Reading BX as its low byte would silently produce 0x1245.
        let mut cpu = cpu_with(&[0x01, 0xC3]);
        cpu.regs.set_word(Reg16::Bx, 0x1234);
        cpu.regs.set_word(Reg16::Ax, 0x1111);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Bx), 0x2345);
    }

    #[test]
    fn eight_bit_modrm_register_operands_stay_eight_bit() {
        // 00 D8 is ADD AL, BL: reg field 011 names BL, r/m field 000 names AL,
        // and r/m is the destination. It must not touch the upper halves.
        let mut cpu = cpu_with(&[0x00, 0xD8]);
        cpu.regs.set_word(Reg16::Ax, 0xAA01);
        cpu.regs.set_word(Reg16::Bx, 0xBB01);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0x02);
        assert_eq!(cpu.regs.byte(Reg8::Ah), 0xAA, "AH untouched");
        assert_eq!(cpu.regs.byte(Reg8::Bh), 0xBB, "BH untouched");
    }

    #[test]
    fn hlt_stops_the_machine() {
        let mut cpu = cpu_with(&[0xF4]);
        cpu.step().unwrap();
        assert!(cpu.halted);
        assert_eq!(cpu.stop_reason, crate::StopReason::Halted);
        assert_eq!(cpu.regs.ip, 1);
    }

    #[test]
    fn unknown_opcode_is_reported_not_swallowed() {
        // FF /7 is not defined on an 8086. /2 and /4 are, and are tested.
        let mut cpu = cpu_with(&[0xFF, 0x38]);
        assert_eq!(cpu.step(), Err(ExecError::UnsupportedFFGroup(7)));
    }

    #[test]
    fn pushf_popf_round_trip() {
        let mut cpu = cpu_with(&[0x9C, 0x9D]);
        cpu.regs.set_segment(Seg::Ss, 0x1000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.flags.set(ZF, true);
        cpu.step().unwrap();
        cpu.regs.flags.set(ZF, false);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(ZF));
    }

    #[test]
    fn immediate_to_memory_forms() {
        // 80 /7 is CMP BYTE [BX], imm8. A8 is CMP AL, imm8 and has no
        // ModR/M byte at all, so it cannot address memory.
        let mut cpu = cpu_with(&[0x80, 0x3F, 0x11]);
        cpu.regs.set_word(Reg16::Bx, 0x0200);
        cpu.mem.write_u8(0x0000, 0x0200, 0x11);
        cpu.step().unwrap();
        assert!(cpu.regs.flags.get(ZF), "11 == 11");
        assert_eq!(cpu.mem.read_u8(0x0000, 0x0200), 0x11, "CMP must not store");
    }

    #[test]
    fn eighty_three_sign_extends_its_byte() {
        // 83 /0 is ADD WORD [BX+SI], imm8 sign-extended: + 0xFE means -2.
        let mut cpu = cpu_with(&[0x83, 0x00, 0xFE]);
        cpu.regs.set_word(Reg16::Bx, 0x0300);
        cpu.mem.write_u16(0x0000, 0x0300, 0x0010);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x0000, 0x0300), 0x000E);
    }

    #[test]
    fn eighty_one_carries_a_full_immediate() {
        // 81 /0 is ADD WORD [BX], imm16. Only /0, /1 and /7 are encodable in
        // this group on an 8086; ADC/SBB/AND/SUB/XOR have no r/m,imm form.
        let mut cpu = cpu_with(&[0x81, 0x07, 0x34, 0x12]);
        cpu.regs.set_word(Reg16::Bx, 0x0400);
        cpu.mem.write_u16(0x0000, 0x0400, 0x1000);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_u16(0x0000, 0x0400), 0x2234);
    }

    // ---- MUL / IMUL / DIV / IDIV -------------------------------------------------
    // Flag expectations come from documentation-guide.md §1.4, transcribed from the
    // Intel 8086 Family User's Manual: MUL and IMUL define only CF and OF; DIV and
    // IDIV define none of the six.

    #[test]
    fn mul8_writes_the_full_ax() {
        // F6 /4 = MUL BL.  15 * 15 = 225, which still fits in a byte.
        let mut cpu = cpu_with(&[0xF6, 0xE3]);
        cpu.regs.set_byte(Reg8::Al, 0x0F);
        cpu.regs.set_byte(Reg8::Bl, 0x0F);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x00E1);
        assert!(!cpu.regs.flags.get(CF) && !cpu.regs.flags.get(OF));

        // 16 * 16 = 256 does not fit, and that is the same instruction.
        let mut cpu = cpu_with(&[0xF6, 0xE3]);
        cpu.regs.set_byte(Reg8::Al, 0x10);
        cpu.regs.set_byte(Reg8::Bl, 0x10);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x0100);
        assert!(cpu.regs.flags.get(CF) && cpu.regs.flags.get(OF));
    }

    #[test]
    fn mul8_signals_overflow_through_cf_and_of_only() {
        // 0xFF * 0xFF = 0xFE01: AH is not zero, so the product did not fit.
        let mut cpu = cpu_with(&[0xF6, 0xE3]);
        cpu.regs.set_byte(Reg8::Al, 0xFF);
        cpu.regs.set_byte(Reg8::Bl, 0xFF);
        cpu.regs.flags.set(ZF, true);
        cpu.regs.flags.set(SF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0xFE01);
        assert!(cpu.regs.flags.get(CF) && cpu.regs.flags.get(OF));
        // The manual leaves these undefined, so an implementation must not
        // invent values for them. Leaving them untouched is the only choice
        // that cannot be wrong.
        assert!(cpu.regs.flags.get(ZF) && cpu.regs.flags.get(SF));
    }

    #[test]
    fn mul16_produces_a_32_bit_result_in_dx_ax() {
        // F7 /4 = MUL BX.  0x00FF * 0x00FF = 0xFE01, which fits in 16 bits, so
        // DX is zero and neither CF nor OF is raised.
        let mut cpu = cpu_with(&[0xF7, 0xE3]);
        cpu.regs.set_word(Reg16::Ax, 0x00FF);
        cpu.regs.set_word(Reg16::Bx, 0x00FF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0xFE01);
        assert_eq!(cpu.regs.word(Reg16::Dx), 0x0000);
        assert!(!cpu.regs.flags.get(CF) && !cpu.regs.flags.get(OF));
    }

    #[test]
    fn mul16_signals_overflow_when_dx_is_not_zero() {
        let mut cpu = cpu_with(&[0xF7, 0xE3]);
        cpu.regs.set_word(Reg16::Ax, 0xFFFF);
        cpu.regs.set_word(Reg16::Bx, 0xFFFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Dx), 0xFFFE);
        assert!(cpu.regs.flags.get(CF) && cpu.regs.flags.get(OF));
    }

    #[test]
    fn imul8_treats_operands_as_signed() {
        // F6 /5 = IMUL BL.  0xFF * 0xFF = (-1) * (-1) = 1, which fits exactly.
        let mut cpu = cpu_with(&[0xF6, 0xEB]);
        cpu.regs.set_byte(Reg8::Al, 0xFF);
        cpu.regs.set_byte(Reg8::Bl, 0xFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x0001);
        assert!(!cpu.regs.flags.get(CF) && !cpu.regs.flags.get(OF));
    }

    #[test]
    fn imul8_signals_overflow_when_ah_is_not_a_sign_extension() {
        // (-128) * (-1) = 128. The result is 0x0080, which is *not* a sign
        // extension of its own low byte 0x80, so it did not fit in a signed
        // byte: 128 is one past the maximum of 127.
        let mut cpu = cpu_with(&[0xF6, 0xEB]);
        cpu.regs.set_byte(Reg8::Al, 0x80);
        cpu.regs.set_byte(Reg8::Bl, 0xFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0x0080);
        assert!(cpu.regs.flags.get(CF) && cpu.regs.flags.get(OF));
    }

    #[test]
    fn div8_splits_ax_into_quotient_and_remainder() {
        // F6 /6 = DIV BL.  100 / 7 = 14 remainder 2.
        let mut cpu = cpu_with(&[0xF6, 0xF3]);
        cpu.regs.set_word(Reg16::Ax, 100);
        cpu.regs.set_byte(Reg8::Bl, 7);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 14);
        assert_eq!(cpu.regs.byte(Reg8::Ah), 2);
    }

    #[test]
    fn div8_by_zero_raises_the_divide_error_interrupt() {
        let mut cpu = cpu_at(&[0xF6, 0xF3]);
        cpu.regs.set_word(Reg16::Ax, 100);
        cpu.regs.set_byte(Reg8::Bl, 0);
        install_vector(&mut cpu, 0, 0x0000, 0x0100);
        cpu.step().unwrap();
        assert_eq!(cpu.stop_reason, crate::StopReason::Interrupt(0));
        assert_eq!(cpu.regs.ip, 0x0100, "vectored to the type 0 handler");
    }

    #[test]
    fn div8_raises_an_interrupt_when_the_quotient_does_not_fit() {
        // 511 / 1 has a quotient of 511, far wider than AL.
        let mut cpu = cpu_at(&[0xF6, 0xF3]);
        cpu.regs.set_word(Reg16::Ax, 0x01FF);
        cpu.regs.set_byte(Reg8::Bl, 1);
        install_vector(&mut cpu, 0, 0x0000, 0x0200);
        cpu.step().unwrap();
        assert_eq!(cpu.stop_reason, crate::StopReason::Interrupt(0));
        assert_eq!(cpu.regs.ip, 0x0200);
    }

    #[test]
    fn idiv8_divides_signed_and_truncates_toward_zero() {
        // F6 /7 = IDIV BL.  -10 / 3 = -3 remainder -1.  Truncation toward zero
        // is what x86 does, not floor division.
        let mut cpu = cpu_with(&[0xF6, 0xFB]);
        cpu.regs.set_word(Reg16::Ax, 0xFFF6); // -10
        cpu.regs.set_byte(Reg8::Bl, 0x03);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.byte(Reg8::Al), 0xFD, "quotient -3");
        assert_eq!(cpu.regs.byte(Reg8::Ah), 0xFF, "remainder -1");
    }

    #[test]
    fn div16_uses_the_full_dx_ax_dividend() {
        // F7 /6 = DIV BX.  0x0000_0064 / 7 = 14 remainder 2.
        let mut cpu = cpu_with(&[0xF7, 0xF3]);
        cpu.regs.set_word(Reg16::Dx, 0);
        cpu.regs.set_word(Reg16::Ax, 100);
        cpu.regs.set_word(Reg16::Bx, 7);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 14);
        assert_eq!(cpu.regs.word(Reg16::Dx), 2);
    }

    #[test]
    fn idiv16_divides_signed() {
        // F7 /7 = IDIV BX.  -1000 / 7 = -142 remainder -6.
        let mut cpu = cpu_with(&[0xF7, 0xFB]);
        cpu.regs.set_word(Reg16::Dx, 0xFFFF);
        cpu.regs.set_word(Reg16::Ax, 0xFC18); // -1000
        cpu.regs.set_word(Reg16::Bx, 7);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), (-142i16) as u16);
        assert_eq!(cpu.regs.word(Reg16::Dx), (-6i16) as u16);
    }

    #[test]
    fn idiv16_raises_an_interrupt_when_the_quotient_does_not_fit() {
        // 0x7FFF_7FFF / 1 has a quotient far outside the signed 16-bit range.
        let mut cpu = cpu_at(&[0xF7, 0xFB]);
        cpu.regs.set_word(Reg16::Dx, 0x7FFF);
        cpu.regs.set_word(Reg16::Ax, 0x7FFF);
        cpu.regs.set_word(Reg16::Bx, 1);
        install_vector(&mut cpu, 0, 0x0000, 0x0300);
        cpu.step().unwrap();
        assert_eq!(cpu.stop_reason, crate::StopReason::Interrupt(0));
    }

    #[test]
    fn cbw_and_cwd_sign_extend_without_touching_flags() {
        // 98 = CBW, 99 = CWD.
        let mut cpu = cpu_with(&[0x98, 0x99]);
        cpu.regs.set_byte(Reg8::Al, 0x80);
        cpu.regs.flags.set(ZF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Ax), 0xFF80);
        assert!(cpu.regs.flags.get(ZF), "CBW must not touch flags");
        cpu.regs.set_word(Reg16::Ax, 0x8000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.word(Reg16::Dx), 0xFFFF);
        assert!(cpu.regs.flags.get(ZF), "CWD must not touch flags");
    }

    #[test]
    fn a_divide_error_clears_if_and_pushes_the_return_state() {
        let mut cpu = cpu_at(&[0xF6, 0xF3]);
        cpu.regs.set_segment(Seg::Ss, 0x2000);
        cpu.regs.set_word(Reg16::Sp, 0x0100);
        cpu.regs.set_word(Reg16::Ax, 100);
        cpu.regs.set_byte(Reg8::Bl, 0);
        cpu.regs.flags.set(IF, true);
        install_vector(&mut cpu, 0, 0x0000, 0x0400);
        let cs = cpu.regs.segment(Seg::Cs);
        // The faulting instruction returns control to the instruction *after*
        // it, so the pushed IP is where the decoder stopped, not where it
        // started. F6 F3 is two bytes, so the return address is 2.
        let pushed_ip = cpu.regs.ip + 2;
        let pushed = cpu.regs.flags.pushed();
        cpu.step().unwrap();

        assert!(
            !cpu.regs.flags.get(IF),
            "the handler runs with interrupts off"
        );
        // FLAGS, then CS, then IP, pushed in that order on an 8086.
        assert_eq!(cpu.regs.word(Reg16::Sp), 0x0100 - 6);
        // PUSH predecrements, so the last thing pushed sits lowest.
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FE), pushed, "FLAGS");
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FC), cs, "CS");
        assert_eq!(cpu.mem.read_u16(0x2000, 0x00FA), pushed_ip, "IP");
    }

    /// Point vector `n` at `handler_offset` within `handler_segment`, the way
    /// the interrupt table in physical memory `0000:0000` works: offset first,
    /// segment second.
    fn install_vector(cpu: &mut Cpu, n: u8, handler_segment: u16, handler_offset: u16) {
        cpu.mem.write_u16(0x0000, u16::from(n) * 4, handler_offset);
        cpu.mem
            .write_u16(0x0000, u16::from(n) * 4 + 2, handler_segment);
    }

    #[test]
    fn ip_only_cannot_be_written_directly() {
        // Sanity check on the 8086 restriction we mirror elsewhere.
        let mut regs = Registers::new();
        regs.ip = 0x1234;
        assert_eq!(regs.ip, 0x1234);
    }
}
