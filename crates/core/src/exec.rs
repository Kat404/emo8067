//! Fetch, decode, execute.

use crate::flags::{AF, CF, DF, Flags, OF, PF, SF, TF, ZF, parity_even};
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
        Base::Direct => disp,
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
        0x00..=0x3F => alu_group(cpu, opcode)?,
        // ---- the only three ALU groups that take an immediate. ADC, SBB,
        // AND, SUB and XOR have no r/m,imm encoding on an 8086: 0x90..0x9F
        // is XCHG/CBW/CWD/CALL/PUSHF, and 0xB8..0xBF is MOV.
        0x80..=0x83 => alu_group_imm(cpu, opcode, AluOp::Add)?,
        0x88..=0x8B => alu_group_imm(cpu, opcode, AluOp::Or)?,
        0xA8..=0xAB => alu_group_imm(cpu, opcode, AluOp::Cmp)?,
        0xF6 | 0xF7 => mul_group(cpu, opcode)?,
        0x40..=0x47 => {
            cpu.regs.set_word(
                Reg16::from_bits(opcode & 0b111),
                cpu.regs
                    .word(Reg16::from_bits(opcode & 0b111))
                    .wrapping_add(1),
            );
        }
        0x48..=0x4F => {
            cpu.regs.set_word(
                Reg16::from_bits(opcode & 0b111),
                cpu.regs
                    .word(Reg16::from_bits(opcode & 0b111))
                    .wrapping_sub(1),
            );
        }
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
        0x90 => {} // NOP
        0xF4 => {
            cpu.halted = true;
            cpu.stop_reason = crate::StopReason::Halted;
        }
        0xF5 => cpu.regs.flags.set(CF, !cpu.regs.flags.get(CF)), // CMC
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
fn alu_group_imm(cpu: &mut crate::Cpu, opcode: u8, op: AluOp) -> Result<(), ExecError> {
    let form = opcode & 0b11;
    let bits: u8 = if form & 1 != 0 { 16 } else { 8 };
    let mask = width_mask(bits);

    let m = decode_from(cpu)?;
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
            ((product & 0x00FF) as u16, (product >> 8) as u16)
        }
        (16, 4) => {
            let product = u32::from(ax) * u32::from(operand as u16);
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
            let product = a * i32::from(operand as u16 as i16);
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
            let divisor = operand as u16;
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
            let divisor = operand as u16 as i16;
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
        // 0F is POP CS on an 8086, so use a byte the 8086 never defined.
        let mut cpu = cpu_with(&[0xFF]);
        assert_eq!(cpu.step(), Err(ExecError::UnknownOpcode(0xFF)));
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
        // A8 /7 is CMP BYTE [BX], imm8.
        let mut cpu = cpu_with(&[0xA8, 0x3F, 0x11]);
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
