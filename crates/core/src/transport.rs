//! A framed protocol for the I/O bus.
//!
//! The reference implementation made the bus a 64 KB file that every device
//! polled, and a device written against that protocol had to live on the same
//! machine as the emulator. A socket breaks that: the emulator listens, a
//! device connects, and the device's reads and writes go to the bus.
//!
//! The wire format is a frame at a time, so that a stream of them is
//! unambiguous - a byte stream cannot say where one request ends and the next
//! begins, and a device that reads a port and writes it has to be able to
//! find the answer.
//!
//! ```text
//!     request   type(1) port(2, little endian) value(1, writes only)
//!     reply     value(1)
//! ```
//!
//! The type is [`READ`] or [`WRITE`]. A read is answered with the byte the
//! port holds, which is all ones when nobody owns it.

use crate::devices::DeviceBus;

/// A read: the port, and nothing else.
pub const READ: u8 = 0;
/// A write: the port, and the byte to write to it.
pub const WRITE: u8 = 1;

/// One message on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Read a port.
    Read(u16),
    /// Write a port.
    Write(u16, u8),
    /// The answer to a read: the byte the port holds.
    Value(u8),
}

impl Frame {
    /// The bytes of a request.
    #[must_use]
    pub fn encode_request(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4);
        match *self {
            Frame::Read(port) => {
                out.push(READ);
                out.extend_from_slice(&port.to_le_bytes());
            }
            Frame::Write(port, value) => {
                out.push(WRITE);
                out.extend_from_slice(&port.to_le_bytes());
                out.push(value);
            }
            Frame::Value(_) => out.push(READ),
        }
        out
    }

    /// The bytes of a reply.
    #[must_use]
    pub fn encode_reply(&self) -> Vec<u8> {
        match *self {
            Frame::Value(value) => vec![value],
            Frame::Read(port) => {
                let mut out = Vec::with_capacity(3);
                out.push(READ);
                out.extend_from_slice(&port.to_le_bytes());
                out
            }
            Frame::Write(port, value) => {
                let mut out = Vec::with_capacity(4);
                out.push(WRITE);
                out.extend_from_slice(&port.to_le_bytes());
                out.push(value);
                out
            }
        }
    }
}

/// Read one request out of a buffer, and say how many bytes it used.
///
/// `None` means the buffer does not hold a whole request yet.
#[must_use]
pub fn decode_request(buf: &[u8]) -> Option<(Frame, usize)> {
    let kind = *buf.first()?;
    let port = u16::from_le_bytes([*buf.get(1)?, *buf.get(2)?]);
    match kind {
        READ => Some((Frame::Read(port), 3)),
        WRITE => Some((Frame::Write(port, *buf.get(3)?), 4)),
        _ => None,
    }
}

/// Read one reply out of a buffer, and say how many bytes it used.
#[must_use]
pub fn decode_reply(buf: &[u8]) -> Option<(Frame, usize)> {
    let value = *buf.first()?;
    Some((Frame::Value(value), 1))
}

/// Apply a request to a bus, and say what to answer.
pub fn serve(bus: &mut DeviceBus, request: Frame) -> Frame {
    match request {
        Frame::Read(port) => Frame::Value(bus.read(port)),
        Frame::Write(port, value) => {
            bus.write(port, value);
            Frame::Value(bus.read(port))
        }
        Frame::Value(value) => Frame::Value(value),
    }
}

/// The socket the device server listens on.
///
/// Unix only, because that is where a device on the same machine can reach
/// it; a device on another machine wants a TCP socket, which is the same
/// frames over a different pipe.
#[cfg(unix)]
pub mod socket {
    use super::{decode_request, serve};
    use crate::devices::DeviceBus;
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;

    /// A device server: it listens on a socket and serves the bus.
    #[derive(Debug)]
    pub struct DeviceServer {
        listener: UnixListener,
        path: std::path::PathBuf,
        clients: Vec<(UnixStream, Vec<u8>)>,
    }

    impl DeviceServer {
        /// Listen on a path.
        ///
        /// A socket path is a file, and a file left behind by a run that
        /// ended badly would keep the next run from binding, so the path is
        /// removed first.
        pub fn bind(path: &Path) -> std::io::Result<Self> {
            let _ = std::fs::remove_file(path);
            let listener = UnixListener::bind(path)?;
            listener.set_nonblocking(true)?;
            Ok(Self {
                listener,
                path: path.to_path_buf(),
                clients: Vec::new(),
            })
        }

        /// The path this server listens on.
        #[must_use]
        pub fn path(&self) -> &Path {
            &self.path
        }

        /// Answer whatever a connected device has sent, without waiting.
        ///
        /// The server is served between the emulator's instructions, so it
        /// never blocks the program: a device that asks a question gets its
        /// answer at the next instruction boundary.
        pub fn poll(&mut self, bus: &mut DeviceBus) -> std::io::Result<()> {
            loop {
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(true)?;
                        self.clients.push((stream, Vec::new()));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
            for (stream, buf) in &mut self.clients {
                let mut chunk = [0u8; 256];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    }
                }
                while let Some((frame, used)) = decode_request(buf) {
                    buf.drain(..used);
                    let answer = serve(bus, frame);
                    stream.write_all(&answer.encode_reply())?;
                }
            }
            Ok(())
        }

        /// Serve one connection, reading frames until the client goes away.
        pub fn serve_once(&self, bus: &mut DeviceBus) -> std::io::Result<()> {
            let (stream, _) = self.listener.accept()?;
            serve_stream(stream, bus)
        }
    }

    /// Serve a connection: read frames, answer them.
    pub fn serve_stream(mut stream: UnixStream, bus: &mut DeviceBus) -> std::io::Result<()> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                return Ok(());
            }
            buf.extend_from_slice(&chunk[..read]);
            while let Some((frame, used)) = decode_request(&buf) {
                buf.drain(..used);
                let answer = serve(bus, frame);
                stream.write_all(&answer.encode_reply())?;
            }
        }
    }
}
