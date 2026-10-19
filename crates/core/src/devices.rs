//! The I/O bus.
//!
//! A port is not a memory address. The 8086 has a separate I/O bus, and a
//! program that reads or writes a port is talking to whatever is plugged
//! into it. The reference implementation made that bus a 64 KB file at
//! `C:\emu8086.io`, with byte N of the file behind port N, which meant a
//! program could not reach a device without a writable file at the root of
//! the drive and the rights that came with it.
//!
//! This is the same shape and none of the cost: the bus is a set of devices
//! in the process, and the file is gone. A device is anything that answers
//! on a port, and the bus hands the port to the device that owns it.
//!
//! What the bus does with a port nobody owns is the question the captures
//! answer: with nothing attached, a read returns all ones. The 8086 pulls
//! the data lines high when nothing drives them, and the captures from a
//! real P80C86A-2 agree - "All reads of IO port addresses should return
//! 0xFF and incur no wait states".

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A device on the I/O bus.
///
/// The 8086's bus is a byte bus: a word transfer is two byte transfers, and
/// the port number is what the device answers on.
pub trait Device {
    /// The port this device answers on.
    fn port(&self) -> u16;

    /// The device is read. A device with nothing to say returns all ones,
    /// which is what the bus reads when it pulls the lines high.
    fn read(&mut self) -> u8 {
        0xFF
    }

    /// The device is written.
    fn write(&mut self, _value: u8) {}
}

/// The I/O bus: the devices, each on its own port.
///
/// A port nobody owns reads as all ones and swallows what is written to it,
/// which is the bare chip's behaviour with nothing plugged in.
#[derive(Default)]
pub struct DeviceBus {
    devices: BTreeMap<u16, Box<dyn Device>>,
}

impl std::fmt::Debug for DeviceBus {
    /// The ports on the bus, and nothing about the devices themselves: a
    /// device is a box, and what is behind it is the caller's business.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceBus")
            .field("ports", &self.devices.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl DeviceBus {
    /// An empty bus: no devices, and every port reads as all ones.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Plug a device in, taking the port from the device itself.
    pub fn add(&mut self, device: Box<dyn Device>) {
        self.devices.insert(device.port(), device);
    }

    /// Whether a device answers on this port.
    #[must_use]
    pub fn has(&self, port: u16) -> bool {
        self.devices.contains_key(&port)
    }

    /// The number of devices on the bus.
    #[must_use]
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Whether the bus is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Read a port.
    pub fn read(&mut self, port: u16) -> u8 {
        match self.devices.get_mut(&port) {
            Some(device) => device.read(),
            // Nothing drives the lines, so the chip reads ones.
            None => 0xFF,
        }
    }

    /// Write a port.
    pub fn write(&mut self, port: u16, value: u8) {
        if let Some(device) = self.devices.get_mut(&port) {
            device.write(value);
        }
    }
}

/// The bus a [`crate::Cpu`] talks to.
///
/// It is shared rather than owned, so that a device can be watched from
/// outside the emulator while it runs - which is the whole point of a
/// traffic light or a stepper motor.
pub type SharedBus = Rc<RefCell<DeviceBus>>;

/// A shared, empty bus.
#[must_use]
pub fn empty_bus() -> SharedBus {
    Rc::new(RefCell::new(DeviceBus::new()))
}
