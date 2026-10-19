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

    /// The device is read as a word.
    ///
    /// The 8086's bus is a byte bus, so a word is two transfers of the same
    /// port, the low byte first. A device that answers with a word says so
    /// here; one that does not is read twice, which is what the chip does.
    fn read_word(&mut self) -> u16 {
        let low = self.read();
        let high = self.read();
        u16::from_le_bytes([low, high])
    }

    /// The device is written a word: the low byte first, then the high one.
    fn write_word(&mut self, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.write(low);
        self.write(high);
    }
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

    /// Read a port as a word.
    pub fn read_word(&mut self, port: u16) -> u16 {
        match self.devices.get_mut(&port) {
            Some(device) => device.read_word(),
            // Nothing drives the lines, so the chip reads ones.
            None => 0xFFFF,
        }
    }

    /// Write a port as a word.
    pub fn write_word(&mut self, port: u16, value: u16) {
        if let Some(device) = self.devices.get_mut(&port) {
            device.write_word(value);
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

/// The traffic lights: twelve lamps on a word port.
///
/// Port 4, a word. There are twelve lamps, four green, four yellow and four
/// red, and a bit set is a lamp that is lit. Only the twelve low bits of the
/// word are used; the rest are unused.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficLights {
    lit: u16,
}

impl TrafficLights {
    /// The port the lights answer on.
    pub const PORT: u16 = 4;
    /// How many lamps there are: four green, four yellow, four red.
    pub const LAMPS: usize = 12;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The lamps that are lit: the twelve low bits of the word.
    #[must_use]
    pub fn lit(&self) -> u16 {
        self.lit & 0x0FFF
    }

    /// The green, the yellow and the red lamps, four bits each.
    #[must_use]
    pub fn colours(&self) -> [u16; 3] {
        [
            self.lit & 0x000F,
            (self.lit >> 4) & 0x000F,
            (self.lit >> 8) & 0x000F,
        ]
    }
}

impl Device for TrafficLights {
    fn port(&self) -> u16 {
        Self::PORT
    }

    fn read_word(&mut self) -> u16 {
        self.lit
    }

    fn write_word(&mut self, value: u16) {
        self.lit = value & 0x0FFF;
    }
}

/// The stepper motor: three magnets on bits 0 to 2.
///
/// Port 7, a byte. The motor is a three-phase one, so the low three bits
/// say which magnets are working, and the other bits are unused. The topmost
/// bit of the byte in the port says the motor is ready for the next phase.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepperMotor {
    phase: u8,
}

impl StepperMotor {
    /// The port the motor answers on.
    pub const PORT: u16 = 7;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The three magnets that are working, in bits 0 to 2.
    #[must_use]
    pub fn phase(&self) -> u8 {
        self.phase & 0x07
    }

    /// Whether the motor is ready for the next phase.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.phase & 0x80 != 0
    }
}

impl Device for StepperMotor {
    fn port(&self) -> u16 {
        Self::PORT
    }

    fn read(&mut self) -> u8 {
        self.phase
    }

    fn write(&mut self, value: u8) {
        // The top bit is the ready flag, and the motor is ready once it holds
        // a phase: the reference's motor takes time to turn, and this one does
        // not.
        self.phase = (value & 0x07) | 0x80;
    }
}

/// The robot: a command register, a data register and a status register.
///
/// Port 9 is the command register, port 10 the data register and port 11 the
/// status register. The status register's bit 0 says the data register holds
/// something new, bit 1 says the robot is busy and bit 2 says the last command
/// failed.
#[derive(Clone, Debug, Default)]
pub struct Robot {
    /// What the robot last saw.
    data: u8,
    /// The status bits.
    status: u8,
    /// The robot's position on the map, and the way it faces.
    x: i16,
    y: i16,
    /// The way it faces: 0 is north, and it turns a quarter at a time.
    facing: u8,
    /// The lamps it has switched on.
    lamps: Vec<(i16, i16)>,
}

/// The data register's values.
pub mod robot_data {
    /// The robot examined a wall.
    pub const WALL: u8 = 255;
    /// The robot examined nothing.
    pub const NOTHING: u8 = 0;
    /// The robot examined a lamp that is on.
    pub const LAMP_ON: u8 = 7;
    /// The robot examined a lamp that is off.
    pub const LAMP_OFF: u8 = 8;
}

/// The status register's bits.
pub mod robot_status {
    /// The data register holds something new.
    pub const NEW_DATA: u8 = 1 << 0;
    /// The robot is busy.
    pub const BUSY: u8 = 1 << 1;
    /// The last command failed.
    pub const ERROR: u8 = 1 << 2;
}

/// The robot's command register.
#[derive(Clone, Debug)]
pub struct RobotCommand {
    robot: Rc<RefCell<Robot>>,
}

impl Device for RobotCommand {
    fn port(&self) -> u16 {
        9
    }

    fn write(&mut self, value: u8) {
        self.robot.borrow_mut().command(value);
    }
}

/// The robot's data register.
#[derive(Clone, Debug)]
pub struct RobotData {
    robot: Rc<RefCell<Robot>>,
}

impl Device for RobotData {
    fn port(&self) -> u16 {
        10
    }

    fn read(&mut self) -> u8 {
        self.robot.borrow().data
    }
}

/// The robot's status register.
#[derive(Clone, Debug)]
pub struct RobotStatus {
    robot: Rc<RefCell<Robot>>,
}

impl Device for RobotStatus {
    fn port(&self) -> u16 {
        11
    }

    fn read(&mut self) -> u8 {
        self.robot.borrow().status
    }
}

impl Robot {
    /// Plug the robot's three registers into a bus, and hand back the handle
    /// the registers share, so that the robot can be watched from outside.
    pub fn attach(self, bus: &mut DeviceBus) -> Rc<RefCell<Robot>> {
        let robot = Rc::new(RefCell::new(self));
        bus.add(Box::new(RobotCommand {
            robot: Rc::clone(&robot),
        }));
        bus.add(Box::new(RobotData {
            robot: Rc::clone(&robot),
        }));
        bus.add(Box::new(RobotStatus {
            robot: Rc::clone(&robot),
        }));
        robot
    }

    /// Where the robot is.
    #[must_use]
    pub fn position(&self) -> (i16, i16) {
        (self.x, self.y)
    }

    /// The way the robot faces: 0 is north, 1 east, 2 south, 3 west.
    #[must_use]
    pub fn facing(&self) -> u8 {
        self.facing
    }

    /// What the robot last saw, in the data register's values.
    #[must_use]
    pub fn data(&self) -> u8 {
        self.data
    }

    /// The status bits.
    #[must_use]
    pub fn status(&self) -> u8 {
        self.status
    }

    /// The lamps the robot has switched on.
    #[must_use]
    pub fn lamps(&self) -> &[(i16, i16)] {
        &self.lamps
    }

    /// Carry out a command from the command register.
    fn command(&mut self, value: u8) {
        // The robot is not busy: it completes the work at once, where the
        // reference's takes time and asks the program to wait for bit 1.
        self.status &= !robot_status::NEW_DATA;
        self.status &= !robot_status::ERROR;
        match value {
            0 => {}
            1 => {
                // Forward, in the way it faces.
                match self.facing {
                    0 => self.y -= 1,
                    1 => self.x += 1,
                    2 => self.y += 1,
                    _ => self.x -= 1,
                }
            }
            2 => self.facing = (self.facing + 3) % 4,
            3 => self.facing = (self.facing + 1) % 4,
            4 => {
                // Examine what is in front. There is no world map here, so
                // there is nothing to see.
                self.data = robot_data::NOTHING;
                self.status |= robot_status::NEW_DATA;
            }
            5 => {
                let lamp = self.lamp_in_front();
                if !self.lamps.contains(&lamp) {
                    self.lamps.push(lamp);
                }
            }
            6 => {
                let lamp = self.lamp_in_front();
                self.lamps.retain(|l| *l != lamp);
            }
            _ => self.status |= robot_status::ERROR,
        }
    }

    /// The lamp in front of the robot, which is where it stands.
    fn lamp_in_front(&self) -> (i16, i16) {
        (self.x, self.y)
    }
}
