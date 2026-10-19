//! The group tables: the forms a `/digit` chooses between.
//!
//! The grid in §1.3 names a group rather than an instruction: the cell for
//! `80` says "Group 1", and which operation that is depends on the ModR/M
//! byte's digit. The document keeps those choices in three tables of its
//! own — Group 1, Shift and Group 2 — and the assembler reads them the same
//! way it reads the grid.

/// The guide, read at compile time.
const GUIDE: &str = include_str!("../../../odd/research/documentation-guide.md");

/// One form of a group: the opcode, the digit that chooses it, and the
/// form the document prints for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GroupForm {
    /// The opcode the group hangs off.
    pub opcode: u8,
    /// The ModR/M digit that selects this form.
    pub digit: u8,
    /// The mnemonic and the operand form, verbatim: "ADD r/m8,i8".
    pub text: &'static str,
}

impl GroupForm {
    /// The mnemonic, with the operands taken off.
    #[must_use]
    pub fn mnemonic(&self) -> &'static str {
        self.text.split_once(' ').map_or(self.text, |(m, _)| m)
    }

    /// The operand form, with the mnemonic taken off.
    #[must_use]
    pub fn operands(&self) -> &'static str {
        self.text.split_once(' ').map_or("", |(_, o)| o)
    }
}

/// Read the group tables out of the document.
#[must_use]
pub fn forms() -> Vec<GroupForm> {
    let mut out = Vec::new();
    let mut opcodes: Vec<u8> = Vec::new();
    let mut in_table = false;
    for line in GUIDE.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let Some(first) = cells.first() else {
            continue;
        };
        if first.trim_matches('`') == "REG" {
            // The header row: each cell names the opcode it belongs to.
            opcodes = cells[1..]
                .iter()
                .filter_map(|cell| {
                    let hex = cell.split('`').nth(1)?;
                    u8::from_str_radix(hex, 16).ok()
                })
                .collect();
            in_table = true;
            continue;
        }
        if !in_table || opcodes.is_empty() {
            continue;
        }
        let Ok(digit) = u8::from_str_radix(first.trim_matches('`'), 2) else {
            continue;
        };
        for (opcode, cell) in opcodes.iter().zip(cells[1..].iter()) {
            let text = cell.trim();
            if text.is_empty() {
                continue;
            }
            out.push(GroupForm {
                opcode: *opcode,
                digit,
                text,
            });
        }
    }
    out
}

/// The form a group gives one of its operations.
///
/// A group is chosen by what the source says, not by the digit: `ADD` in
/// Group 1 is the digit whose cell says `ADD`, and the same for every other
/// operation. That is how the reference chose it too — the mnemonic is the
/// operation, and the digit is the encoding.
#[must_use]
pub fn find(opcode: u8, mnemonic: &str) -> Option<GroupForm> {
    forms()
        .into_iter()
        .find(|form| form.opcode == opcode && form.mnemonic() == mnemonic)
}
