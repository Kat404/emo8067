//! The terminal debugger.
//!
//! The TUI is the product's primary frontend: the reference's value surface
//! is text, and a terminal is the only frontend that works over SSH and in
//! CI (`odd/research/features-and-ux.md` §4.3).
//!
//! This crate draws the panel model the `emo8067-debug` crate holds, and
//! nothing else: the panels are a pure function of the machine's state and of
//! a UI state struct, and the GUI will draw the same struct.

pub mod app;
pub mod ui;
