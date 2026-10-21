//! The prefixes the source writes are emitted as the bytes the map gives them.
//!
//! S10 of `odd/tasks/assembler.md`: the prefix bytes come out of the guide's
//! own map, the prefix comes before the opcode, and the prefixes come in the
//! order the source wrote them.

use emo8067_asm::program::assemble_program;

/// The bytes of a program that assembles.
fn bytes_of(source: &str) -> Vec<u8> {
    assemble_program(source)
        .expect("the program assembles")
        .bytes
}

#[test]
fn the_repeat_prefixes_are_the_bytes_the_map_gives_them() {
    assert_eq!(bytes_of("org 100h\nrep movsb\n"), vec![0xF3, 0xA4]);
    assert_eq!(bytes_of("org 100h\nrepe cmpsb\n"), vec![0xF3, 0xA6]);
    assert_eq!(bytes_of("org 100h\nrepz cmpsw\n"), vec![0xF3, 0xA7]);
    assert_eq!(bytes_of("org 100h\nrepne scasb\n"), vec![0xF2, 0xAE]);
    assert_eq!(bytes_of("org 100h\nrepnz scasw\n"), vec![0xF2, 0xAF]);
}

#[test]
fn the_lock_prefix_is_the_byte_the_map_gives_it() {
    assert_eq!(
        bytes_of("org 100h\nlock xchg ax,bx\n"),
        vec![0xF0, 0x87, 0xC3]
    );
}

#[test]
fn the_segment_overrides_are_the_cells_that_read_seg() {
    assert_eq!(
        bytes_of("org 100h\ncs: mov ax,1\n"),
        vec![0x2E, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\nss: mov ax,1\n"),
        vec![0x36, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\nds: mov ax,1\n"),
        vec![0x3E, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\nes: mov ax,1\n"),
        vec![0x26, 0xB8, 0x01, 0x00]
    );
}

#[test]
fn the_dotted_override_is_the_same_prefix() {
    assert_eq!(
        bytes_of("org 100h\n.cs: mov ax,1\n"),
        vec![0x2E, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\n.ss: mov ax,1\n"),
        vec![0x36, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\n.ds: mov ax,1\n"),
        vec![0x3E, 0xB8, 0x01, 0x00]
    );
    assert_eq!(
        bytes_of("org 100h\n.es: mov ax,1\n"),
        vec![0x26, 0xB8, 0x01, 0x00]
    );
}

#[test]
fn the_prefix_comes_before_the_opcode_in_the_order_it_was_written() {
    assert_eq!(
        bytes_of("org 100h\ncs: lock xchg ax,bx\n"),
        vec![0x2E, 0xF0, 0x87, 0xC3]
    );
    assert_eq!(
        bytes_of("org 100h\nlock cs: xchg ax,bx\n"),
        vec![0xF0, 0x2E, 0x87, 0xC3]
    );
}

#[test]
fn an_instruction_without_a_prefix_has_no_prefix_byte() {
    assert_eq!(bytes_of("org 100h\nmovsb\n"), vec![0xA4]);
}
