//! The gate over every capture, memory included.
//!
//! The committed oracle in `odd/research/capture_oracle.txt` keeps a
//! handful of cases per opcode: 2 689 of them, enough to say what the
//! flags become. The capture set holds 646 000, and this is the gate over
//! all of them: it runs every case, compares the memory as well as the
//! registers and the flags, and names every case that disagrees.
//!
//! The full oracle is generated and it is not in the repository:
//!
//! ```text
//! python3 odd/research/extract_capture_oracle.py --all
//! ```
//!
//! writes `odd/research/capture_oracle_full.txt`, which is gitignored
//! beside the sample. The gate stands aside when that file is not there,
//! the way the examples gate stands aside when the reference's examples
//! are not there, so `cargo test --workspace` never depends on a file
//! that is not in the repository.
//!
//! **The shape is the committed gate's.** The families the full set found
//! are listed below with the chip's rule as the reason, each family's
//! count is pinned exactly, and a divergence outside those families fails
//! the gate. A family whose count grows or shrinks is a failure too: it
//! means the emulator and the chip disagree about something else as well.
//! **All three families are fixed**, so every count is pinned at zero and
//! the full set has no disagreement left with the emulator.
//!
//! **What this gate is not.** It is not a proof that the emulator matches
//! silicon in general, and it is not a unit test. It is a research gate:
//! with the full oracle in place it says which cases the emulator and the
//! chip disagree about, and every one of those is a finding.

#[path = "hardware_flags.rs"]
mod gate;

use std::collections::BTreeMap;
use std::path::Path;

use gate::{parse, run};

/// The generated full oracle: every capture the test set holds.
///
/// It is written by `odd/research/extract_capture_oracle.py --all` and it
/// is gitignored, so it is only there when somebody asked for it.
fn full_oracle() -> Option<String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../odd/research/capture_oracle_full.txt");
    std::fs::read_to_string(path).ok()
}

/// How many cases the capture set holds.
///
/// The count is pinned exactly, not bounded: a file the network failed to
/// deliver would leave a hole in the oracle, and a hole that shrinks the
/// gate quietly is exactly what this project keeps paying for.
const CASES: usize = 646_000;

/// A family of cases the full set found, and the chip's rule behind it.
///
/// The count is pinned exactly, the way the committed gate pins its case
/// count: a family that grows or shrinks is a failure.
struct Family {
    /// What the family is called, for the report.
    name: &'static str,
    /// The chip's rule, which is why the family is a family.
    rule: &'static str,
    /// The opcodes in the family, and how many cases of each the gate
    /// found.
    opcodes: &'static [(&'static str, usize)],
}

/// The families the full set found.
///
/// Each one was a defect in the emulator, and each one is now fixed: the
/// count is pinned at **zero**, so a family that grows again fails the gate
/// the way the count it had did. The chip's rule stays in the comment,
/// because it is the reason the fix is what it is, and the plan's log names
/// the case that shows it.
const FAMILIES: &[Family] = &[
    Family {
        name: "DAA and DAS",
        rule: "the chip applies the tens correction when the carry was set or the \
               old AL was over 99h; with the AF flag set the old AL has to be \
               over 9Fh",
        opcodes: &[("27", 0), ("2F", 0)],
    },
    Family {
        name: "AAD with a zero immediate",
        rule: "the chip does not fault on a zero divisor: it computes the result \
               and leaves AH clear",
        opcodes: &[("D5", 0)],
    },
    Family {
        name: "IDIV r/m8",
        rule: "the chip faults when the quotient does not fit in seven bits, and \
               -128h does not",
        opcodes: &[("F6.7", 0)],
    },
];

/// Whether the opcode is one of the families the full set found.
fn is_known(opcode: &str) -> bool {
    FAMILIES
        .iter()
        .any(|family| family.opcodes.iter().any(|(known, _)| *known == opcode))
}

#[test]
fn every_capture_comes_out_the_way_the_chip_leaves_it() {
    let Some(oracle) = full_oracle() else {
        // The full oracle is generated and not in the repository.
        return;
    };
    let cases = parse(&oracle);
    assert_eq!(
        cases.len(),
        CASES,
        "the full oracle must hold every capture the test set has"
    );

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut findings = Vec::new();
    let mut failures = Vec::new();
    for case in &cases {
        if let Err(reason) = run(case) {
            if is_known(&case.opcode) {
                *counts.entry(case.opcode.as_str()).or_default() += 1;
                findings.push(format!("opcode {}: {reason}", case.opcode));
            } else {
                failures.push(format!("opcode {}: {reason}", case.opcode));
            }
        }
    }

    // A divergence outside the families is a finding of its own and it
    // fails the gate: nothing is skipped and nothing passes quietly.
    assert!(
        failures.is_empty(),
        "{} of {} hardware cases disagree with the emulator outside the {} \
         families the full set found:\n  {}",
        failures.len(),
        cases.len(),
        FAMILIES.len(),
        failures.join("\n  ")
    );

    // Every family is named with the chip's rule as the reason, and every
    // count is pinned exactly.
    for family in FAMILIES {
        let mut total = 0usize;
        for (opcode, want) in family.opcodes {
            let got = counts.get(opcode).copied().unwrap_or(0);
            total += got;
            assert_eq!(
                got, *want,
                "the {} family changed: {opcode} holds {got} cases, not {want} — \
                 the chip's rule: {}",
                family.name, family.rule
            );
        }
        println!("{}: {total} cases — {}", family.name, family.rule);
    }
    for finding in &findings {
        println!("  {finding}");
    }
}

/// A case whose IP merely reaches 0400h is not a divide fault, so its flags
/// are compared. The expected flags are doctored so the comparison has
/// something to catch, and the row is one the guide does not have so the
/// document check stays out of the way.
#[test]
fn a_case_whose_ip_reaches_0400h_is_not_a_divide_fault() {
    let oracle = "op 20\n  mask 0FD5\n  rows ZZ\n  case\n    bytes 2E 20 7C 50\n    regs ax=1C5D cx=3912 dx=F720 bx=4A0B sp=62CB bp=FF60 si=F0E4 di=919F cs=BEE9 ds=9B72 es=EEDE ss=DEB2 ip=03FC\n    flags F803\n    mem BF28C=2E BF28D=20 BF28E=7C BF28F=50 BF290=90 BF291=90 CDFC4=86\n    finalmem BF28C=2E BF28D=20 BF28E=7C BF28F=50 BF290=90 BF291=90 CDFC4=02\n    final flags=F003 ip=0400\n";
    let cases = gate::parse(oracle);
    assert!(
        gate::run(&cases[0]).is_err(),
        "the flags of a case that is not a divide fault are compared"
    );
}

/// `INT 0` reaches the same interrupt, but its flags are defined, so they
/// are compared.
#[test]
fn int_zero_is_not_a_divide_fault() {
    let oracle = "op CD\n  mask 0FD5\n  rows ZZ\n  case\n    bytes CD 00\n    regs ax=E878 cx=185C dx=0000 bx=219C sp=605A bp=5B31 si=F5BF di=4EEC cs=7E4B ds=64AA es=69F7 ss=0A64 ip=EE05\n    flags F406\n    mem 00000=00 00001=04 00002=00 00003=00 8D2B5=CD 8D2B6=00 8D2B7=90 8D2B8=90 8D2B9=90\n    finalmem 00000=00 00001=04 00002=00 00003=00 10694=07 10695=EE 10696=4B 10697=7E 10698=06 10699=F4 8D2B5=CD 8D2B6=00 8D2B7=90 8D2B8=90 8D2B9=90\n    final flags=F407 sp=6054 cs=0000 ip=0400\n";
    let cases = gate::parse(oracle);
    assert!(
        gate::run(&cases[0]).is_err(),
        "the flags of INT 0 are defined and they are compared"
    );
}

/// `AAM` with a zero immediate divides by zero, and the chip faults.
///
/// The twelve `AAM 00h` captures in the capture set all fault: the chip
/// leaves `IP` at `0400h`, `CS` at `0000h` and the stack pointer six bytes
/// below where it started, and the flags it leaves are undefined. The
/// flags of a divide fault are noise, so the gate does not compare them:
/// this case is the one that says so.
#[test]
fn aam_zero_divides_fault_matches_the_chip() {
    let oracle = "op D4\n  mask 07C4\n  rows AAM\n  case\n    bytes D4 00\n    regs ax=C4C4 cx=B5FC dx=1528 bx=0CBF sp=34DE bp=14B4 si=0D9D di=7D8F cs=3F61 ds=EC7F es=6DF4 ss=5A99 ip=1ED1\n    flags FC43\n    mem 00000=00 00001=04 00002=00 00003=00 414E1=D4 414E2=00 414E3=90 414E4=90 414E5=90\n    finalmem 00000=00 00001=04 00002=00 00003=00 414E1=D4 414E2=00 414E3=90 414E4=90 414E5=90 5DE68=D3 5DE69=1E 5DE6A=61 5DE6B=3F 5DE6C=46 5DE6D=F4\n    final flags=F446 sp=34D8 cs=0000 ip=0400\n";
    let cases = gate::parse(oracle);
    assert!(gate::run(&cases[0]).is_ok(), "{:?}", gate::run(&cases[0]));
}
