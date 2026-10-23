//! The disassembler, and the reference's own print formats.

use emo8067_asm::disasm::{self, Error};

/// The text the disassembler prints for the bytes.
fn text(bytes: &[u8]) -> String {
    disasm::disassemble(bytes, 0)
        .expect("the bytes decode")
        .text
}

#[test]
fn the_mnemonic_comes_from_the_guides_own_table() {
    // The guide's §1.3 says `MOV r/m16,r16` for 8Bh.
    assert_eq!(text(&[0x8B, 0x06, 0x0B, 0x0A]), "MOV AX, [00A0Bh]");
    // And `CBW` for 98h.
    assert_eq!(text(&[0x98]), "CBW");
}

#[test]
fn the_bracket_form_holds_the_address() {
    assert_eq!(text(&[0x8B, 0x06, 0x0B, 0x0A]), "MOV AX, [00A0Bh]");
    assert_eq!(text(&[0x8B, 0x46, 0x0F]), "MOV AX, [BP+0Fh]");
    assert_eq!(text(&[0x8B, 0x46, 0xFE]), "MOV AX, [BP-02h]");
}

#[test]
fn the_bare_sixteen_bit_hex_form() {
    assert_eq!(text(&[0xB8, 0x0B, 0x0A]), "MOV AX, 00A0Bh");
}

#[test]
fn the_logical_address_is_a_segment_and_an_offset() {
    assert_eq!(text(&[0x9A, 0x0B, 0x0A, 0x34, 0x12]), "CALL 01234h:00A0Bh");
}

#[test]
fn the_eight_bit_form_is_a_leading_zero_and_the_hex() {
    assert_eq!(text(&[0xB0, 0x0A]), "MOV AL, 0Ah");
    assert_eq!(text(&[0xB0, 0xFF]), "MOV AL, 0FFh");
    assert_eq!(text(&[0xB0, 0x10]), "MOV AL, 010h");
}

#[test]
fn the_two_operand_forms() {
    // The mnemonic and a register.
    assert_eq!(text(&[0x89, 0xD8]), "MOV AX, BX");
    // The mnemonic and a reference.
    assert_eq!(text(&[0x8B, 0x06, 0x0B, 0x0A]), "MOV AX, [00A0Bh]");
    // The mnemonic and an immediate.
    assert_eq!(text(&[0xB8, 0x0B, 0x0A]), "MOV AX, 00A0Bh");
    // A reference and the mnemonic's other operand.
    assert_eq!(text(&[0xA2, 0x0B, 0x0A]), "MOV [00A0Bh], AL");
    // A port and a register: the 8086 writes the port first.
    assert_eq!(text(&[0xE6, 0x0A]), "OUT 0Ah, AL");
}

#[test]
fn a_jump_holds_its_absolute_address() {
    // The short jump at 0000h goes to 007Fh: the displacement is 7Dh and
    // the instruction is two bytes long.
    assert_eq!(text(&[0xEB, 0x7D]), "JMP SHORT 0007Fh");
    // The conditional jump says the same.
    assert_eq!(text(&[0x74, 0x7D]), "JE SHORT 0007Fh");
    // The near jump is three bytes long.
    assert_eq!(text(&[0xE9, 0x7C, 0x00]), "JMP 0007Fh");
}

#[test]
fn the_house_style_is_a_leading_zero_and_two_digit_bytes() {
    // Every hex value in the text is `0`, the digits, and an `h`.
    for bytes in [
        &[0x8B, 0x06, 0x0B, 0x0A][..],
        &[0x9A, 0x0B, 0x0A, 0x34, 0x12][..],
        &[0xB0, 0xFF][..],
        &[0xEB, 0x7D][..],
        &[0x8B, 0x46, 0xFE][..],
    ] {
        let text = text(bytes);
        let mut value = String::new();
        for ch in text.chars().chain([' ']) {
            if ch.is_ascii_hexdigit() {
                value.push(ch);
                continue;
            }
            if ch == 'h' && !value.is_empty() {
                assert!(
                    value.starts_with('0') && value.len() > 1,
                    "`{value}` is not in the house style"
                );
            }
            value.clear();
        }
    }
}

#[test]
fn the_prefixes_come_before_the_mnemonic() {
    assert_eq!(text(&[0xF3, 0xA4]), "REP MOVSB");
    assert_eq!(text(&[0xF2, 0xAE]), "REPNE SCASB");
    assert_eq!(text(&[0xF0, 0x40]), "LOCK INC AX");
    assert_eq!(
        text(&[0x26, 0x8B, 0x06, 0x0B, 0x0A]),
        "ES: MOV AX, [00A0Bh]"
    );
}

#[test]
fn the_string_instructions_carry_their_width() {
    // The guide writes `MOVS b` and the reference writes `MOVSB`.
    assert_eq!(text(&[0xA4]), "MOVSB");
    assert_eq!(text(&[0xA5]), "MOVSW");
    assert_eq!(text(&[0xA6]), "CMPSB");
    assert_eq!(text(&[0xA7]), "CMPSW");
    assert_eq!(text(&[0xAA]), "STOSB");
    assert_eq!(text(&[0xAC]), "LODSB");
    assert_eq!(text(&[0xAE]), "SCASB");
    // The guide's `XLAT` has no width of its own.
    assert_eq!(text(&[0xD7]), "XLATB");
}

#[test]
fn the_far_return_is_the_references_retf() {
    assert_eq!(text(&[0xCB]), "RETF");
    assert_eq!(text(&[0xCA, 0x0B, 0x0A]), "RETF 00A0Bh");
    // The near return is the guide's own.
    assert_eq!(text(&[0xC3]), "RET");
}

#[test]
fn the_x87_range_is_an_escape_index_and_not_an_illegal_opcode() {
    // The reference decodes the 80187 mnemonics for display only.
    assert_eq!(text(&[0xD8, 0xC0]), "FADD st0");
    assert_eq!(text(&[0xD8, 0xC1]), "FADD st1");
    assert_eq!(text(&[0xD8, 0x06, 0x0B, 0x0A]), "FADD [00A0Bh]");
    assert_eq!(text(&[0xD9, 0xF0]), "FCHS");
    assert_eq!(text(&[0xD9, 0xF8]), "FLDLG2");
    assert_eq!(text(&[0xD9, 0xE1]), "FDISI");
    assert_eq!(text(&[0xD9, 0xE2]), "FCLEX");
    assert_eq!(text(&[0xD9, 0xE3]), "FINIT");
    assert_eq!(text(&[0xDA, 0xE9]), "FUCOMPP st0");
    assert_eq!(text(&[0xDE, 0xD9]), "FCOMPP st0");
}

#[test]
fn the_x87_has_two_fallbacks() {
    // The `DD` group with the register 001 is not one the table names: the
    // family is known, the operation is not.
    assert_eq!(text(&[0xDD, 0xC8]), "F??? st0");
    // The `D9` group with the register 011 is not a group at all.
    assert_eq!(text(&[0xD9, 0xD8]), "??? st0");
}

#[test]
fn the_bios_stub_is_decoded_as_bios_di() {
    // The FFFF prefix in front of CD is the BIOS device interrupt, so
    // FFFFCD10 is the interrupt 10h and not `int 0FFh`.
    let instruction = disasm::disassemble(&[0xFF, 0xFF, 0xCD, 0x10], 0).expect("the stub decodes");
    assert_eq!(instruction.text, "BIOS DI");
    assert_eq!(instruction.length, 4);
    // The default stub at F400:0100 is the same shape.
    assert_eq!(text(&[0xFF, 0xFF, 0xCD, 0xFF]), "BIOS DI");
    // The interrupt on its own is the interrupt.
    assert_eq!(text(&[0xCD, 0x10]), "INT 010h");
}

#[test]
fn an_undefined_opcode_says_what_the_reference_says() {
    assert_eq!(
        disasm::UNDEFINED,
        "no such opcode in 8086 instruction set",
        "the reference's own words"
    );
    // The ESC range is not the 8086's: the coprocessor's table answers for
    // it, and the 8086 instruction set has nothing there.
    assert!(disasm::instruction_of(0xD8).is_none());
    assert!(disasm::instruction_of(0x00).is_some());
}

#[test]
fn the_bytes_that_run_out_are_refused() {
    assert_eq!(
        disasm::disassemble(&[0x8B], 0),
        Err(Error::Truncated),
        "the ModR/M byte is missing"
    );
    assert_eq!(
        disasm::disassemble(&[0x8B, 0x06, 0x0B], 0),
        Err(Error::Truncated),
        "the displacement is short"
    );
    assert_eq!(
        disasm::disassemble(&[0xB8, 0x0B], 0),
        Err(Error::Truncated),
        "the immediate is short"
    );
}

#[test]
fn the_disassembler_walks_the_bytes() {
    let bytes = [0xB8, 0x0B, 0x0A, 0x8B, 0x06, 0x0B, 0x0A, 0xC3];
    let instructions = disasm::disassemble_all(&bytes, 0);
    let texts: Vec<&str> = instructions.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["MOV AX, 00A0Bh", "MOV AX, [00A0Bh]", "RET"]);
    // Every instruction says how long it is.
    let lengths: Vec<usize> = instructions.iter().map(|i| i.length).collect();
    assert_eq!(lengths, [3, 4, 1]);
}

#[test]
fn the_address_is_where_the_instruction_is() {
    // The jump at 0100h goes to 017Fh.
    let instruction = disasm::disassemble(&[0xEB, 0x7D], 0x0100).expect("the jump decodes");
    assert_eq!(instruction.text, "JMP SHORT 0017Fh");
}
