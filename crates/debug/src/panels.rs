//! The panel model.
//!
//! Every panel is a pure function of the machine's state and of a UI state
//! struct, which is what the research settled (`features-and-ux.md` §4.3):
//! the TUI and the GUI are two renderers of the same model, and nothing
//! here draws.
//!
//! The panels themselves are the ones the reference has, and the layout the
//! research draws (`features-and-ux.md` §4.4): the disassembly, the
//! registers with their flags, the memory as hex and characters, and the
//! stack.

use emo8067_core::Cpu;
use emo8067_core::flags::{AF, CF, DF, IF, OF, PF, SF, TF, ZF};
use emo8067_core::regs::{Reg16, Seg};

/// What the panels show, besides the machine's own state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ui {
    /// Where the memory panel looks.
    pub memory_at: u16,
    /// The segment the memory panel looks through.
    pub memory_segment: u16,
    /// How many bytes the memory panel shows.
    pub memory_len: usize,
    /// Where the disassembly starts.
    pub disassembly_at: u16,
    /// How many instructions the disassembly lists.
    pub disassembly_len: usize,
    /// How many words the stack panel shows.
    pub stack_len: usize,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            memory_at: 0,
            memory_segment: 0x1000,
            memory_len: 16,
            disassembly_at: 0,
            disassembly_len: 16,
            stack_len: 8,
        }
    }
}

/// One line of the disassembly panel.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Line {
    /// The logical address the instruction is at.
    pub segment: u16,
    /// The offset it is at.
    pub offset: u16,
    /// The instruction's bytes.
    pub bytes: Vec<u8>,
    /// The reference's own text for it.
    pub text: String,
}

/// The registers panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Registers {
    /// `AX`.
    pub ax: u16,
    /// `BX`.
    pub bx: u16,
    /// `CX`.
    pub cx: u16,
    /// `DX`.
    pub dx: u16,
    /// `SP`.
    pub sp: u16,
    /// `BP`.
    pub bp: u16,
    /// `SI`.
    pub si: u16,
    /// `DI`.
    pub di: u16,
    /// `CS`.
    pub cs: u16,
    /// `DS`.
    pub ds: u16,
    /// `ES`.
    pub es: u16,
    /// `SS`.
    pub ss: u16,
    /// `IP`.
    pub ip: u16,
    /// The flags word, as the machine holds it.
    pub flags: u16,
}

/// The registers the panel shows, in the order it shows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Register {
    /// One of the eight general registers.
    Word(Reg16),
    /// One of the four segment registers.
    Segment(Seg),
    /// The instruction pointer.
    Ip,
}

impl Register {
    /// Every register, in the order the panel shows them.
    pub const ALL: [Register; 13] = [
        Register::Word(Reg16::Ax),
        Register::Word(Reg16::Bx),
        Register::Word(Reg16::Cx),
        Register::Word(Reg16::Dx),
        Register::Word(Reg16::Si),
        Register::Word(Reg16::Di),
        Register::Word(Reg16::Bp),
        Register::Word(Reg16::Sp),
        Register::Segment(Seg::Cs),
        Register::Segment(Seg::Ds),
        Register::Segment(Seg::Es),
        Register::Segment(Seg::Ss),
        Register::Ip,
    ];

    /// The name the panel shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Register::Word(Reg16::Ax) => "AX",
            Register::Word(Reg16::Bx) => "BX",
            Register::Word(Reg16::Cx) => "CX",
            Register::Word(Reg16::Dx) => "DX",
            Register::Word(Reg16::Si) => "SI",
            Register::Word(Reg16::Di) => "DI",
            Register::Word(Reg16::Bp) => "BP",
            Register::Word(Reg16::Sp) => "SP",
            Register::Segment(Seg::Cs) => "CS",
            Register::Segment(Seg::Ds) => "DS",
            Register::Segment(Seg::Es) => "ES",
            Register::Segment(Seg::Ss) => "SS",
            Register::Ip => "IP",
        }
    }
}

impl Register {
    /// Write a value into this register.
    ///
    /// The other side of the panel's door: the model reads the machine, and
    /// this writes it.
    pub fn write(self, cpu: &mut Cpu, value: u16) {
        match self {
            Register::Word(reg) => cpu.regs.set_word(reg, value),
            Register::Segment(seg) => cpu.regs.set_segment(seg, value),
            Register::Ip => cpu.regs.ip = value,
        }
    }
}

/// One register of the panel: which one it is, and what it holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    /// Which register it is.
    pub register: Register,
    /// The value the machine holds in it.
    pub value: u16,
}

impl Cell {
    /// A cell of the panel.
    #[must_use]
    pub const fn new(register: Register, value: u16) -> Cell {
        Cell { register, value }
    }
}

/// The nine flags of the 8086.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    /// Carry.
    Cf,
    /// Parity.
    Pf,
    /// Auxiliary carry.
    Af,
    /// Zero.
    Zf,
    /// Sign.
    Sf,
    /// Trap.
    Tf,
    /// Overflow.
    Of,
    /// Interrupt enable — the manual's `IF`.
    If,
    /// Direction.
    Df,
}

impl Flag {
    /// Every flag, in the order the panel shows them.
    pub const ALL: [Flag; 9] = [
        Flag::Cf,
        Flag::Pf,
        Flag::Af,
        Flag::Zf,
        Flag::Sf,
        Flag::Tf,
        Flag::Of,
        Flag::If,
        Flag::Df,
    ];

    /// The name the manual prints.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Flag::Cf => "CF",
            Flag::Pf => "PF",
            Flag::Af => "AF",
            Flag::Zf => "ZF",
            Flag::Sf => "SF",
            Flag::Tf => "TF",
            Flag::Of => "OF",
            Flag::If => "IF",
            Flag::Df => "DF",
        }
    }

    /// The bit the flag occupies in the flags word.
    #[must_use]
    pub fn mask(self) -> u16 {
        match self {
            Flag::Cf => CF,
            Flag::Pf => PF,
            Flag::Af => AF,
            Flag::Zf => ZF,
            Flag::Sf => SF,
            Flag::Tf => TF,
            Flag::Of => OF,
            Flag::If => IF,
            Flag::Df => DF,
        }
    }
}

impl Registers {
    /// The registers, in the order the panel shows them: which one each is,
    /// and what the machine holds in it.
    #[must_use]
    pub fn cells(&self) -> [Cell; 13] {
        [
            Cell::new(Register::Word(Reg16::Ax), self.ax),
            Cell::new(Register::Word(Reg16::Bx), self.bx),
            Cell::new(Register::Word(Reg16::Cx), self.cx),
            Cell::new(Register::Word(Reg16::Dx), self.dx),
            Cell::new(Register::Word(Reg16::Si), self.si),
            Cell::new(Register::Word(Reg16::Di), self.di),
            Cell::new(Register::Word(Reg16::Bp), self.bp),
            Cell::new(Register::Word(Reg16::Sp), self.sp),
            Cell::new(Register::Segment(Seg::Cs), self.cs),
            Cell::new(Register::Segment(Seg::Ds), self.ds),
            Cell::new(Register::Segment(Seg::Es), self.es),
            Cell::new(Register::Segment(Seg::Ss), self.ss),
            Cell::new(Register::Ip, self.ip),
        ]
    }

    /// The value of one of the nine flags.
    #[must_use]
    pub fn flag(&self, flag: Flag) -> bool {
        self.flags & flag.mask() != 0
    }
}

/// The memory panel: the bytes, and the same bytes as characters.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemoryView {
    /// The segment the bytes are read through.
    pub segment: u16,
    /// The offset the view starts at.
    pub at: u16,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// The same bytes as characters: the printable ones, and a dot for the
    /// rest, which is what a hex panel shows.
    pub ascii: String,
}

/// The stack panel: the words on the stack, from the top down.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackView {
    /// The segment the stack is in.
    pub segment: u16,
    /// The offset the stack pointer is at.
    pub sp: u16,
    /// The words, from the top of the stack down.
    pub words: Vec<u16>,
}

/// Every panel, drawn from the machine and the UI's state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panels {
    /// The disassembly panel.
    pub disassembly: Vec<Line>,
    /// The registers panel.
    pub registers: Registers,
    /// The memory panel.
    pub memory: MemoryView,
    /// The stack panel.
    pub stack: StackView,
}

impl Panels {
    /// Draw every panel from the machine and the UI's state.
    ///
    /// Nothing here reads a terminal: the same call serves the TUI and the
    /// GUI.
    #[must_use]
    pub fn of(cpu: &Cpu, ui: &Ui) -> Panels {
        let registers = Registers {
            ax: cpu.regs.word(Reg16::Ax),
            bx: cpu.regs.word(Reg16::Bx),
            cx: cpu.regs.word(Reg16::Cx),
            dx: cpu.regs.word(Reg16::Dx),
            sp: cpu.regs.word(Reg16::Sp),
            bp: cpu.regs.word(Reg16::Bp),
            si: cpu.regs.word(Reg16::Si),
            di: cpu.regs.word(Reg16::Di),
            cs: cpu.regs.segment(Seg::Cs),
            ds: cpu.regs.segment(Seg::Ds),
            es: cpu.regs.segment(Seg::Es),
            ss: cpu.regs.segment(Seg::Ss),
            ip: cpu.regs.ip,
            flags: cpu.regs.flags.0,
        };
        Panels {
            disassembly: disassembly_of(cpu, ui),
            registers,
            memory: memory_of(cpu, ui),
            stack: stack_of(cpu, ui),
        }
    }
}

/// The disassembly panel: the instructions from where the UI says.
fn disassembly_of(cpu: &Cpu, ui: &Ui) -> Vec<Line> {
    let segment = cpu.regs.segment(Seg::Cs);
    let mut bytes = Vec::new();
    for offset in 0..=255u16 {
        bytes.push(
            cpu.mem
                .read_u8(segment, ui.disassembly_at.wrapping_add(offset)),
        );
    }
    let mut out = Vec::new();
    let mut at = ui.disassembly_at;
    for _ in 0..ui.disassembly_len {
        let rest = &bytes[usize::from(at.wrapping_sub(ui.disassembly_at))..];
        let Ok(instruction) = emo8067_asm::disasm::disassemble(rest, at) else {
            break;
        };
        let length = instruction.length;
        out.push(Line {
            segment,
            offset: at,
            bytes: rest[..length.min(rest.len())].to_vec(),
            text: instruction.text,
        });
        at = at.wrapping_add(length as u16);
    }
    out
}

/// The memory panel: the bytes from where the UI says.
fn memory_of(cpu: &Cpu, ui: &Ui) -> MemoryView {
    let bytes = (0..ui.memory_len)
        .map(|offset| {
            cpu.mem
                .read_u8(ui.memory_segment, ui.memory_at.wrapping_add(offset as u16))
        })
        .collect::<Vec<u8>>();
    let ascii = bytes
        .iter()
        .map(|byte| {
            if (0x20..0x7F).contains(byte) {
                *byte as char
            } else {
                '.'
            }
        })
        .collect();
    MemoryView {
        segment: ui.memory_segment,
        at: ui.memory_at,
        bytes,
        ascii,
    }
}

/// The stack panel: the words on the stack, from the top down.
fn stack_of(cpu: &Cpu, ui: &Ui) -> StackView {
    let segment = cpu.regs.segment(Seg::Ss);
    let sp = cpu.regs.word(Reg16::Sp);
    let words = (0..ui.stack_len)
        .map(|index| {
            let offset = sp.wrapping_add(2 * index as u16);
            cpu.mem.read_u16(segment, offset)
        })
        .collect();
    StackView { segment, sp, words }
}
