//! The terminal debugger.
//!
//! The TUI is the product's primary frontend: the reference's value surface
//! is text, and a terminal is the only frontend that works over SSH and in
//! CI (`odd/research/features-and-ux.md` §4.3).
//!
//! This crate draws the panel model the `emo8067-debug` crate holds, and
//! nothing else: the panels are a pure function of the machine's state and of
//! a UI state struct, and the GUI will draw the same struct. The cursors,
//! the choices and the prompts live in that struct too, and so does the
//! console's parser: the state the two front ends share is the debug
//! crate's.

pub mod app;
pub mod ui;
