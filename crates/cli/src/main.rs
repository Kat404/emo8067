//! Headless runner for the 8086 core.
//!
//! Loads a raw binary, runs it, and prints where it stopped. This exists so
//! the core can be exercised end to end without a UI: every instruction the
//! emulator adds can be proven against a program that actually runs.

use std::process::ExitCode;

use emo8067_core::flags::{AF, CF, DF, Flags, IF, OF, PF, SF, TF, ZF};
use emo8067_core::regs::{Reg16, Seg};
use emo8067_core::{Cpu, StopReason};

const USAGE: &str = "\
emo8067 — headless 8086 runner

USAGE:
    emo8067 run <file.bin> [options]

OPTIONS:
    --load SEG:OFF     load the file at SEG:OFF        (default 0100:0000)
    --org SEG          set CS and IP to SEG:0000       (default 0100:0000)
    --ss SEG           set SS                           (default 0100)
    --sp VALUE         set SP                           (default FFFE)
    --ds SEG           set DS                           (default 0100)
    --max N            stop after N instructions       (default 100000)
    --trace            print one line per instruction
    --dump ADDR:LEN    hex dump of memory at physical ADDR
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("emo8067: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    if args[0] != "run" {
        return Err(format!("unknown command `{}`\n\n{USAGE}", args[0]));
    }

    let mut path: Option<&str> = None;
    let mut load = 0x0100u32 * 16;
    let mut org = (0x0100u16, 0x0000u16);
    let mut ss = 0x0100u16;
    let mut sp = 0xFFFEu16;
    let mut ds = 0x0100u16;
    let mut max = 100_000u64;
    let mut trace = false;
    let mut dump: Option<(u32, usize)> = None;

    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        let value = || -> Result<&str, String> {
            args.get(i + 1)
                .map(String::as_str)
                .ok_or_else(|| format!("`{a}` needs a value"))
        };
        match a {
            "--load" => {
                let v = value()?;
                let (s, o) = split_addr(v)?;
                load = u32::from(s) * 16 + u32::from(o);
                i += 1;
            }
            "--org" => {
                let v = value()?;
                org = parse_addr(v)?;
                i += 1;
            }
            "--ss" => {
                ss = parse_hex(value()?)?;
                i += 1;
            }
            "--sp" => {
                sp = parse_hex(value()?)?;
                i += 1;
            }
            "--ds" => {
                ds = parse_hex(value()?)?;
                i += 1;
            }
            "--max" => {
                max = value()?.parse().map_err(|_| "bad --max")?;
                i += 1;
            }
            "--trace" => trace = true,
            "--dump" => {
                let v = value()?;
                let (a, l) = v.split_once(':').ok_or("--dump wants ADDR:LEN")?;
                dump = Some((u32::from(parse_hex(a)?), parse_hex(l)? as usize));
                i += 1;
            }
            other if other.starts_with("--") => return Err(format!("unknown option `{other}`")),
            other => path = Some(other),
        }
        i += 1;
    }

    let path = path.ok_or_else(|| "no input file".to_string())?;
    let image = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;

    let mut cpu = Cpu::new();
    cpu.mem.load_at(load, &image).map_err(|_a| {
        format!(
            "image does not fit: {load:#x} + {} bytes spills past 1 MiB",
            image.len()
        )
    })?;

    cpu.regs.set_segment(Seg::Cs, org.0);
    cpu.regs.ip = org.1;
    cpu.regs.set_segment(Seg::Ss, ss);
    cpu.regs.set_word(Reg16::Sp, sp);
    cpu.regs.set_segment(Seg::Ds, ds);
    cpu.regs.set_segment(Seg::Es, ds);

    let mut executed = 0u64;
    let mut failure: Option<String> = None;
    while executed < max {
        let before = cpu.regs.ip;
        match cpu.step() {
            Ok(info) => {
                executed += 1;
                if trace {
                    println!(
                        "{:04X}:{:04X}  {:02X}   -> {:04X}:{:04X}",
                        cpu.regs.segment(Seg::Cs),
                        before,
                        info.opcode,
                        cpu.regs.segment(Seg::Cs),
                        info.next_ip
                    );
                }
                if cpu.stop_reason == StopReason::Halted {
                    break;
                }
                if let StopReason::Interrupt(n) = cpu.stop_reason {
                    failure = Some(format!("took INT {n:#04x}"));
                    break;
                }
            }
            Err(e) => {
                failure = Some(format!(
                    "{e:?} at {:04X}:{before:04X}",
                    cpu.regs.segment(Seg::Cs)
                ));
                break;
            }
        }
    }

    print_state(&cpu, executed, max, &failure);
    if let Some((addr, len)) = dump {
        hexdump(&cpu, addr, len);
    }
    Ok(if failure.is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn parse_hex(s: &str) -> Result<u16, String> {
    let t = s.trim_start_matches("0x").trim_start_matches('$');
    u16::from_str_radix(t, 16).map_err(|_| format!("`{s}` is not a hex number"))
}

fn parse_addr(s: &str) -> Result<(u16, u16), String> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| format!("`{s}` wants SEG:OFF"))?;
    Ok((parse_hex(a)?, parse_hex(b)?))
}

fn split_addr(s: &str) -> Result<(u16, u16), String> {
    parse_addr(s)
}

fn print_state(cpu: &Cpu, executed: u64, max: u64, failure: &Option<String>) {
    let r = &cpu.regs;
    let out = |x: u16| format!("{x:04X}");
    println!(
        "--- stopped after {executed} instruction(s){} ---",
        if executed >= max {
            " (step budget exhausted)"
        } else {
            ""
        }
    );
    println!(
        "CS:IP  {}:{}   SS:SP  {}:{}   DS {}  ES {}",
        out(r.segment(Seg::Cs)),
        out(r.ip),
        out(r.segment(Seg::Ss)),
        out(r.word(Reg16::Sp)),
        out(r.segment(Seg::Ds)),
        out(r.segment(Seg::Es))
    );
    println!(
        "AX {}  BX {}  CX {}  DX {}",
        out(r.word(Reg16::Ax)),
        out(r.word(Reg16::Bx)),
        out(r.word(Reg16::Cx)),
        out(r.word(Reg16::Dx))
    );
    println!(
        "SI {}  DI {}  BP {}  SP {}",
        out(r.word(Reg16::Si)),
        out(r.word(Reg16::Di)),
        out(r.word(Reg16::Bp)),
        out(r.word(Reg16::Sp))
    );
    println!("FLAGS {:#06x}  {}", r.flags.0, flag_line(&r.flags));
    if let Some(f) = failure {
        println!("STOP  {f}");
    }
}

fn flag_line(f: &Flags) -> String {
    let mut v: Vec<(&str, bool)> = vec![
        ("OF", f.get(OF)),
        ("DF", f.get(DF)),
        ("IF", f.get(IF)),
        ("TF", f.get(TF)),
        ("SF", f.get(SF)),
        ("ZF", f.get(ZF)),
        ("AF", f.get(AF)),
        ("PF", f.get(PF)),
        ("CF", f.get(CF)),
    ];
    v.sort_by_key(|(_, on)| !on);
    v.into_iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| n)
        .collect::<Vec<_>>()
        .join(" ")
}

fn hexdump(cpu: &Cpu, addr: u32, len: usize) {
    println!("--- {addr:#08x} ({len} bytes) ---");
    for row in 0..len.div_ceil(16) {
        let base = row * 16;
        let cells = cpu.mem.slice(addr + base as u32, 16);
        let hex: Vec<String> = cells.iter().take(16).map(|b| format!("{b:02X}")).collect();
        let ascii: String = cells
            .iter()
            .take(16)
            .map(|b| {
                if (0x20..0x7F).contains(b) {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{base:08X}  {:<47}  |{ascii}|", hex.join(" "));
    }
}
