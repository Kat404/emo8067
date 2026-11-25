//! Independent verification probes for the screen panel (S8) and for the
//! guest-writable geometry the panel reads.

use emo8067_core::Cpu;
use emo8067_core::regs::{Reg8, Reg16, Seg};
use emo8067_debug::panels::{Panels, Ui};

/// The segment the BIOS data area lives in.
const DATA: u16 = 0x0040;
/// The number of columns on screen.
const COLUMNS: u16 = 0x004A;
/// The current video page number.
const PAGE: u16 = 0x0062;
/// The rows on screen, minus one.
const ROWS: u16 = 0x0084;

/// The segment the text pages live in.
const VIDEO: u16 = 0xB800;
/// The segment the test programs run in.
const CODE: u16 = 0x1000;

/// A machine that has called `INT 10h`, with the registers the test sets
/// before the call.
fn bios(setup: impl FnOnce(&mut Cpu)) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.mem
        .load_at(0x10000, &[0xCD, 0x10, 0xF4]) // int 10h; hlt
        .expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, CODE);
    setup(&mut cpu);
    cpu.step().expect("the call runs");
    cpu.step().expect("the service runs");
    cpu
}

/// S8 -- the panel shows the geometry the machine has.
#[test]
fn s8_the_panel_shows_the_geometry_the_machine_has() {
    let mut cpu = Cpu::new();
    // Mode 00h: forty columns.
    cpu.regs.set_byte(Reg8::Ah, 0x00);
    cpu.regs.set_byte(Reg8::Al, 0x00);
    cpu.mem
        .load_at(0x10000, &[0xCD, 0x10, 0xF4])
        .expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, CODE);
    cpu.step().expect("the call runs");
    cpu.step().expect("the service runs");

    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(screen.columns, 40, "the panel shows the machine's columns");
    assert_eq!(screen.rows, 25, "and the machine's rows");
}

/// S8 -- the panel holds no copy: it reads the machine again every time.
#[test]
fn s8_the_panel_holds_no_copy_of_the_machine() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u8(VIDEO, 0x0000, b'A');
    let before = Panels::of(&cpu, &Ui::default());
    assert_eq!(before.screen.cells[0].character, 'A', "the page is read");

    // The guest writes to the page after the panel was built.
    cpu.mem.write_u8(VIDEO, 0x0000, b'B');
    let after = Panels::of(&cpu, &Ui::default());
    assert_eq!(
        after.screen.cells[0].character, 'B',
        "the panel reads the machine again: it holds no copy"
    );

    // And the machine's own geometry: the guest changes it, and the panel
    // follows.
    cpu.mem.write_u16(DATA, COLUMNS, 40);
    let changed = Panels::of(&cpu, &Ui::default());
    assert_eq!(
        changed.screen.columns, 40,
        "the panel follows the geometry the machine has"
    );
}

/// S8 -- the panel's own page selector stays what it is: the panel shows the
/// page the UI names, and it does not move the selector.
#[test]
fn s8_the_panels_own_page_selector_stays_what_it_is() {
    let mut cpu = Cpu::new();
    // The guest writes to the second page.
    cpu.mem.write_u8(VIDEO, 0x1000, b'P');
    let ui = Ui {
        screen_page: 1,
        ..Ui::default()
    };
    let panels = Panels::of(&cpu, &ui);
    assert_eq!(
        panels.screen.page, 1,
        "the panel shows the page the UI says"
    );
    assert_eq!(
        panels.screen.cells[0].character, 'P',
        "and it reads that page"
    );
    assert_eq!(ui.screen_page, 1, "and the UI's own selector did not move");

    // The machine's active page is a different page: the panel still shows
    // the one the UI names.
    cpu.mem.write_u8(DATA, PAGE, 0);
    let panels = Panels::of(&cpu, &ui);
    assert_eq!(
        panels.screen.page, 1,
        "the panel's own selector is the panel's: the machine's active page does not move it"
    );
    assert_eq!(
        panels.screen.cursor, None,
        "and the cursor is not drawn: the panel shows another page"
    );
}

/// S8 -- the cursor is drawn where the BIOS says it is.
#[test]
fn s8_the_cursor_is_where_the_bios_says_it_is() {
    let cpu = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x02);
        c.regs.set_byte(Reg8::Dh, 4);
        c.regs.set_byte(Reg8::Dl, 6);
    });
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    let cursor = screen.cursor.expect("the cursor is on the page");
    assert_eq!(cursor.row, 4, "the row the BIOS set");
    assert_eq!(cursor.column, 6, "and the column the BIOS set");
}

/// S8 -- a cursor the guest hid is not drawn at all.
#[test]
fn s8_a_cursor_the_guest_hid_is_not_drawn() {
    let cpu = bios(|c| {
        c.regs.set_byte(Reg8::Ah, 0x01);
        c.regs.set_byte(Reg8::Ch, 0x20); // bit 5: the cursor is not visible
        c.regs.set_byte(Reg8::Cl, 0x07);
    });
    let screen = &Panels::of(&cpu, &Ui::default()).screen;
    assert_eq!(screen.cursor, None, "no cursor is drawn");
}

/// Finding 3 -- the panel's cell vector is sized from the guest-writable
/// geometry, so a guest that stores a huge column count makes every panel
/// rebuild allocate without bound.
#[test]
fn finding_the_panel_must_not_allocate_from_the_guests_geometry() {
    let mut cpu = Cpu::new();
    // The guest stores 0FFFFh in the columns word and 0FFh in the rows word.
    cpu.mem.write_u16(DATA, COLUMNS, 0xFFFF);
    cpu.mem.write_u8(DATA, ROWS, 0xFF);

    let panels = Panels::of(&cpu, &Ui::default());
    let screen = &panels.screen;
    assert!(
        screen.cells.len() <= 80 * 25,
        "the panel's page is bounded by the hardware, not by the guest: {} cells",
        screen.cells.len()
    );
}

/// Finding 3, second form: the guest's column count makes the panel's own
/// address arithmetic overflow.
#[test]
fn finding_the_panel_must_not_overflow_on_the_guests_geometry() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u16(DATA, COLUMNS, 0x4000);
    cpu.mem.write_u8(DATA, ROWS, 0xFF);

    let panels = Panels::of(&cpu, &Ui::default());
    assert_eq!(
        panels.screen.cells.len(),
        80 * 25,
        "the panel's page is bounded by the hardware"
    );
}

/// The guest writes the geometry itself: the data area is ordinary memory
/// and the guest can store in it.
#[test]
fn the_guest_can_write_the_geometry_itself() {
    let mut cpu = Cpu::new();
    // xor ax, ax / mov ds, ax / mov word [044Ah], 0FFFFh / hlt
    cpu.mem
        .load_at(
            0x10000,
            &[
                0x31, 0xC0, 0x8E, 0xD8, 0xC7, 0x06, 0x4A, 0x04, 0xFF, 0xFF, 0xF4,
            ],
        )
        .expect("the program loads");
    cpu.regs.set_segment(Seg::Cs, CODE);
    for _ in 0..10 {
        if cpu.halted {
            break;
        }
        cpu.step().expect("the program runs");
    }
    assert_eq!(
        cpu.mem.read_u16(DATA, COLUMNS),
        0xFFFF,
        "the guest stored the column count in the data area"
    );
    assert_eq!(
        cpu.mem.read_u16(0x0000, 0x044A),
        0xFFFF,
        "and physical 044Ah is the data area's columns word"
    );
    let _ = Reg16::Cx;
}

/// Finding 3, measured: how many cells the panel allocates for the guest's
/// geometry.
#[test]
fn finding_the_panel_allocation_size_for_the_guests_geometry() {
    let mut cpu = Cpu::new();
    cpu.mem.write_u16(DATA, COLUMNS, 0xFFFF);
    cpu.mem.write_u8(DATA, ROWS, 0xFF);
    let panels = Panels::of(&cpu, &Ui::default());
    println!(
        "the panel allocated {} cells for the guest's geometry",
        panels.screen.cells.len()
    );
}
