//! The 80187 range, decoded for display only.
//!
//! The 8086 has no instruction in the `D8`–`DF` range: the bytes are an
//! ESC index, and the reference's own disassembler shows the
//! coprocessor's mnemonics for them without ever executing them. The
//! reference's table is the list of mnemonics in `re/FINDINGS.md`, and the
//! crate reads that document at compile time the way the assembler reads
//! the guide: the names the reference used are the names the crate prints.

/// The reference's own findings, read at compile time.
const FINDINGS: &str = include_str!("../../../re/FINDINGS.md");

/// The reference's own spelling of a mnemonic, taken from the document.
///
/// The recovery is a partial one - it names forty-six of the mnemonics the
/// reference's table holds - so a name the document does not carry is the
/// crate's own.
fn spelling(name: &'static str) -> &'static str {
    FINDINGS
        .find(name)
        .map(|at| &FINDINGS[at..at + name.len()])
        .unwrap_or(name)
}

/// What the x87 decoder found for one byte.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Operation {
    /// A named operation: `FADD`, `FLD`.
    Named(&'static str),
    /// An F operation whose name the table does not give.
    UnknownF,
    /// An operation that is not known to be an F one at all.
    Unknown,
}

/// What the operation works on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// Nothing: the operation takes no operand.
    None,
    /// The stack register `ST(i)`.
    Stack(u8),
    /// A memory reference.
    Memory,
}

/// The real arithmetic: the `D8` and `DC` groups.
const REAL: [&str; 8] = [
    "FADD", "FMUL", "FCOM", "FCOMP", "FSUB", "FSUBR", "FDIV", "FDIVR",
];

/// The integer arithmetic: the `DA` and `DE` groups.
const INTEGER: [&str; 8] = [
    "FIADD", "FIMUL", "FICOM", "FICOMP", "FISUB", "FISUBR", "FIDIV", "FIDIVR",
];

/// The `DE` group with the pop: the reference names the register forms
/// after the subtraction, not after the integer one.
const POP: [&str; 8] = [
    "FADDP", "FMULP", "FICOM", "FCOMPP", "FSUBRP", "FSUBP", "FDIVRP", "FDIVP",
];

/// The `D9` group with the control words: the 8087's `FNDISI`, `FNCLEX`
/// and `FNINIT`, which the 80187 renamed.
const CONTROL: [&str; 8] = [
    "FNENI",
    "FDISI",
    "FCLEX",
    "FINIT",
    "FSETPM",
    "FNSTSW AX",
    "",
    "",
];

/// The `D9` group with the status word.
const STATUS: [&str; 8] = [
    "FCHS", "FABS", "FTST", "FXAM", "FSTP1", "FLD1", "FLDL2T", "FLDL2E",
];

/// The `D9` group with the constants.
const CONSTANTS: [&str; 8] = [
    "FLDLG2", "FLDLN2", "FLDPI", "FLDZ", "FLDST0", "FLDST1", "F2XM1", "FYL2X",
];

/// The `D9` group with the memory: the environment and the control word.
const ENVIRONMENT: [&str; 8] = [
    "FLD", "", "FST", "FSTP", "FLDENV", "FLDCW", "FNSTENV", "FNSTCW",
];

/// The `DD` group with the memory.
const STORE: [&str; 8] = [
    "FLD", "", "FST", "FSTP", "FRSTOR", "FLDENV", "FNSAVE", "FNSTSW",
];

/// The `DB` and `DF` group with the integers.
const INTEGER_STORE: [&str; 8] = [
    "FILD", "FIST", "FISTP", "", "FBLD", "FBSTP", "FISTP", "FISTP",
];

/// The operation the bytes name, and what it works on.
///
/// The ModR/M byte's MOD field says whether the operand is a register or a
/// reference, and the REG field says which group it is. The `D9` groups
/// with the control words and the constants are the escape blocks: the
/// REG field names the block and the R/M field the operation in it.
#[must_use]
pub fn decode(opcode: u8, modrm: u8) -> (Operation, Target) {
    let mod_ = modrm >> 6;
    let reg = (modrm >> 3) & 0b111;
    let rm = modrm & 0b111;
    if opcode == 0xD9 {
        if mod_ == 0b11 {
            return match reg {
                0b000 => (named("FLD"), Target::Stack(rm)),
                0b001 => (named("FXCH"), Target::Stack(rm)),
                0b010 => (
                    if rm == 0 {
                        named("FNOP")
                    } else {
                        Operation::UnknownF
                    },
                    Target::None,
                ),
                0b011 | 0b101 => (Operation::Unknown, Target::Stack(rm)),
                0b100 => (block(&CONTROL, rm), Target::None),
                0b110 => (block(&STATUS, rm), Target::None),
                _ => (block(&CONSTANTS, rm), Target::None),
            };
        }
        return (group(&ENVIRONMENT, reg), Target::Memory);
    }
    let table: &[&'static str; 8] = match opcode {
        0xD8 | 0xDC => &REAL,
        0xDA => {
            if mod_ == 0b11 && rm == 0b001 {
                // The `DA` group with the pop: the R/M field says 001 and
                // the REG field 101 is the comparison that pops twice.
                if reg == 0b101 {
                    return (named("FUCOMPP"), Target::Stack(0));
                }
                return (Operation::UnknownF, Target::Stack(0));
            }
            &INTEGER
        }
        0xDE => {
            if mod_ == 0b11 && rm == 0b001 {
                // The `DE` group with the pop: the R/M field says 001 and
                // the REG field which operation it is.
                return (group(&POP, reg), Target::Stack(0));
            }
            &INTEGER
        }
        0xDB | 0xDF => &INTEGER_STORE,
        0xDD => &STORE,
        _ => return (Operation::Unknown, Target::None),
    };
    let operation = group(table, reg);
    let target = if mod_ == 0b11 {
        Target::Stack(rm)
    } else {
        Target::Memory
    };
    (operation, target)
}

/// A named operation, spelled the way the reference spells it.
fn named(name: &'static str) -> Operation {
    Operation::Named(spelling(name))
}

/// The operation of a group, or the fallback when the group does not name
/// it.
fn group(table: &[&'static str; 8], reg: u8) -> Operation {
    match table[reg as usize] {
        "" => Operation::UnknownF,
        name => named(name),
    }
}

/// The operation of an escape block, or the fallback when the block does
/// not name it.
fn block(table: &[&'static str; 8], rm: u8) -> Operation {
    match table[rm as usize] {
        "" => Operation::UnknownF,
        name => named(name),
    }
}
