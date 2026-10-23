//! Headless runner for the 8086 core.
//!
//! Loads a program, runs it, and prints where it stopped. This exists so
//! the core can be exercised end to end without a UI: every instruction the
//! emulator adds can be proven against a program that actually runs.
//!
//! A `.asm` file is assembled first and run where the assembler says it
//! starts; anything else is loaded raw, as it always was.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use emo8067_asm::program::{self, Error};
use emo8067_core::flags::{AF, CF, DF, Flags, IF, OF, PF, SF, TF, ZF};
use emo8067_core::regs::{Reg16, Seg};
use emo8067_core::{Cpu, StopReason};

/// The size of a page of memory, in bytes.
const PAGE: usize = 4096;

/// How many pages of memory there are.
const PAGES: usize = (1 << 20) / PAGE;

const USAGE: &str = "\
emo8067 — headless 8086 runner

USAGE:
    emo8067 run <file.asm|file.bin> [options]

OPTIONS:
    --load SEG:OFF     load the file at SEG:OFF        (default 0100:0000, or 0000:ORG)
    --load-seg SEG     the segment to load the file in (default 0100, or 0000)
    --org SEG:OFF      set CS and IP to SEG:OFF        (default 0100:0000, or 0000:ORG)
    --ss SEG           set SS                           (default 0100, or 0000)
    --sp VALUE         set SP                           (default FFFE)
    --ds SEG           set DS                           (default 0100, or 0000)
    --max N            stop after N instructions       (default 100000)
    --include DIR      look for includes in DIR        (repeatable)
    --golden           print the memory that is not zero, page by page
    --trace            print one line per instruction
    --dump ADDR:LEN    hex dump of memory at physical ADDR
    --device-socket P serve the I/O bus on a unix socket at P
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
    let mut load: Option<u32> = None;
    let mut load_seg: Option<u16> = None;
    let mut org: Option<(u16, u16)> = None;
    let mut ss: Option<u16> = None;
    let mut sp: Option<u16> = None;
    let mut ds: Option<u16> = None;
    let mut max = 100_000u64;
    let mut trace = false;
    let mut golden = false;
    let mut dump: Option<(u32, usize)> = None;
    let mut device_socket: Option<PathBuf> = None;
    let mut includes: Vec<PathBuf> = Vec::new();

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
                load = Some(u32::from(s) * 16 + u32::from(o));
                i += 1;
            }
            "--load-seg" => {
                load_seg = Some(parse_hex(value()?)?);
                i += 1;
            }
            "--org" => {
                let v = value()?;
                org = Some(parse_addr(v)?);
                i += 1;
            }
            "--ss" => {
                ss = Some(parse_hex(value()?)?);
                i += 1;
            }
            "--sp" => {
                sp = Some(parse_hex(value()?)?);
                i += 1;
            }
            "--ds" => {
                ds = Some(parse_hex(value()?)?);
                i += 1;
            }
            "--max" => {
                max = value()?.parse().map_err(|_| "bad --max")?;
                i += 1;
            }
            "--device-socket" => {
                let v = value()?;
                device_socket = Some(std::path::PathBuf::from(v));
                i += 1;
            }
            "--trace" => trace = true,
            "--golden" => golden = true,
            "--include" => {
                let v = value()?;
                includes.push(PathBuf::from(v));
                i += 1;
            }
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
    let file = Path::new(path);

    // A `.asm` is assembled; anything else is loaded raw, as it always was.
    let (image, origin) = if file
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("asm"))
    {
        let program = program::assemble_file(file, &includes)
            .map_err(|error| assembly_error(path, &error))?;
        (program.bytes, Some(program.origin))
    } else {
        (
            std::fs::read(file).map_err(|e| format!("{path}: {e}"))?,
            None,
        )
    };

    // The load segment and the load offset. The reference's own model is
    // in its boot template: "load segment: 0000, load offset: 7c00, same
    // values are set to CS and IP registers accordingly", so a program is
    // loaded at its ORG in the segment 0000 and the segments follow it.
    // A raw image keeps the old defaults.
    let (default_seg, default_off) = match origin {
        Some(origin) => (0x0000u16, origin),
        None => (0x0100u16, 0x0000u16),
    };
    let load_seg = load_seg.unwrap_or(default_seg);
    let load = load.unwrap_or(u32::from(load_seg) * 16 + u32::from(default_off));
    let (cs, ip) = org.unwrap_or((load_seg, default_off));
    let ss = ss.unwrap_or(load_seg);
    let sp = sp.unwrap_or(0xFFFE);
    let ds = ds.unwrap_or(load_seg);

    let mut cpu = Cpu::new();
    cpu.mem.load_at(load, &image).map_err(|_a| {
        format!(
            "image does not fit: {load:#x} + {} bytes spills past 1 MiB",
            image.len()
        )
    })?;

    cpu.regs.set_segment(Seg::Cs, cs);
    cpu.regs.ip = ip;
    cpu.regs.set_segment(Seg::Ss, ss);
    cpu.regs.set_word(Reg16::Sp, sp);
    cpu.regs.set_segment(Seg::Ds, ds);
    cpu.regs.set_segment(Seg::Es, ds);

    // The device server: a device connects to the socket and its reads and
    // writes go to the bus. It is served between instructions, from this
    // thread, because the bus is shared with the emulator and a device is
    // not `Send`.
    let mut server = match &device_socket {
        Some(path) => Some(
            emo8067_core::transport::socket::DeviceServer::bind(path)
                .map_err(|e| format!("{}: {e}", path.display()))?,
        ),
        None => None,
    };

    let mut executed = 0u64;
    let mut failure: Option<String> = None;
    while executed < max {
        if let Some(server) = &mut server {
            if let Err(e) = server.poll(&mut cpu.bus.borrow_mut()) {
                failure = Some(format!("device socket: {e}"));
                break;
            }
        }
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
                    // The interrupt was taken. If the table has no handler
                    // for it, the program asked for a service the emulator
                    // does not have, and the run ends there.
                    if !handler_installed(&cpu, n) {
                        failure = Some(format!("took INT {n:#04x}"));
                        break;
                    }
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
    if golden {
        pages(&cpu);
    }
    if let Some((addr, len)) = dump {
        hexdump(&cpu, addr, len);
    }
    Ok(if failure.is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Whether the interrupt table holds a handler for the vector.
///
/// The table is four bytes per vector at `0000:0000`: the offset at `n*4`
/// and the segment at `n*4+2`. A vector of zeros is no handler at all.
fn handler_installed(cpu: &Cpu, n: u8) -> bool {
    let at = u16::from(n) * 4;
    cpu.mem.read_u16(0x0000, at) != 0 || cpu.mem.read_u16(0x0000, at + 2) != 0
}

/// The assembler's refusal, in its own words.
fn assembly_error(path: &str, error: &Error) -> String {
    fn message(error: &Error) -> String {
        match error {
            Error::AtLine { line, error } => format!("line {line}: {}", message(error)),
            Error::Include { message }
            | Error::RecursiveMacro { message }
            | Error::JumpOutOfRange { message }
            | Error::NoConvergence { message } => message.clone(),
            Error::UnknownInstruction { name } => format!("unknown instruction `{name}`"),
            Error::BadOperand { text } => format!("bad operand `{text}`"),
            Error::UnknownName { name } => format!("unknown name `{name}`"),
            Error::Empty => "the line has no instruction on it".to_string(),
        }
    }
    format!("{path}: {}", message(error))
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

/// The memory that is not zero, page by page, as hex.
///
/// This is the other half of the golden text: the state says what the
/// processor holds, and this says what the program wrote.
fn pages(cpu: &Cpu) {
    println!("--- memory ---");
    for page in 0..PAGES {
        let base = page * PAGE;
        let bytes = cpu.mem.slice(base as u32, PAGE);
        if bytes.iter().all(|byte| *byte == 0) {
            continue;
        }
        println!("page {base:05X}");
        for (row, chunk) in bytes.chunks(16).enumerate() {
            if chunk.iter().all(|byte| *byte == 0) {
                continue;
            }
            let hex: Vec<String> = chunk.iter().map(|byte| format!("{byte:02X}")).collect();
            println!("{:05X}  {}", base + row * 16, hex.join(" "));
        }
    }
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
