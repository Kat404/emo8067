//! The windowed debugger.
//!
//! The GUI is the debugger's second face: the same panels the TUI draws,
//! in a window a person can click. The panel model is the product and the
//! two are two renderers of it (`odd/research/features-and-ux.md` §4.3),
//! so this crate draws what `emo8067-debug` holds and reads the machine
//! nowhere else.
//!
//! It is a sibling of the TUI and not a copy of it: the TUI draws the
//! model with ratatui and this one draws it with egui, and neither of them
//! knows how the other reads the machine.

pub mod app;
pub mod theme;
pub mod ui;
