//! The window: the backends it opens on, and the options it opens with.
//!
//! The window can come up on two backends: the Vulkan one and the OpenGL
//! one. The first is tried first, and the second is the way a host with no
//! usable Vulkan adapter — a VM, a CI image, a vendor-locked GPU — still
//! gets a window.

use eframe::{NativeOptions, Renderer};
use egui::ViewportBuilder;

use crate::app::App;
use crate::theme;

/// The backends, in the order they are tried.
pub const BACKENDS: [Renderer; 2] = [Renderer::Wgpu, Renderer::Glow];

/// The size the window opens at.
const SIZE: [f32; 2] = [960.0, 640.0];

/// The options a backend opens the window with.
#[must_use]
pub fn options(renderer: Renderer) -> NativeOptions {
    NativeOptions {
        renderer,
        viewport: ViewportBuilder::default().with_inner_size(SIZE),
        ..Default::default()
    }
}

/// Open the window, on the first backend that can.
///
/// The Vulkan path is tried first and the OpenGL one second, and the
/// refusal says what each of them said: a host with no Vulkan adapter and
/// no OpenGL either is a host without a window, and it is told so.
pub fn open(app: &App, name: &str) -> Result<(), String> {
    let mut failures = String::new();
    for renderer in BACKENDS {
        let app = app.clone();
        match eframe::run_native(
            name,
            options(renderer),
            Box::new(move |cc| {
                cc.egui_ctx.set_visuals(theme::visuals());
                Ok(Box::new(app))
            }),
        ) {
            Ok(()) => return Ok(()),
            Err(error) => failures.push_str(&format!("{renderer}: {error}\n")),
        }
    }
    Err(format!("the window could not be opened:\n{failures}"))
}
