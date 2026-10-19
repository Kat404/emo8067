//! The macros.

use emo8067_asm::program::assemble_program;

#[test]
fn a_macro_stands_for_its_body() {
    let program =
        assemble_program("MACRO twice value\n  mov ax, value\n  add ax, value\nENDM\ntwice 5\n")
            .unwrap();
    // MOV AX,5 is B8 05 00 and ADD AX,5 is 05 05 00.
    assert_eq!(program.bytes, vec![0xB8, 0x05, 0x00, 0x05, 0x05, 0x00]);
}

#[test]
fn a_macro_may_call_another() {
    let program = assemble_program(
        "MACRO inner value\n  mov ax, value\nENDM\nMACRO outer value\n  inner value\n  add ax, 1\nENDM\nouter 7\n",
    )
    .unwrap();
    assert_eq!(program.bytes, vec![0xB8, 0x07, 0x00, 0x05, 0x01, 0x00]);
}

#[test]
fn a_macro_that_calls_itself_is_refused() {
    let error = assemble_program("MACRO loop\n  loop\nENDM\nloop\n").unwrap_err();
    match error {
        emo8067_asm::program::Error::RecursiveMacro { message } => {
            assert!(
                message.starts_with("recursive MACRO definition is over"),
                "{message}"
            );
        }
        other => panic!("expected the refusal, got {other:?}"),
    }
}
