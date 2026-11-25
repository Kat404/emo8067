//! The `INT` against a real 8086.
//!
//! The oracle is the chip's own words about itself: the captures of an
//! Intel P80C86A-2 in `SingleStepTests/8086`, through
//! `odd/research/extract_capture_oracle.py`.
//!
//! The probe is the verifier's own: it parses the oracle's `op CD` section
//! and runs every `INT n` capture against the emulator, comparing the
//! stack, the landing place, the flags and the memory the chip left behind.
//! The four `CD 10` captures of the full oracle are in it as well, because
//! they are the ones that show the video vector.

use emo8067_core::Cpu;
use emo8067_core::flags::Flags;
use emo8067_core::regs::{Reg16, Seg};

/// The generated oracle, vendored in the repository.
const ORACLE: &str = include_str!("../../../odd/research/capture_oracle.txt");

/// One capture: the machine before the instruction, and what the chip left.
struct Case {
    bytes: Vec<u8>,
    regs: Vec<(String, u16)>,
    flags: u16,
    mask: u16,
    mem: Vec<(u32, u8)>,
    final_mem: Vec<(u32, u8)>,
    final_flags: u16,
    final_regs: Vec<(String, u16)>,
}

/// The `op CD` section of the oracle.
fn int_cases(oracle: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut mask = 0u16;
    let mut current: Option<Case> = None;
    let mut in_section = false;
    for line in oracle.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("op ") {
            in_section = rest.trim() == "CD";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix("mask ") {
            mask = u16::from_str_radix(rest.trim(), 16).expect("a mask");
            continue;
        }
        if line == "case" {
            if let Some(case) = current.take() {
                cases.push(case);
            }
            current = Some(Case {
                bytes: Vec::new(),
                regs: Vec::new(),
                flags: 0,
                mask,
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
                .map(|b| u8::from_str_radix(b, 16).expect("a byte"))
                .collect();
        } else if let Some(rest) = line.strip_prefix("regs ") {
            for pair in rest.split_whitespace() {
                let (name, value) = pair.split_once('=').expect("a pair");
                case.regs
                    .push((name.to_string(), u16::from_str_radix(value, 16).unwrap()));
            }
        } else if let Some(rest) = line.strip_prefix("flags ") {
            case.flags = u16::from_str_radix(rest.trim(), 16).unwrap();
        } else if let Some(rest) = line.strip_prefix("mem ") {
            for pair in rest.split_whitespace() {
                let (address, value) = pair.split_once('=').expect("a pair");
                case.mem.push((
                    u32::from_str_radix(address, 16).unwrap(),
                    u8::from_str_radix(value, 16).unwrap(),
                ));
            }
        } else if let Some(rest) = line.strip_prefix("finalmem ") {
            for pair in rest.split_whitespace() {
                let (address, value) = pair.split_once('=').expect("a pair");
                case.final_mem.push((
                    u32::from_str_radix(address, 16).unwrap(),
                    u8::from_str_radix(value, 16).unwrap(),
                ));
            }
        } else if let Some(rest) = line.strip_prefix("final ") {
            for pair in rest.split_whitespace() {
                let (name, value) = pair.split_once('=').expect("a pair");
                let value = u16::from_str_radix(value, 16).unwrap();
                if name == "flags" {
                    case.final_flags = value;
                } else {
                    case.final_regs.push((name.to_string(), value));
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
fn set_reg(cpu: &mut Cpu, name: &str, value: u16) {
    match name {
        "ax" => cpu.regs.set_word(Reg16::Ax, value),
        "cx" => cpu.regs.set_word(Reg16::Cx, value),
        "dx" => cpu.regs.set_word(Reg16::Dx, value),
        "bx" => cpu.regs.set_word(Reg16::Bx, value),
        "sp" => cpu.regs.set_word(Reg16::Sp, value),
        "bp" => cpu.regs.set_word(Reg16::Bp, value),
        "si" => cpu.regs.set_word(Reg16::Si, value),
        "di" => cpu.regs.set_word(Reg16::Di, value),
        "cs" => cpu.regs.set_segment(Seg::Cs, value),
        "ds" => cpu.regs.set_segment(Seg::Ds, value),
        "es" => cpu.regs.set_segment(Seg::Es, value),
        "ss" => cpu.regs.set_segment(Seg::Ss, value),
        "ip" => cpu.regs.ip = value,
        other => panic!("a register name in the oracle: {other}"),
    }
}

/// Read a register the way the captures name it.
fn get_reg(cpu: &Cpu, name: &str) -> u16 {
    match name {
        "ax" => cpu.regs.word(Reg16::Ax),
        "cx" => cpu.regs.word(Reg16::Cx),
        "dx" => cpu.regs.word(Reg16::Dx),
        "bx" => cpu.regs.word(Reg16::Bx),
        "sp" => cpu.regs.word(Reg16::Sp),
        "bp" => cpu.regs.word(Reg16::Bp),
        "si" => cpu.regs.word(Reg16::Si),
        "di" => cpu.regs.word(Reg16::Di),
        "cs" => cpu.regs.segment(Seg::Cs),
        "ds" => cpu.regs.segment(Seg::Ds),
        "es" => cpu.regs.segment(Seg::Es),
        "ss" => cpu.regs.segment(Seg::Ss),
        "ip" => cpu.regs.ip,
        other => panic!("a register name in the oracle: {other}"),
    }
}

/// Run one capture against the emulator and say what disagrees.
fn run(case: &Case) -> Vec<String> {
    let mut cpu = Cpu::new();
    for (address, value) in &case.mem {
        cpu.mem
            .write_u8((*address >> 4) as u16, (*address & 0x0F) as u16, *value);
    }
    for (name, value) in &case.regs {
        set_reg(&mut cpu, name, *value);
    }
    cpu.regs.flags = Flags::popped(case.flags);

    if let Err(error) = cpu.step() {
        return vec![format!("the instruction faulted: {error:?}")];
    }

    let mut out = Vec::new();
    let want_flags = case.final_flags & case.mask;
    let got_flags = cpu.regs.flags.0 & case.mask;
    if want_flags != got_flags {
        out.push(format!("flags {got_flags:#06x} are not {want_flags:#06x}"));
    }
    for (name, value) in &case.final_regs {
        let got = get_reg(&cpu, name);
        if got != *value {
            out.push(format!("{name} is {got:#06x}, not {value:#06x}"));
        }
    }
    for (address, value) in &case.final_mem {
        let got = cpu
            .mem
            .read_u8((*address >> 4) as u16, (*address & 0x0F) as u16);
        if got != *value {
            out.push(format!(
                "memory {address:05X} is {got:02X}, not {value:02X}"
            ));
        }
    }
    out
}

/// S1 -- every `INT n` capture of the oracle: the emulator does what the
/// chip did.
#[test]
fn s1_every_int_capture_of_the_oracle_matches_the_chip() {
    let cases = int_cases(ORACLE);
    assert!(
        cases.len() >= 5,
        "the oracle's INT captures: {}",
        cases.len()
    );
    let mut failures = Vec::new();
    for case in &cases {
        let out = run(case);
        if !out.is_empty() {
            failures.push(format!("{:02X?}: {}", case.bytes, out.join("; ")));
        }
    }
    assert!(
        failures.is_empty(),
        "the INT captures that disagree with the chip: {failures:#?}"
    );
}

/// S1 -- the four `CD 10` captures of the full oracle: the machine leaves
/// the chip's place for the capture's own entry in the vector table, the
/// stack is six bytes lower, and the frame is the frame the chip pushed.
///
/// These four are transcribed from `capture_oracle_full.txt`, `op CD`, in
/// the oracle's own text so that the same parser reads them. The first
/// capture leaves `4360:9171` for `753C:5C91`, and the second `A671:D7F6`
/// for `6AD6:FBF4`.
#[test]
fn s1_the_four_video_vector_captures_of_the_full_oracle_match_the_chip() {
    const FULL: &str = "\
op CD
  mask 0FD5
  rows INT
  case
    bytes CD 10
    regs ax=0973 cx=0000 dx=61CD bx=E50F sp=0000 bp=49F0 si=2F7D di=DDEC cs=4360 ds=B6DB es=2B65 ss=5476 ip=9171
    flags F8D3
    mem 00040=91 00041=5C 00042=3C 00043=75 4C771=CD 4C772=10 4C773=90 4C774=90 4C775=90
    finalmem 00040=91 00041=5C 00042=3C 00043=75 4C771=CD 4C772=10 4C773=90 4C774=90 4C775=90 6475A=73 6475B=91 6475C=60 6475D=43 6475E=D3 6475F=F8
    final flags=F8D3 sp=FFFA cs=753C ip=5C91
  case
    bytes CD 10
    regs ax=0000 cx=CC52 dx=3715 bx=DEB7 sp=5802 bp=4E70 si=67F5 di=C5F7 cs=A671 ds=350E es=CF8F ss=0B27 ip=D7F6
    flags F846
    mem 00040=F4 00041=FB 00042=D6 00043=6A B3F06=CD B3F07=10 B3F08=90 B3F09=90 B3F0A=90 B3F0B=90
    finalmem 00040=F4 00041=FB 00042=D6 00043=6A 10A6C=F8 10A6D=D7 10A6E=71 10A6F=A6 10A70=46 10A71=F8 B3F06=CD B3F07=10 B3F08=90 B3F09=90 B3F0A=90 B3F0B=90
    final flags=F846 sp=57FC cs=6AD6 ip=FBF4
  case
    bytes CD 10
    regs ax=E200 cx=2E1E dx=CBEE bx=1C2F sp=445D bp=141F si=B4E8 di=9BD7 cs=ADC7 ds=6AB2 es=7419 ss=3289 ip=80D6
    flags F482
    mem 00040=4A 00041=7D 00042=5A 00043=39 B5D46=CD B5D47=10 B5D48=90 B5D49=90 B5D4A=90
    finalmem 00040=4A 00041=7D 00042=5A 00043=39 36CE7=D8 36CE8=80 36CE9=C7 36CEA=AD 36CEB=82 36CEC=F4 B5D46=CD B5D47=10 B5D48=90 B5D49=90 B5D4A=90
    final flags=F482 sp=4457 cs=395A ip=7D4A
  case
    bytes CD 10
    regs ax=4B4C cx=89BA dx=0687 bx=277A sp=2D47 bp=0781 si=B849 di=B62A cs=02D4 ds=854D es=B1F3 ss=DEEF ip=7669
    flags F052
    mem 00040=FE 00041=9D 00042=30 00043=A4 0A3A9=CD 0A3AA=10 0A3AB=90 0A3AC=90 0A3AD=90
    finalmem 00040=FE 00041=9D 00042=30 00043=A4 0A3A9=CD 0A3AA=10 0A3AB=90 0A3AC=90 0A3AD=90 E1C31=6B E1C32=76 E1C33=D4 E1C34=02 E1C35=52 E1C36=F0
    final flags=F052 sp=2D41 cs=A430 ip=9DFE
";

    let cases = int_cases(FULL);
    assert_eq!(cases.len(), 4, "the four video vector captures");
    let mut failures = Vec::new();
    for case in &cases {
        let out = run(case);
        if !out.is_empty() {
            failures.push(format!("{:02X?}: {}", case.bytes, out.join("; ")));
        }
    }
    assert!(
        failures.is_empty(),
        "the video vector captures that disagree with the chip: {failures:#?}"
    );
}
