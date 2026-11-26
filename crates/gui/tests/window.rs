//! The window's backends: the paths a window can open on.

use eframe::Renderer;
use emo8067_gui::window::{BACKENDS, options};

#[test]
fn the_window_can_open_on_vulkan_and_on_opengl() {
    assert_eq!(
        BACKENDS,
        [Renderer::Wgpu, Renderer::Glow],
        "the Vulkan path first and the OpenGL one second: a host with no usable Vulkan adapter \
         still gets a window"
    );
}

#[test]
fn the_options_carry_the_backend_and_the_size() {
    let options = options(Renderer::Glow);

    assert_eq!(
        options.renderer,
        Renderer::Glow,
        "the backend is the one asked for"
    );
    assert_eq!(
        options.viewport.inner_size,
        Some(egui::Vec2::new(960.0, 640.0)),
        "and the window opens at the size it is given"
    );
}
