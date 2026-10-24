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

#[path = "common/mod.rs"]
mod common;

use emo8067_core::Cpu;
use emo8067_core::flags::Flags;
use emo8067_core::memory::Memory;
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
pub(crate) struct Case {
    /// The opcode, and its /digit when the instruction has one: `F6.0`.
    pub(crate) opcode: String,
    /// The instruction bytes, including a prefix when the capture had one.
    bytes: Vec<u8>,
    /// The §1.4 rows the chip's own names put this instruction in: `ADD`.
    /// The names come from the captures' disassembler, so the mapping needs
    /// no list of this project's own.
    rows: Vec<String>,
    /// The flags the chip leaves undefined, cleared from both sides.
    mask: u16,
    regs: Vec<(Reg, u16)>,
    flags: u16,
    mem: Vec<(u32, u8)>,
    /// The memory the chip left behind: every address the oracle names, with
    /// the value the chip holds there.
    final_mem: Vec<(u32, u8)>,
    final_flags: u16,
    final_regs: Vec<(Reg, u16)>,
}

/// Parse the generated oracle.
pub(crate) fn parse(oracle: &str) -> Vec<Case> {
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
                bytes: Vec::new(),
                rows: rows.clone(),
                mask,
                regs: Vec::new(),
                flags: 0,
                mem: Vec::new(),
                final_mem: Vec::new(),
                final_flags: 0,
                final_regs: Vec::new(),
            });
            continue;
        }
        let Some(case) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = line.strip_prefix("bytes ") {
            case.bytes = rest
                .split_whitespace()
                .map(|byte| u8::from_str_radix(byte, 16).expect("an instruction byte"))
                .collect();
        } else if let Some(rest) = line.strip_prefix("regs ") {
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
        } else if let Some(rest) = line.strip_prefix("finalmem ") {
            for pair in rest.split_whitespace() {
                let (address, value) = pair.split_once('=').expect("an address=value pair");
                case.final_mem.push((
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
pub(crate) fn run(case: &Case) -> Result<(), String> {
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

    // The chip took the type 0 interrupt, which is what a divide that
    // overflowed or divided by zero does. The flags are undefined after a
    // divide error, so neither the flags the chip left in the register nor
    // the word it pushed onto the stack are compared: there is nothing in
    // them to compare.
    let fault = divide_fault(case);

    let want = case.final_flags & case.mask;
    let got = cpu.regs.flags.0 & case.mask;
    if fault.is_none() && want != got {
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
    // The memory the chip left behind. Every address the oracle names has
    // to hold the chip's value: the bytes the chip wrote, and the bytes it
    // left alone.
    for (address, want) in &case.final_mem {
        if let Some(word) = fault {
            if *address == word || *address == word.wrapping_add(1) {
                // The word the chip pushed is the flags word, and the
                // flags are undefined after a divide error.
                continue;
            }
        }
        let got = read_physical(&cpu, *address);
        if got != *want {
            return Err(format!(
                "the chip's memory at {address:05X} is {want:02X} and the emulator's is {got:02X} [{}]",
                describe(case)
            ));
        }
    }
    Ok(())
}

/// Read one byte at a physical address, the way the capture names it.
///
/// The capture names a physical address and the memory takes a
/// `segment:offset` pair, so the address is split the same way the setup
/// above writes it: the segment is the address above its low nibble and
/// the offset is the low nibble.
fn read_physical(cpu: &Cpu, address: u32) -> u8 {
    cpu.mem
        .read_u8((address >> 4) as u16, (address & 0x0F) as u16)
}

/// The opcodes whose divide can take the type 0 interrupt.
///
/// What makes the flags undefined is the **divide error**, and only a
/// divide can raise one. `INT 0` reaches the same vector with the same
/// stack frame, and the flags it leaves are defined, so they are compared
/// like any other instruction's.
const DIVIDE_OPCODES: &[&str] = &["D4", "D5", "F6.6", "F6.7", "F7.6", "F7.7"];

/// The address of the flags word the chip pushed onto the stack, when the
/// chip took the type 0 interrupt because a divide faulted.
///
/// A divide that overflows or divides by zero takes it. The interrupt
/// pushes the flags, the code segment and the instruction pointer, so the
/// flags word sits at the stack pointer the machine started with, less
/// two. The chip left `IP` at the vector the harness pointed the interrupt
/// table at, `0400h`, `CS` at `0000h`, and the stack pointer six bytes
/// below where it started, because the interrupt pushed three words.
///
/// `None` for every other case, and for a case where the chip did not take
/// the interrupt at all.
fn divide_fault(case: &Case) -> Option<u32> {
    // The cause first: a case whose opcode cannot raise a divide error has
    // defined flags, whatever the interrupt did.
    if !DIVIDE_OPCODES.contains(&case.opcode.as_str()) {
        return None;
    }
    // The final state lists only what changed, so a register that is not in
    // it is the one the machine started with.
    let final_reg = |reg: Reg| {
        case.final_regs
            .iter()
            .find(|(r, _)| *r == reg)
            .map(|(_, value)| *value)
            .or_else(|| {
                case.regs
                    .iter()
                    .find(|(r, _)| *r == reg)
                    .map(|(_, value)| *value)
            })
    };
    if final_reg(Reg::Ip) != Some(0x0400) || final_reg(Reg::Cs) != Some(0x0000) {
        return None;
    }
    let start = case
        .regs
        .iter()
        .find(|(r, _)| *r == Reg::Sp)
        .map(|(_, v)| *v)?;
    // The interrupt pushed three words, so the stack pointer moved down six.
    if final_reg(Reg::Sp) != Some(start.wrapping_sub(6)) {
        return None;
    }
    let ss = case
        .regs
        .iter()
        .find(|(r, _)| *r == Reg::Ss)
        .map(|(_, v)| *v)?;
    Some(Memory::physical(ss, start.wrapping_sub(2)))
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

/// The ModR/M byte of a case, after any prefix byte.
fn modrm(case: &Case) -> u8 {
    let base = case.opcode.split('.').next().unwrap();
    let opcode = u8::from_str_radix(base, 16).unwrap();
    let at = case
        .bytes
        .iter()
        .position(|byte| *byte == opcode)
        .expect("the opcode byte is in the case");
    case.bytes[at + 1]
}

/// The rotate count in CL, when the case has one.
fn count(case: &Case) -> u8 {
    case.regs
        .iter()
        .find(|(reg, _)| *reg == Reg::Cx)
        .map(|(_, value)| *value as u8)
        .unwrap_or(0)
}

/// The oracle must contain the forms the two silicon questions need.
///
/// The displacement forms of `8C`/`8E` are the S1 evidence, and a rotate
/// whose count is above one is the S2 evidence.  Keeping these explicit in
/// the gate stops a future change to the extraction from quietly removing
/// the cases that answer the questions.
#[test]
fn the_oracle_keeps_the_cases_the_silicon_questions_need() {
    let cases = parse(ORACLE);

    for opcode in ["8C", "8E"] {
        for mode in [1u8, 2] {
            assert!(
                cases
                    .iter()
                    .any(|case| { case.opcode == opcode && modrm(case) >> 6 == mode }),
                "the oracle needs {opcode} with MOD={mode:02b}"
            );
        }
    }

    for opcode in [
        "D2.0", "D2.1", "D2.2", "D2.3", "D3.0", "D3.1", "D3.2", "D3.3",
    ] {
        assert!(
            cases
                .iter()
                .any(|case| case.opcode == opcode && count(case) > 1),
            "the oracle needs {opcode} with a count above one"
        );
    }
}

/// A missing flat file is not a missing capture when the opcode has a
/// `/digit` form.  The generated header has to say which gaps are real.
#[test]
fn the_oracle_header_distinguishes_a_missing_flat_file_from_no_capture() {
    let no_flat = ORACLE
        .lines()
        .find(|line| line.starts_with("# opcodes with no flat capture file"))
        .expect("the generated header names the missing flat files");
    let no_capture = ORACLE
        .lines()
        .find(|line| line.starts_with("# opcodes with no captures in any form"))
        .expect("the generated header names the opcodes with no capture");

    for opcode in ["80", "D0", "D1", "D2", "D3", "F6", "F7", "FE", "FF"] {
        assert!(
            no_flat.contains(opcode),
            "{opcode} has no flat file but does have dotted captures"
        );
        assert!(
            !no_capture.contains(opcode),
            "{opcode} does have dotted captures"
        );
    }
}

/// The families the chip and this emulator still disagree about, and why.
///
/// There are none left. The rotate's carry and overflow now follow the
/// P80C86A-2, and the port reads already follow the empty I/O bus. A future
/// disagreement must either fix the emulator or enter this list with the
/// chip's rule as its reason.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[];

/// How many cases are allowed to land in the families above.
const KNOWN_DIVERGENCE_COUNT: usize = 0;

#[test]
fn the_flags_come_out_the_way_a_real_8086_leaves_them() {
    let cases = parse(ORACLE);
    // The count is pinned exactly, not bounded: a file the network failed to
    // deliver would leave a hole in the oracle, and a hole that shrinks the
    // gate quietly is exactly what this project keeps paying for.
    assert_eq!(
        cases.len(),
        2689,
        "the oracle must hold every case the extraction writes"
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
