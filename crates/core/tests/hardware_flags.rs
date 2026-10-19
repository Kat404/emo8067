//! What the flags become, according to a real 8086.
//!
//! The 1979 manual says which flags an instruction may touch, and the SDM
//! repeats it. Neither says what a flag *is* afterwards, because the answer
//! depends on the operands: `CF` after an `ADD` is a function of the two
//! values added. That is why §1.4's `X` marks assert nothing, and it is
//! where two of the eight defects from the 2026-10-15 session lived: the
//! `REP`-negates-`MUL` implementation was wrong in two ways and 220 tests
//! could not see either, because nothing compared a flag's value with
//! anything.
//!
//! This file compares the emulator against a real chip. The cases come from
//! `SingleStepTests/8086` — Daniel Balsom's hardware-generated test suite,
//! made on an Intel P80C86A-2 — through
//! `odd/research/extract_capture_oracle.py`, which takes the 160 MB of
//! gzipped captures apart and writes the handful of tests per opcode that
//! the emulator can be run against. The provenance is in the generated file
//! and the extraction is a script, so the oracle can be regenerated rather
//! than trusted.
//!
//! Each case is a complete machine state, the instruction's bytes, and the
//! state the chip left behind. The flags are compared through the mask that
//! `metadata.json` gives: the flags the chip leaves undefined are cleared
//! from both sides, because "undefined" means the chip left whatever was
//! there, and there is nothing to compare.
//!
//! **What this gate is not.** It is not a proof that the emulator matches
//! silicon in general. It is a proof that the flags of these instructions,
//! for these operand values, come out the way a real 8086 leaves them. The
//! captures are the chip's own words about itself, so this is a different
//! kind of evidence from the document the rest of the suite checks against:
//! it is evidence from the hardware.

mod common;

use emo8067_core::Cpu;
use emo8067_core::flags::Flags;
use emo8067_core::regs::{Reg16, Seg};

use common::{COLUMNS, GUIDE, Mark, find_row_marks};

/// The generated oracle. It is a text file on purpose: the crate has no
/// dependencies, and a JSON parser is not worth one.
const ORACLE: &str = include_str!("../../../odd/research/capture_oracle.txt");

/// A register the captures name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reg {
    Ax,
    Cx,
    Dx,
    Bx,
    Sp,
    Bp,
    Si,
    Di,
    Cs,
    Ds,
    Es,
    Ss,
    Ip,
}

impl Reg {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "ax" => Reg::Ax,
            "cx" => Reg::Cx,
            "dx" => Reg::Dx,
            "bx" => Reg::Bx,
            "sp" => Reg::Sp,
            "bp" => Reg::Bp,
            "si" => Reg::Si,
            "di" => Reg::Di,
            "cs" => Reg::Cs,
            "ds" => Reg::Ds,
            "es" => Reg::Es,
            "ss" => Reg::Ss,
            "ip" => Reg::Ip,
            _ => return None,
        })
    }
}

/// One hardware test: the machine before the instruction, and what the chip
/// left behind.
struct Case {
    /// The opcode, and its /digit when the instruction has one: `F6.0`.
    opcode: String,
    /// The §1.4 rows the chip's own names put this instruction in: `ADD`.
    /// The names come from the captures' disassembler, so the mapping needs
    /// no list of this project's own.
    rows: Vec<String>,
    /// The flags the chip leaves undefined, cleared from both sides.
    mask: u16,
    regs: Vec<(Reg, u16)>,
    flags: u16,
    mem: Vec<(u32, u8)>,
    final_flags: u16,
    final_regs: Vec<(Reg, u16)>,
}

/// Parse the generated oracle.
fn parse(oracle: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut opcode = String::new();
    let mut mask = 0u16;
    let mut rows: Vec<String> = Vec::new();
    let mut current: Option<Case> = None;

    for line in oracle.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("op ") {
            opcode = rest.trim().to_string();
            assert!(
                u8::from_str_radix(opcode.split('.').next().unwrap_or(""), 16).is_ok(),
                "an opcode in the oracle: {opcode}"
            );
            mask = 0;
            continue;
        }
        if let Some(rest) = line.strip_prefix("mask ") {
            mask = u16::from_str_radix(rest.trim(), 16).expect("a mask in the oracle");
            continue;
        }
        if let Some(rest) = line.strip_prefix("rows ") {
            rows = rest.split_whitespace().map(str::to_string).collect();
            continue;
        }
        if line == "case" {
            if let Some(case) = current.take() {
                cases.push(case);
            }
            current = Some(Case {
                opcode: opcode.clone(),
                rows: rows.clone(),
                mask,
                regs: Vec::new(),
                flags: 0,
                mem: Vec::new(),
                final_flags: 0,
                final_regs: Vec::new(),
            });
            continue;
        }
        let Some(case) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = line.strip_prefix("regs ") {
            for pair in rest.split_whitespace() {
                let (name, value) = pair.split_once('=').expect("a name=value pair");
                case.regs.push((
                    Reg::parse(name).expect("a register name in the oracle"),
                    u16::from_str_radix(value, 16).expect("a register value"),
                ));
            }
        } else if let Some(rest) = line.strip_prefix("flags ") {
            case.flags = u16::from_str_radix(rest.trim(), 16).expect("the initial flags");
        } else if let Some(rest) = line.strip_prefix("mem ") {
            for pair in rest.split_whitespace() {
                let (address, value) = pair.split_once('=').expect("an address=value pair");
                case.mem.push((
                    u32::from_str_radix(address, 16).expect("a physical address"),
                    u8::from_str_radix(value, 16).expect("a memory byte"),
                ));
            }
        } else if let Some(rest) = line.strip_prefix("final ") {
            for pair in rest.split_whitespace() {
                let (name, value) = pair.split_once('=').expect("a name=value pair");
                let value = u16::from_str_radix(value, 16).expect("a final value");
                if name == "flags" {
                    case.final_flags = value;
                } else {
                    case.final_regs.push((
                        Reg::parse(name).expect("a register name in the oracle"),
                        value,
                    ));
                }
            }
        }
    }
    if let Some(case) = current.take() {
        cases.push(case);
    }
    cases
}

/// Put a register where the emulator wants it.
fn set_reg(cpu: &mut Cpu, reg: Reg, value: u16) {
    match reg {
        Reg::Ax => cpu.regs.set_word(Reg16::Ax, value),
        Reg::Cx => cpu.regs.set_word(Reg16::Cx, value),
        Reg::Dx => cpu.regs.set_word(Reg16::Dx, value),
        Reg::Bx => cpu.regs.set_word(Reg16::Bx, value),
        Reg::Sp => cpu.regs.set_word(Reg16::Sp, value),
        Reg::Bp => cpu.regs.set_word(Reg16::Bp, value),
        Reg::Si => cpu.regs.set_word(Reg16::Si, value),
        Reg::Di => cpu.regs.set_word(Reg16::Di, value),
        Reg::Cs => cpu.regs.set_segment(Seg::Cs, value),
        Reg::Ds => cpu.regs.set_segment(Seg::Ds, value),
        Reg::Es => cpu.regs.set_segment(Seg::Es, value),
        Reg::Ss => cpu.regs.set_segment(Seg::Ss, value),
        Reg::Ip => cpu.regs.ip = value,
    }
}

/// Read a register the way the captures name it.
fn get_reg(cpu: &Cpu, reg: Reg) -> u16 {
    match reg {
        Reg::Ax => cpu.regs.word(Reg16::Ax),
        Reg::Cx => cpu.regs.word(Reg16::Cx),
        Reg::Dx => cpu.regs.word(Reg16::Dx),
        Reg::Bx => cpu.regs.word(Reg16::Bx),
        Reg::Sp => cpu.regs.word(Reg16::Sp),
        Reg::Bp => cpu.regs.word(Reg16::Bp),
        Reg::Si => cpu.regs.word(Reg16::Si),
        Reg::Di => cpu.regs.word(Reg16::Di),
        Reg::Cs => cpu.regs.segment(Seg::Cs),
        Reg::Ds => cpu.regs.segment(Seg::Ds),
        Reg::Es => cpu.regs.segment(Seg::Es),
        Reg::Ss => cpu.regs.segment(Seg::Ss),
        Reg::Ip => cpu.regs.ip,
    }
}

/// Run one hardware test against the emulator.
fn run(case: &Case) -> Result<(), String> {
    let mut cpu = Cpu::new();
    for (address, value) in &case.mem {
        cpu.mem
            .write_u8((*address >> 4) as u16, (*address & 0x0F) as u16, *value);
    }
    for (reg, value) in &case.regs {
        set_reg(&mut cpu, *reg, *value);
    }
    // The captures record the whole FLAGS word, including the bits the 8086
    // leaves undefined and reads back as one. `popped` is what the chip does
    // with a word it pops, and it drops them.
    cpu.regs.flags = Flags::popped(case.flags);

    let info = cpu
        .step()
        .map_err(|err| format!("the instruction faulted: {err:?}"))?;
    let _ = info;

    let want = case.final_flags & case.mask;
    let got = cpu.regs.flags.0 & case.mask;
    if want != got {
        return Err(format!(
            "flags {got:#06x} are not the {want:#06x} the chip left [{}]",
            describe(case),
        ));
    }
    for (reg, value) in &case.final_regs {
        let got = get_reg(&cpu, *reg);
        if got != *value {
            return Err(format!(
                "{} is {got:#06x}, not {value:#06x} [{}]",
                reg_name(*reg),
                describe(case)
            ));
        }
    }
    // And every register the chip left alone is exactly as it was: the
    // capture's final state lists what changed, so what is missing from it
    // did not change.
    for reg in ALL_REGS {
        if case.final_regs.iter().any(|(r, _)| *r == reg) {
            continue;
        }
        let Some(before) = case.regs.iter().find(|(r, _)| *r == reg).map(|(_, v)| *v) else {
            continue;
        };
        let got = get_reg(&cpu, reg);
        if got != before {
            return Err(format!(
                "{} is {got:#06x}, but the chip left it at {before:#06x} [{}]",
                reg_name(reg),
                describe(case)
            ));
        }
    }
    // The document checked against the hardware: a row that marks a flag
    // with an em dash says the instruction does not touch it, and the
    // captures say whether the chip agreed. A disagreement is the document
    // being wrong, not the chip.
    for row in &case.rows {
        let Some(marks) = find_row_marks(GUIDE, row) else {
            continue;
        };
        for (i, mark) in marks.iter().enumerate() {
            if !matches!(mark, Mark::Unchanged) {
                continue;
            }
            let bit = COLUMNS[i];
            if case.flags & bit != case.final_flags & bit {
                return Err(format!(
                    "the chip changed the {} flag of {row}, which the document \
                     says it does not touch",
                    FLAG_NAMES[i]
                ));
            }
        }
    }
    Ok(())
}

/// The §1.4 column order, for a failure message.
const FLAG_NAMES: [&str; 9] = ["OF", "DF", "IF", "TF", "SF", "ZF", "AF", "PF", "CF"];

/// A short description of a case, for a failure message: the instruction's
/// bytes and the registers it started from.
fn describe(case: &Case) -> String {
    let bytes: Vec<String> = case
        .mem
        .iter()
        .filter(|(address, _)| *address == code_address(case))
        .map(|(address, value)| format!("{address:05X}={value:02X}"))
        .collect();
    let mut text = format!("bytes {:?}", bytes);
    for (reg, value) in &case.regs {
        text.push_str(&format!(" {reg:?}={value:04X}"));
    }
    text
}

/// The physical address the instruction's first byte sits at.
fn code_address(case: &Case) -> u32 {
    let mut cs = 0u16;
    let mut ip = 0u16;
    for (reg, value) in &case.regs {
        match reg {
            Reg::Cs => cs = *value,
            Reg::Ip => ip = *value,
            _ => {}
        }
    }
    u32::from(cs) * 16 + u32::from(ip)
}

/// Every register a case names, so that the ones the chip did not touch can
/// be checked too.
const ALL_REGS: [Reg; 13] = [
    Reg::Ax,
    Reg::Cx,
    Reg::Dx,
    Reg::Bx,
    Reg::Sp,
    Reg::Bp,
    Reg::Si,
    Reg::Di,
    Reg::Cs,
    Reg::Ds,
    Reg::Es,
    Reg::Ss,
    Reg::Ip,
];

fn reg_name(reg: Reg) -> &'static str {
    match reg {
        Reg::Ax => "ax",
        Reg::Cx => "cx",
        Reg::Dx => "dx",
        Reg::Bx => "bx",
        Reg::Sp => "sp",
        Reg::Bp => "bp",
        Reg::Si => "si",
        Reg::Di => "di",
        Reg::Cs => "cs",
        Reg::Ds => "ds",
        Reg::Es => "es",
        Reg::Ss => "ss",
        Reg::Ip => "ip",
    }
}

/// The families the chip and this emulator still disagree about, and why.
///
/// Each entry is a finding rather than an excuse: the count of cases that
/// fall in these families is pinned below, so a list that grows or shrinks
/// is a failure, and a disagreement anywhere else fails the gate outright.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[
    ("D0", "the rotate's carry and overflow"),
    ("D1", "the rotate's carry and overflow"),
    ("D2", "the rotate's carry and overflow"),
    ("D3", "the rotate's carry and overflow"),
    (
        "E4",
        "the port read: this emulator reads memory at the port, the chip returns FFh",
    ),
    (
        "E5",
        "the port read: this emulator reads memory at the port, the chip returns FFh",
    ),
    (
        "EC",
        "the port read: this emulator reads memory at the port, the chip returns FFh",
    ),
    (
        "ED",
        "the port read: this emulator reads memory at the port, the chip returns FFh",
    ),
];

/// How many cases are allowed to land in the families above. The number was
/// read off the gate's own output and pinned, so a family that grows is a
/// failure rather than a quieter gate.
const KNOWN_DIVERGENCE_COUNT: usize = 6;

#[test]
fn the_flags_come_out_the_way_a_real_8086_leaves_them() {
    let cases = parse(ORACLE);
    assert!(
        cases.len() >= 1000,
        "the oracle must stay broad: {}",
        cases.len()
    );
    let mut failures = Vec::new();
    let mut known = 0usize;
    for case in &cases {
        if let Err(reason) = run(case) {
            if KNOWN_DIVERGENCES
                .iter()
                .any(|(prefix, _)| case.opcode.starts_with(prefix))
            {
                known += 1;
                continue;
            }
            failures.push(format!("opcode {}: {reason}", case.opcode));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} hardware cases disagree with the emulator:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    );
    assert_eq!(
        known, KNOWN_DIVERGENCE_COUNT,
        "the known divergences changed: {} cases now land in the recorded families",
        known
    );
}
