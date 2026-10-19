//! The framed protocol the I/O bus speaks over a socket.

use emo8067_core::devices::DeviceBus;
use emo8067_core::transport::{Frame, decode_reply, decode_request, serve};

/// A device that counts what it is told.
struct Counter {
    port: u16,
    count: u32,
}

impl emo8067_core::devices::Device for Counter {
    fn port(&self) -> u16 {
        self.port
    }
    fn read(&mut self) -> u8 {
        self.count as u8
    }
    fn write(&mut self, value: u8) {
        self.count = u32::from(value) + 1;
    }
}

#[test]
fn a_read_is_answered_with_the_byte_the_port_holds() {
    let mut bus = DeviceBus::new();
    bus.add(Box::new(Counter {
        port: 0x60,
        count: 7,
    }));
    let answer = serve(&mut bus, Frame::Read(0x60));
    assert_eq!(answer, Frame::Value(7), "the device answered");

    let answer = serve(&mut bus, Frame::Read(0x61));
    assert_eq!(answer, Frame::Value(0xFF), "and nobody owns that one");
}

#[test]
fn a_write_reaches_the_device() {
    let mut bus = DeviceBus::new();
    bus.add(Box::new(Counter {
        port: 0x60,
        count: 0,
    }));
    serve(&mut bus, Frame::Write(0x60, 41));
    let answer = serve(&mut bus, Frame::Read(0x60));
    assert_eq!(answer, Frame::Value(42), "the device counted it");
}

#[test]
fn a_frame_survives_the_wire() {
    let request = Frame::Write(0x1234, 0x56);
    let bytes = request.encode_request();
    assert_eq!(bytes.len(), 4, "a write is four bytes");
    let (decoded, used) = decode_request(&bytes).expect("a whole request");
    assert_eq!(decoded, request);
    assert_eq!(used, bytes.len());

    let reply = Frame::Value(0x78);
    let bytes = reply.encode_reply();
    let (decoded, used) = decode_reply(&bytes).expect("a whole reply");
    assert_eq!(decoded, reply);
    assert_eq!(used, 1);
}

#[test]
fn half_a_frame_is_not_a_frame() {
    assert!(decode_request(&[0]).is_none(), "no port yet");
    assert!(decode_request(&[0, 0x34]).is_none(), "no port yet");
    assert!(
        decode_request(&[0, 0x34, 0x12]).is_some(),
        "and now there is one"
    );
    assert!(
        decode_request(&[9, 0, 0]).is_none(),
        "an unknown type is not a request"
    );
}

#[cfg(unix)]
#[test]
fn a_device_reaches_the_bus_over_a_socket() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let dir = std::env::temp_dir().join(format!("emo8067-socket-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bus.sock");
    let mut server = emo8067_core::transport::socket::DeviceServer::bind(&path).unwrap();

    let client = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&path).unwrap();
        // Write a port, and read the answer.
        stream
            .write_all(&Frame::Write(0x60, 0x2A).encode_request())
            .unwrap();
        let mut answer = [0u8; 1];
        stream.read_exact(&mut answer).unwrap();
        // Then read a port.
        stream
            .write_all(&Frame::Read(0x60).encode_request())
            .unwrap();
        let mut value = [0u8; 1];
        stream.read_exact(&mut value).unwrap();
        (answer[0], value[0])
    });

    let mut bus = DeviceBus::new();
    bus.add(Box::new(Counter {
        port: 0x60,
        count: 0,
    }));
    // The server is served between the emulator's instructions, so it is
    // polled until the device has what it asked for.
    while !client.is_finished() {
        server.poll(&mut bus).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let (answer, value) = client.join().unwrap();
    assert_eq!(
        answer, 0x2B,
        "the write was answered with what the port holds"
    );
    assert_eq!(value, 0x2B, "and the port holds what the device made of it");
    std::fs::remove_dir_all(&dir).ok();
}
