//! The expression evaluator, and the language the reference documents.

use emo8067_debug::expr::{self, Context, Error, Value};
use std::collections::BTreeMap;

/// A machine with a few registers and one word of memory.
struct Machine {
    registers: BTreeMap<String, Value>,
    words: BTreeMap<(u16, u16), Value>,
}

impl Machine {
    fn new() -> Machine {
        Machine {
            registers: BTreeMap::new(),
            words: BTreeMap::new(),
        }
    }

    fn with_register(&mut self, name: &str, value: Value) -> &mut Machine {
        self.registers.insert(name.to_string(), value);
        self
    }

    fn with_word(&mut self, segment: u16, offset: u16, value: Value) -> &mut Machine {
        self.words.insert((segment, offset), value);
        self
    }
}

impl Context for Machine {
    fn register(&self, name: &str) -> Option<Value> {
        self.registers.get(name).copied()
    }

    fn word(&self, segment: u16, offset: u16) -> Option<Value> {
        self.words.get(&(segment, offset)).copied()
    }
}

fn evaluate(text: &str, machine: &Machine) -> Result<Value, Error> {
    expr::evaluate(text, machine)
}

#[test]
fn a_literal_is_decimal_binary_or_hexadecimal() {
    let machine = Machine::new();
    assert_eq!(evaluate("5", &machine).unwrap(), 5);
    assert_eq!(evaluate("101b", &machine).unwrap(), 0b101);
    assert_eq!(evaluate("34h", &machine).unwrap(), 0x34);
    // The reference asks for the `h` suffix on a hexadecimal value: without
    // it the number is decimal.
    assert_eq!(evaluate("34", &machine).unwrap(), 34);
}

#[test]
fn the_reference_s_own_examples_compute() {
    let machine = Machine::new();
    assert_eq!(evaluate("1234h + 1", &machine).unwrap(), 0x1235);
    assert_eq!(evaluate("0FFh - 101b", &machine).unwrap(), 0xFF - 0b101);
    assert_eq!(evaluate("5 << 2", &machine).unwrap(), 20);
}

#[test]
fn the_operators_are_the_reference_s_own() {
    let machine = Machine::new();
    assert_eq!(evaluate("~0", &machine).unwrap(), u32::MAX);
    assert_eq!(evaluate("-1", &machine).unwrap(), u32::MAX);
    assert_eq!(evaluate("6 * 7", &machine).unwrap(), 42);
    assert_eq!(evaluate("43 / 5", &machine).unwrap(), 8);
    assert_eq!(evaluate("43 % 5", &machine).unwrap(), 3);
    assert_eq!(evaluate("10 + 5", &machine).unwrap(), 15);
    assert_eq!(evaluate("10 - 5", &machine).unwrap(), 5);
    assert_eq!(evaluate("1 << 4", &machine).unwrap(), 16);
    assert_eq!(evaluate("256 >> 4", &machine).unwrap(), 16);
    assert_eq!(evaluate("0F0h & 0FFh", &machine).unwrap(), 0xF0);
    assert_eq!(evaluate("0F0h ^ 0FFh", &machine).unwrap(), 0x0F);
    assert_eq!(evaluate("0F0h | 0F0h", &machine).unwrap(), 0xF0);
}

#[test]
fn the_shift_count_is_masked_to_five_bits() {
    // The reference masks its shift count to five bits, and so does the
    // 8086: a count of 32 is a count of zero.
    let machine = Machine::new();
    assert_eq!(evaluate("1 << 32", &machine).unwrap(), 1);
    assert_eq!(evaluate("256 >> 32", &machine).unwrap(), 256);
    assert_eq!(evaluate("1 << 31", &machine).unwrap(), 0x8000_0000);
}

#[test]
fn the_operators_bind_as_they_do_in_c() {
    let machine = Machine::new();
    assert_eq!(evaluate("1 + 2 * 3", &machine).unwrap(), 7);
    assert_eq!(evaluate("(1 + 2) * 3", &machine).unwrap(), 9);
    assert_eq!(evaluate("1 << 2 + 1", &machine).unwrap(), 8);
    assert_eq!(evaluate("1 | 2 & 3", &machine).unwrap(), 3);
    assert_eq!(evaluate("1 ^ 2 | 3", &machine).unwrap(), 3);
}

#[test]
fn a_register_and_a_word_of_memory_are_the_machines_own() {
    let mut machine = Machine::new();
    machine
        .with_register("AX", 0x1234)
        .with_register("CS", 0x0200);
    machine.with_word(0x0100, 0x0004, 0xBEEF);
    assert_eq!(evaluate("AX", &machine).unwrap(), 0x1234);
    assert_eq!(evaluate("0100:0004", &machine).unwrap(), 0xBEEF);
    assert_eq!(
        evaluate("AX + 0100:0004", &machine).unwrap(),
        0x1234 + 0xBEEF
    );
}

#[test]
fn the_comparisons_and_the_logical_operators_are_ours() {
    // The reference's expression has no comparison in it, so these are this
    // project's addition: a breakpoint's condition needs them.
    let mut machine = Machine::new();
    machine
        .with_register("AX", 0x1234)
        .with_register("CS", 0x0200);
    assert_eq!(evaluate("AX == 0x1234", &machine).unwrap(), 1);
    assert_eq!(evaluate("AX != 0x1234", &machine).unwrap(), 0);
    assert_eq!(evaluate("AX < 0x1235", &machine).unwrap(), 1);
    assert_eq!(evaluate("AX > 0x1235", &machine).unwrap(), 0);
    assert_eq!(evaluate("AX <= 0x1234", &machine).unwrap(), 1);
    assert_eq!(evaluate("AX >= 0x1235", &machine).unwrap(), 0);
    assert_eq!(evaluate("AX==0x1234 && CS==0x0200", &machine).unwrap(), 1);
    assert_eq!(evaluate("AX==0x1234 && CS==0x0201", &machine).unwrap(), 0);
    assert_eq!(evaluate("AX==0 || CS==0x0200", &machine).unwrap(), 1);
}

#[test]
fn the_refusals_are_the_reference_s_own_words() {
    let machine = Machine::new();
    assert_eq!(
        evaluate("5 $", &machine).unwrap_err().to_string(),
        "bad syntax"
    );
    assert_eq!(
        evaluate("* 5", &machine).unwrap_err().to_string(),
        "missing an operand"
    );
    assert_eq!(
        evaluate("", &machine).unwrap_err().to_string(),
        "bad expression"
    );
    assert_eq!(
        evaluate("1 = 2", &machine).unwrap_err().to_string(),
        "bad operator in stack!"
    );
    assert_eq!(
        evaluate("(", &machine).unwrap_err().to_string(),
        "bad syntax"
    );
    assert_eq!(
        evaluate("AX", &machine).unwrap_err().to_string(),
        "bad expression"
    );
    let deep = "~".repeat(200) + "1";
    assert_eq!(
        evaluate(&deep, &machine).unwrap_err().to_string(),
        "expression too complex"
    );
}

#[test]
fn a_condition_can_be_read_before_the_machine_gets_there() {
    assert!(expr::check("AX==0x1234 && CS==0x0200").is_ok());
    assert!(expr::check("AX == ").is_err());
}
