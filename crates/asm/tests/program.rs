//! A program, assembled.

use emo8067_asm::program::assemble_program;

#[test]
fn a_program_knows_where_it_starts_and_what_it_holds() {
    let program = assemble_program("org 100h\nmov ax, 3\ndb 'H', 0\n").unwrap();
    assert_eq!(program.origin, 0x100);
    assert_eq!(program.bytes, vec![0xB8, 0x03, 0x00, b'H', 0x00]);
}

#[test]
fn a_word_is_two_bytes_little_endian() {
    let program = assemble_program("dw 1234h\ndd 1\n").unwrap();
    assert_eq!(program.bytes, vec![0x34, 0x12, 0x01, 0x00, 0x00, 0x00]);
}

#[test]
fn a_label_is_remembered_where_it_is() {
    let program = assemble_program("org 100h\nmov ax, 1\nhere:\nnop\n").unwrap();
    assert_eq!(program.names, vec![("here".to_string(), 0x103)]);
}

#[test]
fn a_constant_is_folded_into_the_operand_that_names_it() {
    let program = assemble_program("SIZE EQU 4\nmov cx, SIZE\n").unwrap();
    assert_eq!(program.bytes, vec![0xB9, 0x04, 0x00]);
}

#[test]
fn the_directives_that_say_nothing_are_passed_over() {
    let program = assemble_program(
        "name \"a-program\"\nsegment code\nassume cs:code\nstart:\nmov ax, 0\nend\n",
    )
    .unwrap();
    assert_eq!(program.bytes, vec![0xB8, 0x00, 0x00]);
    assert_eq!(program.names, vec![("start".to_string(), 0)]);
}

#[test]
fn a_jump_knows_where_it_is_going() {
    // The program is walked twice: the first time the names are collected
    // and the addresses counted, the second time the bytes are made.
    let program = assemble_program("org 100h\nmov ax, 1\nhere:\njmp here\n").unwrap();
    assert_eq!(program.bytes, vec![0xB8, 0x01, 0x00, 0xE9, 0xFD, 0xFF]);
}

#[test]
fn a_conditional_jump_is_the_short_form() {
    let program = assemble_program("org 0\nje there\nnop\nthere:\nnop\n").unwrap();
    // The jump is at 0 and ends at 2; the label is after the nop, at 3, so
    // the displacement is one. The canonical opcode is 74h: the guide's row
    // 60 is the chip's alias of the 70h row, and the assembler writes the
    // canonical one.
    assert_eq!(program.bytes, vec![0x74, 0x01, 0x90, 0x90]);
}

#[test]
fn an_offset_can_be_part_of_an_expression() {
    let program = assemble_program(
        "org 100h\nmsg db 'hello'\nmsg_end:\nmov cx, offset msg_end - offset msg\n",
    )
    .unwrap();
    // The message is five bytes long, and MOV CX,5 is B9 05 00.
    assert_eq!(
        program.bytes,
        vec![b'h', b'e', b'l', b'l', b'o', 0xB9, 0x05, 0x00]
    );
}

#[test]
fn an_offset_can_have_an_addition() {
    let program =
        assemble_program("org 100h\nfilename db 'ab'\nmov dx, offset filename+2\n").unwrap();
    // The name is at 100h, and MOV DX,102h is BA 02 01.
    assert_eq!(program.bytes, vec![b'a', b'b', 0xBA, 0x02, 0x01]);
}

#[test]
fn a_conditional_jump_that_is_too_far_is_replaced_by_its_opposite() {
    let mut source = String::from("org 0\nje there\n");
    for _ in 0..128 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nnop\n");
    let program = assemble_program(&source).unwrap();
    // The reference's own words: "short conditional jumps are replaced with
    // the opposite instruction and one normal jump", five bytes instead of
    // two. `JE` is 74h and its opposite is 75h, the JNZ that skips the big
    // jump, and the big jump is E9h with the displacement measured from the
    // end of itself: the label is at 133, and the jump ends at five.
    assert_eq!(&program.bytes[..5], &[0x75, 0x03, 0xE9, 0x80, 0x00]);
    // The label is at 133: the replacement is three bytes longer than the
    // short form, and the nops that follow it start after that.
    assert_eq!(program.names, vec![("there".to_string(), 133)]);
    // The program is three bytes longer than the short form would be.
    assert_eq!(program.bytes.len(), 134);
}

#[test]
fn a_jcxz_that_is_too_far_is_replaced_by_or_cx_cx_and_two_jumps() {
    let mut source = String::from("org 0\njcxz there\n");
    for _ in 0..128 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nnop\n");
    let program = assemble_program(&source).unwrap();
    // The reference's own words: the instruction "is assembled into OR CX,
    // CX and two JMPs". `OR CX, CX` is 09h C9h - the guide's R/M field
    // names CX 001, so the byte is C9h and not C8h, which would be
    // `OR AX, CX` - the JNZ skips the big jump, and the big jump is E9h
    // with the displacement measured from the end of itself: the label is
    // at 135, and the jump ends at seven.
    assert_eq!(
        &program.bytes[..7],
        &[0x09, 0xC9, 0x75, 0x03, 0xE9, 0x80, 0x00]
    );
    assert_eq!(program.names, vec![("there".to_string(), 135)]);
    assert_eq!(program.bytes.len(), 136);
}

#[test]
fn the_opposite_of_every_conditional_jump_is_the_one_next_to_it() {
    // The guide's row 70 is pairs of opposites: JO JNO, JB/JNAE JNB/JAE,
    // JE/JZ JNE/JNZ, JBE/JNA JNBE/JA, JS JNS, JP/JPE JNP/JPO, JL/JNGE
    // JNL/JGE, JLE/JNG JNLE/JG.
    let pairs = [
        ("JO", "JNO"),
        ("JB", "JNB"),
        ("JE", "JNZ"),
        ("JBE", "JA"),
        ("JS", "JNS"),
        ("JP", "JNP"),
        ("JL", "JNL"),
        ("JLE", "JG"),
    ];
    for (jump, opposite) in pairs {
        let mut source = format!("org 0\n{jump} there\n");
        for _ in 0..128 {
            source.push_str("nop\n");
        }
        source.push_str("there:\nnop\n");
        let program = assemble_program(&source).unwrap();
        // The replacement starts with the opposite instruction, and the
        // opposite of every pair of the row is the opcode with its low bit
        // turned over.
        assert_eq!(program.bytes[0], opcode_of(opposite), "{jump}");
        assert_eq!(program.bytes[0], opcode_of(jump) ^ 1, "{jump}");
    }
}

#[test]
fn the_count_walk_counts_a_jump_that_is_too_far_as_five_bytes() {
    let mut source = String::from("org 0\nje there\nafter:\n");
    for _ in 0..200 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nnop\n");
    let program = assemble_program(&source).unwrap();
    // The label that follows the jump is at five and not at two: the walk
    // that counts the addresses knows the size the second walk will make.
    assert_eq!(
        program.names,
        vec![("after".to_string(), 5), ("there".to_string(), 205)]
    );
}

#[test]
fn the_layout_settles_before_the_bytes_are_made() {
    // The jump's own size moves the label it goes to, and the label's
    // address is what the big jump's displacement is measured from, so the
    // program is walked again until the names stop moving.
    let mut source = String::from("org 0\nje there\n");
    for _ in 0..128 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nmov ax, 1\n");
    let program = assemble_program(&source).unwrap();
    // The label is at 133, and the big jump's displacement is measured from
    // the end of the jump itself, which is at five: 133 - 5 is 128.
    assert_eq!(&program.bytes[..5], &[0x75, 0x03, 0xE9, 0x80, 0x00]);
    // The bytes at the label's address are the ones the label names.
    let at = program
        .names
        .iter()
        .find(|(name, _)| name == "there")
        .expect("the label")
        .1 as usize;
    assert_eq!(&program.bytes[at..at + 3], &[0xB8, 0x01, 0x00]);
}

#[test]
fn a_jump_to_a_label_that_comes_later_is_counted_before_it_is_known() {
    let program = assemble_program("org 0\njmp later\nnop\nlater:\nnop\n").unwrap();
    // The near jump is three bytes whether the label is known or not, and
    // the label is at four: the nop is between the jump and the label.
    assert_eq!(program.bytes, vec![0xE9, 0x01, 0x00, 0x90, 0x90]);
    assert_eq!(program.names, vec![("later".to_string(), 4)]);
}

#[test]
fn a_loop_that_is_too_far_is_refused_in_the_reference_words() {
    let mut source = String::from("org 0\nloop there\n");
    for _ in 0..200 {
        source.push_str("nop\n");
    }
    source.push_str("there:\nnop\n");
    let error = assemble_program(&source).unwrap_err();
    // The refusal says which line it happened on.
    let inner = match error {
        emo8067_asm::program::Error::AtLine { error, .. } => *error,
        other => panic!("expected the refusal, got {other:?}"),
    };
    match inner {
        emo8067_asm::program::Error::JumpOutOfRange { message } => {
            assert_eq!(
                message,
                "conditional jump is out of range: allowed values are from -128 to 127"
            );
        }
        other => panic!("expected the refusal, got {other:?}"),
    }
}

#[test]
fn the_repeat_prefix_is_the_byte_the_map_gives_it() {
    // `rep movsb` is F3h A4h: the map's row F0 gives F3 to `REP/REPE/REPZ`.
    let program = assemble_program("rep movsb\n").unwrap();
    assert_eq!(program.bytes, vec![0xF3, 0xA4]);
}

#[test]
fn the_repeat_not_equal_prefix_is_the_byte_the_map_gives_it() {
    // `repne scasb` is F2h AEh: the map gives F2 to `REPNE/REPNZ`.
    let program = assemble_program("repne scasb\n").unwrap();
    assert_eq!(program.bytes, vec![0xF2, 0xAE]);
}

#[test]
fn the_lock_prefix_is_the_byte_the_map_gives_it() {
    // `lock` is F0h: the map's row F0 gives F0 to `LOCK`, and `F1` is the
    // same prefix a second time.
    let program = assemble_program("lock inc ax\n").unwrap();
    assert_eq!(program.bytes, vec![0xF0, 0x40]);
}

#[test]
fn a_segment_override_prefix_is_the_byte_the_map_gives_it() {
    // The map writes the segment overrides as `SEG CS:` and so on, and the
    // bytes are the ones it gives them: CS is 2Eh, SS is 36h, DS is 3Eh
    // and ES is 26h.
    for (prefix, byte) in [("cs:", 0x2E), ("ss:", 0x36), ("ds:", 0x3E), ("es:", 0x26)] {
        let program = assemble_program(&format!("{prefix} mov ax, [si]\n")).unwrap();
        assert_eq!(program.bytes, vec![byte, 0x8B, 0x04], "{prefix}");
    }
}

#[test]
fn the_prefixes_come_before_the_opcode_in_the_order_they_were_written() {
    // `cs: rep movsb` is 2Eh F3h A4h: the segment override first, then the
    // repeat, and the opcode last.
    let program = assemble_program("cs: rep movsb\n").unwrap();
    assert_eq!(program.bytes, vec![0x2E, 0xF3, 0xA4]);
}

#[test]
fn a_segment_override_prefix_may_carry_a_dot() {
    // The reference's own list of the prefixes it accepts names the segment
    // overrides both ways, and the byte is the same one.
    let program = assemble_program(".cs: mov ax, [si]\n").unwrap();
    assert_eq!(program.bytes, vec![0x2E, 0x8B, 0x04]);
}

#[test]
fn an_instruction_grows_by_the_bytes_of_its_prefixes() {
    let program = assemble_program("org 100h\nrep movsb\nhere:\nnop\n").unwrap();
    // The prefix is a byte of the instruction, so the label that follows
    // it is at 102h.
    assert_eq!(program.names, vec![("here".to_string(), 0x102)]);
    assert_eq!(program.bytes, vec![0xF3, 0xA4, 0x90]);
}

#[test]
fn a_prefixed_jump_reaches_the_same_label() {
    let program = assemble_program("org 0\nrep je there\nnop\nthere:\nnop\n").unwrap();
    // The prefix is a byte of the jump, so the short jump ends at three and
    // the label is at four: the displacement is one.
    assert_eq!(program.bytes, vec![0xF3, 0x74, 0x01, 0x90, 0x90]);
}

/// The opcode the guide's table gives a short jump.
fn opcode_of(mnemonic: &str) -> u8 {
    emo8067_asm::assemble::options_for_pub(mnemonic)
        .into_iter()
        .find(|(_, form, _)| form.eq_ignore_ascii_case("short"))
        .map(|(opcode, _, _)| opcode)
        .unwrap_or_else(|| panic!("{mnemonic} is not in the table"))
}
