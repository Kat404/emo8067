//! The debug console: the commands it takes, and the words it says.
//!
//! The reference has a debug console of its own (`re/docs-txt/debug.txt`)
//! and it documents ten commands in it: `c`, `i`, `iw`, `o`, `ow`, `d`,
//! `t`, `p`, `r` and `rf`. Of them it says: "All output values are in hex.
//! All input parameters must be in hex. The h suffix is not required."
//!
//! **The reference documents the commands and not what they print**, so
//! every string this console says is this project's own: the registers and
//! the flags are printed as the panels draw them, so the project grows one
//! format and not two, and the refusals are the console's own words. The
//! only words of the reference this console reuses are the evaluator's
//! five, and only where the value is read by the evaluator.
//!
//! **Two forms of an address are not implemented**, and the console says
//! so instead of pretending: the effective addresses (`d bx+5`), because
//! the console reads plain addresses; and the symbols (`d lion L 10`),
//! because this project has no symbol table.

use emo8067_core::Cpu;
use emo8067_core::regs::Seg;
use emo8067_debug::expr;
use emo8067_debug::machine::Machine;
use emo8067_debug::panels::{Flag, Register};

/// The bytes a dump holds when the command names no count.
///
/// The reference's own words: "dump 128 bytes at specified address".
pub const DUMP_BYTES: usize = 128;

/// The bytes a compare holds when the command names no count.
///
/// The reference's own words: "If L parameter is not specified this
/// command compares 8 bytes by default".
pub const COMPARE_BYTES: usize = 8;

/// The lines the log keeps.
///
/// The log grows with the commands, and a long session must not grow it
/// without end: the oldest lines are the ones that go.
pub const OUTPUT_LINES: usize = 1000;

/// The registers the reference's expressions can name.
///
/// The names are the model's own, and a name in an address is an
/// effective address and not a number.
const REGISTERS: [&str; 13] = [
    "AX", "BX", "CX", "DX", "SI", "DI", "BP", "SP", "CS", "DS", "ES", "SS", "IP",
];

/// The console: the command line, the log of what the commands said, and
/// where the next dump begins.
#[derive(Clone, Debug, Default)]
pub struct Console {
    /// What has been typed on the command line.
    pub text: String,
    /// The log: one line at a time, the oldest first.
    pub output: Vec<String>,
    /// The last dump: where it was and how many bytes it held.
    pub dump: Option<Dump>,
}

/// The last dump: the segment it was in, the offset it began at, and how
/// many bytes it held.
///
/// The console keeps it because the reference's own `d` shows the next
/// block when it is used with no parameters at all: "Subsequent use of d
/// command without parameters shows next block of the memory."
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dump {
    /// The segment the dump was in.
    pub segment: u16,
    /// The offset the dump began at.
    pub offset: u16,
    /// How many bytes the dump held.
    pub count: usize,
}

impl Dump {
    /// The offset the next block begins at.
    #[must_use]
    pub fn next(&self) -> u16 {
        self.offset.wrapping_add(self.count as u16)
    }
}

/// A command of the console.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// `r` — the registers and the instruction at `CS:IP`.
    Registers,
    /// `r <register>` — one register, read.
    ReadRegister(Register),
    /// `r <register> <value>` — one register, written.
    WriteRegister {
        /// The register that is written.
        register: Register,
        /// The value it is given.
        value: u16,
    },
    /// `rf` — the flags.
    Flags,
    /// `rf <flag>` — one flag, read.
    ReadFlag(Flag),
    /// `rf <flag> <value>` — one flag, written.
    WriteFlag {
        /// The flag that is written.
        flag: Flag,
        /// The value it is given.
        value: bool,
    },
    /// `t` — one instruction.
    Step,
    /// `p` — a procedure or an interrupt, run to its end.
    StepOver,
    /// `d [address] [L count]` — a dump of the memory.
    Dump {
        /// Where the dump is, when the command named an address.
        address: Option<Address>,
        /// How many bytes the dump holds.
        count: usize,
    },
    /// `i <port>` and `iw <port>` — a port, read.
    ReadPort {
        /// The port that is read.
        port: u16,
        /// Whether the port is read as a word.
        word: bool,
    },
    /// `o <port> <value>` and `ow <port> <value>` — a port, written.
    WritePort {
        /// The port that is written.
        port: u16,
        /// The value it is given.
        value: u16,
        /// Whether the port is written as a word.
        word: bool,
    },
    /// `c <a> [L count] <b>` — two ranges of bytes, compared.
    Compare {
        /// Where the first range is.
        first: Address,
        /// How many bytes are compared.
        count: usize,
        /// Where the second range is.
        second: Address,
    },
}

/// Where an address points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Address {
    /// The segment, when the command named one.
    ///
    /// The segment is a name or a number, and the machine's own value of a
    /// named segment is the segment: `DS:0100` is the data segment the
    /// machine holds and not the number thirteen.
    pub segment: Option<Segment>,
    /// The offset the address is at.
    pub offset: u16,
}

/// The segment an address names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Segment {
    /// A segment register, by its own name.
    Register(Seg),
    /// A segment, by its number.
    Number(u16),
}

/// What the console refuses.
///
/// The words are this project's own: the reference documents its commands
/// and not what they say when they cannot do what they are asked, and the
/// only words of the reference the console reuses are the evaluator's
/// five — and only where the value is read by the evaluator.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The console does not know the command.
    UnknownCommand(String),
    /// The console does not know the register.
    UnknownRegister(String),
    /// The console does not know the flag.
    UnknownFlag(String),
    /// The argument is not a hexadecimal number.
    NotHexadecimal(String),
    /// The value does not fit where it is going.
    OutOfRange(i64),
    /// The command needs an argument that is not there.
    MissingArgument(&'static str),
    /// The command takes more arguments than it can.
    TooManyArguments,
    /// The command is one the console does not do.
    NotImplemented {
        /// What is not implemented.
        what: &'static str,
        /// Why it is not implemented.
        why: &'static str,
    },
    /// The value does not read, in the evaluator's own words.
    Expression(expr::Error),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::UnknownCommand(command) => {
                write!(f, "the console does not know the command: {command}")
            }
            Refusal::UnknownRegister(register) => {
                write!(f, "the console does not know the register: {register}")
            }
            Refusal::UnknownFlag(flag) => {
                write!(f, "the console does not know the flag: {flag}")
            }
            Refusal::NotHexadecimal(text) => write!(f, "not a hexadecimal number: {text}"),
            Refusal::OutOfRange(value) => write!(f, "value out of range: {value}"),
            Refusal::MissingArgument(what) => write!(f, "the command needs {what}"),
            Refusal::TooManyArguments => write!(f, "the command takes too many arguments"),
            Refusal::NotImplemented { what, why } => {
                write!(f, "not implemented: {what} ({why})")
            }
            Refusal::Expression(error) => write!(f, "{error}"),
        }
    }
}

/// Read the command that was typed.
///
/// The console's own grammar: the command word and its arguments. Every
/// parameter is hexadecimal and the `h` suffix is not required, the way
/// the reference types them: `d 0100` and `d 0100h` are the same address.
///
/// The machine is here because a value may be an expression, and an
/// expression is read against the machine: `BX+1` is a value the console
/// can write into a register.
pub fn parse(text: &str, cpu: &Cpu) -> Result<Command, Refusal> {
    let text = text.trim();
    let mut words = text.split_whitespace();
    let Some(word) = words.next() else {
        return Err(Refusal::MissingArgument("a command"));
    };
    let arguments: Vec<&str> = words.collect();
    match word.to_ascii_lowercase().as_str() {
        "r" => parse_registers(&arguments, cpu),
        "rf" => parse_flags(&arguments, cpu),
        "t" => {
            if arguments.is_empty() {
                Ok(Command::Step)
            } else {
                Err(Refusal::TooManyArguments)
            }
        }
        "d" => parse_dump(&arguments),
        "i" => parse_read_port(&arguments, false),
        "iw" => parse_read_port(&arguments, true),
        "o" => parse_write_port(&arguments, false, cpu),
        "ow" => parse_write_port(&arguments, true, cpu),
        "c" => parse_compare(&arguments),
        "p" => {
            if arguments.is_empty() {
                Ok(Command::StepOver)
            } else {
                Err(Refusal::TooManyArguments)
            }
        }
        other => Err(Refusal::UnknownCommand(other.to_string())),
    }
}

/// Read the `r` command: the registers, or one of them.
///
/// `r` shows every register and the instruction at `CS:IP`, `r AX` reads
/// one register, and `r AX 1234h` writes one.
fn parse_registers(arguments: &[&str], cpu: &Cpu) -> Result<Command, Refusal> {
    match arguments.len() {
        0 => Ok(Command::Registers),
        1 => Ok(Command::ReadRegister(register_of(arguments[0])?)),
        2 => {
            let register = register_of(arguments[0])?;
            let value = value_of(arguments[1], cpu)?;
            let value = u16::try_from(value).map_err(|_| Refusal::OutOfRange(value))?;
            Ok(Command::WriteRegister { register, value })
        }
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Read the `rf` command: the flags, or one of them.
///
/// `rf` shows every flag, `rf CF` reads one flag, and `rf CF 1` writes
/// one: the value of a flag is a zero or a one.
fn parse_flags(arguments: &[&str], cpu: &Cpu) -> Result<Command, Refusal> {
    match arguments.len() {
        0 => Ok(Command::Flags),
        1 => Ok(Command::ReadFlag(flag_of(arguments[0])?)),
        2 => {
            let flag = flag_of(arguments[0])?;
            let value = value_of(arguments[1], cpu)?;
            let value = match value {
                0 => false,
                1 => true,
                other => return Err(Refusal::OutOfRange(other)),
            };
            Ok(Command::WriteFlag { flag, value })
        }
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Read the `d` command: a dump of the memory.
///
/// `d` with no arguments shows the next block after the last dump, `d 0100`
/// dumps at an address, and `d 0100 L 20` dumps twenty bytes there. The
/// `L` is a word of its own, the way the reference types it: "There must
/// be spaces around L to distinguish from variable offset".
fn parse_dump(arguments: &[&str]) -> Result<Command, Refusal> {
    match arguments.len() {
        0 => Ok(Command::Dump {
            address: None,
            count: DUMP_BYTES,
        }),
        1 => {
            if is_length(arguments[0]) {
                return Err(Refusal::MissingArgument("a count"));
            }
            Ok(Command::Dump {
                address: Some(address_of(arguments[0])?),
                count: DUMP_BYTES,
            })
        }
        2 => {
            if is_length(arguments[0]) {
                return Ok(Command::Dump {
                    address: None,
                    count: count_of(arguments[1])?,
                });
            }
            if is_length(arguments[1]) {
                return Err(Refusal::MissingArgument("a count"));
            }
            Err(Refusal::MissingArgument("an L before the count"))
        }
        3 => {
            if !is_length(arguments[1]) {
                return Err(Refusal::MissingArgument("an L before the count"));
            }
            Ok(Command::Dump {
                address: Some(address_of(arguments[0])?),
                count: count_of(arguments[2])?,
            })
        }
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Read the `i` and `iw` commands: a port, read.
fn parse_read_port(arguments: &[&str], word: bool) -> Result<Command, Refusal> {
    match arguments.len() {
        1 => Ok(Command::ReadPort {
            port: port_of(arguments[0])?,
            word,
        }),
        0 => Err(Refusal::MissingArgument("a port")),
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Read the `o` and `ow` commands: a port, written.
///
/// The value of a port is in hexadecimal, and a byte port takes a byte and
/// a word port takes a word.
fn parse_write_port(arguments: &[&str], word: bool, cpu: &Cpu) -> Result<Command, Refusal> {
    match arguments.len() {
        2 => {
            let port = port_of(arguments[0])?;
            let value = value_of(arguments[1], cpu)?;
            let limit = if word { 0xFFFF } else { 0xFF };
            if !(0..=limit).contains(&value) {
                return Err(Refusal::OutOfRange(value));
            }
            Ok(Command::WritePort {
                port,
                value: value as u16,
                word,
            })
        }
        0 | 1 => Err(Refusal::MissingArgument("a value")),
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Read the `c` command: two ranges of bytes, compared.
///
/// `c 0100 0200` compares eight bytes, and `c 0100 L 10 0200` compares
/// sixteen: the `L` is a word of its own, the way the reference types it.
fn parse_compare(arguments: &[&str]) -> Result<Command, Refusal> {
    match arguments.len() {
        2 => Ok(Command::Compare {
            first: address_of(arguments[0])?,
            count: COMPARE_BYTES,
            second: address_of(arguments[1])?,
        }),
        4 => {
            if !is_length(arguments[1]) {
                return Err(Refusal::MissingArgument("an L before the count"));
            }
            Ok(Command::Compare {
                first: address_of(arguments[0])?,
                count: count_of(arguments[2])?,
                second: address_of(arguments[3])?,
            })
        }
        3 => {
            if is_length(arguments[1]) {
                return Err(Refusal::MissingArgument("a second address"));
            }
            Err(Refusal::TooManyArguments)
        }
        0 | 1 => Err(Refusal::MissingArgument("two addresses")),
        _ => Err(Refusal::TooManyArguments),
    }
}

/// Whether a word is the reference's own `L`.
///
/// The `L` is the count of a dump or of a compare, and it is a word of
/// its own: "There must be spaces around L to distinguish from variable
/// offset".
fn is_length(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("l")
}

/// Read the register a command names.
///
/// The registers are the model's own: the thirteen the panel shows, by the
/// names the panel gives them.
fn register_of(text: &str) -> Result<Register, Refusal> {
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    Register::ALL
        .iter()
        .find(|register| register.name() == upper)
        .copied()
        .ok_or_else(|| Refusal::UnknownRegister(trimmed.to_string()))
}

/// Read the flag a command names.
///
/// The flags are the model's own: the nine the panel shows, by the names
/// the panel gives them.
fn flag_of(text: &str) -> Result<Flag, Refusal> {
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    Flag::ALL
        .iter()
        .find(|flag| flag.name() == upper)
        .copied()
        .ok_or_else(|| Refusal::UnknownFlag(trimmed.to_string()))
}

/// Read the value of a command.
///
/// The console's parameters are hexadecimal and the `h` suffix is not
/// required, so `BEEF` and `BEEFh` are the same value. A text that is not
/// a plain hexadecimal number is read in the reference's own expression
/// language, so `BX+1` is a value too, and a value that does not read is
/// refused in the evaluator's own words.
fn value_of(text: &str, cpu: &Cpu) -> Result<i64, Refusal> {
    let trimmed = text.trim();
    if let Some(value) = hexadecimal(trimmed) {
        return Ok(value);
    }
    expr::evaluate(trimmed, &Machine::new(cpu))
        .map(i64::from)
        .map_err(Refusal::Expression)
}

/// Read a hexadecimal number, with the `h` suffix optional.
///
/// The suffix is not required, the way the reference types its numbers:
/// "The h suffix is not required."
fn hexadecimal(text: &str) -> Option<i64> {
    let digits = text.strip_suffix(['h', 'H']).unwrap_or(text);
    u32::from_str_radix(digits, 16).ok().map(i64::from)
}

/// Read the port a command names.
///
/// The port space is sixteen bits wide, and a port that is not a number of
/// it cannot be reached.
fn port_of(text: &str) -> Result<u16, Refusal> {
    let trimmed = text.trim();
    let value = hexadecimal(trimmed).ok_or_else(|| Refusal::NotHexadecimal(trimmed.to_string()))?;
    u16::try_from(value).map_err(|_| Refusal::OutOfRange(value))
}

/// Read the count a command takes.
///
/// The count is hexadecimal, the way the reference types it: `L 10` is
/// sixteen bytes.
fn count_of(text: &str) -> Result<usize, Refusal> {
    let trimmed = text.trim();
    let value = hexadecimal(trimmed).ok_or_else(|| Refusal::NotHexadecimal(trimmed.to_string()))?;
    u16::try_from(value)
        .map(usize::from)
        .map_err(|_| Refusal::OutOfRange(value))
}

/// Read the address a command takes.
///
/// The address is hexadecimal and the segment defaults to the code
/// segment, the way the reference types it: "If no segment is specified CS
/// is used by default."
fn address_of(text: &str) -> Result<Address, Refusal> {
    let trimmed = text.trim();
    if let Some((left, right)) = trimmed.split_once(':') {
        return Ok(Address {
            segment: Some(segment_of(left)?),
            offset: offset_of(right)?,
        });
    }
    Ok(Address {
        segment: None,
        offset: offset_of(trimmed)?,
    })
}

/// Read the segment a memory reference names.
///
/// The four segment registers have names, and the machine's own value of
/// the one that is named is the segment: `DS:0100` is the data segment the
/// machine holds and not the number thirteen.
fn segment_of(text: &str) -> Result<Segment, Refusal> {
    let trimmed = text.trim();
    let segment = match trimmed.to_ascii_uppercase().as_str() {
        "CS" => Seg::Cs,
        "DS" => Seg::Ds,
        "ES" => Seg::Es,
        "SS" => Seg::Ss,
        _ => {
            let value =
                hexadecimal(trimmed).ok_or_else(|| Refusal::NotHexadecimal(trimmed.to_string()))?;
            let value = u16::try_from(value).map_err(|_| Refusal::OutOfRange(value))?;
            return Ok(Segment::Number(value));
        }
    };
    Ok(Segment::Register(segment))
}

/// Read the offset of an address.
///
/// The offset is hexadecimal and the `h` suffix is not required. An offset
/// the console cannot read is refused in the console's own words: an
/// effective address and a symbol are not implemented, and anything else
/// that is not a hexadecimal number is refused as one.
fn offset_of(text: &str) -> Result<u16, Refusal> {
    let trimmed = text.trim();
    if let Some(value) = hexadecimal(trimmed) {
        return u16::try_from(value).map_err(|_| Refusal::OutOfRange(value));
    }
    Err(address_refusal(trimmed))
}

/// The refusal of an address the console cannot read.
///
/// The reference has three kinds of address and this console reads one of
/// them: a plain hexadecimal offset, with a segment in front of it when
/// the command names one. An effective address is not implemented — the
/// console reads plain addresses — and a name is a symbol, and this
/// project has no symbol table for them.
fn address_refusal(text: &str) -> Refusal {
    if text.contains(['[', ']', '+', '-']) {
        return Refusal::NotImplemented {
            what: "effective addresses",
            why: "the console reads plain addresses",
        };
    }
    let upper = text.to_ascii_uppercase();
    if REGISTERS.contains(&upper.as_str()) {
        return Refusal::NotImplemented {
            what: "effective addresses",
            why: "the console reads plain addresses",
        };
    }
    if text
        .chars()
        .next()
        .is_some_and(|letter| letter.is_ascii_alphabetic())
    {
        return Refusal::NotImplemented {
            what: "symbols",
            why: "this project has no symbol table",
        };
    }
    Refusal::NotHexadecimal(text.to_string())
}

/// A row of a dump: the address, the bytes as hex, and the printable side.
///
/// The row is drawn the way the memory pane draws its own rows: the
/// address in the gutter, the bytes as hex, and the printable side beside
/// them.
#[must_use]
pub fn dump_row(segment: u16, offset: u16, bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    let printable: String = bytes.iter().map(|byte| printable_of(*byte)).collect();
    format!("{segment:04X}:{offset:04X} {hex} {printable}")
}

/// The printable side of a byte: the character it is, or a dot.
///
/// The dot is what a hex panel shows for a byte that is not a character.
fn printable_of(byte: u8) -> char {
    if (0x20..0x7F).contains(&byte) {
        byte as char
    } else {
        '.'
    }
}

/// A row of a compare: the two addresses and the two bytes that differ.
///
/// The reference's own words: "The command outputs only those addresses
/// with values that are different. Equal bytes are not printed out."
#[must_use]
pub fn compare_row(
    first_segment: u16,
    first_offset: u16,
    first: u8,
    second_segment: u16,
    second_offset: u16,
    second: u8,
) -> String {
    format!(
        "{first_segment:04X}:{first_offset:04X}  {first:02X} != {second_segment:04X}:{second_offset:04X}  {second:02X}"
    )
}
