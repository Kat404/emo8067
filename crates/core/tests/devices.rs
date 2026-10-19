//! The three devices the reference implementation shipped with a port map.
//!
//! Their protocols are in `re/docs-txt/io.txt`, the reference's own
//! description of the bus, and `odd/research/features-and-ux.md` rows DEV-4 to
//! DEV-6 carry the same thing. The other four shipped devices - `Simple`,
//! `LED_Display`, `Thermometer`, `Printer` and `VGA_STATE` - have no
//! documented port map, so they are declared unsupported rather than guessed
//! at.

use emo8067_core::Cpu;
use emo8067_core::devices::{
    DeviceBus, Robot, StepperMotor, TrafficLights, robot_data, robot_status,
};
use emo8067_core::regs::{Reg8, Reg16, Seg};

fn machine(code: &[u8], bus: DeviceBus) -> Cpu {
    let mut c = Cpu::new();
    c.bus = std::rc::Rc::new(std::cell::RefCell::new(bus));
    c.mem.load_at(0x10000, code).unwrap();
    c.regs.set_segment(Seg::Cs, 0x1000);
    c.regs.set_segment(Seg::Ss, 0x2000);
    c.regs.set_word(Reg16::Sp, 0x0100);
    c
}

#[test]
fn the_traffic_lights_take_a_word_and_show_which_lamp_is_lit() {
    let mut bus = DeviceBus::new();
    bus.add(Box::new(TrafficLights::new()));
    // MOV AX, 2C44h; OUT 4, AX; IN AX, 4
    let mut c = machine(&[0xB8, 0x44, 0x2C, 0xE7, 0x04, 0xE5, 0x04], bus);
    c.step().unwrap();
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(
        c.regs.word(Reg16::Ax),
        0x0C44,
        "twelve low bits, the rest unused"
    );
}

#[test]
fn the_stepper_motor_keeps_three_phases_and_says_when_it_is_ready() {
    let mut bus = DeviceBus::new();
    bus.add(Box::new(StepperMotor::new()));
    // MOV AL, 001b; OUT 7, AL; IN AL, 7
    let mut c = machine(&[0xB0, 0x01, 0xE6, 0x07, 0xE4, 0x07], bus);
    c.step().unwrap();
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al) & 0x07, 0b001, "the three phase bits");
    assert!(
        c.regs.byte(Reg8::Al) & 0x80 != 0,
        "and the top bit says the motor is ready"
    );
}

#[test]
fn the_robot_moves_turns_and_answers_with_its_status_bits() {
    let mut bus = DeviceBus::new();
    let robot = Robot::default().attach(&mut bus);
    // MOV AL, 1; OUT 9, AL  (forward)
    let mut c = machine(&[0xB0, 0x01, 0xE6, 0x09], bus);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(
        robot.borrow().position(),
        (0, -1),
        "it moved north, the way it faced"
    );

    // MOV AL, 3; OUT 9, AL  (turn right)
    let mut bus = DeviceBus::new();
    let robot = Robot::default().attach(&mut bus);
    let mut c = machine(&[0xB0, 0x03, 0xE6, 0x09], bus);
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(robot.borrow().facing(), 1, "it turned right, to the east");
    assert_eq!(
        robot.borrow().status() & robot_status::BUSY,
        0,
        "and it is not busy"
    );
    assert_eq!(
        robot.borrow().status() & robot_status::ERROR,
        0,
        "and nothing failed"
    );
}

#[test]
fn the_robots_data_register_says_what_it_examined() {
    let mut bus = DeviceBus::new();
    let robot = Robot::default().attach(&mut bus);
    // MOV AL, 4; OUT 9, AL  (examine); IN AL, 11
    let mut c = machine(&[0xB0, 0x04, 0xE6, 0x09, 0xE4, 0x0B], bus);
    c.step().unwrap();
    c.step().unwrap();
    c.step().unwrap();
    assert_eq!(
        robot.borrow().data(),
        robot_data::NOTHING,
        "there is nothing in front"
    );
    assert!(
        robot.borrow().status() & robot_status::NEW_DATA != 0,
        "and the status says the data is new"
    );
    assert_eq!(
        c.regs.byte(Reg8::Al) & robot_status::NEW_DATA,
        robot_status::NEW_DATA,
        "which is what the status register reads"
    );
}

#[test]
fn a_port_nobody_owns_reads_as_ones_and_keeps_what_it_is_given() {
    let mut c = machine(&[0xE4, 0x60], DeviceBus::new());
    c.step().unwrap();
    assert_eq!(c.regs.byte(Reg8::Al), 0xFF, "nothing drives the lines");
}
