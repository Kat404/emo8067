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
fn a_parameter_followed_by_a_comma_is_replaced() {
    // The reference's own library writes `s_dcl DB sdat, 0`: the parameter
    // is followed by a comma, and the comma is the edge of the word.
    let program =
        assemble_program("PRINT MACRO sdat\n  s_dcl DB sdat, 0\nENDM\nPRINT 'hi'\n").unwrap();
    assert_eq!(program.bytes, vec![b'h', b'i', 0]);
}

#[test]
fn a_macro_ends_with_its_own_name_and_endm() {
    // The reference writes the macro's name first and the word ENDM after
    // it, exactly as it writes MACRO: `PUTC    ENDM`.
    let program =
        assemble_program("PUTC MACRO char\n  mov al, char\nPUTC ENDM\nPUTC 'A'\n").unwrap();
    assert_eq!(program.bytes, vec![0xB0, b'A']);
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
