//! The flag contract, checked against `odd/research/documentation-guide.md` §1.4.
//!
//! §1.4 is a transcription of Table 2-21 of the 1979 manual, and nothing checked
//! that transcription against the emulator. So a row could claim a flag is
//! untouched while the code wrote it, or claim it is cleared while the code left
//! it alone. `A8`/`A9` did exactly that for two units, running `CMP` where the
//! row said `TEST`, and the whole suite stayed green.
//!
//! `verify_opcodes.py` cannot catch that class of error, and neither can any
//! structural gate: it checks that each row of the opcode grid has sixteen cells
//! and that each flag cell is inside the legend. Structure is all it can see.
//!
//! This reads the rows out of the document and runs one representative
//! instruction per row. Three properties are asserted, none of them depending on
//! operand values:
//!
//! - `—` means the instruction leaves that flag exactly as it found it,
//! - `0` means it leaves it cleared,
//! - `1` means it leaves it set.
//!
//! Every case runs twice, once with all nine defined flags set and once with all
//! nine cleared, so a flag written in *either* direction is caught rather than
//! only the ones that happen to be written to zero.
//!
//! **What this gate is not.** The expectations come from the document, so this
//! enforces consistency between the code and the oracle, not truth: a row
//! misread and a decoder written from the same misreading will agree, and this
//! passes. That limit is why the rows the document itself records as corrected —
//! `SHL`, `SAR`, `LDS` — carry their evidence there and not here. Replacing the
//! document with hardware captures as the oracle is the open decision in
//! `odd/HANDOFF.md`, "A second cause", option 3.
//!
//! Coverage note: `INT`/`INTO` are absent because they trap, and `IRET`/`POPF`
//! because they *restore* the flags rather than write them, so no mark in those
//! rows is assertable without modelling the stack.
//!
//! **`X` and `U` are no longer a hole.** They assert nothing *here*, because
//! `X` is value-dependent and `U` is undefined — but the hardware says what
//! those values are. `crates/core/tests/hardware_flags.rs` runs 1533 cases
//! from a real P80C86A-2 and compares every flag the chip defined, and it
//! checks the document against the chip in the other direction too: a row
//! that marks a flag with an em dash says the instruction does not touch it,
//! and the captures say whether the chip agreed. So the `X` rows are backed by
//! the chip's own numbers, and a row that claims a flag is untouched while
//! the chip writes it fails the gate.

use emo8067_core::Cpu;
use emo8067_core::flags::{CF, OF};
use emo8067_core::regs::{Reg8, Reg16, Seg};

/// The guide and its §1.4 rows are read by the shared module, so that the
/// parser exists once: two copies of it are two copies that can drift.
mod common;

use common::{COLUMNS, GUIDE, Mark, row_marks};

/// One representative execution per §1.4 row that has something assertable.
struct Case {
    row: &'static str,
    code: &'static [u8],
    /// Register and memory state the instruction needs to run at all.
    pre: fn(&mut Cpu),
}

fn none(_: &mut Cpu) {}

/// Set every architectural flag to `value`, including TF, IF and DF.
fn seed(c: &mut Cpu, value: bool) {
    for bit in COLUMNS {
        c.regs.flags.set(bit, value);
    }
}

fn machine(code: &[u8]) -> Cpu {
    let mut c = Cpu::new();
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ss, 0x1000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

#[test]
fn every_flag_the_guide_says_is_untouched_really_is() {
    let cases = cases();
    assert!(
        cases.len() >= 40,
        "the gate must stay broad: {}",
        cases.len()
    );
    for case in &cases {
        let marks = row_marks(GUIDE, case.row);
        for start_set in [true, false] {
            let mut c = machine(case.code);
            (case.pre)(&mut c);
            seed(&mut c, start_set);
            let before = c.regs.flags;
            c.step()
                .unwrap_or_else(|e| panic!("{} did not execute: {e:?}", case.row));
            let after = c.regs.flags;

            for (i, bit) in COLUMNS.iter().enumerate() {
                match marks[i] {
                    Mark::Unchanged => assert_eq!(
                        after.get(*bit),
                        start_set,
                        "{} writes a flag section 1.4 marks as untouched (flags \
                         starting {start_set}): {:?} -> {:?}",
                        case.row,
                        before,
                        after
                    ),
                    Mark::Cleared => assert!(
                        !after.get(*bit),
                        "{} leaves a flag section 1.4 says it clears",
                        case.row
                    ),
                    Mark::Set => assert!(
                        after.get(*bit),
                        "{} leaves a flag section 1.4 says it sets",
                        case.row
                    ),
                    Mark::Free => {}
                }
            }
        }
    }
}

/// Three anchors independent of the document, so a misread row cannot hide
/// behind a matching misread decoder.
#[test]
fn the_guide_and_the_chip_agree_on_the_three_rows_that_mattered_most() {
    // TEST and the logical ops clear CF and OF and say nothing about AF; CMP
    // defines all six. This is the distinction a real P80C86A-2 records, and
    // this project got it backwards for two units.
    let borrow = |code: &[u8]| {
        let mut c = machine(code);
        seed(&mut c, true);
        c.step().unwrap();
        c.regs.flags
    };
    let a8 = borrow(&[0xA8, 0x01]); // TEST AL, 01h with AL = 00h set by the seed
    assert!(
        !a8.get(CF),
        "TEST never sets CF, so AL=00h against 01h does not borrow"
    );
    assert!(!a8.get(OF));

    let mut c = machine(&[0x3C, 0x01]); // CMP AL, 01h
    seed(&mut c, true);
    c.step().unwrap();
    assert!(
        c.regs.flags.get(CF),
        "CMP AL,01h with AL=00h does borrow, which is how TEST and CMP differ"
    );

    // INC and DEC leave CF alone; ADD does not.
    let mut c = machine(&[0x40]);
    seed(&mut c, true);
    c.step().unwrap();
    assert!(c.regs.flags.get(CF), "INC must preserve CF");
    let mut c = machine(&[0x04, 0x01]);
    al_ff(&mut c);
    seed(&mut c, false);
    c.step().unwrap();
    assert!(c.regs.flags.get(CF), "ADD AL,01h with AL=FFh carries");
}

fn al(value: u8, c: &mut Cpu) {
    c.regs.set_byte(Reg8::Al, value);
}

// A closure that captures cannot become a `fn` pointer, so the handful of
// operand values the cases need are named functions rather than a constructor.
fn al_00(c: &mut Cpu) {
    al(0x00, c)
}
fn al_05(c: &mut Cpu) {
    al(0x05, c)
}
fn al_0b(c: &mut Cpu) {
    al(0x0B, c)
}
fn al_10(c: &mut Cpu) {
    al(0x10, c)
}
fn al_19(c: &mut Cpu) {
    al(0x19, c)
}
fn al_5a(c: &mut Cpu) {
    al(0x5A, c)
}
fn al_80(c: &mut Cpu) {
    al(0x80, c)
}
fn al_81(c: &mut Cpu) {
    al(0x81, c)
}
fn al_ff(c: &mut Cpu) {
    al(0xFF, c)
}

/// The 16-bit REG field is the order `Reg16`'s discriminants run in, and it
/// is **not** the order every modern reference quotes. Intel Table 4-9 of
/// the 1979 manual gives `011` to `BX` and `100` to `SP`, while the 8-bit
/// field gives `011` to `BL` and `100` to `AH`; the 80186 moved the 16-bit
/// registers to `AX CX DX SP BP SI DI BX` to make room for the high bytes,
/// and that later table is the one most references print. A real P80C86A-2
/// agrees with the manual: `MOV SP,AX` is `8B E0`.
///
/// Reading a hardware field through the wrong order permutes five of the
/// eight registers, and every test written as `set_word(Reg16::Bx, ..)` then
/// agrees with it, so nothing notices.
///
/// This gate goes through instruction bytes rather than the API, because the
/// API is where the mistake hides.
#[test]
fn the_word_register_field_names_the_register_intel_table_4_9_says() {
    // B8+r is MOV r16, imm16, and the low three bits of the opcode carry the
    // very same field a ModR/M byte would.
    let table = [
        (0xB8u8, Reg16::Ax, "AX"),
        (0xB9, Reg16::Cx, "CX"),
        (0xBA, Reg16::Dx, "DX"),
        (0xBB, Reg16::Bx, "BX"),
        (0xBC, Reg16::Sp, "SP"),
        (0xBD, Reg16::Bp, "BP"),
        (0xBE, Reg16::Si, "SI"),
        (0xBF, Reg16::Di, "DI"),
    ];
    for (opcode, want, name) in table {
        let mut c = machine(&[opcode, 0x34, 0x12]);
        seed(&mut c, false);
        c.step().unwrap();
        assert_eq!(
            c.regs.word(want),
            0x1234,
            "{opcode:#04x} is MOV {name},1234h"
        );
        for (_, other, _) in table {
            if other != want {
                assert_ne!(
                    c.regs.word(other),
                    0x1234,
                    "{opcode:#04x} must not touch {other:?}"
                );
            }
        }
    }
}

/// The same field inside a ModR/M byte. `8B /r` is `MOV r16, r/m16`, so the r/m
/// field is the source here, and a ModR/M byte is read as a *word* register
/// field because the width comes from the opcode, not the byte.
#[test]
fn the_modrm_register_operand_follows_the_same_field() {
    let table = [
        (0xC0u8, Reg16::Ax, "AX"),
        (0xC1, Reg16::Cx, "CX"),
        (0xC2, Reg16::Dx, "DX"),
        (0xC3, Reg16::Bx, "BX"),
        (0xC4, Reg16::Sp, "SP"),
        (0xC5, Reg16::Bp, "BP"),
        (0xC6, Reg16::Si, "SI"),
        (0xC7, Reg16::Di, "DI"),
    ];
    for (modrm, source, name) in table {
        let mut c = machine(&[0x8B, modrm]);
        // Registers start at zero, so writing the source after building the
        // machine is enough — including when the source is AX itself.
        c.regs.set_word(source, 0x1234);
        c.step().unwrap();
        assert_eq!(
            c.regs.word(Reg16::Ax),
            0x1234,
            "8B {modrm:#04x} is MOV AX,{name}"
        );
    }
}

fn cases() -> Vec<Case> {
    use emo8067_core::regs::Reg16::Ax;
    let string_setup = |c: &mut Cpu| {
        c.regs.set_word(Ax, 0x0020);
        c.regs.set_word(Reg16::Si, 0x0020);
        c.regs.set_word(Reg16::Di, 0x0030);
        c.regs.set_segment(Seg::Es, 0x9000);
        c.mem.write_u8(0x0000, 0x0020, 0x11);
        c.mem.write_u8(0x9000, 0x0030, 0x11);
    };
    vec![
        // ---- the eight ALU operations against an immediate, plus the logic ones
        Case {
            row: "ADD",
            code: &[0x04, 0x01],
            pre: al_ff,
        },
        Case {
            row: "ADC",
            code: &[0x14, 0x01],
            pre: al_ff,
        },
        Case {
            row: "SUB",
            code: &[0x2C, 0x01],
            pre: al_ff,
        },
        Case {
            row: "SBB",
            code: &[0x1C, 0x01],
            pre: al_ff,
        },
        Case {
            row: "AND",
            code: &[0x24, 0x01],
            pre: none,
        },
        Case {
            row: "OR",
            code: &[0x0C, 0x01],
            pre: none,
        },
        Case {
            row: "XOR",
            code: &[0x34, 0x01],
            pre: none,
        },
        Case {
            row: "TEST",
            code: &[0xA8, 0x01],
            pre: none,
        },
        Case {
            row: "CMP",
            code: &[0x3C, 0x01],
            pre: al_ff,
        },
        Case {
            row: "TEST",
            code: &[0xF6, 0xC0, 0x01],
            pre: none,
        },
        Case {
            row: "TEST",
            code: &[0x84, 0xC0],
            pre: none,
        },
        // ---- BCD and ASCII adjustments
        Case {
            row: "DAA",
            code: &[0x27],
            pre: al_19,
        },
        Case {
            row: "DAS",
            code: &[0x2F],
            pre: al_19,
        },
        Case {
            row: "AAA",
            code: &[0x37],
            pre: al_0b,
        },
        Case {
            row: "AAS",
            code: &[0x3F],
            pre: al_0b,
        },
        Case {
            row: "AAD",
            code: &[0xD5, 0x0A],
            pre: al_19,
        },
        Case {
            row: "AAM",
            code: &[0xD4, 0x0A],
            pre: al_19,
        },
        // ---- increment and decrement: CF is the row that matters
        Case {
            row: "INC",
            code: &[0x40],
            pre: none,
        },
        Case {
            row: "DEC",
            code: &[0x48],
            pre: none,
        },
        Case {
            row: "INC",
            code: &[0xFE, 0xC0],
            pre: al_ff,
        },
        Case {
            row: "DEC",
            code: &[0xFE, 0xC8],
            pre: al_00,
        },
        // ---- unary and multiply/divide
        Case {
            row: "NEG",
            code: &[0xF6, 0xD8],
            pre: al_05,
        },
        Case {
            row: "NOT",
            code: &[0xF6, 0xD0],
            pre: al_05,
        },
        Case {
            row: "MUL",
            code: &[0xF6, 0xE1],
            pre: al_10,
        },
        Case {
            row: "IMUL",
            code: &[0xF6, 0xE9],
            pre: al_10,
        },
        Case {
            row: "DIV",
            code: &[0xF6, 0xF1],
            pre: |c: &mut Cpu| {
                c.regs.set_byte(Reg8::Al, 0x64);
                c.regs.set_byte(Reg8::Cl, 0x07);
            },
        },
        Case {
            row: "IDIV",
            code: &[0xF6, 0xF9],
            pre: |c: &mut Cpu| {
                c.regs.set_byte(Reg8::Al, 0x64);
                c.regs.set_byte(Reg8::Cl, 0x07);
            },
        },
        // ---- shifts and rotates: the rows §1.4 itself corrected
        Case {
            row: "SHL",
            code: &[0xD0, 0xE0],
            pre: al_81,
        },
        Case {
            row: "SHR",
            code: &[0xD0, 0xE8],
            pre: al_81,
        },
        Case {
            row: "SAR",
            code: &[0xD0, 0xF8],
            pre: al_81,
        },
        Case {
            row: "ROL",
            code: &[0xD0, 0xC0],
            pre: al_81,
        },
        Case {
            row: "ROR",
            code: &[0xD0, 0xC8],
            pre: al_81,
        },
        Case {
            row: "RCL",
            code: &[0xD0, 0xD0],
            pre: al_81,
        },
        Case {
            row: "RCR",
            code: &[0xD0, 0xD8],
            pre: al_81,
        },
        // ---- compares and string operations
        Case {
            row: "CMPS",
            code: &[0xA6],
            pre: string_setup,
        },
        Case {
            row: "SCAS",
            code: &[0xAE],
            pre: string_setup,
        },
        Case {
            row: "MOVS",
            code: &[0xA4],
            pre: string_setup,
        },
        Case {
            row: "MOVS",
            code: &[0xAC],
            pre: string_setup,
        },
        Case {
            row: "MOVS",
            code: &[0xAA],
            pre: string_setup,
        },
        // ---- single-bit flag instructions
        Case {
            row: "CLC",
            code: &[0xF8],
            pre: none,
        },
        Case {
            row: "STC",
            code: &[0xF9],
            pre: none,
        },
        Case {
            row: "CMC",
            code: &[0xF5],
            pre: none,
        },
        Case {
            row: "CLD",
            code: &[0xFC],
            pre: none,
        },
        Case {
            row: "STD",
            code: &[0xFD],
            pre: none,
        },
        Case {
            row: "CLI",
            code: &[0xFA],
            pre: none,
        },
        Case {
            row: "STI",
            code: &[0xFB],
            pre: none,
        },
        Case {
            row: "LAHF",
            code: &[0x9F],
            pre: none,
        },
        // ---- everything that must not touch a flag at all
        Case {
            row: "MOV",
            code: &[0x89, 0xD8],
            pre: none,
        },
        Case {
            row: "NOP",
            code: &[0x90],
            pre: none,
        },
        Case {
            row: "CBW",
            code: &[0x98],
            pre: al_80,
        },
        Case {
            row: "CBW",
            code: &[0x99],
            pre: al_80,
        },
        Case {
            row: "LEA",
            code: &[0x8D, 0x00],
            pre: none,
        },
        Case {
            row: "LEA",
            code: &[0xC4, 0x06, 0x20, 0x00],
            pre: none,
        },
        Case {
            row: "PUSH",
            code: &[0x50],
            pre: none,
        },
        Case {
            row: "PUSH",
            code: &[0x58],
            pre: none,
        },
        Case {
            row: "CALL",
            code: &[0xE8, 0x00, 0x00],
            pre: none,
        },
        Case {
            row: "JMP",
            code: &[0xEB, 0x00],
            pre: none,
        },
        Case {
            row: "LOOP",
            code: &[0xE2, 0x00],
            pre: |c: &mut Cpu| {
                c.regs.set_word(Reg16::Cx, 2);
            },
        },
        Case {
            row: "XCHG",
            code: &[0x93],
            pre: |c: &mut Cpu| c.regs.set_word(Ax, 0x1234),
        },
        Case {
            row: "IN",
            code: &[0xE4, 0x10],
            pre: none,
        },
        Case {
            row: "IN",
            code: &[0xEE, 0x10],
            pre: al_5a,
        },
        Case {
            row: "HLT",
            code: &[0x9B],
            pre: none,
        },
        Case {
            row: "XLAT",
            code: &[0xD7],
            pre: |c: &mut Cpu| {
                c.regs.set_word(Reg16::Bx, 0x0000);
                c.regs.set_byte(Reg8::Al, 0x02);
                c.mem.write_u8(0x0000, 0x0002, 0x5A);
            },
        },
        Case {
            row: "ESC",
            code: &[0xF0, 0x90],
            pre: none,
        },
    ]
}
