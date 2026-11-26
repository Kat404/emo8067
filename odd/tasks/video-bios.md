# The video BIOS — `INT 10h` — and the keyboard of the BIOS — `INT 16h`

Objective: the BIOS's services, in the core, so a guest that asks the BIOS
for something gets it: the video services `INT 10h` (T1) and the keyboard's
`INT 16h` (T2). Branch: `feat/tui-memory-window` (T1), `feat/bios-keyboard`
(T2, cut from `main`). Delivery: one commit and one independent verifier per
unit. Test runner: `cargo nextest run --workspace`.

**Risk: item 3** — the core's interrupt path is shared code that the runner,
the CLI, the programs gate, the TUI and every program consume, and this unit
changes what the machine does when a program calls `INT 10h`. T2 carries the
same risk and one more of the same kind: it gives the core a door the host
writes into, so the `Cpu` and the BIOS's data area become a contract that the
TUI and the CLI consume.

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
  the columns and the rows into the data area. A mode the machine does not
  have — `07h` and every other — leaves the geometry as it was.
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
- **S12** — The `INT 16h` is the 8086's `INT`, exactly as the `INT 10h` is
  (S1). It pushes FLAGS, CS and IP, it clears TF and IF, and it loads
  `CS:IP` from the interrupt vector table. The BIOS is a handler at an
  address: the machine installs the vector for `16h` when it starts, and the
  entry is the one the reference's own map gives — "16 16x4 = 58 F400:01C0 -
  keyboard functions" (`memory.txt:66`) — the mirror of the video entry
  `F400:0190` (`memory.txt:60`). The ROM is this project's own code, and
  the byte at the entry is the `IRET` that comes out of the service, the way
  `F400:0190` holds the video's.
- **S13** — The two services are the reference's own two, with the
  reference's own register contracts, quoted verbatim from
  `re/docs-txt/8086_bios_and_dos_interrupts.txt:304`-`:315`:
  - `AH = 00h` — "get keystroke from keyboard (no echo)." "return: AH = BIOS
    scan code. AL = ASCII character. (if a keystroke is present, it is
    removed from the keyboard buffer)."
  - `AH = 01h` — "check for keystroke in the keyboard buffer." "return: ZF =
    1 if keystroke is not available. ZF = 0 if keystroke available. AH = BIOS
    scan code. AL = ASCII character. (if a keystroke is present, it is not
    removed from the keyboard buffer)."
  No other `AH` is answered: the reference documents no other `INT 16h`
  function, and the corpus calls no other.
- **S14** — The keyboard buffer is the BIOS data area, at the offsets the
  maintainer approved: the head at `0040:001A`, the tail at `0040:001C`,
  the start at `0040:001E` and the size at `0040:0030` — the range
  `0040:001A`-`:0031` and nothing outside it. The head and the tail hold
  the **absolute** offsets of the entry to read and of the entry to write,
  the way the IBM's own buffer holds them, so a guest that reads
  `0040:001A` sees where the buffer is. The size word holds the ring's
  size in bytes — `0012h`, eighteen — and the ring's end is derived from it
  (`start` + `size`), not stored: the four words the range holds are the
  head, the tail, the start and the size. The body is `0040:001E`-`:002F`:
  nine two-byte entries, the ASCII character and the scan code.
  **The reference's own map documents none of this**: `memory.txt` gives
  the area `0040:0000`-`:0084` seven entries, all of video (`memory.txt:85`,
  `:109`, `:115`, `:119`, `:123`, `:128`, `:132`). The **shape** is the IBM
  PC's — a head, a tail, a start and a size in the BIOS data area — and the
  offsets are this project's own, so the code says so where it is written.
  The BIOS keeps no state of its own, as in S3: the buffer is the machine's
  memory.
  - **One slot stays free.** The ring holds nine slots, so that "the head
    equals the tail" is unambiguously empty; the ninth slot is what tells
    full from empty, and **eight keystrokes fit**. A keystroke that does
    not fit is dropped. The reference detects the overflow and signals it
    audibly (`odd/research/features-and-ux.md:153`, SCR-7); this project has
    no bell.
  - **The data area is guest-writable memory**, and every value the BIOS
    reads from it is untrusted: the BIOS validates the head and the tail
    against the ring before it trusts them, and a guest that scribbles there
    sees an empty buffer, never a panic. The video BIOS learned this the
    hard way — the guest's geometry overflowed `u16` and killed the
    emulator — and the same rule holds here.
- **S15** — The scan code is the IBM PC's set 1. The corpus is the evidence
  inside the repository: `re/extracted/app/examples/snake.asm:37`-`:41`
  names them "bios key codes" — `left equ 4bh`, `right equ 4dh`, `up equ
  48h`, `down equ 50h` — and compares the returned `AH` against them (`:143`,
  `:204`-`:211`); it is the only program of the eighty-five that reads `AH`
  at all. The published table is the oracle for the rest: the set 1 table
  at `https://www.marjorie.de/ps2/scancode-set1.htm`, the one Wikipedia's
  "Scancode" article cites. **The letters are not contiguous in it** — the
  rows break at the bracket keys and at the `` ` `` and `\` keys — so `Q =
  10h`, `E = 12h`, `I = 17h`, `Z = 2Ch` and `M = 32h`; a table that counts
  from `A = 1Eh` is not set 1's, and it is wrong for every letter but the
  first. A character the shift made — `?`, `:`, `|`, the rest — has the code
  of the **key** it is on (`35h`, `27h`, `2Bh`), the way the IBM's own BIOS
  answers, because the shift itself lives in the `AH = 02h` service this
  unit does not have.
- **S16** — `AH = 00h` waits when the buffer is empty. The project's own
  research says so of the reference: "`INT 16h/00h` blocks for a key,
  `INT 16h/01h` polls the buffer" (`odd/research/features-and-ux.md:152`,
  SCR-6), and the IBM's BIOS does. **The reference's own `INT 16h` block
  says nothing about waiting**: the sentence "if there is no character in
  the keyboard buffer, the function waits until any key is pressed"
  (`8086_bios_and_dos_interrupts.txt:339`) belongs to the `INT 21h / AH=1`
  block, not to `INT 16h`. That is a correction to the brief this task was
  written from.
  - **The wait costs the machine one step at a time.** The service stays at
    the ROM's entry and the machine re-enters it on the next step, so the
    step budget still bounds the run and the TUI still gets its turn between
    steps. The wait never spins inside a single step. The 646 000-capture
    gate is the judge of the shape (L2).
- **S17** — `AH = 01h` never waits. With an empty buffer it sets ZF = 1 and
  returns; with a keystroke it sets ZF = 0 and returns the pair **without**
  removing the keystroke, and the next `AH = 00h` returns the same one. The
  three programs that poll rely on this: `keybrd.asm:28`-`:30`,
  `bintest.asm:68`, `snake.asm:133`.
- **S18** — Where the keys come from: the host injects them. There is no
  keyboard hardware in the emulator, and the core gains no channel, no thread
  and no queue for it.
  - **The TUI forwards the terminal's keys.** A key the TUI binds to a
    debugger command is the debugger's, and every other key goes to the
    guest's keyboard buffer. **When the guest's screen has the focus — the
    takeover the `x` key gives it — the keys reach the guest as well, all
    of them but the TUI's quit keys.** The takeover **adds** the guest's
    keys, it does not take the debugger's away: the design document says
    the debugger's own keys keep working while the takeover is open
    (`design/DESIGN.md:90`-`:99`) and the tree's own tests pin it. The
    arrows are the guest's most wanted key: `snake.asm` reads `48h`, `4Bh`,
    `4Dh` and `50h`.
  - **The headless runner takes the keys from a script** on the command line
    (`--keys`), and it feeds the script as the guest consumes it, so a
    script longer than the buffer works. With no script the runner has no
    keyboard at all, and the guest waits (S16).
- **S19** — The core's one door: a keystroke goes into the BIOS data area's
  buffer, and the BIOS reads it from there. The BIOS module owns the
  buffer's layout, so the door it offers is the pair of the scan code and
  the ASCII character, and what it reports back is how many keystrokes are
  waiting **and whether the keystroke it was given was taken**. The TUI and
  the CLI use that door and nothing else.
- **S20** — The recorded states move. The programs that ask the BIOS for a
  key now run past it, and the ones that wait for a key that never comes end
  with the step budget spent ("step budget exhausted"), which is not a
  failure. The recorded states are this project's own (S9), so they are
  recorded again.
- **S21** — The help and the docs move with the change: the CLI's `--keys`
  option is in the CLI's own help, and the TUI's help says which keys reach
  the guest.
- **S22** — The mode `13h` is a mode the machine has. `AH = 00h` with
  `AL = 13h` sets it, and the mode is the reference's own: "13h - graphical
  mode. 40x25. 256 colors. 320x200 pixels. 1 page."
  (`8086_bios_and_dos_interrupts.txt:76`). The data area's words keep their
  meaning and their shape: the columns become **40** and the rows **25**,
  the mode's own cell grid, whose cells are eight pixels wide and eight
  pixels tall — "13h = G 40x25 8x8 320x200 256/256k A000"
  (`https://pdos.csail.mit.edu/6.828/2008/readings/hardware/vgadoc/VGABIOS.TXT`)
  — and the active page becomes zero, because the mode has one page. Every
  other mode leaves the machine as it was (S5).
- **S23** — The framebuffer is the published mode `13h` one: "the screen is
  an array of 64000 contiguous bytes, arranged as 200 rows of 320 bytes,
  with one byte per pixel, located in memory at absolute address
  $A000:$0000" (Dr Bill Verts, *Notes on VGA 320x200 256-Color Mode*,
  `https://people.cs.umass.edu/~verts/cs32/vga_320.html`; the same in
  Abrash's *Graphics Programming Black Book*, ch. 27, and in
  `https://en.wikipedia.org/wiki/Mode_13h`). So the framebuffer is at
  `A000:0000`, a row is 320 bytes, there are 200 rows, and **the pixel at
  the column and the row is at the offset `row * 320 + column`** (Abrash's
  own formula). One byte carries one pixel's colour index.
- **S24** — `AH = 0Ch` writes a pixel, with the reference's own register
  contract: "INT 10h / AH = 0Ch - change color for a single pixel. input:
  AL = pixel color. CX = column. DX = row."
  (`8086_bios_and_dos_interrupts.txt:150`-`:154`). The byte goes at the
  framebuffer's offset (S23). **There is no page register**: the reference
  names none, and the mode has one page.
- **S25** — `AH = 0Dh` reads a pixel, with the reference's own contract:
  "INT 10h / AH = 0Dh - get color of a single pixel. input: CX = column.
  DX = row. output: AL = pixel color"
  (`8086_bios_and_dos_interrupts.txt:164`-`:169`). The byte comes from the
  same offset the write used.
- **S26** — The two functions answer only in the graphics mode: the IBM's
  own BIOS says the pixel functions are "not supported in text modes"
  (Ralf Brown's Interrupt List, `INT 10h/AH = 0Ch`), and in a text mode
  this BIOS's answer is nothing at all. A coordinate outside the framebuffer
  writes nothing and reads zero — the IBM ignores a pixel that is outside
  the screen, and this project does the same rather than wrapping the
  address into the page.
- **S27** — The machine's mode is one byte in the data area, at
  `0040:0049`: "40:49 byte current CRT mode (hex value)"
  (`https://pic.hallikainen.org/techref/bios/dataarea.htm`; the same in
  Ralf Brown's Interrupt List's data-area map and in `VGABIOS.TXT`'s "0:449h
  BYTE Video current mode"). **The reference's own map documents none of
  it** — `memory.txt:115`-`:134` gives `004A`, `004E`, `0050`, `0062` and
  `0084` — so the word is this project's assumption, the way the cursor's
  shape at `0040:0080` already is (S3). It is one byte, not a word; and
  `0040:0063` is **not** it: that is the 6845's base I/O port.
- **S28** — The panel shows the graphics page. In the graphics mode the
  panel reads the framebuffer from `A000:0000` and draws it **scaled**: the
  framebuffer's 320×200 do not fit a terminal's cells, so each cell of the
  panel's area is a block of the framebuffer's pixels and the cell's colour
  is the **average** of the block's colours. The palette is the VGA's own
  DAC: the sixteen the project already has are the published table's first
  sixteen exactly (the EGFF table's sixteen EGA colours, `00`/`55`/`aa`/`ff`
  steps), and the DAC's own default palette is "the first 16 locations"
  (the EGA text colours), then "16 evenly spaced gray shades", then "the
  remaining 216 locations ... based on a hue-saturation-intensity model",
  then eight unused blacks (IBM's *VGA Technical Reference*, p. 4-39,
  quoted at `https://retrocomputing.stackexchange.com/questions/27994`; the
  table itself at `https://www.fileformat.info/mirror/egff/ch02_02.htm`).
  **The `{0, 51, 102, 153, 204, 255}` cube and the twenty-four greys of the
  step 10 are Windows 3.x's and xterm's, not the VGA's** (Netscape's
  technical note 10117, `http://www.bonniehill.net/pages/netscape.colors.html`;
  xterm's own cube is `{0, 95, 135, 175, 215, 255}`), and they are not the
  project's palette.
- **S29** — The recorded states move. The programs that use the graphics
  mode now run past it: `pixel.asm` reads its pixel back and gets the
  colour it wrote, and `0_sample_vga_graphics.asm` draws its rectangle.
  **`mouse2.asm` does not**: it takes `INT 33h` after seven instructions,
  long before its pixel code — **at the baseline too** — because the mouse
  device is out of scope, so that program never reaches the pixels it
  reads, XORs and writes. The runner's dump covers the whole megabyte
  (`crates/cli/src/main.rs`, `PAGES = (1 << 20) / PAGE`), so the
  golden carries the mode byte at `0040:0049` and the framebuffer's pixels
  as well — `0_sample_vga_graphics`'s rectangle is in the golden at `A1960`.
  **Seventy-nine of the eighty-five goldens move**, not only those two:
  the mode byte names the machine's mode from the moment the machine starts
  (`03h`, the eighty columns the machine boots with, S4) and after every
  `AH = 00h`, and the dump reaches that byte. **L10 says which of them
  differ in more, and which do not move at all.**

## Out of scope

- **The rest of the BIOS and the DOS.** Thirty-eight `AH` functions across
  eleven vectors is the reference's whole surface, and the research's verdict
  is to match the recovered set and stop.
- **The reference's ROM.** `INT_VECT` and `BIOS_ROM` are the reference's own
  code, local gold that is never committed, and no code of the reference is
  ever copied.
- **The keyboard's other functions.** The reference documents two `INT 16h`
  functions and the corpus calls two (S13); the IBM's `AH = 02h` (the shift
  flags), `AH = 05h` (storing a keystroke) and the enhanced-keyboard
  functions are not in the reference's surface, so they are not in ours.
- **The mode read-back.** The IBM's `INT 10h/AH = 0Fh` returns the display
  mode in `AL`, but the reference does not document it and the corpus does
  not call it, so it is not in the reference's surface and it is not in ours.
- **The XOR clause.** The IBM's pixel functions XOR the colour onto the
  screen when bit 7 is set — "except in 256-color modes", which is where we
  are. The reference's own contract says nothing about it, and `mouse2.asm`
  does its own XOR by hand, so the clause is not in ours.

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
  2026-11-25; DONE AND REVIEWED** — the four decisions are the
  maintainer's, taken the same day through the question tool, and the specs
  S12-S21 above are written from them. The decisions, verbatim:
  - "Área de datos del BIOS (0040:001A-0031)" — the buffer lives in the BIOS
    data area, at the IBM PC's conventional offsets.
  - "Inyección del host: TUI + guion del CLI" — the keystrokes come from
    the host: the TUI forwards the terminal's keys, and the headless runner
    takes a script.
  - "Espera hasta que haya tecla" — `INT 16h/00h` waits when the buffer is
    empty.
  - "Set 1 del IBM" — the scan code table is the IBM's set 1.
  - **The unit.** The `INT 16h` is the 8086's `INT` (S12): it pushes FLAGS,
    CS and IP, it clears TF and IF, and it loads `CS:IP` from the vector
    table. The BIOS is a handler at `F400:01C0` — the entry the reference's
    own map gives for the keyboard (`memory.txt:66`), the mirror of the
    video's `F400:0190` — the ROM is this project's own, and the byte at the
    entry is the `IRET` that comes out of the service. The two services are
    the reference's own two, with its own register contracts (S13). The
    buffer is the BIOS data area at the IBM's conventional offsets (S14),
    one slot kept free so that "the head equals the tail" is unambiguously
    empty, and the head and the tail validated before they are trusted,
    because the data area is guest-writable memory. The scan code is the
    IBM's set 1 (S15), whose four arrow codes the corpus itself names.
    `AH = 00h` waits, one step at a time, so the step budget still bounds
    the run (S16); `AH = 01h` never waits and never removes the keystroke
    (S17). The keys come from the host — the TUI forwards the terminal's
    keys, the runner takes a script (S18) — through the one door the BIOS
    module offers (S19). The recorded states move (S20) and the help moves
    with the change (S21).
  - **The files** are `crates/core/src/bios.rs`, `crates/core/src/lib.rs`,
    `crates/core/src/exec.rs`, `crates/core/tests/bios.rs`,
    `crates/cli/src/main.rs`, `crates/tui/src/app.rs`, `crates/tui/src/ui.rs`
    and the docs that carry the help.
  - **Do not forget the rule L2 states**: a hook that changes what an
    instruction does is decided by the oracle, not by the brief. The
    646 000-capture gate is the judge, and it is the one that caught the
    first attempt.

- **T3** — The graphics modes of the video BIOS: `13h`, `0Ch`, `0Dh`. Route:
  delegated, one bounded writer, **two work-unit commits**, one independent
  verifier. **CHOSEN 2026-11-25.** The maintainer's decisions, verbatim:
  - "Una unidad: el BIOS y el panel" — the unit is the BIOS's functions and
    the panel together, not two units.
  - "Bloques escalados, con el color promedio" — the panel draws the
    framebuffer scaled, each cell a block of pixels coloured with the
    average of the block's colours.
  - "Abre el PR del teclado; fusiono yo; gráficos desde main" — the
    keyboard's pull request is opened and merged first, and this unit takes
    a fresh branch from `main`.
  - **The unit.** The mode `13h` is a mode the machine has (S22): the
    columns and the rows become the mode's own cell grid, the active page
    becomes zero, and the mode itself is one byte in the data area at
    `0040:0049` (S27). The framebuffer is the published mode `13h` one
    (S23): 320 pixels of one byte each at `A000:0000`, a row of 320 bytes,
    the pixel's offset `row * 320 + column`. The two pixel functions are
    the reference's own, with the reference's own register contracts (S24,
    S25), and they answer only in the graphics mode (S26). The panel draws
    the graphics page scaled, with the VGA's own 256-colour DAC (S28).
  - **The files** are `crates/core/src/bios.rs`, `crates/core/src/lib.rs`,
    `crates/core/tests/bios.rs`, `crates/debug/src/panels.rs`,
    `crates/debug/tests/screen.rs`, `crates/tui/src/ui.rs`,
    `crates/tui/src/theme.rs`, `crates/tui/tests/screen.rs` and
    `design/DESIGN.md`.
  - **The recorded states are not in that list, and they are not in the
    commit.** They live in `re/extracted/app/goldens/`, they are gitignored
    local gold (`.gitignore:8`), and the gate writes them when the file is
    absent — so the unit deleted the ones that moved and let the gate
    record them again. A reader who must reproduce the gate needs the
    corpus's own programs and the gate, not a file in the tree.
  - **The two work units.** **W1** is the BIOS: the mode, the mode byte, the
    two pixel functions, the framebuffer's bounds and the core's tests.
    **W2** is the panel: the palette, the framebuffer's reading, the scaled
    drawing and the TUI's tests. The palette belongs to W2: the BIOS writes
    a colour's index and never sees a colour.

## Log

- **L11 (2026-11-25).** The review's advisories, the independent
  verifier's two findings and the targeted validator's third follow-up are
  in. The unit is reviewed and approved, and this batch changes **how the
  code says and shapes things**, not what the machine does.
  - **The ten advisories.**
    1. **The scaled drawing truncated the framebuffer.** The block's size
       was the framebuffer's size over the area's size with a floor, so at
       the takeover's eighty columns by twenty-three rows the block was
       4x8 and the picture's last sixteen rows were never drawn. **The
       blocks tile the framebuffer now**: each cell owns the share of it
       the area's size gives it, and the whole picture is drawn. **The
       uniform ceiling the advisory asked for would have broken the
       verifier's own probe** — it pins the first cell's average at
       thirty-two pixels a cell — so the goal is reached the way the probe
       allows, and the doc comment says what the code does.
    2. **The graphics view published a geometry it did not have.** The
       view's cells are as many as its geometry claims in every mode, so a
       consumer that maps a cell to `row * columns + column` finds one; a
       graphics page has no characters, so its cells are blank.
    3. **The palette's prose was wrong.** The greys the table ships are the
       VGA's own ladder (`00`, `14`, `20`, `2c`, `38`, `45`, `51`, `61`,
       `71`, `82`, `92`, `a2`, `b6`, `cb`, `e3`, `ff`), not evenly spaced
       ones. The table is unchanged; the prose says what the table has.
    4. **The mode's offset and the mode's value differ by a suffix.** The
       offset is `MODE_OFFSET` now, and the machine's three modes are
       named: `MODE_TEXT_40`, `MODE_TEXT_80` and `MODE_GRAPHICS`.
    5. **The test helper mirrors the door.** A full ring refuses the
       keystroke and leaves the tail where it was; the helper used to wrap
       the tail back and leave the buffer reading as empty where the
       machine keeps everything it accepted.
    6. **The CLI's warning names the stage.** The count is taken at the
       **enqueue**: the message says the keys "never entered the guest's
       keyboard", and not that they never reached the guest — in the test's
       own run the guest read none of the ten.
    7. **The CLI's test derives its count** from the keyboard buffer's
       published capacity, and the capacity is asserted in the core's own
       test, so the two cannot drift.
    8. **The function-key guard admits zero no more.** The keys are
       numbered from one, so `F(0)` is not a keystroke: it would reach the
       guest as a NUL.
    9. **The help's header is left as it is, and here is why.** The line
       may be 39 characters at most, and the promise's exception — the keys
       the BIOS's scan-code set has no code for, the lock keys, print
       screen, the media keys — does not fit in what is left of the line
       once the promise itself is said; and the test in
       `crates/tui/tests/drawing.rs` pins the phrase "the rest go to the
       guest" in the header, so the line cannot be reworded at all
       without breaking a test this batch may not edit. **The help's
       promise is true of every key the set has a code for, and the keys it
       has none for reach neither the guest nor any indicator.**
    10. **The palette's type says what it is.** The DAC is a table of
        three channels, and `theme::pixel` hands the three channels over:
        the drawing's dead arm — every entry of the DAC is an `Rgb` — is
        gone.
  - **The measurement the two lenses asked for.** The panel materialises
    the whole 64 000-byte framebuffer on every rebuild, and the TUI walks
    it again per cell; the text path reads 2 000 bytes. The numbers, on
    this machine, in a release build (the median of three runs):
    - **The panel's build**: the text page **5.7 µs**, the graphics page
      **11.2 µs** — twice the time, for sixteen times the bytes read
      (4 000 against 64 000). The graphics path reads the framebuffer in
      one tight loop and copies it once; the text path builds two thousand
      cells, and that is what it pays for.
    - **The takeover's whole drawing** (80x24): the text page **170 µs**,
      the graphics page **207 µs** — the graphics drawing walks 58 880
      pixels, and the framebuffer's copy is the rest of the difference.
    - **The CLI's run over the corpus with `--golden`**: **3.10 s** at the
      base commit's binary, **3.00 s** now — no measurable difference, and
      there is none to have: the runner never builds a panel and never
      reads the framebuffer.
    No benchmark harness went into the tree: the numbers come from a
    scratch program under `/tmp/opencode`, and it is not part of the
    repository.
  - **The verifier's two findings and the validator's follow-up.**
    - **L10's differential claim was inaccurate** and is corrected above:
      seventy-nine of the eighty-five recorded states move, seventy-four
      of them in the `00440` line alone, five in more — `paramexe`, `z01`
      and `z02` among them, because they execute memory as code — and six
      do not move at all.
    - **S29's `mouse2` clause was unmet and cannot be met**: the program
      takes `INT 33h` after seven instructions, at the baseline too, so it
      never reaches the pixels it reads, XORs and writes. S29 says so now.
    - **`odd/HANDOFF.md` repeated the claims** and is brought in line.
  - **The verifier's own probes are in the tree**, copied verbatim:
    `crates/core/tests/verify_graphics.rs` (thirteen) and
    `crates/tui/tests/verify_graphics.rs` (seven). They are the unit's
    regression tests, and they lean on the published sources and not on the
    implementation's own constants.
  - **RED**: the helper's test — "the ninth keystroke was refused: the
    buffer is not empty: left 0, right 8"; the view's shape — "the
    graphics page's cells are the grid's: left 0, right 2000"; the
    drawing — "the picture's last row is on the screen: left Rgb(0, 0, 0),
    right Rgb(0, 0, 0)"; the function keys — "F0 is not a key: left
    Some((0, 0)), right None"; and the CLI's warning, whose new wording
    the old message does not match.
  - **GREEN**: **862 passed, 0 failed** (the tree's 836, the twenty probes
    and the six new tests), clippy clean in every crate, `cargo fmt --all
    --check` exits 0, and the full gate over all 646 000 captures passes
    with no family added. **No golden moved**: the batch changes what the
    panel says and how it is shaped, not what the machine does.

- **L10 (2026-11-25).** The graphics modes are in: W1 is the BIOS and W2
  the panel, and the work corrected three of the parent's own sentences.
  - **W1, the BIOS** (`d677321`): the mode `13h` is a mode the machine has
    (S22), the mode is one byte at `0040:0049` (S27), the framebuffer is
    the published one (S23), and the two pixel functions are the
    reference's own (S24, S25) and answer only in the graphics mode (S26).
    The BIOS keeps no state of its own: the mode is read out of the data
    area every time, the way the geometry is.
  - **W2, the panel** (`e3f8a2a`): the panel reads the framebuffer and
    draws it scaled, each cell a block of pixels coloured with the average
    of the block's colours (S28), and the palette is the VGA's own DAC,
    transcribed from the published table — with the project's sixteen now
    *defined* as that table's first sixteen, so the two cannot drift.
  - **RED**: six failures in the core — "the mode's cell grid is forty
    columns: left 80, right 40", "the pixel is at the offset row * 320 +
    column: left 0, right 42", "the colour is at the pixel the column and
    the row name", "the read returns the colour that was written", "a text
    mode writes no pixel", "the mode is one byte, at 0040:0049: left 0,
    right 19" — and, in the panel, tests that did not compile ("no field
    'graphics' on type '&ScreenView'", "cannot find value 'VGA_DAC' in
    module 'theme'").
  - **GREEN**: **836 passed, 0 failed**; clippy clean in every crate;
    `cargo fmt --all --check` exits 0; the full gate over all 646 000
    captures passes with no family added. The parent's spot check: the same
    836. A differential against the base commit's binary over all
    eighty-five examples: **seventy-nine of the recorded states move, and
    the first version of this sentence said otherwise — it said that every
    text-mode program differs in exactly one byte and nothing else, and
    that is not what the measurement says.** The measured truth:
    - **Seventy-four differ in the `00440` line alone** — the mode byte,
      `00` → `03` — and in nothing else.
    - **Five differ in more**: `0_sample_vga_graphics` (the mode byte and
      the rectangle's pixels at `A1960`), `pixel` (the mode byte, `AX 0000`
      → `AX 000F`, and the pixels), and `paramexe`, `z01` and `z02` —
      DOS programs that fall into the interrupt vector table and **execute
      memory as code**, so the mode byte's change alters the byte they
      execute and their recorded behaviour changes with it.
    - **Six do not move at all**: `Calculator`, `float`, `mouse`,
      `simplest`, `sort` and `writebin` — they set a text mode of their
      own or never reach the data area, so the byte they leave there is
      the byte they left there before.
  - **Three corrections the work itself made**, to the parent's own
    sentences: **S5** still said "`13h` and every other" leaves the
    geometry as it was — `13h` is a mode now, so the sentence names `07h`;
    **S29** said the framebuffer is not in the recorded states and that the
    runner's dump reaches the low 32 kilobytes — the dump covers the whole
    megabyte (`PAGES = (1 << 20) / PAGE`), so the graphics page **is** in
    the golden (`0_sample_vga_graphics`'s rectangle is at `A1960`); and the
    out-of-scope section's entry about the graphics page in the recorded
    states is gone for the same reason.
  - **Where the recorded states live.** `re/extracted/app/goldens/`, one
    file per program, **gitignored** (`.gitignore:8`) and never committed:
    they are this project's own local gold, the gate writes them when the
    file is absent, and the unit deleted the ones that moved and let the
    gate record them again. The gate reports "85 examples, 85 matched, 0
    changed" afterwards, and the goldens carry the mode byte and the
    framebuffer's pixels ✓ — `pixel.asm`'s `AX` is `000F`, the colour it
    wrote and read back, and `0_sample_vga_graphics`'s rectangle is at
    `A1960`.
  - **Two choices the implementer flagged and the parent took.** The mode
    byte is written at boot with `03h` — the mode the machine's eighty
    columns are — because the data area holds the machine's state from the
    moment it starts (S4); **seventy-nine of the eighty-five goldens
    move** for that reason, not only the three graphics programs, and the
    byte has to be written for the text modes too, or a guest that leaves
    `13h` would still get pixel answers (S26). And in a text mode the
    pixel read **does not answer at all** (`AL` is left as it was), while a
    read outside the framebuffer in the graphics mode answers zero — the
    distinction S26 draws.

- **L9 (2026-11-25).** The next unit is the graphics modes, and its three
  decisions are the maintainer's.
  - **The decisions**, the maintainer's own words: "Una unidad: el BIOS y el
    panel"; "Bloques escalados, con el color promedio"; "Abre el PR del
    teclado; fusiono yo; gráficos desde main".
  - **The unit's own research, and what it corrected.** The census of the
    corpus: **three** programs use the graphics mode —
    `0_sample_vga_graphics.asm` (four `0Ch`), `pixel.asm` (a `0Ch` and a
    `0Dh`) and `mouse2.asm` (two of each) — and all three set the mode
    through the BIOS, and none writes the graphics page directly. **The
    list L8 carried was wrong**: `attrib.asm` is a DOS file-attribute
    program, `scroll.asm` a text-mode window scroll, and `pages.asm`,
    `colors.asm` and `mouse.asm` are text mode. L8 and the research's own
    SCR-4 are corrected.
  - **Three corrections to the brief, from the published sources.**
    - **The framebuffer is one byte per pixel**, not "a row of 320 pixels in
      eight-pixel bytes": "an array of 64000 contiguous bytes, arranged as
      200 rows of 320 bytes, with one byte per pixel, located in memory at
      absolute address $A000:$0000" (Dr Bill Verts,
      `https://people.cs.umass.edu/~verts/cs32/vga_320.html`; the same in
      Abrash's *Graphics Programming Black Book* ch. 27 and in
      `https://en.wikipedia.org/wiki/Mode_13h`).
    - **The palette is the VGA's own DAC, not the cube of Windows 3.x.** The
      DAC's default is "the first 16 locations" (the EGA text colours),
      then "16 evenly spaced gray shades", then "the remaining 216
      locations ... based on a hue-saturation-intensity model", then eight
      unused blacks (IBM's *VGA Technical Reference* p. 4-39, quoted at
      `https://retrocomputing.stackexchange.com/questions/27994`; the table
      at `https://www.fileformat.info/mirror/egff/ch02_02.htm`). The
      `{0, 51, 102, 153, 204, 255}` cube and the greys of the step 10 are
      Netscape's and xterm's — xterm's own cube is `{0, 95, 135, 175, 215,
      255}` — and **the project's existing sixteen are the published DAC's
      first sixteen exactly**, so the palette has one home and one rule.
    - **The mode's word is `0040:0049`, one byte**: "40:49 byte current CRT
      mode (hex value)" (`https://pic.hallikainen.org/techref/bios/dataarea.htm`;
      the same in Ralf Brown's data-area map and in `VGABIOS.TXT`'s "0:449h
      BYTE Video current mode"). **`0040:0063` is not it**: that is the
      6845's base I/O port. The reference's own map documents none of it, so
      it is the project's assumption (S27), the way the cursor's shape at
      `0040:0080` already is.
  - **The reference's own ROM stays unobserved.** Its identity (sha256
    `8ff8497d11497710`) and its load address (`F4000`, `memory.txt:47-50`)
    are recorded; no Ghidra project was ever made for it, so the published
    tables are this unit's oracle (`odd/research/softonic-build-comparison.md:85`,
    `:403-405`).
  - **What the unit does not do** (the out-of-scope section, rewritten):
    the IBM's `AH = 0Fh` mode read-back, which the reference does not
    document; and the IBM's XOR clause, which the reference does not mention
    and which the mode's own 256 colours would not apply to anyway.

- **L8 (2026-11-25).** The next unit is chosen and the construction
  pauses.
  - **The decision**, the maintainer's own words: "Los modos gráficos del
    BIOS (13h, 0Ch/0Dh) pero solo guarda la respuesta en memoria y en
    odd/, seguiremos la construcción en otro momento". So: the unit is
    recorded here and in the handoff, and the construction stops for now.
  - **What the unit is.** The video BIOS's other half: the mode `13h`
    (forty by twenty-five in 256 colours with a 320×200 framebuffer), the
    page of graphics at `A0000:B1000` (`memory.txt:24`-`:27`), the
    pixel's address, and the pixel functions `0Ch` (write a pixel) and
    `0Dh` (read a pixel). **Three** of the corpus's programs use it:
    `0_sample_vga_graphics.asm` (four `0Ch`), `pixel.asm` (a `0Ch` and a
    `0Dh`) and `mouse2.asm` (two of each). All three set the mode
    through the BIOS, and none writes the graphics page directly.
  - **What it will have to settle.** The screen panel is a grid of
    characters, and a 320×200 framebuffer is not that: either the panel
    draws pixels or the unit keeps to the BIOS's functions and the panel
    comes later. The mode `13h` also changes the machine's geometry, and
    the BIOS data area's own video state — the page's start, the rows —
    has one shape for text and another for graphics. The reference
    documents `0Ch` and `0Dh` in
    `re/docs-txt/8086_bios_and_dos_interrupts.txt`, and its map gives the
    page's home.
  - **Where the work stands.** Branch `feat/bios-keyboard`, pushed to
    GitHub and Codeberg at `4809944`: the unit (`47d2b4e`), the
    verification round's log (`b112196`), the native review's correction
    (`3c5741e`), the advisories' follow-up (`030756a`) and this log
    (`4809944`). **822 passed, 0 failed**; clippy clean; `cargo fmt
    --all --check` exits 0; the full gate over all 646 000 captures passes
    with no family added. The unit's own native review is closed and its
    authority burned; the follow-up commit's review is the one this
    session opened last.

- **L7 (2026-11-25).** The native review of the unit: four lenses, a
  refuter and a targeted validator, and the authority burned.
  - **What the review found.** The risk lens and the reliability lens
    reported the same CRITICAL: `keyboard_end` derived the ring's end with
    an unchecked `u16` addition of the size word, which is the guest's own,
    so a guest that wrote `0xFFE2` into `0040:0030` and called `INT 16h`
    killed the emulator with "attempt to add with overflow". **It is the
    sibling of the `SP + 4` this unit had already fixed**, and the parent
    had read the same class in the diff and missed this one. The refuter
    corroborated both claims, and the parent reproduced it:
    `cargo run -p emo8067-cli -- run --ds 0040 size.asm` panicked with
    exit 101.
  - **The correction** — one bounded correction, the budget's 200 lines:
    the size word's addition is checked now (`KEYBOARD_BUFFER.checked_add(size)?`),
    and a size that would leave the segment leaves the buffer empty, which
    is what S14 asks of every value the BIOS reads from that area. The probe
    `s14_a_guest_that_scribbles_the_size_word_sees_an_empty_buffer` pins it:
    RED (the overflow panic on the unfixed code) and GREEN (19/19). The
    commit is `3c5741e`, and the targeted validator passed both
    `original_criteria` and `correction_regression`.
  - **The review is closed and its authority burned**: the acknowledgement
    was the exact one the bound STATUS named, and the envelope says
    `authority: burned`.
  - **The advisories are not corrections.** The review's own words: "Every
    finding listed here is non-blocking... Treat them as separate later
    work, never as a reason to re-run review on this candidate."
    **What the follow-up commit took**: the key mapper's catch-all no
    longer turns an unnamed key into a NUL keystroke (three lenses
    reported it, and the code contradicted its own doc comment); the CLI
    says out loud the keys of a script that never reached the guest; the
    CLI's own test now passes the option its name claims; the ring's end
    comes from the validated ends once instead of being derived twice
    with an unreachable fallback; and the test helper wraps the tail the
    way the door does. **What is left, accepted**: the TUI's help names the
    keys in its header ("keys · the rest go to the guest"), because a line
    of the list may be 39 characters at most; the TUI still drops a
    keystroke silently when the guest's buffer is full, because this
    project has no bell; the probes re-declare the layout's offsets on
    purpose, so that they do not lean on the implementation's constants; and
    the quit keys are matched twice, which is a duplication and not a
    defect.
  - **The tooling, for whoever runs the next review here.** The OpenCode
    host relay refuses at the `start` operation with the typed frame
    `provider_failed`, **but the reviewer task runs anyway** and its result
    arrives as a background result. The prompt must be the provider's exact
    `GENTLE_AI_REVIEW_BINDING` line: a prompt that only points at a file is
    refused with `binding_mismatch` **before the session is created**, so
    the task never runs and the desktop card lies about it. The refuter's
    and the validator's results are captured with
    `gentle-ai review capture-refuter --input <file> --agent pi` (and
    `capture-validation`): `--execute --agent pi` fails because the role's
    agent is not installed for pi, `--execute --agent claude-code` fails
    because that CLI is not logged in, and `--input --agent opencode` is
    refused because opencode has no role capture contract.

- **L6 (2026-11-25).** The verification round: one blocker from the
  independent verifier, three more from the parent's own reading of the
  diff, and one verdict the parent does not accept.
  - **The verifier's blocker.** `set_zero_flag` adds four to the stack
    pointer without wrapping (`crates/core/src/bios.rs`, `let at =
    cpu.regs.word(Reg16::Sp) + 4;`). The `INT` pushes three words, so a
    guest whose `SP` starts at 2..5 puts the frame's flags word across the
    segment's end — and the addition panics, "attempt to add with
    overflow", killing the emulator. The verifier reproduced it with
    `--sp 3`; the baseline `5e4a702` stops with "took INT 0x16" and does
    not panic. **The fix is `wrapping_add(4)`**: the frame's flags word is
    at the wrapped address, which is where the push put it.
  - **The verifier's S15 verdict does not stand.** It ruled the scan-code
    table "met — every shipped entry is set 1", on a table it built itself:
    `b' = 1Fh`, `c = 20h`, `q = 2Eh` — the count from `A = 1Eh` that set 1
    is not. The published table (`https://www.marjorie.de/ps2/scancode-set1.htm`,
    the one Wikipedia's "Scancode" article cites) says `B = 30h`, `C =
    2Eh`, `Q = 10h`, `E = 12h`, `I = 17h`, `Z = 2Ch`, `M = 32h`, and the
    code's table is wrong for every letter but `a`. The parent's reading
    had found this before the verifier reported; the two agree, and the
    published table is the oracle.
  - **The parent's own three findings.**
    1. **The size word sat inside the ring.** S14 put the size at
       `0040:0030` and the body at `0040:001E`-`:003E`, so the tenth
       keystroke wrote over the size word, and `init` wrote it as a word,
       over two ring bytes at once. **S14 is rewritten**: the range is
       `0040:001A`-`:0031` and nothing outside it, the body is
       `0040:001E`-`:002F`, the size word holds `0012h`, and the ring's end
       is derived from it instead of stored.
    2. **The head and the tail were ring-relative.** They are absolute now,
       the IBM's way, so a guest that reads `0040:001A` sees where the
       buffer is.
    3. **The CLI's `\r` meant the right arrow**, and a literal newline gave
       the guest ASCII `0Ah` where the corpus's programs compare `AL`
       against `0Dh`. `\n` and `\r` are the enter key now, and the arrows
       are `\up`, `\down`, `\left` and `\right`.
  - **The verifier's advisories.** A (the size word, and the ring outside
    the approved range) is the parent's finding 1. B (the takeover's
    additive reading) is what **S18 now says**: the design document has
    the last word there, and it says the debugger's own keys keep working
    while the takeover is open (`design/DESIGN.md:90`-`:99`). C (the
    shifted punctuation carried no code) is now in **S15**: a shifted
    character has the code of the key it is on. D (the runner ate the whole
    script when the door refused a keystroke) is fixed by the door's new
    answer — it says whether the keystroke was taken, and the runner only
    advances its script when it was. E (the TUI's help does not say which
    keys) is **accepted**: the help's widest line may be 39 characters and
    the line it has says the keys go to the guest's screen. F (the CLI's
    help did not list every escape) is fixed with the new format.
  - **The correction round's evidence.** **RED**: the verifier's own
    probes, against the tree as it stood, ran 18 with **6 failed** — the
    panic of C1 ("attempt to add with overflow"), the table of C2 ("the set
    1 code of 'q' is 0x10, left: 46, right: 16"), and the three of C3. The
    blocker end to end: `cargo run -p emo8067-cli -- run --sp 3 poll.asm`
    panicked with exit 101. **GREEN**: the probes are 18/18 and 6/6, the
    whole tree is **821 passed, 0 failed** (the base's 781, the unit's 15,
    the probes' 24 and one new CLI test), the clippy run is clean in every
    crate, `cargo fmt --all --check` exits 0, and the full gate over all
    646 000 captures passes with no family added. The parent's own spot
    check: `--sp 3` now stops with the empty buffer's answer, `ZF` set and
    exit 0. The recorded states moved again — the layout changed the data
    area — so 79 of the 85 goldens moved, all 85 were recorded again, and
    the gate reports "85 examples, 85 matched, 0 changed"; the census is
    the same: 38 stop at `INT 21h`, 33 at the step budget, 9 halt, 2 at
    `INT 15h`, 2 at `INT 33h`, 1 at `INT 13h`.
  - **Two of the probe's expectations changed, and the parent's reason is
    recorded.** The verifier's `s14_the_size_word_is_inside_the_body_and_the_ring_clobbers_it`
    asserted that the size word IS inside the body — it was a probe of the
    defect, and under the corrected S14 it is a probe of its absence, so it
    is now `s14_the_size_word_is_outside_the_body_and_the_ring_never_clobbers_it`.
    Its `s12`/`s13`/`s19` expectations still read the head and the tail the
    ring-relative way, and the TUI probe's helpers added the ring's start to
    the head; those five are now absolute, the same correction the parent's
    own `s14` probes got. **A probe's expectation changes only when the spec
    it was written from changes, and the reason is in this log.**

- **L5 (2026-11-25).** The keyboard's four decisions are the maintainer's,
  and the specs S12-S21 are written from them.
  - **The decisions**, the maintainer's answers verbatim: "Área de datos del
    BIOS (0040:001A-0031)"; "Inyección del host: TUI + guion del CLI";
    "Espera hasta que haya tecla"; "Set 1 del IBM". They are S12-S21 above,
    and T2 above carries them.
  - **What the research changed in the brief.** Three findings:
    1. **The reference's own map DOES name the keyboard's entry**:
       `memory.txt:66` — "16 16x4 = 58 F400:01C0 - keyboard functions" — the
       mirror of the video's `F400:0190` (`memory.txt:60`). T2's brief said
       the map gave the keyboard nothing; that is true of the **buffer** and
       false of the **entry**. The buffer's offsets are still the IBM's
       convention and still this project's assumption (S14); the entry is
       the reference's own.
    2. **The wait is not in the reference's `INT 16h` contract.** The
       sentence "if there is no character in the keyboard buffer, the
       function waits until any key is pressed" (`:339`) is inside the
       `INT 21h / AH=1` block; the `INT 16h` section (`:304`-`:315`) says
       nothing about waiting. The project's own research already had it
       right — `odd/research/features-and-ux.md:152` (SCR-6): "`INT 16h/00h`
       blocks for a key, `INT 16h/01h` polls the buffer" — and so does the
       IBM's BIOS.
    3. **The corpus is the scan code's evidence**: `snake.asm:37`-`:41`
       calls `4Bh`, `4Dh`, `48h` and `50h` "bios key codes" and compares the
       returned `AH` against them; it is the only one of the eighty-five
       that reads `AH` at all. The census: **62** programs call `INT 16h`
       (the brief said sixty), 79 call sites, 76 of them `AH = 00h` and 3 of
       them `AH = 01h` — the three that poll first (`keybrd.asm`,
       `bintest.asm`, `snake.asm`).
  - **What the map found in the code.** The core has **no input surface at
    all** — no channel, no keyboard, no queue; the only host doors are the
    I/O port bus and the device socket. **23 of the 85 recorded states**
    already stop with "took INT 0x16". The TUI consumes every key itself
    (28 bound keys) and none of them ever reaches the `Cpu`. The headless
    runner has an instruction budget (`100_000` by default) and no input
    channel, and spending the budget is not a failure.
  - **Where the work stands.** Branch `feat/bios-keyboard`, cut from `main`
    at `5e4a702`; `main` and the merged feature branch have the same tree,
    and both remotes carry `main` at `5e4a702`.

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
    carry the branch, and the branch went into `main` through pull request
    #1 (merge commit `11422fb`); **the next unit takes a fresh branch from
    `main`**, and `main` and the branch have the same tree.
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
