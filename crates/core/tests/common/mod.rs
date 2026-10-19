//! Shared readers of `odd/research/documentation-guide.md`.
//!
//! §1.3 and §1.4 are this project's oracle: the opcode map and the flag
//! effects, both transcribed from the 1979 manual. Two test files need to
//! read them, and a parser that exists twice is a parser that will drift, so
//! it lives here once.

use emo8067_core::flags::{AF, CF, DF, IF, OF, PF, SF, TF, ZF};

/// The guide, read at compile time so a test and its oracle cannot disagree
/// about which document they are talking about.
pub const GUIDE: &str = include_str!("../../../../odd/research/documentation-guide.md");

/// §1.4 transcribes Table 2-18's `ODITSZAPC` column order: nine flags.
pub const COLUMNS: [u16; 9] = [OF, DF, IF, TF, SF, ZF, AF, PF, CF];

/// The em dash §1.4 uses for "not altered", which is not the same as `U`.
pub const UNCHANGED_STR: &str = "\u{2014}";

/// What a §1.4 cell says about one flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    /// `—`: the flag must come out exactly as it went in.
    Unchanged,
    /// `0`: cleared.
    Cleared,
    /// `1`: set.
    Set,
    /// `X`, `U` or `R`: nothing assertable without operand values.
    Free,
}

/// Read one §1.4 row out of the document, by the instruction name its first
/// cell starts with.
pub fn row_marks(doc: &str, name: &str) -> [Mark; 9] {
    let needle = format!("| `{name}`");
    let line = doc
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("no section 1.4 row starting with {needle}"));
    let cells: Vec<&str> = line.split('|').collect();
    assert!(
        cells.len() >= 11,
        "row for {name} has {} cells, expected 11: {line}",
        cells.len()
    );
    std::array::from_fn(|i| match cells[i + 2].trim() {
        UNCHANGED_STR => Mark::Unchanged,
        "0" => Mark::Cleared,
        "1" => Mark::Set,
        other => {
            assert!(
                matches!(other, "X" | "U" | "R"),
                "unexpected mark {other:?} in the row for {name}"
            );
            Mark::Free
        }
    })
}
