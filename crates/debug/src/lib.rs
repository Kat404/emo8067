//! The debugger's core.
//!
//! This is the part of the debugger that is not a picture: the expression
//! evaluator, the panel model and the breakpoints. The TUI and the GUI are
//! two renderers of the same model, and the model's rules are the ones the
//! research settled (`odd/research/features-and-ux.md` §4.3):
//!
//! > build the TUI to the point where every panel is a pure function of
//! > machine state plus a UI state struct, then the GUI is a rendering of
//! > that struct. The panel model is the product; ratatui and egui are two
//! > renderers of it.
//!
//! So nothing here draws, and nothing here reads a terminal.

pub mod breakpoints;
pub mod console;
pub mod expr;
pub mod machine;
pub mod panels;
pub mod source;

pub use breakpoints::{
    Access, Breakpoints, ConditionError, Entry, Kind, Missed, Report, Watchpoint,
};
pub use console::{Address, Command, Console, Dump, Refusal, Segment};
pub use expr::{Context, Error, Value, evaluate};
pub use machine::Machine;
pub use panels::{
    Cell, Edit, Flag, Goto, MemoryView, Panels, Register, Registers, ScreenCell, ScreenView,
    StackView, Ui,
};
pub use source::SourceView;
