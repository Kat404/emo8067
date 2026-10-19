//! What each instruction touches, and what it must not touch.
//!
//! Nothing in this repository asserted that. `every_first_opcode_byte_the_
//! 8086_defines_decodes` proves a byte is not a hole, and the flag contract
//! proves the flags a §1.4 row calls untouched are untouched. Between them
//! there is a hole the width of a memory operand: a `MOV m8,AL` that writes
//! through `ES` instead of `DS` passes every gate in the tree, because no
//! gate looks at where the bytes went.
//!
//! So this file says, for one instruction of every family, exactly what may
//! change, and checks that nothing else did:
//!
//! - the registers the instruction writes hold the value it wrote,
//! - every other register holds the value it held,
//! - the memory it writes holds the bytes it wrote,
//! - every other byte of the whole megabyte is untouched,
//! - the flags §1.4 marks `—` come out exactly as they went in.
//!
//! The last one is not typed by hand: the flags an instruction may write are
//! read out of the document's own §1.4 row, so the claim comes from the
//! oracle rather than from the same vocabulary the code is written in. A
//! row that says `—` for `CF` is a promise, and this file checks it.
//!
//! # What this gate is not
//!
//! It is not a proof that the emulator matches silicon. It is a proof that
//! the emulator matches the document *and* that an instruction's blast radius
//! is the one the document describes. Where the document and the chip
//! disagree, the document is the one this file believes, and the
//! disagreement is recorded in `odd/tasks/decode-geometry-gate.md`.
//!
//! Two limits are worth stating plainly. `DF` is left clear in both passes,
//! because the stack operations follow it, so a bug that clears `DF` instead
//! of setting it is not caught here. And the instructions whose *result*
//! depends on a flag — `ADC`, `SBB`, `RCL`, `RCR`, `DAA`, `AAA` and their
//! kin — are in a separate test with one fixed flag state, because their
//! expected value is not the same in both passes.

mod common;

use emo8067_core::Cpu;
use emo8067_core::flags::{AF, CF, DF, IF, OF, PF, SF, TF, ZF};
use emo8067_core::memory::{ADDR_SPACE, Memory};
use emo8067_core::regs::{Reg16, Seg};

use common::{GUIDE, Mark, row_marks};

/// Code lives here, and so does `DS`, so that a memory operand through `DS`
/// and one through `ES` land in different places.
const CODE: u16 = 0x1000;
/// The data segment: `DS = 0x4000`, so `DS:0134` is physical `0x40134`.
const DATA: u16 = 0x4000;
/// `ES` is a different segment on purpose, and so is the stack.
const EXTRA: u16 = 0x5000;
const STACK: u16 = 0x3000;

/// The code is loaded at physical `0x10000`, which is `CS:0000`.
const CODE_AT: u32 = 0x10000;

/// One instruction, and everything it is allowed to change.
struct Effect {
    /// What Table 4-13 calls it, for the failure message.
    name: &'static str,
    /// The instruction as it appears in memory.
    code: &'static [u8],
    /// The §1.4 row that says which flags it may write. `None` means the
    /// instruction writes no flags at all.
    row: Option<&'static str>,
    /// The registers it writes, and the value each must hold afterwards.
    regs: &'static [(Reg16, u16)],
    /// The segments it writes, and the value each must hold afterwards.
    segs: &'static [(Seg, u16)],
    /// The memory it writes: segment, offset, and the bytes it must hold.
    mem: &'static [(u16, u16, &'static [u8])],
    /// Where `IP` must land, when the instruction is a transfer.
    ip: Option<u16>,
    /// A state the instruction needs and the common one does not provide.
    pre: Option<fn(&mut Cpu)>,
}

/// Build an [`Effect`]. The lists are empty when the instruction touches
/// nothing of that kind, and saying so is the point.
macro_rules! effect {
    (
        $name:expr, $code:expr, row: $row:expr,
        regs: $regs:expr,
        segs: $segs:expr,
        mem: $mem:expr,
        ip: $ip:expr,
        pre: $pre:expr $(,)?
    ) => {
        Effect {
            name: $name,
            code: $code,
            row: $row,
            regs: $regs,
            segs: $segs,
            mem: $mem,
            ip: $ip,
            pre: $pre,
        }
    };
}

/// Every instruction whose result does not depend on a flag.
///
/// The flags are seeded twice, once set and once clear, so that a flag
/// written in *either* direction is caught rather than only the ones that
/// happen to land on zero. The six status flags are what is seeded; `DF`,
/// `IF` and `TF` are left alone, and the reason is in the file's header.
const EFFECTS: &[Effect] = &[
    effect!(
        "XCHG AX,AX (NOP)",
        &[0x90],
        row: None,
        regs: &[],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "ADD AL,i8",
        &[0x04, 0x01],
        row: Some("ADD"),
        regs: &[(Reg16::Ax, 0x1112)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "ADD r/m8,r8",
        &[0x00, 0xD8],
        row: Some("ADD"),
        regs: &[(Reg16::Ax, 0x1155)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "ADD r/m16,r16",
        &[0x01, 0xD8],
        row: Some("ADD"),
        // The 16-bit REG field names AX CX DX BX SP BP SI DI, so `011` is BX.
        regs: &[(Reg16::Ax, 0x5555)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "ADD r8,r/m8",
        &[0x02, 0xD8],
        row: Some("ADD"),
        regs: &[(Reg16::Bx, 0x4455)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV r/m8,r8",
        &[0x88, 0xD8],
        row: Some("MOV"),
        regs: &[(Reg16::Ax, 0x1144)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV r/m16,r16",
        &[0x89, 0xD8],
        row: Some("MOV"),
        regs: &[(Reg16::Ax, 0x4444)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV r8,r/m8",
        &[0x8A, 0xD8],
        row: Some("MOV"),
        regs: &[(Reg16::Bx, 0x4411)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV r16,r/m16",
        &[0x8B, 0xD8],
        row: Some("MOV"),
        regs: &[(Reg16::Bx, 0x1111)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV AL,m8",
        &[0xA0, 0x34, 0x01],
        row: Some("MOV"),
        regs: &[(Reg16::Ax, 0x1134)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV m8,AL",
        &[0xA2, 0x34, 0x01],
        row: Some("MOV"),
        regs: &[],
        segs: &[],
        mem: &[(DATA, 0x0134, b"\x11")],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV m16,AX",
        &[0xA3, 0x34, 0x01],
        row: Some("MOV"),
        regs: &[],
        segs: &[],
        mem: &[(DATA, 0x0134, b"\x11\x11")],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV r/m16,sreg",
        &[0x8C, 0x1E, 0x34, 0x01],
        row: Some("MOV"),
        regs: &[],
        segs: &[],
        mem: &[(DATA, 0x0134, b"\x00\x40")],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOV sreg,r/m16",
        &[0x8E, 0x1E, 0x34, 0x01],
        row: Some("MOV"),
        regs: &[],
        segs: &[(Seg::Ds, 0x3534)],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "LEA r16,r/m16",
        &[0x8D, 0x06, 0x34, 0x01],
        row: Some("LEA"),
        regs: &[(Reg16::Ax, 0x0134)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "XCHG AX,CX",
        &[0x91],
        row: None,
        regs: &[(Reg16::Ax, 0x2222), (Reg16::Cx, 0x1111)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "XCHG r/m8,r8",
        &[0x86, 0xD8],
        row: None,
        regs: &[(Reg16::Ax, 0x1144), (Reg16::Bx, 0x4411)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "INC AX",
        &[0x40],
        row: Some("INC"),
        regs: &[(Reg16::Ax, 0x1112)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "DEC AX",
        &[0x48],
        row: Some("DEC"),
        regs: &[(Reg16::Ax, 0x1110)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "TEST r/m8,r8",
        &[0x84, 0xD8],
        row: Some("TEST"),
        regs: &[],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "CMP r/m8,r8",
        &[0x38, 0xD8],
        row: Some("CMP"),
        regs: &[],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "SHL r/m8,1",
        &[0xD0, 0xE0],
        row: Some("SHL"),
        regs: &[(Reg16::Ax, 0x1122)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "NOT r/m8",
        &[0xF6, 0xD0],
        row: None,
        regs: &[(Reg16::Ax, 0x11EE)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "NEG r/m8",
        &[0xF6, 0xD8],
        row: Some("NEG"),
        regs: &[(Reg16::Ax, 0x11EF)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MUL r/m8",
        &[0xF6, 0xE0],
        row: Some("MUL"),
        regs: &[(Reg16::Ax, 0x0121)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "DIV r/m8",
        &[0xF6, 0xF0],
        row: Some("DIV"),
        // The 8-bit form divides the double-length dividend in AL and AH by
        // the byte operand, and the quotient has to fit in AL: the manual
        // says a quotient above FFh raises the type 0 interrupt. The common
        // state is not a dividend small enough for that, so this one brings
        // its own.
        regs: &[(Reg16::Ax, 0x0001)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: Some(small_dividend),
    ),
    effect!(
        "CLC",
        &[0xF8],
        row: Some("CLC"),
        regs: &[],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "STD",
        &[0xFD],
        row: Some("STD"),
        regs: &[],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "AAM i8",
        &[0xD4, 0x0A],
        row: Some("AAM"),
        regs: &[(Reg16::Ax, 0x0107)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "AAD i8",
        &[0xD5, 0x0A],
        row: Some("AAD"),
        // AAD turns the pair into the numerator of the division that follows:
        // AL becomes AH * base + AL, and AH clears. With AH = 11h and
        // AL = 11h that is 11h * 10 + 11h = BBh.
        regs: &[(Reg16::Ax, 0x00BB)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "XLAT",
        &[0xD7],
        row: None,
        regs: &[(Reg16::Ax, 0x1155)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "MOVSB",
        &[0xA4],
        row: None,
        regs: &[(Reg16::Si, 0x0601), (Reg16::Di, 0x0702)],
        segs: &[],
        mem: &[(EXTRA, 0x0701, b"\x00")],
        ip: None,
        // The copy has to be visible: the source and the destination hold the
        // same byte in the common state, because the pattern depends on the
        // low byte of the address and the two segments differ by 10000h. So
        // the destination is moved one byte along, where the pattern differs.
        pre: Some(move_destination),
    ),
    effect!(
        "STOSB",
        &[0xAA],
        row: None,
        regs: &[(Reg16::Di, 0x0701)],
        segs: &[],
        mem: &[(EXTRA, 0x0700, b"\x11")],
        ip: None,
        pre: None,
    ),
    effect!(
        "LODSB",
        &[0xAC],
        row: None,
        regs: &[(Reg16::Si, 0x0601), (Reg16::Ax, 0x1100)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "SCASB",
        &[0xAE],
        row: Some("SCAS"),
        regs: &[(Reg16::Di, 0x0701)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "CMPSB",
        &[0xA6],
        row: Some("CMPS"),
        regs: &[(Reg16::Si, 0x0601), (Reg16::Di, 0x0701)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "IN AL,i8",
        &[0xE4, 0x60],
        row: None,
        regs: &[(Reg16::Ax, 0x1160)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "OUT i8,AL",
        &[0xE6, 0x60],
        row: None,
        regs: &[],
        segs: &[],
        mem: &[(0x0000, 0x0060, b"\x11")],
        ip: None,
        pre: None,
    ),
    effect!(
        "LES r16,r/m16",
        &[0xC4, 0x06, 0x34, 0x01],
        row: Some("LEA"),
        regs: &[(Reg16::Ax, 0x3534)],
        segs: &[(Seg::Es, 0x3736)],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "LDS r16,r/m16",
        &[0xC5, 0x06, 0x34, 0x01],
        row: Some("LDS"),
        regs: &[(Reg16::Ax, 0x3534)],
        segs: &[(Seg::Ds, 0x3736)],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "JMP near",
        &[0xE9, 0x00, 0x00],
        row: Some("JMP"),
        regs: &[],
        segs: &[],
        mem: &[],
        ip: Some(0x0003),
        pre: None,
    ),
];

/// The instructions whose result depends on a flag, with one fixed flag
/// state: every status flag set, `DF` clear.
///
/// The flags are the same in every case here, so the expectations are too:
/// `ADC` adds the carry, `RCL` rotates it in, and `DAA` reads the auxiliary
/// carry. The document's own rows say which flags each of them may write.
const FLAG_DEPENDENT: &[Effect] = &[
    effect!(
        "ADC r/m8,r8",
        &[0x10, 0xD8],
        row: Some("ADC"),
        regs: &[(Reg16::Ax, 0x1156)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "SBB r/m8,r8",
        &[0x18, 0xD8],
        row: Some("SBB"),
        regs: &[(Reg16::Ax, 0x11CC)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "RCL r/m8,1",
        &[0xD0, 0xD0],
        row: Some("RCL"),
        regs: &[(Reg16::Ax, 0x1123)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "RCR r/m8,1",
        &[0xD0, 0xD8],
        row: Some("RCR"),
        regs: &[(Reg16::Ax, 0x1188)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "DAA",
        &[0x27],
        row: Some("DAA"),
        regs: &[(Reg16::Ax, 0x1177)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "AAA",
        &[0x37],
        row: Some("AAA"),
        regs: &[(Reg16::Ax, 0x1207)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "LAHF",
        &[0x9F],
        row: Some("LAHF"),
        // LAHF loads the status flags into AH: the low byte of FLAGS, which
        // is CF PF AF ZF SF, and not the high byte with OF and DF in it.
        regs: &[(Reg16::Ax, 0xD711)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
];

/// The instructions that move the stack, with `DF` clear so the stack grows
/// down the way it does on a real machine.
const STACK_MOVING: &[Effect] = &[
    effect!(
        "PUSH AX",
        &[0x50],
        row: None,
        regs: &[(Reg16::Sp, 0x00FE)],
        segs: &[],
        mem: &[(STACK, 0x00FE, b"\x11\x11")],
        ip: None,
        pre: None,
    ),
    effect!(
        "POP AX",
        &[0x58],
        row: None,
        regs: &[(Reg16::Ax, 0x0100), (Reg16::Sp, 0x0102)],
        segs: &[],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "PUSH ES",
        &[0x06],
        row: None,
        regs: &[(Reg16::Sp, 0x00FE)],
        segs: &[],
        mem: &[(STACK, 0x00FE, b"\x00\x50")],
        ip: None,
        pre: None,
    ),
    effect!(
        "POP ES",
        &[0x07],
        row: None,
        regs: &[(Reg16::Sp, 0x0102)],
        segs: &[(Seg::Es, 0x0100)],
        mem: &[],
        ip: None,
        pre: None,
    ),
    effect!(
        "CALL near",
        &[0xE8, 0x00, 0x00],
        row: Some("CALL"),
        regs: &[(Reg16::Sp, 0x00FE)],
        segs: &[],
        // The return address is the offset of the next instruction, not the
        // physical address: CALL pushes IP.
        mem: &[(STACK, 0x00FE, b"\x03\x00")],
        ip: Some(0x0003),
        pre: None,
    ),
];

/// A machine in a known state.
///
/// Every register, every segment and every status flag starts at a value no
/// instruction would produce by accident, and every byte of memory holds a
/// pattern that depends on its address — so a write to the wrong place is
/// visible even when the byte it lands on happens to be the byte it would
/// have held anyway.
fn machine(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    for p in 0..ADDR_SPACE as u32 {
        c.mem
            .write_u8((p >> 4) as u16, (p & 0x0F) as u16, (p & 0xFF) as u8);
    }
    c.regs.set_word(Reg16::Ax, 0x1111);
    c.regs.set_word(Reg16::Cx, 0x2222);
    c.regs.set_word(Reg16::Dx, 0x3333);
    c.regs.set_word(Reg16::Bx, 0x4444);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c.regs.set_word(Reg16::Bp, 0x0500);
    c.regs.set_word(Reg16::Si, 0x0600);
    c.regs.set_word(Reg16::Di, 0x0700);
    c.regs.set_segment(Seg::Cs, CODE);
    c.regs.set_segment(Seg::Ds, DATA);
    c.regs.set_segment(Seg::Es, EXTRA);
    c.regs.set_segment(Seg::Ss, STACK);
    c.mem.load_at(CODE_AT, code).unwrap();
    c
}

/// Put the string destination one byte along, so that a copy into it is a
/// visible change.
fn move_destination(c: &mut Cpu) {
    c.regs.set_word(Reg16::Di, 0x0701);
}

/// A dividend small enough for the 8-bit divide to have a quotient that fits
/// in `AL`.
fn small_dividend(c: &mut Cpu) {
    c.regs.set_word(Reg16::Ax, 0x0011);
}

/// The flags a §1.4 row says an instruction may write: every column that is
/// not `—`.
///
/// A `—` column is a promise that the flag comes out exactly as it went in,
/// and this is the mask of everything else — the flags the instruction is
/// allowed to touch, whether it clears them, sets them or leaves them
/// undefined.
fn writable_flags(name: &str) -> u16 {
    row_marks(GUIDE, name)
        .iter()
        .enumerate()
        .filter(|(_, m)| !matches!(m, Mark::Unchanged))
        .fold(0, |mask, (i, _)| mask | common::COLUMNS[i])
}

/// The eight general registers, in the order the file holds them.
const ALL_WORDS: [Reg16; 8] = [
    Reg16::Ax,
    Reg16::Cx,
    Reg16::Dx,
    Reg16::Bx,
    Reg16::Sp,
    Reg16::Bp,
    Reg16::Si,
    Reg16::Di,
];

/// The four segment registers, in the order the file holds them.
const ALL_SEGS: [Seg; 4] = [Seg::Es, Seg::Cs, Seg::Ss, Seg::Ds];

/// The status flags, in the order §1.4 lists them.
const STATUS: [u16; 6] = [CF, PF, AF, ZF, SF, OF];

/// Seed the status flags. `DF`, `IF` and `TF` are left as they are: `DF`
/// would reverse the stack, `IF` would let an interrupt in, and `TF` is not
/// emulated.
fn seed(c: &mut Cpu, set: bool) {
    for bit in STATUS {
        c.regs.flags.set(bit, set);
    }
    c.regs.flags.set(DF, false);
    c.regs.flags.set(IF, false);
    c.regs.flags.set(TF, false);
}

/// Check one instruction against everything it was not allowed to touch.
fn check(e: &Effect, c: &mut Cpu) {
    if let Some(pre) = e.pre {
        pre(c);
    }
    let before_regs = c.regs.words;
    let before_segs = c.regs.segments;
    let before_flags = c.regs.flags.0;
    let before_mem = c.mem.slice(0, ADDR_SPACE).to_vec();

    let info = c
        .step()
        .unwrap_or_else(|err| panic!("{}: the instruction must execute: {err:?}", e.name));

    // The registers it writes hold what it wrote.
    for (reg, value) in e.regs {
        assert_eq!(
            c.regs.word(*reg),
            *value,
            "{}: it did not write the register it claims to write",
            e.name
        );
    }
    // And every other register is exactly as it was.
    for reg in ALL_WORDS {
        let i = reg as usize;
        if e.regs.iter().any(|(r, _)| *r == reg) {
            continue;
        }
        assert_eq!(
            c.regs.words[i],
            before_regs[i],
            "{}: it changed register {}, which it must not touch",
            e.name,
            reg.name()
        );
    }
    // The same for the segment registers.
    for (seg, value) in e.segs {
        assert_eq!(
            c.regs.segment(*seg),
            *value,
            "{}: it did not write the segment it claims to write",
            e.name
        );
    }
    for seg in ALL_SEGS {
        let i = seg as usize;
        if e.segs.iter().any(|(s, _)| *s == seg) {
            continue;
        }
        assert_eq!(
            c.regs.segments[i],
            before_segs[i],
            "{}: it changed segment {}, which it must not touch",
            e.name,
            seg.name()
        );
    }
    // The memory it writes holds what it wrote.
    for (segment, offset, bytes) in e.mem {
        for (i, byte) in bytes.iter().enumerate() {
            let at = Memory::physical(*segment, offset.wrapping_add(i as u16));
            assert_eq!(
                c.mem.slice(at, 1)[0],
                *byte,
                "{}: it did not write the byte it claims to write",
                e.name
            );
        }
    }
    // And every other byte of the whole megabyte is untouched.
    for (p, (before, after)) in before_mem
        .iter()
        .zip(c.mem.slice(0, ADDR_SPACE).iter())
        .enumerate()
    {
        if before == after {
            continue;
        }
        assert!(
            e.mem.iter().any(|(segment, offset, bytes)| {
                let start = Memory::physical(*segment, *offset);
                let p = p as u32;
                (start..start + bytes.len() as u32).any(|at| at == p)
            }),
            "{}: it wrote at physical {p:#07x}, which it must not touch",
            e.name
        );
    }
    // The flags the document says it may not touch are exactly as they were.
    let allowed = e.row.map_or(0, writable_flags);
    assert_eq!(
        before_flags & !allowed,
        c.regs.flags.0 & !allowed,
        "{}: it changed a flag the guide says it does not touch",
        e.name
    );
    if let Some(ip) = e.ip {
        assert_eq!(info.next_ip, ip, "{}: it landed somewhere else", e.name);
    }
}

#[test]
fn an_instruction_touches_only_what_it_is_documented_to_touch() {
    assert!(
        EFFECTS.len() >= 40,
        "the gate must stay broad: {}",
        EFFECTS.len()
    );
    for e in EFFECTS {
        for set in [true, false] {
            let mut c = machine(e.code);
            seed(&mut c, set);
            check(e, &mut c);
        }
    }
}

#[test]
fn an_instruction_whose_result_depends_on_a_flag_touches_only_what_it_should() {
    for e in FLAG_DEPENDENT {
        let mut c = machine(e.code);
        seed(&mut c, true);
        check(e, &mut c);
    }
}

#[test]
fn an_instruction_that_moves_the_stack_touches_only_what_it_should() {
    for e in STACK_MOVING {
        let mut c = machine(e.code);
        seed(&mut c, true);
        check(e, &mut c);
    }
}

/// The forms the 8086 does not have must be refused, not executed.
///
/// The guide's §1.3 and its notes are the oracle: `8C` with a register
/// destination is an 80186 encoding, `8F` is only defined for `MOD=000`,
/// `001` and `010` with `REG=000`, `LEA` needs a memory operand, and `LES`
/// and `LDS` read a pointer through the R/M field, so a register operand
/// leaves them nothing to read.
#[test]
fn the_forms_the_8086_does_not_have_are_refused() {
    let cases: [(&str, &[u8]); 6] = [
        ("8F with a register destination", &[0x8F, 0xC3]),
        ("8F with a non-zero digit", &[0x8F, 0xC8]),
        ("LEA with a register operand", &[0x8D, 0xC3]),
        ("LES with a register operand", &[0xC4, 0xC3]),
        ("LDS with a register operand", &[0xC5, 0xC3]),
        (
            "8F with a displacement and a non-zero digit",
            &[0x8F, 0x48, 0x00],
        ),
    ];
    for (name, code) in cases {
        let mut c = machine(code);
        let err = c.step();
        assert!(
            err.is_err(),
            "{name}: the 8086 has no such encoding, so it must be refused"
        );
    }
}

/// `8C` and `8E` run every `MOD` the 8086 has, and the manual's print is
/// what is wrong.
///
/// The 1979 print shows `8C MOD 1--` and `8E MOD 1--` as `(not used)`, and
/// this project believed it: it refused `8C` with a register destination as an
/// 80186 encoding. The captures from a real P80C86A-2 show otherwise — the
/// chip runs all four `MOD` values for both, and `8C DF` is `MOV DI,DS` with
/// the segment landing in the register.
#[test]
fn the_segment_moves_run_on_the_8086_the_way_the_chip_runs_them() {
    // `8C DF`: the segment goes into the general register the R/M field
    // names, which under the 8086's field order is `DI`.
    let mut c = machine(&[0x8C, 0xDF]);
    c.regs.set_segment(Seg::Ds, 0x1234);
    c.step().expect("the 8086 runs MOV r16,Sreg");
    assert_eq!(c.regs.word(Reg16::Di), 0x1234, "the segment landed in DI");

    // `8E` with a displacement reads the word through the effective address,
    // and the segment register is the destination: `8E 06 34 12` is
    // `MOV ES,[1234h]`.
    let mut c = machine(&[0x8E, 0x06, 0x34, 0x01]);
    c.mem.write_u8(DATA, 0x0134, 0x78);
    c.mem.write_u8(DATA, 0x0135, 0x56);
    c.step()
        .expect("the 8086 runs MOV Sseg,r/m16 with a displacement");
    assert_eq!(c.regs.segment(Seg::Es), 0x5678, "the word landed in ES");
}
