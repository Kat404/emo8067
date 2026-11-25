# The video BIOS — `INT 10h`

Objective: the video services `INT 10h`, in the core, so a guest that asks
the BIOS for something gets it, and the guest's screen shows what the guest
asked for. Branch: `feat/tui-memory-window`. Delivery: one commit, one
independent verifier. Test runner: `cargo nextest run --workspace`.

**Risk: item 3** — the core's interrupt path is shared code that the runner,
the CLI, the programs gate, the TUI and every program consume, and this unit
changes what the machine does when a program calls `INT 10h`.

Written 2026-11-24. Continues `odd/tasks/tui.md` (T15, the guest's screen),
whose passive viewer is what the BIOS's output is seen through.

## Why this and not more

The research's own verdict on a BIOS is settled (`features-and-ux.md` §3.1):
"Full IBM-PC BIOS/DOS compatibility … The reference implements 38 `AH`
functions across 11 vectors … **Verdict: never build. Match the recovered
function set and stop.**"

The reference's own documentation defines `INT 10h` —
`re/docs-txt/8086_bios_and_dos_interrupts.txt` — and the corpus calls twelve
of the fifteen functions it lists, in thirty-eight of its eighty-five
examples. The maintainer chose the thirteen text-mode ones: the ten the
corpus calls and the three it does not, which are a handful of lines each and
leave the surface coherent.

**The reference's ROM is not used.** `re/extracted/app/INT_VECT` and
`re/extracted/app/BIOS_ROM` are the reference's own code, they are local gold
that is never committed, and this project's clean-room rule is that reverse
engineering establishes observable behaviour and specifications only — no code
is ever copied or transliterated from another emulator. A feature that
depended on them would not ship. The services are written in Rust against the
documented contract.

**The reference's own map is where the state lives.** `re/docs-txt/memory.txt`
documents the BIOS data area, and the BIOS keeps its state there: the guest
can read it, and the BIOS is a function of the machine's memory and its
registers with no state of its own.

## Specs

- **S1** — The `INT 10h` is the 8086's `INT`. It pushes FLAGS, CS and IP, it
  clears TF and IF, and it loads `CS:IP` from the interrupt vector table,
  exactly as it does for every other vector. **The BIOS does not answer
  the call in place and does not change what the instruction does.** The
  hardware captures say the chip pushes the frame and vectors — two of the
  646 000 captures are `INT 10h`, and the chip leaves the stack six bytes
  lower and the machine at the vector — and the emulator does the same. A
  machine whose vector table points somewhere else runs whatever is there,
  the way the bare chip does. Every other vector is untouched, and so is
  every vector the BIOS has no service for.
- **S2** — The services are the reference's own thirteen, with the
  reference's own register contracts, quoted from
  `re/docs-txt/8086_bios_and_dos_interrupts.txt`:
  - `AH = 00h` — set video mode. `AL` = desired video mode. "00h - text mode.
    40x25. 16 colors. 8 pages." "03h - text mode. 80x25. 16 colors. 8 pages."
  - `AH = 01h` — set text-mode cursor shape. `CH` = cursor start line (bits
    0-4) and options (bits 5-7). `CL` = bottom cursor line (bits 0-4).
    "when bit 5 of CH is set to 0, the cursor is visible. when bit 5 is 1,
    the cursor is not visible."
  - `AH = 02h` — set cursor position. `DH` = row. `DL` = column. `BH` = page
    number (0..7).
  - `AH = 03h` — get cursor position and size. `BH` = page number. Returns
    `DH` = row, `DL` = column, `CH` = cursor start line, `CL` = cursor bottom
    line.
  - `AH = 05h` — select active video page. `AL` = new page number (0..7).
  - `AH = 06h` — scroll up window. `AH = 07h` — scroll down window.
    `AL` = number of lines by which to scroll (00h = clear entire window).
    `BH` = attribute used to write blank lines at bottom of window.
    `CH, CL` = row, column of window's upper left corner.
    `DH, DL` = row, column of window's lower right corner.
  - `AH = 08h` — read character and attribute at cursor position. `BH` = page
    number. Returns `AH` = attribute, `AL` = character.
  - `AH = 09h` — write character and attribute at cursor position. `AL` =
    character to display. `BH` = page number. `BL` = attribute. `CX` = number
    of times to write character.
  - `AH = 0Ah` — write character only at cursor position. `AL` = character to
    display. `BH` = page number. `CX` = number of times to write character.
  - `AH = 0Eh` — teletype output. `AL` = character to write. "this functions
    displays a character on the screen, advancing the cursor and scrolling the
    screen as necessary. the printing is always done to current active page."
  - `AH = 13h` — write string. `AL` = write mode: bit 0: update cursor after
    writing; bit 1: string contains attributes. `BH` = page number. `BL` =
    attribute if string contains only characters (bit 1 of AL is zero). `CX` =
    number of characters in string (attributes are not counted). `DL,DH` =
    column, row at which to start writing. `ES:BP` points to string to be
    printed.
  - `AX = 1003h` — toggle intensity/blinking. `BL` = write mode: 0: enable
    intensive colors. 1: enable blinking. `BH` = 0.
- **S3** — The state is the BIOS data area, at the offsets the reference's own
  memory map gives (`re/docs-txt/memory.txt:115-134`): the number of columns
  on screen at `0040:004A`, the current video page's start in video memory at
  `0040:004E`, the eight cursors at `0040:0050`, the current video page
  number at `0040:0062`, and the rows on screen minus one at `0040:0084`.
  The BIOS keeps no state of its own: every service is a function of the
  machine's memory and its registers.
  - **Two assumptions, and they are assumptions.** The cursor's shape has no
    home in the reference's map; it is kept at `0040:0080`, the two bytes the
    IBM BIOS keeps the cursor's scan lines in. And the cursor's row and column
    are kept the IBM way — the column in the low byte and the row in the high
    byte, one pair a page — because every other offset the reference's map
    names is the IBM one, and the IBM BIOS loads `DX` from that word with the
    column in the low byte.
- **S4** — The machine starts in the emulator's own default video mode:
  eighty columns by twenty-five rows — the equipment list's "80x25 CGA color
  (emulator default)" (`memory.txt:102`). So the data area holds that geometry
  from the moment the machine starts, with the cursor of every page at the
  origin, the current page zero and its start at zero.
- **S5** — The mode sets the geometry. `00h` is forty columns by twenty-five
  rows and `03h` is eighty columns by twenty-five rows, and the mode writes
  the columns and the rows into the data area. A mode this slice does not
  have — `13h` and every other — leaves the geometry as it was.
- **S6** — The page is laid out the way the geometry says: a row is the
  geometry's columns, a cell is two bytes — the character and the attribute —
  and a page is four kilobytes, because "the emulator always uses 1000h
  (4096 bytes) for each page" (`memory.txt:41`). The eight pages are at
  `B800:0000`, `B800:1000`, `B800:2000`, … and the page a service writes is
  the one its `BH` names.
- **S7** — The services write and read the page. `09h` and `0Ah` write at the
  cursor and leave the cursor where it was; `08h` reads the character and the
  attribute from there. `0Eh` writes at the cursor and advances it, and
  scrolls the page when the cursor reaches the bottom. `13h` writes the
  string at the row and column it is given, and moves the cursor only when
  the write mode says so. `06h` and `07h` scroll the window they are given
  and fill the space with the attribute in `BH`.
- **S8** — The screen panel shows the guest's screen as the machine has it:
  the geometry the machine has, the page the machine has active, and the
  cursor where the BIOS says it is. The panel reads the machine's own memory
  and the machine's own data area; it holds no copy of either. The panel's
  own page selector (`Ui.screen_page`) stays what it is — the page the panel
  shows — and the TUI follows the machine's active page with it, so the panel
  shows the page the guest is drawing on.
- **S9** — The examples' recorded states move. The programs that asked the
  BIOS for something now run past it, so their recorded states are different,
  and the recorded states are recorded again. **The recorded states are this
  project's own, not the reference's** — the gate writes them when they are
  absent (`crates/cli/tests/examples.rs:122-125`) — so what they are after
  this unit is what this project does, and the ones that did not move are
  still the record of what it did before.
- **S10** — The BIOS is a handler at an address, and the machine's vector
  table points at it. The reference's own map gives the video functions'
  entry as `F400:0190` (`memory.txt:60`) and the BIOS area at `F4000`
  (`memory.txt:47-50`), so the machine installs the vector for `10h` when it
  starts, and the BIOS's own code lives at the address the vector names.
  **The ROM is this project's own.** The reference's `BIOS_ROM` is its code
  and is never copied, and the services are native.
- **S11** — The service returns the way a handler returns: it pops the frame
  the `INT` pushed and carries on at the instruction after the call, so the
  guest's stack and the guest's flags are where they were and the machine's
  depth of calls goes down as it came up — the way the `IRET` that comes out
  of an interrupt does.

## Out of scope

- **The graphics modes.** `13h` is forty by twenty-five in 256 colours with a
  320×200 framebuffer, and the pixel functions `0Ch` and `0Dh` belong to it.
  The page of graphics is `A0000:B1000` (`memory.txt:24-27`), the pixel's
  address is a row of 320 pixels in eight-pixel bytes, and the screen panel
  is a character grid. That is its own unit.
- **The rest of the BIOS and the DOS.** Thirty-eight `AH` functions across
  eleven vectors is the reference's whole surface, and the research's verdict
  is to match the recovered set and stop.
- **The reference's ROM.** `INT_VECT` and `BIOS_ROM` are the reference's own
  code, local gold that is never committed, and no code of the reference is
  ever copied.
- **The keyboard.** `INT 16h` is in sixty of the examples and is the BIOS's
  other half; it is not this unit.

## Tasks

- **T1** — The video BIOS and its thirteen services. S1-S11. Route:
  delegated, one bounded writer, one commit, one independent verifier.
  **The unit.** The `INT 10h` is the 8086's `INT`: it pushes FLAGS, CS and
  IP, it clears TF and IF, and it loads `CS:IP` from the vector table. The
  BIOS is a handler at an address — the machine installs the vector for
  `10h` when it starts, and the reference's own map gives the video
  functions' entry as `F400:0190` (`memory.txt:60`) — so the service runs
  where the ROM is and returns the way a handler returns, popping the frame
  the `INT` pushed. The ROM is this project's own: the services are native
  and the ROM's one byte at the entry is the `IRET` that comes out of them.
  The state is the BIOS data area at `0040:0000`, and the data area is all
  the BIOS has: the columns at `0040:004A`, the current page's start at
  `0040:004E`, the eight cursors at `0040:0050` (the column in the low
  byte, the row in the high one), the active page at `0040:0062`, the rows
  minus one at `0040:0084`, and the cursor's shape at `0040:0080` — this
  project's own assumption, the reference's map names no home for it. The
  thirteen services are the reference's own, with its own register
  contracts; `AX = 1003h` is answered and changes nothing, because the
  emulator's text page is the CGA's and the mode the toggle asks for is the
  one the page is already in; and the teletype writes with the attribute
  `07h`, because the reference names none and a blank page's attribute is
  zero, which would be black on black. The panel shows the machine's own
  geometry, the machine's own page and the machine's own cursor, the TUI
  follows the machine's active page with it, and the cursor is drawn where
  the BIOS says it is: underlined, the way the VGA draws its own. The files
  are `crates/core/src/bios.rs`, `crates/core/src/lib.rs`,
  `crates/core/src/exec.rs`, `crates/core/tests/bios.rs`,
  `crates/debug/src/panels.rs`, `crates/debug/tests/screen.rs`,
  `crates/tui/src/app.rs`, `crates/tui/src/ui.rs`,
  `crates/tui/tests/screen.rs` and `design/DESIGN.md`. **RED**: seven
  tests, seven failures — the machine did not answer the call at all ("the
  program did not halt": the interrupt was taken and the machine ran on
  from `0000:0000`), and the data area held no geometry ("eighty columns
  on screen", left 0, right 80); the panel had no cursor to show ("no field
  `cursor` on type `&ScreenView`"), and the takeover drew none ("the cursor
  is underlined where the BIOS says it is"). **The correction's RED**: the
  `INT` did not push the frame and did not vector — "the INT pushes the
  frame and vectors" failed, and so did "a machine whose vector table
  points elsewhere runs what is there". **GREEN**: the whole tree is **751
  passed, 0 failed** on `cargo nextest run`, the clippy run is clean in
  every crate and `cargo fmt --all --check` exits 0; **the full hardware
  gate over all 646 000 captures passes with no family added**, because a
  capture puts its own entry in the vector table and the machine vectors
  there and runs whatever is there, the way the bare chip does; and the
  examples' gate is green with the recorded states recorded again. The
  recorded states moved again with the corrected hook — every one of the
  eighty-five, since the geometry now lives in the data area from the
  moment the machine starts and the frames the `INT` pushes land on the
  stack — and the gate writes them when the file is absent, so they were
  deleted and recorded again.
- **T2** — The keyboard of the BIOS: `INT 16h`. Route: delegated, one
  bounded writer, one commit, one independent verifier. **CHOSEN
  2026-11-25, NOT STARTED.** The maintainer's decision: "El teclado del
  BIOS: INT 16h". It is the BIOS's other half, and the one the corpus
  needs most — **sixty of the eighty-five reference programs call it**,
  and without it those programs sit waiting for a key that never comes.
  What the next session needs to know, and what it still has to decide:
  - **The reference documents exactly two functions**
    (`re/docs-txt/8086_bios_and_dos_interrupts.txt:304`-`:315`): `AH =
    00h` gets a keystroke **without echo** and **removes it from the
    keyboard buffer**, returning `AH` = the BIOS scan code and `AL` = the
    ASCII character; `AH = 01h` checks for a keystroke, sets `ZF = 1`
    when there is none, returns the same pair, and **does not remove**
    the keystroke.
  - **The keyboard buffer has no documented home.** The reference's own
    memory map (`re/docs-txt/memory.txt`) gives the *video* state its
    offsets in the data area and gives the **keyboard buffer none** —
    `0040:001A`-`:0032` are not in it. So the video BIOS could keep its
    state where the reference says it is, and the keyboard cannot. Two
    candidates: the IBM PC's conventional BIOS keyboard buffer
    (`0040:001A` head, `0040:001C` tail, `0040:001E` start, `0040:0030`
    end, `0040:0031` size), or Rust state in the core fed by the host's
    keyboard. **This is the unit's first design decision, and it is not
    the parent's to make alone.**
  - **Where the keystrokes come from is the second decision.** The TUI
    has a keyboard — the terminal's own keys — and the headless runner
    has none, and the reference's `INT 16h/00h` **waits** when the buffer
    is empty ("if there is no character in the keyboard buffer, the
    function waits until any key is pressed", `:339`). So the runner's
    keyboard has to be decided: an empty buffer and programs that wait
    until the budget ends them, or a buffer the runner can be given, or
    something else. The eighty-five recorded states will move whatever
    the answer is.
  - **The scan code table needs its own evidence.** The reference's
    document says only "AH = BIOS scan code" and never gives the table.
    The IBM's set 1 is the obvious candidate, and the project's rule is
    that a claim needs a source: either the published table or the
    reference's own observed behaviour.
  - **The BIOS's shape carries over.** The services live in
    `crates/core/src/bios.rs`, the state in the data area where the
    reference puts it, the `INT` is the 8086's, and the ROM's entry is
    the vector's target. The keyboard's two functions are the same shape
    of thing: the `INT` is the chip's `INT`, and the service answers
    where the ROM is.
  - **Do not forget the rule L2 states**: a hook that changes what an
    instruction does is decided by the oracle, not by the brief. The
    646 000-capture gate is the judge, and it is the one that caught the
    first attempt.

## Log

- **L4 (2026-11-25).** The session ends here, on the maintainer's word:
  "deja de construir por hoy".
  - **The decision.** The next unit is **the keyboard of the BIOS**,
    `INT 16h` — sixty of the eighty-five reference programs call it, and
    without it those programs wait for a key that never comes. It is
    written as **T2** above, with what the reference documents, what its
    own map does **not** document, and the two decisions the next
    session has to make before it writes the specs.
  - **Where the work stands.** Four commits on
    `feat/tui-memory-window`: `dbc45f4` (the BIOS and the `INT` that is
    the chip's), `be6d568` (the scroll moves the cells), `a894cb3` (the
    guest's geometry is bounded by the hardware) and `2562881` (this
    log). **781 passed, 0 failed**; the clippy run is clean in every
    crate; `cargo fmt --all --check` exits 0; **the full gate over all
    646 000 captures passes with no disagreement**, and the examples'
    gate is green with the recorded states recorded again. Both remotes
    carry the branch.
  - **The native review of this unit is closed and its authority burned.**
    Three lenses, a refuter and a targeted validator; one CRITICAL found
    and fixed; the acknowledgement was the exact one the bound STATUS
    named. The candidate that came after it — the bounded correction — is
    **medium** with the reason `under_budget`, so it waits for a
    high-risk commit or the maintainer's word, and it is not a reason to
    re-review the one that was approved.
  - **A note on the tooling, for whoever runs the next review here.** The
    OpenCode host relay for the reviewer tasks refuses at the `start`
    operation with the typed frame `provider_failed` **before it asks for
    the prompt**, and the reviewer tasks run behind the refusal anyway.
    The route that works on this host: submit the lens results with
    `gentle-ai review capture-result --input`, and run the refuter and
    the targeted validator with the `pi` runtime, which is the only one
    with a role capture contract. The occurrence is reported on Gentle
    AI's tracker.

- **L1 (2026-11-24).** The maintainer chose the cut: the video BIOS.
  - **The choice.** "El BIOS de video: INT 10h", and the surface: the
    thirteen text functions of the reference's own documentation, and the
    screen panel shows the cursor and the active page.
  - **The hook.** The service is run where the vector is taken, before the
    machine vectors: nothing is pushed, the instruction pointer has already
    moved past the call, and the interrupt is never reported as taken — so the
    run does not end there, and nothing else in the tree has to know that the
    BIOS exists.
  - **The state is the reference's own map.** The BIOS keeps no state of its
    own; the data area at `0040:0000` holds it, the guest can read it, and the
    panel reads it through the same door.
- **L3 (2026-11-25).** The unit was verified twice, and both rounds found
  something.
  - **The native review** ran three lenses, a refuter and a targeted
    validator over the committed candidate, and it found **one CRITICAL**: the
    window scroll did not move the window's cells. The read-back buffer holds
    every cell of the window, but the write-back took only the **first** cell
    of the source row and wrote it to **every** column of the destination row,
    so scrolling a window of text replaced each row with one repeated
    character. The suite could not see it: the scroll cases seeded and
    asserted only the first cell of each row. It is fixed in `be6d568`, and
    the case now gives every cell of the window a character of its own.
  - **The refuter's two verdicts, and why one of them is the interesting
    one.** The refuter **corroborated** the scroll and **refuted** a second
    claimed CRITICAL — that the screen widget chunks the guest's geometry and
    panics on a zero chunk size. The refuter's proof was the line itself: the
    widget computes `min(area.width, view.columns)` and walks the cells with a
    `get`-guarded lookup, and the only `chunks()` calls in the tree take
    constant sizes. **A lens had reported the opposite, and the parent read
    the code before accepting the verdict.** One of three refuters confirmed
    the lens's claim and two refuted it; the two were right.
  - **The project's own verifier** checked S1-S11 against the specs and
    against the silicon: **all eleven met**, and the four `CD 10` captures of
    the 646 000 agree with the chip on the stack, the flags, the landing place
    and the memory the chip left behind. It found **three more defects, all
    one root cause**: the BIOS's data area is ordinary guest-writable memory,
    and the geometry read out of it was computed with unchecked. A guest
    writing `0FFFFh` into the columns word panicked the emulator on a `u16`
    multiply, a guest writing `0100h` panicked it on the teletype's `u8`
    increment, and the panel sized its cell vector from the guest's word — up
    to 16 776 960 cells, about 134 MB, on **every frame**.
  - **The correction** is one door. The hardware's text modes are forty
    columns and eighty columns, both twenty-five rows tall, so `Cpu::columns()`
    and `Cpu::rows()` bound the count at the hardware's own modes, and every
    consumer reads the geometry through them. The data area keeps the guest's
    words and the guest reads its own value back — the BIOS simply refuses to
    compute with a value the hardware cannot hold. The panel needed no change:
    with the geometry bounded, its arithmetic cannot overflow.
  - **Two recorded states moved, and why.** `simplest` and `writebin` are
    loaded at `0100h` and run past `0400h`, so the program's own code overlaps
    the BIOS data area and the "geometry" the BIOS read was **code bytes**
    (`simplest`'s columns word was `B9 00`, 185; `writebin`'s was `CD 21h`,
    8653). Before the bound the BIOS computed with those numbers; after it,
    it computes with 80x25. Any rule that bounds the geometry moves those two
    states, and they were recorded again — which is what `S9` names.
  - **The verifier's probes are in the tree** (`crates/core/tests/bios_verifier.rs`,
    `crates/debug/tests/bios_verifier.rs`, `crates/core/tests/bios_oracle.rs`,
    `crates/tui/tests/bios_verifier.rs`). They were written from the specs and
    not from the implementer's tests, and they are the only evidence in this
    tree that was written by somebody else.
  - **Still open.** A guest that overwrites the ROM's own byte at `F400:0190`
    gets the native service **and** its byte executed; the chip would run only
    the guest's byte. `AH = 13h`'s write mode with interleaved attributes is
    implemented and now covered, and `AH = 05h` and `AH = 02h` clamp a page
    number above seven to the last page where the reference says 0..7. The
    graphics modes and the pixel functions are their own unit.
- **L2 (2026-11-24).** The silicon gate caught the hook, and the hook was
  wrong.
  - **What the first attempt did.** The BIOS's service ran where the vector
    was taken, before the machine vectored: nothing was pushed, the
    instruction pointer had already moved past the call, and the interrupt
    was never reported as taken. It was simple, it needed no change in the
    runner, and it was **not the 8086's `INT`**.
  - **What the silicon says.** Two of the 646 000 captures are `CD 10`. The
    chip pushes three words — the stack ends six bytes lower — and loads
    `CS:IP` from the vector table: in the first capture the machine leaves
    `4360:9171` for `753C:5C91`, and in the second `A671:D7F6` for
    `6AD6:FBF4`. The attempt left the stack where it was and the machine
    where it was, and it cleared neither TF nor IF. The gate reported the
    stack first and the rest of it was behind.
  - **The rule that settles it.** The chip's rule is the project's rule
    ("silicon wins"), and a family in the gate says "the chip's rule behind
    it". A family that said *this* `INT` does not push the frame would be a
    false claim about the 8086, and the three families the full set found
    were each closed by fixing the emulator, not by pinning them.
  - **The correction.** The BIOS is a handler at an address and the machine's
    vector table points at it (`S10`), so the `INT` is the chip's `INT`
    (`S1`), and the service returns with the `IRET` a handler returns with
    (`S11`). The captures then agree: the capture's own memory puts its own
    entry in the vector table, so the machine vectors there and runs whatever
    is there, the way the bare chip does — and the gate is back to **no
    disagreement at all**, with no new family.
  - **The lesson.** The brief named the hook, and the brief was wrong: it
    chose the shortest path and never asked the oracle. A hook that changes
    what an instruction does is not a design decision the parent makes
    alone; the oracle decides it, and the oracle is one command away.
