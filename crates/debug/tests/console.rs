//! The console's parser: the commands it takes, and the refusals it says.
//!
//! The reference has a debug console of its own (`re/docs-txt/debug.txt`)
//! and it documents ten commands in it: `c`, `i`, `iw`, `o`, `ow`, `d`,
//! `t`, `p`, `r` and `rf`. The parser is the debug crate's now — the two
//! front ends share it — and the words it takes and the refusals it says
//! do not change.

use emo8067_core::Cpu;
use emo8067_debug::console::{self, Address, Command, Refusal};
use emo8067_debug::expr;

/// The machine the commands are read against.
fn machine() -> Cpu {
    Cpu::new()
}

#[test]
fn the_ten_words_the_reference_documents_are_the_words_the_console_takes() {
    let cpu = machine();
    let commands = [
        (
            "c 0100 0200",
            Command::Compare {
                first: Address {
                    segment: None,
                    offset: 0x0100,
                },
                count: console::COMPARE_BYTES,
                second: Address {
                    segment: None,
                    offset: 0x0200,
                },
            },
        ),
        (
            "i 0123",
            Command::ReadPort {
                port: 0x0123,
                word: false,
            },
        ),
        (
            "iw 0123",
            Command::ReadPort {
                port: 0x0123,
                word: true,
            },
        ),
        (
            "o 0123 00",
            Command::WritePort {
                port: 0x0123,
                value: 0x00,
                word: false,
            },
        ),
        (
            "ow 0123 00FF",
            Command::WritePort {
                port: 0x0123,
                value: 0x00FF,
                word: true,
            },
        ),
        (
            "d 0100",
            Command::Dump {
                address: Some(Address {
                    segment: None,
                    offset: 0x0100,
                }),
                count: console::DUMP_BYTES,
            },
        ),
        ("t", Command::Step),
        ("p", Command::StepOver),
        ("r", Command::Registers),
        ("rf", Command::Flags),
    ];

    for (text, command) in commands {
        assert_eq!(
            console::parse(text, &cpu),
            Ok(command),
            "the command {text} is the reference's own"
        );
    }
}

#[test]
fn a_command_the_console_does_not_know_is_refused() {
    let cpu = machine();

    assert_eq!(
        console::parse("x", &cpu),
        Err(Refusal::UnknownCommand("x".to_string())),
        "the refusal is the console's own"
    );
    assert_eq!(
        console::parse("", &cpu),
        Err(Refusal::MissingArgument("a command")),
        "and an empty line is a refusal too"
    );
}

#[test]
fn the_five_refusals_of_the_evaluator_are_the_console_s_too() {
    let cpu = machine();
    let deep = format!("{}1{}", "(".repeat(70), ")".repeat(70));

    for (text, error) in [
        ("r AX =", expr::Error::BadOperator),
        ("r AX @", expr::Error::BadSyntax),
        ("r AX *1", expr::Error::MissingOperand),
        ("r AX ZZ", expr::Error::BadExpression),
        (&format!("r AX {deep}"), expr::Error::TooComplex),
    ] {
        assert_eq!(
            console::parse(text, &cpu),
            Err(Refusal::Expression(error)),
            "the refusal is the evaluator's own: {text}"
        );
    }
}

#[test]
fn a_value_is_read_in_the_reference_s_own_expression_language() {
    let mut cpu = machine();
    cpu.regs.set_word(emo8067_core::regs::Reg16::Bx, 0x0002);

    assert_eq!(
        console::parse("r AX BX+1", &cpu),
        Ok(Command::WriteRegister {
            register: emo8067_debug::panels::Register::Word(emo8067_core::regs::Reg16::Ax,),
            value: 0x0003,
        }),
        "the value is an expression, and the machine is the context"
    );
}

#[test]
fn the_dump_of_the_console_remembers_where_the_next_one_begins() {
    let dump = console::Dump {
        segment: 0x0200,
        offset: 0x0100,
        count: console::DUMP_BYTES,
    };

    assert_eq!(dump.next(), 0x0180, "the next block begins after this one");
}
