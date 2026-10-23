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
#[derive(Clone, PartialEq, Eq, Debug)]
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

    /// The mnemonic and the operand form, in one go.
    #[must_use]
    pub fn mnemonic_form(&self) -> (&'static str, &'static str) {
        match self.text.split_once(' ') {
            Some((mnemonic, operands)) => (mnemonic, operands.trim()),
            None => (self.text, ""),
        }
    }

    /// The names the cell answers to, when it carries more than one:
    /// `SHL/SAL` is both.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.mnemonic().split('/').map(str::trim)
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
            // The footnotes are the document's commentary, not part of the
            // form: `PUSH r/m8 \u{2021}8` is `PUSH r/m8`.
            let text = cell.split('\u{2021}').next().unwrap_or(cell).trim();
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

/// The group's cell whose text is exactly this.
///
/// The group's digit is in the ModR/M byte, and the cell that names the
/// operation also says which operands it takes, so the cell's own text is
/// enough to name it: `CALL m16 (inter)` is `FF /3` and `CALL r16/m16
/// (intra)` is `FF /2`.
#[must_use]
pub fn find_text(opcode: u8, mnemonic: &str, text: &str) -> Option<GroupForm> {
    forms().into_iter().find(|form| {
        form.opcode == opcode
            && form.text == text
            && form.names().any(|name| name.eq_ignore_ascii_case(mnemonic))
    })
}

/// The form a group gives one of its operations.
///
/// A group is chosen by what the source says, not by the digit: `ADD` in
/// Group 1 is the digit whose cell says `ADD`, and the same for every other
/// operation. That is how the reference chose it too — the mnemonic is the
/// operation, and the digit is the encoding.
#[must_use]
pub fn find(opcode: u8, mnemonic: &str) -> Option<GroupForm> {
    find_for(opcode, mnemonic, None)
}

/// The form the group's digit chooses.
///
/// The digit is in the ModR/M byte, so a disassembler reads the byte and
/// asks for the form it names.
#[must_use]
pub fn find_digit(opcode: u8, digit: u8) -> Option<GroupForm> {
    forms()
        .into_iter()
        .find(|form| form.opcode == opcode && form.digit == digit)
}

/// The group's form for an operation, preferring the one that fits.
///
/// A group can name the same operation more than once: the 8086's Group 1
/// has `ADD r/m8,i8` and `ADD r/m16,i16` under the same digit. The digit
/// is the same, so the operands say which one was meant.
pub fn find_for(
    opcode: u8,
    mnemonic: &str,
    fits: Option<&dyn Fn(&str) -> bool>,
) -> Option<GroupForm> {
    let candidates: Vec<GroupForm> = forms()
        .into_iter()
        .filter(|form| {
            form.opcode == opcode && form.names().any(|name| name.eq_ignore_ascii_case(mnemonic))
        })
        .collect();
    if let Some(fits) = fits {
        if let Some(form) = candidates.iter().find(|form| fits(form.operands())) {
            return Some(form.clone());
        }
    }
    candidates.into_iter().next()
}
