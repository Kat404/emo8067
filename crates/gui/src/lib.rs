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
//!
//! **The window shows the guest's own code on the display, and X11 gives
//! no isolation between the clients of one display.** Another process on
//! the same display can read what the window shows, the way it can read
//! what the TUI writes to the terminal. The accessibility story this
//! project has is the TUI's.

pub mod app;
pub mod theme;
pub mod ui;
pub mod window;

use tracing_subscriber::fmt::MakeWriter;

/// Install the logger, so the toolkit's diagnostics reach it.
///
/// The toolkit reports through two facades: `tracing` (wgpu) and `log`
/// (winit, eframe). The subscriber's `tracing-log` feature bridges the
/// second into the first, so both land in the same place, and the adapter,
/// the surface and the device-lost messages — the ones that decide whether
/// the window can open at all — are among them.
///
/// The writer is a parameter so that a test can read what lands there:
/// the binary passes the standard error.
pub fn install_logger<W>(writer: W) -> Result<(), String>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_writer(writer)
        .try_init()
        .map_err(|error| error.to_string())
}
