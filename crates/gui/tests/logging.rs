//! The logger: the toolkit's diagnostics have somewhere to go.
//!
//! The toolkit reports through two facades, and the logger takes both.

use std::io::Write;
use std::sync::{Arc, Mutex};

use emo8067_gui::install_logger;
use tracing_subscriber::fmt::MakeWriter;

/// A sink the test can read.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    /// What has landed in the sink.
    fn text(&self) -> String {
        let bytes = self.0.lock().expect("the sink").clone();
        String::from_utf8(bytes).expect("the text")
    }
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("the sink").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Sink {
    type Writer = Sink;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn the_toolkits_diagnostics_reach_the_logger() {
    let sink = Sink::default();
    install_logger(sink.clone()).expect("the logger is installed");

    // The two facades the toolkit reports through: `tracing` for wgpu and
    // `log` for winit and eframe.
    tracing::info!("the wgpu adapter");
    log::info!("the winit window");

    let text = sink.text();
    assert!(
        text.contains("the wgpu adapter"),
        "the tracing facade reaches it: {text}"
    );
    assert!(
        text.contains("the winit window"),
        "the log facade reaches it: {text}"
    );
}
