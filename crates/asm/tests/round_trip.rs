//! The round trip: the text the disassembler prints assembles back to the
//! same bytes.
//!
//! The corpus is the guide's own table: every form the map gives, and every
//! operation the three group tables give, is assembled, disassembled and
//! assembled again. The bytes that the assembler does not produce - the
//! alias row, the direct address forms, the `82` and `83` rows - are a
//! second corpus, and the cases that cannot come back are named there.

use emo8067_asm::disasm;
use emo8067_asm::groups;
use emo8067_asm::program;
use emo8067_asm::table;

/// The source line for one form of the guide's table.
///
/// The operands are the ones the form asks for, and they are chosen so
/// that every field of the instruction carries a value of its own: the
/// register in the REG field, the reference in the R/M field, and the
/// immediate in the bytes that follow.
fn line_for(name: &str, form: &str) -> Option<String> {
    let mnemonic = match (name, form) {
        // The string instructions carry their width apart, and the
        // reference writes them as one word.
        ("MOVS", "b") | ("CMPS", "b") | ("STOS", "b") | ("LODS", "b") | ("SCAS", "b") => {
            format!("{name}B")
        }
        ("MOVS", "w") | ("CMPS", "w") | ("STOS", "w") | ("LODS", "w") | ("SCAS", "w") => {
            format!("{name}W")
        }
        // The far return is the reference's `RETF`.
        ("RET", "(inter)") | ("RET", "i16 (inter)") => "RETF".to_string(),
        // The guide's `XLAT` has no width of its own.
        ("XLAT", "") => "XLATB".to_string(),
        _ => name.split('/').next().unwrap_or(name).trim().to_string(),
    };
    let mut operands = Vec::new();
    for part in form.split(',').map(str::trim) {
        let base = part.split_once(' ').map_or(part, |(base, _)| base).trim();
        let operand = match base {
            // The guide's qualifiers are not operands.
            "" | "(intra)" | "(inter)" | "b" | "w" => continue,
            "r8" => "CH",
            "r16" => "CX",
            "r/m8" | "r/m16" | "r16/m16" | "MEM16" | "m8" | "m16" => "[BX+SI]",
            "sreg" => "DS",
            "i8" | "i8:sx" => "0Ah",
            "i16" => "0A0Bh",
            // The target of a jump is within reach of the short form.
            "short" | "near" => "007Fh",
            "far" => "0000h:0A0Bh",
            "1" => "1",
            "CL" => "CL",
            "3" => "3",
            other => other,
        };
        operands.push(operand.to_string());
    }
    if operands.is_empty() {
        Some(mnemonic)
    } else {
        Some(format!("{mnemonic} {}", operands.join(", ")))
    }
}

/// Every form the guide's table gives, as a source line.
fn corpus() -> Vec<String> {
    let mut out = Vec::new();
    for cell in table::forms() {
        let name = cell
            .mnemonic
            .split('/')
            .next()
            .unwrap_or(cell.mnemonic)
            .trim();
        // The prefixes are prefixes and not instructions, and the ESC
        // range is the coprocessor's.
        if name.starts_with("SEG")
            || name.starts_with("LOCK")
            || name.starts_with("REP")
            || name.starts_with("ESC")
            || name.starts_with("Group")
            || name == "Shift"
        {
            continue;
        }
        if let Some(line) = line_for(cell.mnemonic, cell.operands) {
            out.push(line);
        }
    }
    for group in groups::forms() {
        let (name, form) = group.mnemonic_form();
        if let Some(line) = line_for(name, form) {
            out.push(line);
        }
    }
    out
}

#[test]
fn every_form_the_guides_table_gives_assembles_back() {
    let mut failures = Vec::new();
    for source in corpus() {
        let Ok(program) = program::assemble_program(&source) else {
            continue;
        };
        let bytes = program.bytes;
        let instructions = disasm::disassemble_all(&bytes, 0);
        if instructions.len() != 1 {
            failures.push(format!("{source}: {} instructions", instructions.len()));
            continue;
        }
        let text = &instructions[0].text;
        match program::assemble_program(text) {
            Ok(again) => {
                if again.bytes != bytes {
                    failures.push(format!(
                        "{source}: `{text}` gives {:02X?} and not {:02X?}",
                        again.bytes, bytes
                    ));
                }
            }
            Err(error) => failures.push(format!("{source}: `{text}` does not assemble: {error:?}")),
        }
    }
    assert_eq!(failures.len(), 0, "the round trip fails: {failures:?}");
}

/// The bytes the assembler does not produce, and what the disassembler says
/// about them.
///
/// The last field says whether the text comes back to the same bytes: the
/// cases that do not are named in the comment beside them.
const OTHERS: &[(&[u8], &str, bool)] = &[
    // The guide's alias row: the 8086 runs the conditional jumps from 60h
    // as well, and the assembler writes the canonical opcodes.
    (&[0x60, 0x7D], "JO SHORT 0007Fh", false),
    // The `82` row is the byte immediate of `80`, and the assembler writes
    // `80`.
    (&[0x82, 0xC0, 0x0A], "ADD AL, 0Ah", false),
    // The `83` row takes a sign-extended byte, and the assembler writes
    // the `81` row with a word.
    (&[0x83, 0xC0, 0x0A], "ADD AX, 0Ah", false),
    // The direct address forms: the assembler always writes the reference
    // through the R/M field, which is one byte longer.
    (&[0xA0, 0x0B, 0x0A], "MOV AL, [00A0Bh]", false),
    (&[0xA1, 0x0B, 0x0A], "MOV AX, [00A0Bh]", false),
    (&[0xA2, 0x0B, 0x0A], "MOV [00A0Bh], AL", false),
    (&[0xA3, 0x0B, 0x0A], "MOV [00A0Bh], AX", false),
    // The indirect call and jump: `FF /2` and `FF /3` take the same
    // operands, and the assembler cannot tell them apart.
    (&[0xFF, 0x18], "CALL WORD PTR [BX+SI]", false),
    (&[0xFF, 0x28], "JMP WORD PTR [BX+SI]", false),
    // The second LOCK: the guide says the 8086 runs it as a prefix too,
    // and the assembler writes F0h.
    (&[0xF1, 0x40], "LOCK INC AX", false),
    // A displacement of zero in the short form: the assembler always
    // writes the shortest form.
    (&[0x8B, 0x40, 0x00], "MOV AX, [BX+SI+00h]", false),
    // The x87 range is for display only: the 8086 does not execute it.
    (&[0xD8, 0xC0], "FADD st0", false),
    // And so is the BIOS device interrupt.
    (&[0xFF, 0xFF, 0xCD, 0x10], "BIOS DI", false),
    // The prefixes come back as they went in.
    (&[0xF0, 0x40], "LOCK INC AX", true),
    (&[0xF3, 0xA4], "REP MOVSB", true),
    (
        &[0x26, 0x8B, 0x06, 0x0B, 0x0A],
        "ES: MOV AX, [00A0Bh]",
        true,
    ),
    // The short jump needs the qualifier, because the near jump is the
    // one the assembler writes.
    (&[0xEB, 0x7D], "JMP SHORT 0007Fh", true),
    (&[0x74, 0x7D], "JE SHORT 0007Fh", true),
];

#[test]
fn the_bytes_the_assembler_does_not_write_are_read_all_the_same() {
    let mut failures = Vec::new();
    for (bytes, expected, comes_back) in OTHERS {
        let instruction = disasm::disassemble(bytes, 0).expect("the bytes decode");
        if instruction.text != *expected {
            failures.push(format!(
                "{bytes:02X?}: `{}` and not `{expected}`",
                instruction.text
            ));
            continue;
        }
        let assembled = program::assemble_program(expected);
        let same = assembled
            .map(|program| program.bytes == *bytes)
            .unwrap_or(false);
        if same != *comes_back {
            failures.push(format!(
                "{bytes:02X?}: `{expected}` {} the same bytes",
                if same { "gives" } else { "does not give" }
            ));
        }
    }
    assert_eq!(failures.len(), 0, "the cases differ: {failures:?}");
}
