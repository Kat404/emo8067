//! How many bytes each instruction consumes.
//!
//! Nothing in this repository measured that. `every_first_opcode_byte_the_
//! 8086_defines_decodes` proves a byte is not a hole, and `every_modrm_digit_
//! the_8086_is_an_instruction_or_a_named_hole` proves a `/digit` is an
//! instruction or a named one. Neither looks at bytes consumed, which is where
//! a real program quietly stops meaning anything: an inverted `disp16`, a `BD`
//! where `BE` belonged, a program one byte short so a `jmp` landed past its end.
//! All three happened while writing the program corpus, and none of them failed
//! a test — they failed to mean what they said.
//!
//! # The table is a claim, not a computation
//!
//! Every row states the length the manual's operand format implies, written out
//! by hand from `documentation-guide.md` §1.3. The test feeds the encoding and
//! asserts the emulator consumed exactly that. The two sides are deliberately
//! not derived from each other: if `length` were computed from the bytes, or the
//! bytes built from `length`, the gate would be a tautology that can only ever
//! agree with itself.
//!
//! So a disagreement between this table and the emulator is a **finding**, not a
//! nuisance. It is resolved by reading the manual, not by editing whichever side
//! is inconvenient.
//!
//! `Components` breaks each declared length into the parts that make it up, and
//! that breakdown *is* checked, because otherwise a typo in `length` would hide
//! inside a correct sum: the opcode byte, a ModR/M byte when the encoding has a
//! register-or-memory operand, and the immediate when the format names one.
//!
//! This file is Phase 1 of the QA hardening plan in
//! `odd/tasks/decode-geometry-gate.md`: the immediates, at MOD=00, and the
//! displacements, at MOD=01 and MOD=10. The execution half — that every
//! defined encoding retires and touches only what it is documented to touch —
//! is the third.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg16, Seg};

/// The parts a declared length is made of. The gate checks that these sum to
/// `length`, so a mistyped length cannot hide behind a correct arithmetic.
#[derive(Clone, Copy)]
struct Components {
    /// The opcode byte itself.
    opcode: u8,
    /// A ModR/M byte, for the formats written `r/m` or `r16/m16`.
    modrm: bool,
    /// Displacement bytes: 0, 1 for the `disp8` MOD=01 carries, 2 for the
    /// `disp16` MOD=10 carries.
    disp: u8,
    /// Immediate bytes: 0, 1 for `i8`, 2 for `i16`.
    imm: u8,
}

impl Components {
    const fn imm(opcode: u8, imm: u8) -> Self {
        Self {
            opcode,
            modrm: false,
            disp: 0,
            imm,
        }
    }
    const fn modrm_imm(opcode: u8, imm: u8) -> Self {
        Self {
            opcode,
            modrm: true,
            disp: 0,
            imm,
        }
    }
    const fn address(opcode: u8) -> Self {
        // `A0`..`A3` have no ModR/M byte: the operand is a direct address,
        // which occupies two bytes exactly like an `i16` does.
        Self {
            opcode,
            modrm: false,
            disp: 0,
            imm: 2,
        }
    }
    /// A ModR/M byte and a displacement, with no immediate.
    const fn modrm_disp(opcode: u8, disp: u8) -> Self {
        Self {
            opcode,
            modrm: true,
            disp,
            imm: 0,
        }
    }
    /// A ModR/M byte, a displacement and an immediate.
    const fn modrm_disp_imm(opcode: u8, disp: u8, imm: u8) -> Self {
        Self {
            opcode,
            modrm: true,
            disp,
            imm,
        }
    }

    const fn total(&self) -> u8 {
        1 + if self.modrm { 1 } else { 0 } + self.disp + self.imm
    }
}

/// What an instruction does to `IP`, which decides whether its length is
/// observable at all.
///
/// A plain instruction leaves `IP` just past its last byte, so `next_ip - at`
/// *is* its length. A relative jump moves `IP` by its length plus a signed
/// displacement, so the length still comes out once the displacement is
/// accounted for — and accounting for it is what catches a `JMP short` that
/// reads a `disp16`, which is exactly the mutant that survived the first
/// version of this file. `INT` and far `CALL` push the byte just past the
/// instruction, so the stack says the length instead.
#[derive(Clone, Copy, PartialEq)]
enum Ip {
    /// `IP` lands just past the instruction.
    Next,
    /// A relative jump whose signed displacement is stated here, so the
    /// expected `IP` is `at + length + displacement`.
    Relative(i16),
    /// The instruction pushes `at + length` as a return address and transfers
    /// control to `ip` in segment `cs`, or leaves `CS` alone when `cs` is
    /// `None`.
    ///
    /// The destination is part of the claim, and without it this arm is not a
    /// length check at all. A decoder that reads a `disp8` and then nudges `IP`
    /// one byte further forward leaves the pushed return address exactly where
    /// it should be — the right address, returned to — while calling the wrong
    /// target and skipping a byte of the program. The audit found that mutant
    /// passing the whole suite, and the only thing that distinguishes it is
    /// where control ends up.
    PushesReturn(u16, Option<u16>),
    /// RET pops its return address from the stack and then adds its immediate
    /// to `SP`, so how far the stack moved says whether the immediate was
    /// fetched and what it was. The payload is the immediate's **value**, not
    /// its width: the width is what `length` already claims, and a return
    /// exposes the value where a forward instruction exposes the width.
    PopsReturn(u16),
}

/// One encoding: the bytes to feed it, the parts it is made of, and the length
/// the manual says that is.
struct Encoding {
    /// What Table 4-13 calls this, for the failure message.
    name: &'static str,
    /// The instruction as it appears in memory.
    code: &'static [u8],
    /// The manual's format, from §1.3.
    parts: Components,
    /// Bytes consumed, stated independently of `code.len()`.
    length: u8,
    /// How this instruction moves `IP`.
    ip: Ip,
}

/// Every encoding that carries an immediate and no displacement, taken from the
/// opcode map in `documentation-guide.md` §1.3. MOD is `00` wherever a ModR/M
/// byte appears, so `rm = 111` names `[BX]` and no displacement follows.
///
/// `EA` is deliberately absent. A far `JMP` loads its pointer and leaves nothing
/// behind recording how many bytes it read, so there is no observation that
/// distinguishes a five-byte read from a six-byte one. Its length is pinned
/// instead by its `9A` sibling, which pushes the byte just past itself, and by
/// `jmp_far_loads_the_pointer_and_pushes_nothing` in the unit tests.
const ENCODINGS: &[Encoding] = &[
    // ---- the accumulator against an immediate, 00..3F columns 4 and 5
    Encoding::new(
        "ADD AL,i8",
        &[0x04, 0x01],
        Components::imm(0x04, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "ADD AX,i16",
        &[0x05, 0x34, 0x12],
        Components::imm(0x05, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "OR AL,i8",
        &[0x0C, 0x01],
        Components::imm(0x0C, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "OR AX,i16",
        &[0x0D, 0x34, 0x12],
        Components::imm(0x0D, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "ADC AL,i8",
        &[0x14, 0x01],
        Components::imm(0x14, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "ADC AX,i16",
        &[0x15, 0x34, 0x12],
        Components::imm(0x15, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "SBB AL,i8",
        &[0x1C, 0x01],
        Components::imm(0x1C, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "SBB AX,i16",
        &[0x1D, 0x34, 0x12],
        Components::imm(0x1D, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "AND AL,i8",
        &[0x24, 0x01],
        Components::imm(0x24, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "AND AX,i16",
        &[0x25, 0x34, 0x12],
        Components::imm(0x25, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "SUB AL,i8",
        &[0x2C, 0x01],
        Components::imm(0x2C, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "SUB AX,i16",
        &[0x2D, 0x34, 0x12],
        Components::imm(0x2D, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "XOR AL,i8",
        &[0x34, 0x01],
        Components::imm(0x34, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "XOR AX,i16",
        &[0x35, 0x34, 0x12],
        Components::imm(0x35, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "CMP AL,i8",
        &[0x3C, 0x01],
        Components::imm(0x3C, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "CMP AX,i16",
        &[0x3D, 0x34, 0x12],
        Components::imm(0x3D, 2),
        3,
        Ip::Next,
    ),
    // ---- Group 1 against an immediate. `83`'s immediate is DATA-SX: an i8
    // sign-extended to a word, so it is one byte on the wire, not two. That is
    // the single easiest length to get wrong in this table.
    Encoding::new(
        "80 /0 ADD r/m8,i8",
        &[0x80, 0x07, 0x01],
        Components::modrm_imm(0x80, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "80 /1 OR r/m8,i8",
        &[0x80, 0x0F, 0x01],
        Components::modrm_imm(0x80, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "80 /4 AND r/m8,i8",
        &[0x80, 0x27, 0x01],
        Components::modrm_imm(0x80, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "80 /7 CMP r/m8,i8",
        &[0x80, 0x3F, 0x01],
        Components::modrm_imm(0x80, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "81 /0 ADD r/m16,i16",
        &[0x81, 0x07, 0x34, 0x12],
        Components::modrm_imm(0x81, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "81 /1 OR r/m16,i16",
        &[0x81, 0x0F, 0x34, 0x12],
        Components::modrm_imm(0x81, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "81 /4 AND r/m16,i16",
        &[0x81, 0x27, 0x34, 0x12],
        Components::modrm_imm(0x81, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "81 /7 CMP r/m16,i16",
        &[0x81, 0x3F, 0x34, 0x12],
        Components::modrm_imm(0x81, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "82 /0 ADD r/m8,i8",
        &[0x82, 0x07, 0x01],
        Components::modrm_imm(0x82, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "83 /0 ADD r/m16,i8:sx",
        &[0x83, 0x07, 0x01],
        Components::modrm_imm(0x83, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "83 /4 AND r/m16,i8:sx",
        &[0x83, 0x27, 0x01],
        Components::modrm_imm(0x83, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "83 /7 CMP r/m16,i8:sx",
        &[0x83, 0x3F, 0x01],
        Components::modrm_imm(0x83, 1),
        3,
        Ip::Next,
    ),
    // ---- the accumulator against a direct address, which occupies the same
    // two bytes an `i16` would and has no ModR/M byte at all
    Encoding::new(
        "A0 MOV AL,m8",
        &[0xA0, 0x00, 0x02],
        Components::address(0xA0),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "A1 MOV AX,m16",
        &[0xA1, 0x00, 0x02],
        Components::address(0xA1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "A2 MOV m8,AL",
        &[0xA2, 0x00, 0x02],
        Components::address(0xA2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "A3 MOV m16,AX",
        &[0xA3, 0x00, 0x02],
        Components::address(0xA3),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "A8 TEST AL,i8",
        &[0xA8, 0x01],
        Components::imm(0xA8, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "A9 TEST AX,i16",
        &[0xA9, 0x34, 0x12],
        Components::imm(0xA9, 2),
        3,
        Ip::Next,
    ),
    // ---- register loads: B0..B7 one immediate byte, B8..BF two
    Encoding::new(
        "B0 MOV AL,i8",
        &[0xB0, 0x42],
        Components::imm(0xB0, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "B7 MOV BH,i8",
        &[0xB7, 0x42],
        Components::imm(0xB7, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "B8 MOV AX,i16",
        &[0xB8, 0x34, 0x12],
        Components::imm(0xB8, 2),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "BF MOV BX,i16",
        &[0xBF, 0x34, 0x12],
        Components::imm(0xBF, 2),
        3,
        Ip::Next,
    ),
    // ---- C2 carries a pop value, C6/C7 carry an immediate to memory, and CD
    // carries the interrupt type. CD pushes its return address, so its length is
    // observable on the stack rather than in `IP`.
    Encoding::new(
        // RET releases whatever the immediate says. The immediate here is
        // 0102h and not 0004h on purpose: its low byte and its whole value
        // differ, so a decoder that read only the low byte would release 2
        // instead of 258 and the stack would land somewhere else. An immediate
        // whose low byte equals its value cannot tell a two-byte read from a
        // one-byte one, which is exactly how the first version of this row
        // measured nothing.
        "C2 RET i16",
        &[0xC2, 0x02, 0x01],
        Components::imm(0xC2, 2),
        3,
        Ip::PopsReturn(0x0102),
    ),
    // The far return carries no immediate, so its one byte is pinned by the
    // stack moving four rather than two.
    Encoding::new(
        "CB RETF",
        &[0xCB],
        Components::imm(0xCB, 0),
        1,
        Ip::PopsReturn(0),
    ),
    Encoding::new(
        "C6 /0 MOV r/m8,i8",
        &[0xC6, 0x07, 0x42],
        Components::modrm_imm(0xC6, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "C7 /0 MOV r/m16,i16",
        &[0xC7, 0x07, 0x34, 0x12],
        Components::modrm_imm(0xC7, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "CD INT i8",
        &[0xCD, 0x21],
        Components::imm(0xCD, 1),
        2,
        // The handler this test installs for type 21h.
        Ip::PushesReturn(0x0500, Some(0x4000)),
    ),
    // ---- the base for AAM and AAD is an immediate, and it is not always ten
    Encoding::new(
        "D4 AAM i8",
        &[0xD4, 0x0A],
        Components::imm(0xD4, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "D5 AAD i8",
        &[0xD5, 0x0A],
        Components::imm(0xD5, 1),
        2,
        Ip::Next,
    ),
    // ---- port I/O with an immediate port
    Encoding::new(
        "E4 IN AL,i8",
        &[0xE4, 0x60],
        Components::imm(0xE4, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "E5 IN AX,i8",
        &[0xE5, 0x60],
        Components::imm(0xE5, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "E6 OUT i8,AL",
        &[0xE6, 0x60],
        Components::imm(0xE6, 1),
        2,
        Ip::Next,
    ),
    Encoding::new(
        "E7 OUT i8,AX",
        &[0xE7, 0x60],
        Components::imm(0xE7, 1),
        2,
        Ip::Next,
    ),
    // ---- the relative and far transfers. E9 and EB move IP by their length
    // plus a displacement, so the length is only checkable once the
    // displacement is accounted for.
    Encoding::new(
        "E8 CALL near",
        // The immediate is 0103h and not 0003h, and the high byte being
        // non-zero is the whole point. With a zero high byte, a decoder that
        // read only the low byte and then nudged IP forward lands on exactly
        // the same target, pushes the same return address and ends at the same
        // place: an equivalent mutant, which no observation can distinguish.
        // The audit called that a blocker and it is worth being precise: the
        // gate was not fooled, the row simply could not tell the two apart.
        &[0xE8, 0x03, 0x01],
        Components::imm(0xE8, 2),
        3,
        // rel 0103h from the byte after the instruction, and a near call
        // leaves CS alone.
        Ip::PushesReturn(0x0106, None),
    ),
    Encoding::new(
        "E9 JMP near",
        &[0xE9, 0x03, 0x00],
        Components::imm(0xE9, 2),
        3,
        Ip::Relative(3),
    ),
    Encoding::new(
        "EB JMP short",
        &[0xEB, 0xFE],
        Components::imm(0xEB, 1),
        2,
        Ip::Relative(-2),
    ),
    // A far pointer is two words, the offset first, and `9A` pushes the byte
    // just past all five, which is what pins its length.
    Encoding::new(
        "9A CALL far",
        &[0x9A, 0x00, 0x02, 0x00, 0x10],
        Components::imm(0x9A, 4),
        5,
        Ip::PushesReturn(0x0200, Some(0x1000)),
    ),
    // ---- F6 and F7 at /0: TEST against an immediate
    Encoding::new(
        "F6 /0 TEST r/m8,i8",
        &[0xF6, 0x07, 0x01],
        Components::modrm_imm(0xF6, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "F7 /0 TEST r/m16,i16",
        &[0xF7, 0x07, 0x34, 0x12],
        Components::modrm_imm(0xF7, 2),
        4,
        Ip::Next,
    ),
];

/// Every encoding that carries a displacement, taken from the same opcode map
/// in `documentation-guide.md` §1.3.
///
/// Table 4-8 says MOD=01 carries an 8-bit displacement and MOD=10 a 16-bit
/// one, and Table 4-10 says the immediate follows the displacement, never the
/// other way round. The rows use `rm = 111`, so the operand is `[BX]` plus the
/// displacement, except for the last pair, which uses `rm = 110` and is
/// therefore `[BP]`. `rm = 111` and not `rm = 011` is not a detail: Table 4-10
/// gives `011` as `[BP+DI]`, and the rows above already use `rm = 111` for
/// their `[BX]`.
///
/// The ALU groups are the eight operations of `00`..`3D`, each in its two
/// memory forms: `r/m,r` and `r,r/m`. The `reg` field names a register in
/// both, the source in the first and the destination in the second, and the
/// rows use `reg = 000` — `AL` or `AX` — for both.
///
/// `FF /2` through `FF /5` are deliberately absent. An indirect `CALL` or
/// `JMP` leaves no trace of how many bytes it read, exactly like `EA`, which
/// the table above excludes for the same reason. The near `CALL` could be
/// measured through the return address it pushes, but that needs a per-row
/// memory setup this file does not have yet.
const DISPLACEMENT_ENCODINGS: &[Encoding] = &[
    // ---- the eight ALU groups, each in its two memory forms
    Encoding::new(
        "00 ADD r/m8,r8 [BX+disp8]",
        &[0x00, 0x47, 0x40],
        Components::modrm_disp(0x00, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "00 ADD r/m8,r8 [BX+disp16]",
        &[0x00, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x00, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "01 ADD r/m16,r16 [BX+disp8]",
        &[0x01, 0x47, 0x40],
        Components::modrm_disp(0x01, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "01 ADD r/m16,r16 [BX+disp16]",
        &[0x01, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x01, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "02 ADD r8,r/m8 [BX+disp8]",
        &[0x02, 0x47, 0x40],
        Components::modrm_disp(0x02, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "02 ADD r8,r/m8 [BX+disp16]",
        &[0x02, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x02, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "03 ADD r16,r/m16 [BX+disp8]",
        &[0x03, 0x47, 0x40],
        Components::modrm_disp(0x03, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "03 ADD r16,r/m16 [BX+disp16]",
        &[0x03, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x03, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "08 OR r/m8,r8 [BX+disp8]",
        &[0x08, 0x47, 0x40],
        Components::modrm_disp(0x08, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "08 OR r/m8,r8 [BX+disp16]",
        &[0x08, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x08, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "09 OR r/m16,r16 [BX+disp8]",
        &[0x09, 0x47, 0x40],
        Components::modrm_disp(0x09, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "09 OR r/m16,r16 [BX+disp16]",
        &[0x09, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x09, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "0A OR r8,r/m8 [BX+disp8]",
        &[0x0A, 0x47, 0x40],
        Components::modrm_disp(0x0A, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "0A OR r8,r/m8 [BX+disp16]",
        &[0x0A, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x0A, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "0B OR r16,r/m16 [BX+disp8]",
        &[0x0B, 0x47, 0x40],
        Components::modrm_disp(0x0B, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "0B OR r16,r/m16 [BX+disp16]",
        &[0x0B, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x0B, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "10 ADC r/m8,r8 [BX+disp8]",
        &[0x10, 0x47, 0x40],
        Components::modrm_disp(0x10, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "10 ADC r/m8,r8 [BX+disp16]",
        &[0x10, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x10, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "11 ADC r/m16,r16 [BX+disp8]",
        &[0x11, 0x47, 0x40],
        Components::modrm_disp(0x11, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "11 ADC r/m16,r16 [BX+disp16]",
        &[0x11, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x11, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "12 ADC r8,r/m8 [BX+disp8]",
        &[0x12, 0x47, 0x40],
        Components::modrm_disp(0x12, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "12 ADC r8,r/m8 [BX+disp16]",
        &[0x12, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x12, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "13 ADC r16,r/m16 [BX+disp8]",
        &[0x13, 0x47, 0x40],
        Components::modrm_disp(0x13, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "13 ADC r16,r/m16 [BX+disp16]",
        &[0x13, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x13, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "18 SBB r/m8,r8 [BX+disp8]",
        &[0x18, 0x47, 0x40],
        Components::modrm_disp(0x18, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "18 SBB r/m8,r8 [BX+disp16]",
        &[0x18, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x18, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "19 SBB r/m16,r16 [BX+disp8]",
        &[0x19, 0x47, 0x40],
        Components::modrm_disp(0x19, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "19 SBB r/m16,r16 [BX+disp16]",
        &[0x19, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x19, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "1A SBB r8,r/m8 [BX+disp8]",
        &[0x1A, 0x47, 0x40],
        Components::modrm_disp(0x1A, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "1A SBB r8,r/m8 [BX+disp16]",
        &[0x1A, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x1A, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "1B SBB r16,r/m16 [BX+disp8]",
        &[0x1B, 0x47, 0x40],
        Components::modrm_disp(0x1B, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "1B SBB r16,r/m16 [BX+disp16]",
        &[0x1B, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x1B, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "20 AND r/m8,r8 [BX+disp8]",
        &[0x20, 0x47, 0x40],
        Components::modrm_disp(0x20, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "20 AND r/m8,r8 [BX+disp16]",
        &[0x20, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x20, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "21 AND r/m16,r16 [BX+disp8]",
        &[0x21, 0x47, 0x40],
        Components::modrm_disp(0x21, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "21 AND r/m16,r16 [BX+disp16]",
        &[0x21, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x21, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "22 AND r8,r/m8 [BX+disp8]",
        &[0x22, 0x47, 0x40],
        Components::modrm_disp(0x22, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "22 AND r8,r/m8 [BX+disp16]",
        &[0x22, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x22, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "23 AND r16,r/m16 [BX+disp8]",
        &[0x23, 0x47, 0x40],
        Components::modrm_disp(0x23, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "23 AND r16,r/m16 [BX+disp16]",
        &[0x23, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x23, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "28 SUB r/m8,r8 [BX+disp8]",
        &[0x28, 0x47, 0x40],
        Components::modrm_disp(0x28, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "28 SUB r/m8,r8 [BX+disp16]",
        &[0x28, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x28, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "29 SUB r/m16,r16 [BX+disp8]",
        &[0x29, 0x47, 0x40],
        Components::modrm_disp(0x29, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "29 SUB r/m16,r16 [BX+disp16]",
        &[0x29, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x29, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "2A SUB r8,r/m8 [BX+disp8]",
        &[0x2A, 0x47, 0x40],
        Components::modrm_disp(0x2A, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "2A SUB r8,r/m8 [BX+disp16]",
        &[0x2A, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x2A, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "2B SUB r16,r/m16 [BX+disp8]",
        &[0x2B, 0x47, 0x40],
        Components::modrm_disp(0x2B, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "2B SUB r16,r/m16 [BX+disp16]",
        &[0x2B, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x2B, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "30 XOR r/m8,r8 [BX+disp8]",
        &[0x30, 0x47, 0x40],
        Components::modrm_disp(0x30, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "30 XOR r/m8,r8 [BX+disp16]",
        &[0x30, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x30, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "31 XOR r/m16,r16 [BX+disp8]",
        &[0x31, 0x47, 0x40],
        Components::modrm_disp(0x31, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "31 XOR r/m16,r16 [BX+disp16]",
        &[0x31, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x31, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "32 XOR r8,r/m8 [BX+disp8]",
        &[0x32, 0x47, 0x40],
        Components::modrm_disp(0x32, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "32 XOR r8,r/m8 [BX+disp16]",
        &[0x32, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x32, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "33 XOR r16,r/m16 [BX+disp8]",
        &[0x33, 0x47, 0x40],
        Components::modrm_disp(0x33, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "33 XOR r16,r/m16 [BX+disp16]",
        &[0x33, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x33, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "38 CMP r/m8,r8 [BX+disp8]",
        &[0x38, 0x47, 0x40],
        Components::modrm_disp(0x38, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "38 CMP r/m8,r8 [BX+disp16]",
        &[0x38, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x38, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "39 CMP r/m16,r16 [BX+disp8]",
        &[0x39, 0x47, 0x40],
        Components::modrm_disp(0x39, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "39 CMP r/m16,r16 [BX+disp16]",
        &[0x39, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x39, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "3A CMP r8,r/m8 [BX+disp8]",
        &[0x3A, 0x47, 0x40],
        Components::modrm_disp(0x3A, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "3A CMP r8,r/m8 [BX+disp16]",
        &[0x3A, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x3A, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "3B CMP r16,r/m16 [BX+disp8]",
        &[0x3B, 0x47, 0x40],
        Components::modrm_disp(0x3B, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "3B CMP r16,r/m16 [BX+disp16]",
        &[0x3B, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x3B, 2),
        4,
        Ip::Next,
    ),
    // ---- Group 1 against an immediate. `83`'s immediate is DATA-SX: one byte
    // on the wire, and the displacement comes before it.
    Encoding::new(
        "80 /0 ADD r/m8,i8 [BX+disp8]",
        &[0x80, 0x47, 0x40, 0x01],
        Components::modrm_disp_imm(0x80, 1, 1),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "80 /0 ADD r/m8,i8 [BX+disp16]",
        &[0x80, 0x87, 0x34, 0x12, 0x01],
        Components::modrm_disp_imm(0x80, 2, 1),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "81 /0 ADD r/m16,i16 [BX+disp8]",
        &[0x81, 0x47, 0x40, 0x34, 0x12],
        Components::modrm_disp_imm(0x81, 1, 2),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "81 /0 ADD r/m16,i16 [BX+disp16]",
        &[0x81, 0x87, 0x34, 0x12, 0x34, 0x12],
        Components::modrm_disp_imm(0x81, 2, 2),
        6,
        Ip::Next,
    ),
    Encoding::new(
        "82 /0 ADD r/m8,i8 [BX+disp8]",
        &[0x82, 0x47, 0x40, 0x01],
        Components::modrm_disp_imm(0x82, 1, 1),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "82 /0 ADD r/m8,i8 [BX+disp16]",
        &[0x82, 0x87, 0x34, 0x12, 0x01],
        Components::modrm_disp_imm(0x82, 2, 1),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "83 /0 ADD r/m16,i8:sx [BX+disp8]",
        &[0x83, 0x47, 0x40, 0x01],
        Components::modrm_disp_imm(0x83, 1, 1),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "83 /0 ADD r/m16,i8:sx [BX+disp16]",
        &[0x83, 0x87, 0x34, 0x12, 0x01],
        Components::modrm_disp_imm(0x83, 2, 1),
        5,
        Ip::Next,
    ),
    // ---- the register forms of TEST, XCHG and MOV, and LEA
    Encoding::new(
        "84 TEST r/m8,r8 [BX+disp8]",
        &[0x84, 0x47, 0x40],
        Components::modrm_disp(0x84, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "84 TEST r/m8,r8 [BX+disp16]",
        &[0x84, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x84, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "85 TEST r/m16,r16 [BX+disp8]",
        &[0x85, 0x47, 0x40],
        Components::modrm_disp(0x85, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "85 TEST r/m16,r16 [BX+disp16]",
        &[0x85, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x85, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "86 XCHG r/m8,r8 [BX+disp8]",
        &[0x86, 0x47, 0x40],
        Components::modrm_disp(0x86, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "86 XCHG r/m8,r8 [BX+disp16]",
        &[0x86, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x86, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "87 XCHG r/m16,r16 [BX+disp8]",
        &[0x87, 0x47, 0x40],
        Components::modrm_disp(0x87, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "87 XCHG r/m16,r16 [BX+disp16]",
        &[0x87, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x87, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "88 MOV r/m8,r8 [BX+disp8]",
        &[0x88, 0x47, 0x40],
        Components::modrm_disp(0x88, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "88 MOV r/m8,r8 [BX+disp16]",
        &[0x88, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x88, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "89 MOV r/m16,r16 [BX+disp8]",
        &[0x89, 0x47, 0x40],
        Components::modrm_disp(0x89, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "89 MOV r/m16,r16 [BX+disp16]",
        &[0x89, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x89, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "8A MOV r8,r/m8 [BX+disp8]",
        &[0x8A, 0x47, 0x40],
        Components::modrm_disp(0x8A, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "8A MOV r8,r/m8 [BX+disp16]",
        &[0x8A, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x8A, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "8B MOV r16,r/m16 [BX+disp8]",
        &[0x8B, 0x47, 0x40],
        Components::modrm_disp(0x8B, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "8B MOV r16,r/m16 [BX+disp16]",
        &[0x8B, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x8B, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "8D LEA r16,r/m16 [BX+disp8]",
        &[0x8D, 0x47, 0x40],
        Components::modrm_disp(0x8D, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "8D LEA r16,r/m16 [BX+disp16]",
        &[0x8D, 0x87, 0x34, 0x12],
        Components::modrm_disp(0x8D, 2),
        4,
        Ip::Next,
    ),
    // ---- an immediate into memory
    Encoding::new(
        "C6 /0 MOV r/m8,i8 [BX+disp8]",
        &[0xC6, 0x47, 0x40, 0x42],
        Components::modrm_disp_imm(0xC6, 1, 1),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "C6 /0 MOV r/m8,i8 [BX+disp16]",
        &[0xC6, 0x87, 0x34, 0x12, 0x42],
        Components::modrm_disp_imm(0xC6, 2, 1),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "C7 /0 MOV r/m16,i16 [BX+disp8]",
        &[0xC7, 0x47, 0x40, 0x34, 0x12],
        Components::modrm_disp_imm(0xC7, 1, 2),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "C7 /0 MOV r/m16,i16 [BX+disp16]",
        &[0xC7, 0x87, 0x34, 0x12, 0x34, 0x12],
        Components::modrm_disp_imm(0xC7, 2, 2),
        6,
        Ip::Next,
    ),
    // ---- the shift group, at /4, which is SHL. The digit only names the
    // operation; the length is the same for all of them.
    Encoding::new(
        "D0 /4 SHL r/m8,1 [BX+disp8]",
        &[0xD0, 0x67, 0x40],
        Components::modrm_disp(0xD0, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "D0 /4 SHL r/m8,1 [BX+disp16]",
        &[0xD0, 0xA7, 0x34, 0x12],
        Components::modrm_disp(0xD0, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "D1 /4 SHL r/m16,1 [BX+disp8]",
        &[0xD1, 0x67, 0x40],
        Components::modrm_disp(0xD1, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "D1 /4 SHL r/m16,1 [BX+disp16]",
        &[0xD1, 0xA7, 0x34, 0x12],
        Components::modrm_disp(0xD1, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "D2 /4 SHL r/m8,CL [BX+disp8]",
        &[0xD2, 0x67, 0x40],
        Components::modrm_disp(0xD2, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "D2 /4 SHL r/m8,CL [BX+disp16]",
        &[0xD2, 0xA7, 0x34, 0x12],
        Components::modrm_disp(0xD2, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "D3 /4 SHL r/m16,CL [BX+disp8]",
        &[0xD3, 0x67, 0x40],
        Components::modrm_disp(0xD3, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "D3 /4 SHL r/m16,CL [BX+disp16]",
        &[0xD3, 0xA7, 0x34, 0x12],
        Components::modrm_disp(0xD3, 2),
        4,
        Ip::Next,
    ),
    // ---- Group 3. `/0` carries an immediate and `/2` and `/3` carry none.
    Encoding::new(
        "F6 /0 TEST r/m8,i8 [BX+disp8]",
        &[0xF6, 0x47, 0x40, 0x01],
        Components::modrm_disp_imm(0xF6, 1, 1),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "F6 /0 TEST r/m8,i8 [BX+disp16]",
        &[0xF6, 0x87, 0x34, 0x12, 0x01],
        Components::modrm_disp_imm(0xF6, 2, 1),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "F6 /2 NOT r/m8 [BX+disp8]",
        &[0xF6, 0x57, 0x40],
        Components::modrm_disp(0xF6, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "F6 /2 NOT r/m8 [BX+disp16]",
        &[0xF6, 0x97, 0x34, 0x12],
        Components::modrm_disp(0xF6, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "F7 /0 TEST r/m16,i16 [BX+disp8]",
        &[0xF7, 0x47, 0x40, 0x34, 0x12],
        Components::modrm_disp_imm(0xF7, 1, 2),
        5,
        Ip::Next,
    ),
    Encoding::new(
        "F7 /0 TEST r/m16,i16 [BX+disp16]",
        &[0xF7, 0x87, 0x34, 0x12, 0x34, 0x12],
        Components::modrm_disp_imm(0xF7, 2, 2),
        6,
        Ip::Next,
    ),
    Encoding::new(
        "F7 /3 NEG r/m16 [BX+disp8]",
        &[0xF7, 0x5F, 0x40],
        Components::modrm_disp(0xF7, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "F7 /3 NEG r/m16 [BX+disp16]",
        &[0xF7, 0x9F, 0x34, 0x12],
        Components::modrm_disp(0xF7, 2),
        4,
        Ip::Next,
    ),
    // ---- the byte and word INC and DEC, and the word PUSH
    Encoding::new(
        "FE /0 INC r/m8 [BX+disp8]",
        &[0xFE, 0x47, 0x40],
        Components::modrm_disp(0xFE, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "FE /0 INC r/m8 [BX+disp16]",
        &[0xFE, 0x87, 0x34, 0x12],
        Components::modrm_disp(0xFE, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "FE /1 DEC r/m8 [BX+disp8]",
        &[0xFE, 0x4F, 0x40],
        Components::modrm_disp(0xFE, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "FE /1 DEC r/m8 [BX+disp16]",
        &[0xFE, 0x8F, 0x34, 0x12],
        Components::modrm_disp(0xFE, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "FF /0 INC r/m16 [BX+disp8]",
        &[0xFF, 0x47, 0x40],
        Components::modrm_disp(0xFF, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "FF /0 INC r/m16 [BX+disp16]",
        &[0xFF, 0x87, 0x34, 0x12],
        Components::modrm_disp(0xFF, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "FF /1 DEC r/m16 [BX+disp8]",
        &[0xFF, 0x4F, 0x40],
        Components::modrm_disp(0xFF, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "FF /1 DEC r/m16 [BX+disp16]",
        &[0xFF, 0x8F, 0x34, 0x12],
        Components::modrm_disp(0xFF, 2),
        4,
        Ip::Next,
    ),
    Encoding::new(
        "FF /6 PUSH r/m16 [BX+disp8]",
        &[0xFF, 0x77, 0x40],
        Components::modrm_disp(0xFF, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "FF /6 PUSH r/m16 [BX+disp16]",
        &[0xFF, 0xB7, 0x34, 0x12],
        Components::modrm_disp(0xFF, 2),
        4,
        Ip::Next,
    ),
    // ---- `rm = 110` is `[BP]`, and at MOD=01 it carries one displacement
    // byte. The word-sized displacement at `rm = 110` is the direct address of
    // `mod=00, rm=110`, and a decoder that mistook this one for it would
    // consume a byte too many.
    Encoding::new(
        "00 ADD r/m8,r8 [BP+disp8]",
        &[0x00, 0x46, 0x40],
        Components::modrm_disp(0x00, 1),
        3,
        Ip::Next,
    ),
    Encoding::new(
        "01 ADD r/m16,r16 [BP+disp8]",
        &[0x01, 0x46, 0x40],
        Components::modrm_disp(0x01, 1),
        3,
        Ip::Next,
    ),
];

impl Encoding {
    const fn new(
        name: &'static str,
        code: &'static [u8],
        parts: Components,
        length: u8,
        ip: Ip,
    ) -> Self {
        Self {
            name,
            code,
            parts,
            length,
            ip,
        }
    }
}

fn machine(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ds, 0x1000);
    c.regs.set_segment(Seg::Es, 0x2000);
    c.regs.set_segment(Seg::Ss, 0x1000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

/// Point vector `n` at a handler, the way the interrupt table works: the offset
/// word first and the segment word second.
fn vector(c: &mut Cpu, n: u8, segment: u16, offset: u16) {
    c.mem.write_u16(0x0000, u16::from(n) * 4, offset);
    c.mem.write_u16(0x0000, u16::from(n) * 4 + 2, segment);
}

#[test]
fn every_encoding_consumes_exactly_what_the_manual_says() {
    // Pinned exactly, not as a lower bound. A lower bound cannot notice rows
    // being deleted; the whole point of the table is that it is the project's
    // record of what exists, so a row removed from it must fail. The number
    // below was itself written as a guess first and the gate caught it, which
    // is the cheapest possible demonstration of why it is pinned rather than
    // bounded.
    assert_eq!(
        ENCODINGS.len(),
        55,
        "a deleted row means the record is now wrong, and the gate must say so"
    );
    assert_eq!(
        DISPLACEMENT_ENCODINGS.len(),
        122,
        "a deleted row means the record is now wrong, and the gate must say so"
    );
    assert_eq!(
        ENCODINGS.len() + DISPLACEMENT_ENCODINGS.len(),
        177,
        "the two tables together are the record of what exists"
    );
    // And every row must be findable by its label, or a failure names nothing.
    for (i, encoding) in ENCODINGS.iter().chain(DISPLACEMENT_ENCODINGS).enumerate() {
        assert!(
            !encoding.name.is_empty(),
            "row {i} has no label to name it by in a failure"
        );
    }
    for encoding in ENCODINGS.iter().chain(DISPLACEMENT_ENCODINGS) {
        let label = encoding.name;
        // The breakdown has to add up to the length, or a typo in the length
        // could hide inside a correct sum.
        assert_eq!(
            encoding.parts.total(),
            encoding.length,
            "{label}: the components do not add up to the declared length"
        );
        assert_eq!(
            encoding.parts.opcode, encoding.code[0],
            "{label}: the opcode byte in `parts` is not the one in `code`"
        );

        let mut c = machine(encoding.code);
        if matches!(encoding.ip, Ip::PushesReturn(..)) {
            // INT and far CALL read their vector, so the table needs one. The
            // expected destination in each row is stated against it.
            let n = if encoding.code[0] == 0xCD {
                encoding.code[1]
            } else {
                0
            };
            vector(&mut c, n, 0x4000, 0x0500);
        }
        let stack_before = c.regs.word(Reg16::Sp);
        // A return needs something on the stack to return *to*, and it has to be
        // there before the step: RET pops whatever it finds, and an empty stack
        // hands it zeroes.
        let ret_words: u16 = if let Ip::PopsReturn(_) = encoding.ip {
            let far = encoding.code[0] == 0xCB;
            c.mem.write_u16(0x1000, stack_before, 0x0777);
            if far {
                c.mem
                    .write_u16(0x1000, stack_before.wrapping_add(2), 0x6000);
                2
            } else {
                1
            }
        } else {
            0
        };
        let info = c.step().expect("a documented encoding must execute");

        let consumed = match encoding.ip {
            Ip::Next => info.next_ip.wrapping_sub(info.at),
            Ip::Relative(rel) => info.next_ip.wrapping_sub(info.at).wrapping_sub(rel as u16),
            Ip::PopsReturn(imm) => {
                assert_eq!(
                    info.next_ip, 0x0777,
                    "{label}: it returned to the address that was on the stack"
                );
                let moved = c.regs.word(Reg16::Sp).wrapping_sub(stack_before);
                assert_eq!(
                    moved,
                    2 * ret_words + imm,
                    "{label}: the stack moved by the pops plus the {imm} it released"
                );
                // Having executed and measured is the whole assertion: the
                // immediate's width is what the stack movement just read off.
                continue;
            }
            Ip::PushesReturn(expected_ip, expected_cs) => {
                assert_eq!(
                    info.next_ip, expected_ip,
                    "{label}: it must land where its own bytes say, not merely \
                     push a correct return address"
                );
                if let Some(expected) = expected_cs {
                    assert_eq!(
                        c.regs.segment(Seg::Cs),
                        expected,
                        "{label}: and into the segment its bytes name"
                    );
                }
                // `IP` belongs to the handler now, so it says nothing about the
                // length. The pushed return address does: it is the byte just
                // past the instruction, and finding it on the stack is what
                // pins how many bytes were read. How many words go down depends
                // on the form — FLAGS, CS and IP for an interrupt, CS and IP
                // for a far call, IP alone for a near one — and the return
                // address is always the lowest of them.
                let words = match encoding.code[0] {
                    0xCD => 3,
                    0x9A => 2,
                    _ => 1,
                };
                let low = stack_before.wrapping_sub(2 * words);
                assert_eq!(
                    c.mem.read_u16(c.regs.segment(Seg::Ss), low),
                    info.at.wrapping_add(u16::from(encoding.length)),
                    "{label}: the pushed return address is the byte just past the instruction"
                );
                // There is deliberately no `consumed` value to assert against
                // here. Returning the declared length would make the shared
                // assertion below a tautology for these rows: it would compare
                // the table with itself. The pushed address is the whole
                // measurement, and it is what a decoder that read the wrong
                // number of bytes gets wrong.
                continue;
            }
        };
        assert_eq!(
            consumed,
            u16::from(encoding.length),
            "{label}: consumed {consumed} of the {} bytes the manual's format implies",
            encoding.length
        );
    }
}

/// The lengths above are a claim about the *bytes*, so this is the claim about
/// the *contents*: each immediate must have been fetched, not skipped. Feeding
/// an encoding and asserting what landed in a register catches a decoder that
/// reads the operand and forgets to advance `IP` — the failure a length check
/// alone would catch only by accident, since the byte count would still be
/// right.
#[test]
fn an_immediate_is_fetched_not_skipped() {
    let mut c = machine(&[0xB8, 0x34, 0x12]);
    c.regs.set_word(Reg16::Ax, 0x0000);
    c.step().unwrap();
    assert_eq!(c.regs.word(Reg16::Ax), 0x1234);

    // 81 07 34 12 is ADD [BX], 1234h. The memory operand is the whole two-byte
    // field here, so an off-by-one on the immediate is visible in memory.
    let mut c = machine(&[0x81, 0x07, 0x34, 0x12]);
    c.regs.set_word(Reg16::Bx, 0x0200);
    c.mem.write_u16(0x1000, 0x0200, 0x0001);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x1000, 0x0200),
        0x1235,
        "the immediate reached memory, which means it was fetched"
    );

    // And `83` really does carry one byte: 83 07 FE is ADD [BX], -2.
    let mut c = machine(&[0x83, 0x07, 0xFE]);
    c.regs.set_word(Reg16::Bx, 0x0200);
    c.mem.write_u16(0x1000, 0x0200, 0x0010);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x1000, 0x0200),
        0x000E,
        "FEh sign-extends to -2, so DATA-SX is one byte on the wire"
    );
}

/// The far pointer is the offset first and the segment second, and getting that
/// order wrong costs the right length and the wrong target. It is called out
/// separately because it is the only encoding here whose two halves are not
/// interchangeable.
#[test]
fn a_far_pointer_is_the_offset_then_the_segment() {
    let mut c = machine(&[0x9A, 0x34, 0x12, 0x78, 0x56]);
    vector(&mut c, 0, 0x4000, 0x0500);
    c.step().unwrap();
    assert_eq!(c.regs.ip, 0x1234, "the low word is the offset");
    assert_eq!(
        c.regs.segment(Seg::Cs),
        0x5678,
        "the high word is the segment"
    );
}

/// A displacement is not only a number of bytes: it is the offset the
/// instruction addresses. A decoder that read the right number of bytes and
/// used the wrong ones would pass the length check above, and an inverted
/// `disp16` is one of the defects this gate was written after.
#[test]
fn a_displacement_is_the_offset_the_operand_lands_at() {
    // `C7 43 40 34 12` is MOV WORD [BX+40h], 1234h. The displacement is the
    // 40h, and the store has to land there.
    let mut c = machine(&[0xC7, 0x47, 0x40, 0x34, 0x12]);
    c.regs.set_word(Reg16::Bx, 0x0200);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x1000, 0x0240),
        0x1234,
        "the disp8 is the offset, and the store landed there"
    );
    assert_eq!(
        c.mem.read_u16(0x1000, 0x0200),
        0x0000,
        "and not at the bare base address"
    );

    // The same move with a 16-bit displacement lands at BX+1234h, and the
    // immediate is still the one that came after it.
    let mut c = machine(&[0xC7, 0x87, 0x34, 0x12, 0x34, 0x12]);
    c.regs.set_word(Reg16::Bx, 0x0200);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x1000, 0x1434),
        0x1234,
        "the disp16 is the offset, low byte first"
    );

    // And the 8-bit displacement is sign-extended: `FE` is -2, not 254.
    let mut c = machine(&[0xC7, 0x47, 0xFE, 0x34, 0x12]);
    c.regs.set_word(Reg16::Bx, 0x0200);
    c.step().unwrap();
    assert_eq!(
        c.mem.read_u16(0x1000, 0x01FE),
        0x1234,
        "a disp8 is sign-extended, so FEh is -2 and not 254"
    );
}
