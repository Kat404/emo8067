//! A MASM-style assembler for the Intel 8086/8088.
//!
//! The instruction table is not written out here. It is read out of
//! `odd/research/documentation-guide.md` §1.3, the same document the flag
//! contract reads §1.4 from and the same document the emulator is checked
//! against, so the table exists once in the project instead of twice.

pub mod table;

pub use table::{CELLS_PER_ROW, Form};
