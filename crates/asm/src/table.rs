//! The opcode map, read out of the document the emulator is checked against.
//!
//! §1.3 of `odd/research/documentation-guide.md` is this project's
//! transcription of the manual's Table 4-13: sixteen rows, one per opcode
//! pair, and sixteen cells in each row, eight per opcode. The assembler reads
//! that table rather than keeping a second copy of it, so that an assembler
//! bug and a transcription bug cannot both be true and cancel out.
//!
//! The table is prose with a shape, and the shape is regular: every cell is
//! a mnemonic and an operand form, separated by a space.

/// The guide, read at compile time so the assembler and the document cannot
/// disagree about which document they are talking about.
const GUIDE: &str = include_str!("../../../odd/research/documentation-guide.md");

/// How many cells each row of the map has: one cell per opcode, so sixteen
/// opcodes from 00h to 0Fh.
pub const CELLS_PER_ROW: usize = 16;

/// One cell of the opcode map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Form {
    /// The opcode the cell belongs to.
    pub opcode: u8,
    /// The cell's place in the row, which is the opcode's low nibble.
    pub column: u8,
    /// The mnemonic, empty when the cell is not an instruction: a prefix
    /// or a group reference.
    pub mnemonic: &'static str,
    /// The operand form, verbatim from the document: `r/m8,r8`, `i8`,
    /// `short`, `sreg`, and so on.
    pub operands: &'static str,
}

impl Form {
    /// Whether the cell names an instruction rather than a group.
    #[must_use]
    pub fn is_instruction(&self) -> bool {
        !self.mnemonic.is_empty() && !self.mnemonic.starts_with('G')
    }
}

/// Read the opcode map out of the document.
#[must_use]
pub fn forms() -> Vec<Form> {
    let mut out = Vec::new();
    let mut opcode = 0u8;
    let mut seen = 0usize;
    for line in GUIDE.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("| **") else {
            continue;
        };
        let Some((label, rest)) = rest.split_once("** |") else {
            continue;
        };
        let Ok(label) = u8::from_str_radix(label.trim(), 16) else {
            continue;
        };
        // The row's cells, with the footnotes taken off: they are the
        // document's commentary, not part of the form.
        let cells: Vec<&str> = rest
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect();
        debug_assert_eq!(cells.len(), CELLS_PER_ROW);
        for (column, cell) in cells.iter().enumerate() {
            let cell = cell.split('‡').next().unwrap_or(cell).trim();
            let (mnemonic, operands) = match cell.split_once(' ') {
                Some((mnemonic, operands)) => (mnemonic.trim(), operands.trim()),
                None => (cell, ""),
            };
            out.push(Form {
                opcode: label.wrapping_add(column as u8),
                column: column as u8,
                mnemonic,
                operands,
            });
        }
        seen += 1;
        opcode = opcode.wrapping_add(2);
    }
    debug_assert_eq!(seen, 16, "the map has sixteen rows");
    debug_assert_eq!(out.len(), 256, "the map has one cell per opcode");
    out
}

/// The forms of one opcode.
#[must_use]
pub fn forms_of(opcode: u8) -> Vec<Form> {
    forms()
        .into_iter()
        .filter(|form| form.opcode == opcode)
        .collect()
}
