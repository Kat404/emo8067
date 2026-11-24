# The TUI (T16)

Objective: the terminal frontend — the product's primary face — drawn from
the panel model `emo8067-debug` holds. Branch: `main`. Delivery: one commit
per slice, each leaving the tree green. Test runner: `cargo test --workspace`.

## Why this and not more

The research's decision is settled (`features-and-ux.md` §4.3): the TUI is
primary, because the reference's whole value surface is text, because
ratatui keeps the dependency budget honest where eframe pulls wgpu, and
because a terminal is the only frontend that works over SSH and in CI.

**The model is the product.** The rule the research settled is that every
panel is a pure function of the machine's state plus a UI state struct, and
that the GUI is a rendering of the same struct. So this frontend draws
`emo8067-debug`'s panels and holds nothing of its own.

The layout and the colours are the research's (`features-and-ux.md` §4.4):
cyan is the current `IP`, grey is what has run, yellow is a breakpoint and
red is a watchpoint.

## Specs

- **S1** — The TUI draws the panel model and nothing else. It holds no
  copy of the memory and no second opinion about the machine: the panels
  come from `Panels::of`, the breakpoints from `Breakpoints`, and the
  machine from the core.
- **S2** — The disassembly follows `CS:IP` and is never asked to
  disassemble a range; the cursor moves over the listing.
- **S3** — The keys are the reference's where the reference has them:
  `F5` runs to the next breakpoint, `F8` steps, `F9` sets a breakpoint at
  the cursor, and `?` is the help overlay.
- **S4** — The drawing is testable without a terminal: ratatui's
  `TestBackend` renders it and the test reads the buffer back.
- **S5** — The registers panel and the flags panel are drawn from the panel
  model, in the layout the research draws (`features-and-ux.md` §4.4): the
  registers in two columns — "AX/BX/CX/DX/SI/DI/BP/SP, segmentos, IP" — and
  the flags as their nine names with their values. The TUI holds no second
  opinion about the machine: every value comes from `Panels::of`.
- **S6** — The register under the cursor is edited in place: `e` opens it,
  the value is typed over it, and `Enter` writes it into the machine. The
  value is written in the reference's own expression language (`34h`,
  `101b`, `5`), which is the evaluator the breakpoint conditions already
  use.
- **S7** — A value that cannot be read is refused in the reference's own
  words (`bad syntax`, `bad expression`, …) and the register is left as it
  was. A value that does not fit in sixteen bits is refused too.
- **S8** — The panes have a focus: `Tab` moves it, `1` takes the
  disassembly and `2` the registers, and the arrows move the cursor in the
  pane that has it. The keys that were there before still do what they did.
- **S9** — The memory panel is a **window**, not a buffer: it holds the
  bytes around one address and nothing else, so there is no `Vec<u8>` of a
  megabyte anywhere in the UI. It comes from the model (`Panels::of`), like
  every other panel, and the TUI holds no copy of the memory. It looks
  through `DS`, the data segment, and its title carries the segment and
  the address the cursor is at. The bytes are drawn as hex with the
  printable side beside them when the panel is wide enough for both, and
  the row address is in the gutter, so a byte is still identifiable after
  it scrolls off.
- **S10** — The memory pane has the focus: `3` takes it, `Tab` cycles the
  three persistent panes, the arrows move the memory cursor a byte at a
  time and a row at a time, and `PageUp` and `PageDown` move it a page.
  The keys that were there before still do what they did.
- **S11** — The memory pane is **jumped to, not scrolled**: `g` opens the
  goto prompt, the address is typed in hexadecimal — the way the reference
  types a memory address (`0100`) — and `Enter` centres the window on it.
  `Esc` leaves the address as it was, and an address that cannot be read is
  refused in the reference's own words (`bad syntax`) and the window does
  not move.
- **S12** — The stack is a pane you open, not a fourth column: `4` opens
  it where the listing is, and `4` again closes it. The window shows the
  segment, the `SP` and the words around it, so a push and a pop are
  visible as a region and not as a number. The arrows move the window.
  Every key that was there before still does what it did.
- **S13** — The source strip is the context of the current instruction:
  the program's own lines, three to five of them, each with its number
  and its own text, and the line the machine is stopped at marked. It is
  drawn under the listing, and a program that has no source has no strip.
- **S14** — The source lives in the program: the assembler records where
  every source line landed, and the debug crate's source panel is a pure
  function of the program and the machine, the way every other panel is a
  pure function of the machine and the UI state.
- **S15** — The memory pane writes bytes: `e` opens the byte under the
  cursor, the value is typed in the reference's own expression language
  (`34h`, `101b`, `5`), and `Enter` writes it into the machine's memory at
  that address. A value that does not fit in a byte is refused with "value
  out of range", and the memory is left as it was.
- **S16** — The flags are edited too: the flags panel has a focus and its
  own cursor, `e` opens the flag under the cursor, and its value is `0` or
  `1`; `Enter` writes it into the flags word. A value that is not a
  boolean is refused with "value out of range", and the flag is left as it
  was.
- **S17** — The four persistent panes have the research's own numbers:
  `1` the disassembly, `2` the registers, `3` the memory, `4` the flags.
  The stack is a window and not a pane of the layout, so it opens with
  `5`.
- **S18** — The goto takes a label and a segment: the address is typed the
  way the reference types a memory reference — `SSSS:OOOO` sets both
  halves, a label sets the offset in the current segment, and a bare
  hexadecimal offset sets the offset. The labels are the program's own
  names. The memory pane looks through the segment the goto named, and
  through `DS` when no goto has named one. A text that cannot be read is
  refused in the reference's own words (`bad syntax`), and the window does
  not move.
- **S19** — The breakpoints are a panel too: the model lists every
  breakpoint and every watchpoint with the segment, the offset and the
  condition it carries, so a pane can show what the machine stops on and
  let a condition be edited. The condition is the reference's own
  expression, and the evaluator that reads it is the one the breakpoints
  already use.
- **S20** — The conditions are a pane: `b` opens it where the listing is,
  and it draws every breakpoint and every watchpoint with its segment, its
  offset and the condition it carries. The condition under the cursor is
  edited in place, in the reference's own expression language, and a
  condition that does not read is refused in the evaluator's own words and
  the old condition stays. The list is the model's own listing, and the
  evaluator that reads a condition is the one the breakpoints use.
- **S21** — The console is a window, like the stack and the conditions: `:`
  opens it where the listing is, `:` again closes it, and `Esc` closes it
  too. It has the command line at the bottom and the output above it, the
  way the reference's own debug window has a log and a box to type in.
  Every parameter the console takes is in hexadecimal and every value it
  prints is in hexadecimal: "All output values are in hex. All input
  parameters must be in hex. The h suffix is not required".
- **S22** — The commands are the reference's own words: `r` shows the
  registers and the instruction at `CS:IP`, `r <register>` reads or sets
  one register, `rf` reads or sets the flags, `t` steps one instruction,
  `d [address] [L count]` dumps memory, `i` and `iw` read a port, `o` and
  `ow` write a port, and `c` compares two byte ranges and prints only the
  addresses whose bytes differ. **The text the console prints is ours**:
  the reference documents the commands and not their output, and the
  registers and the flags are printed as the panels draw them, so the
  project grows one format and not two.
- **S23** — A command that cannot be read changes nothing: a command the
  console does not know, a parameter that is not hexadecimal and a value
  that does not fit are refused in the console's own words, and the
  machine, the breakpoints and the counters are left as they were.
- **S24** — The sections are not cards: the header is one row, the sections
  are separated by a thin rule — a horizontal one between the bands, a
  vertical one between the bottom band's sections — and **no panel is
  framed with a box**.
- **S25** — A state is never colour alone: the instruction the machine is
  at carries the `▸` glyph, a breakpoint carries `◆`, a watchpoint `◇`,
  the focused section's header is the accent, and the memory window's
  cursor is marked whether the section has the focus or not.
- **S26** — The palette is a set of roles and not a set of swatches: the
  canvas, the surface, the rule, the three text roles, the accent and the
  state colours, each with one meaning, and they live in
  `crates/tui/src/theme.rs`. The research's meanings are kept — cyan is
  the current `IP`, grey is what has run, yellow is a breakpoint, red is a
  watchpoint — and the values are the calm ones, not the terminal's ANSI
  ones.
- **S27** — The words stay: the redesign changes the frame and not the
  vocabulary. The sections keep their names ("disassembly", "registers",
  "flags"), the registers and the flags keep their `AX 0000` form, the
  memory row keeps its address and its bytes, and the status bar keeps
  its hints.
- **S28** — The screen has no space that says nothing: the listing asks
  the model for as many instructions as the band holds — the model reads
  a window of bytes and the listing shows what is in it — and the source
  strip takes only what the listing can spare. A tall screen is filled
  with instructions and not with emptiness.
- **S29** — One glyph, one meaning: `▸` means "here" and nothing else.
  The console's caret is the terminal's own — the inverted cell — and not
  a second arrow.
- **S30** — The help overlay is a section and not a box: it is drawn with
  the same rules as the rest of the screen, and the keys it lists are the
  screen's vocabulary.
- **S31** — The narrow terminal is a first-class size: at eighty columns
  every section fits its content, the memory window takes the width its
  bytes need and not one column more, and nothing is cut in a way that
  loses a fact.
- **S32** — The watchpoint is a key: `w` puts a watchpoint on the byte
  under the memory window's cursor, through the segment the window looks
  through, and `w` again takes it away. The watchpoint is the model's own
  — the one the conditions pane already draws — and the TUI holds no copy
  of it. It watches the writes: it fires when the machine writes that
  byte.
- **S33** — The step back is the reference's: `F6` rewinds the machine one
  instruction. The core keeps the journal — the registers, the flags and
  the bytes the instruction wrote — and `F6` puts them back, so the
  machine is where it was before that instruction and the listing's cursor
  goes back with it. A journal with nothing to rewind is refused in the
  reference's own words: "There are no recorded back steps ...."
- **S34** — The step over is the reference's: `F7` runs a procedure or an
  interrupt call to its end and stops where the call returns. An
  instruction that is not a call is stepped once, as `F8` does. The core
  keeps the depth of the calls — a call goes in and a return comes out —
  and the TUI asks it how deep the machine is. A step over that runs out
  of its budget says so, the way `F5` does. A step over stops at a
  breakpoint, the way `F5` does.
- **S35** — The console takes the reference's `p` too: it steps over, the
  way the TUI's `F7` does. The refusal it carried — "the machine keeps no
  call tracking" — is no longer true once the core keeps the depth of the
  calls, and the reference's own command is in its list: "p - step over
  (procedure or interrupt)".
- **S36** — The help loses no key: `w`, `F6` and `F7` are in the overlay,
  and every key the overlay already listed is still there at eighty
  columns. The list is laid out so it fits — the two columns the log
  already names as the remedy — and the words stay the screen's
  vocabulary.
- **S37** — The guest's screen is a panel of the model, the way every
  other panel is: it is the text page the machine wrote to, read through
  the segment the UI says, and the TUI holds no copy of it. The panel is
  a function of the machine's state and the UI's state, exactly like the
  disassembly and the memory window.
- **S38** — The page is the 8086's text page: eighty columns by
  twenty-five rows, two bytes a cell — the character and the attribute —
  read from `B800:0000` by default. A character that is not printable is
  drawn as a space and not as a control glyph, and the page is the
  machine's own memory: what the guest wrote to it is what it shows.
- **S39** — The screen is a takeover: `x` opens it and the guest's screen
  takes the screen, and `x` again closes it. The takeover takes every row
  but the status line: the machine's state is in the status bar and
  nowhere else, so it does not go under the takeover. The debugger's own
  keys keep working while it is open, the way the reference's
  `SCREEN_HOTKEYS` says they do.
- **S40** — The attribute is drawn, and not only the character: the
  foreground and the background of a cell are the ones its attribute byte
  says, and no state is colour alone. The sixteen colours of the
  attribute are the ones the byte carries, and the background's bright
  bit is the eighth background, not a shade of the seventh.
- **S41** — The help loses no key again: `x` is in the overlay, and every
  key the overlay already listed is still whole at eighty columns.
- **S42** — What does not fit is cut: the page is drawn from its top-left
  corner, and the part of it the terminal cannot hold is not shown. The
  page is eighty columns by twenty-five rows, and the terminal is not
  always that big.

## Out of scope

- **The video adapter and the BIOS**: the reference's screen is a device
  of its own, and this slice is the passive viewer of the page the guest
  wrote to. Nothing here is the adapter: the memory is plain RAM, the
  page is read like any other memory, and `INT 10h` is not here — a
  program that calls it still stops where the interrupt took it. The
  graphics modes are not here either: this is the text page and nothing
  else.
- **The commands the console cannot do yet**: the effective-address forms
  (`d bx+5`), and the symbols (`d lion L 10`) — this project has no symbol
  table for them.
- **The stop-on-condition window**: the evaluator and the breakpoints are
  there; the window that shows the conditions and lets them be edited is
  its own slice.
- **The GUI** (T18), which will draw the same struct.

## Tasks

- **T1** — **DONE.** The shell and the disassembly: the title, the listing
  with the cursor and the colours, the status line, the help overlay, and
  the keys `F5`, `F8`, `F9`, the arrows, `?` and `q`. L1, with its
  verification in L2 and L3.
- **T2** — **DONE.** The registers panel and the flags, with the register
  under the cursor editable in place. S5-S8. Commit `a3a2a48`. **RED**:
  the eleven new tests did not compile, because neither the model nor the
  TUI's state existed. **GREEN**: 491 passed, 0 failed; the clippy run is
  clean for the two crates that changed.
- **T3** — The memory window and the goto: the panel with its gutter and
  its printable side, the cursor over the bytes, the focus (`3`), the
  navigation (the arrows and the pages) and the goto prompt. S9-S11.
  Commit `2fd4731`. **RED**: the seven new tests did not compile, because
  `Focus::Memory` did not exist and the model had no window. **GREEN**: the
  drawing test is 18 passed, 0 failed; the model's is 4 passed, 0 failed;
  the whole tree is **499 passed, 0 failed**, and the clippy run is clean
  in every crate.
- **T4** — The stack pane: the pane that opens where the listing is, the
  window over the stack, and the keys that move it. S12. **RED**: the
  seven stack tests failed, because the pane did not exist. **GREEN**:
  the drawing test is 27 passed, 0 failed; the model's is 6 passed, 0
  failed.
- **T5** — The source strip: the assembler records where every source line
  landed, the panel reads them, and the strip is drawn under the listing.
  S13-S14. **RED**: the six strip tests failed, because the source was not
  on the screen at all. **GREEN**: the drawing test is 37 passed, 0
  failed, and the whole tree is **526 passed, 0 failed**.
- **T6** — The editing in the panes: the byte under the memory cursor and
  the flag under the flags cursor, both written in the reference's own
  language, with the flags pane's own focus and cursor. S15-S17. **RED**:
  the new tests did not compile, because `Focus::Flags`, the flag cursor
  and `Flag::write` did not exist. **GREEN**: the drawing test is 52
  passed, 0 failed; the model's is 7 passed, 0 failed.
- **T7** — The goto with labels and segments: the program's own names, the
  reference's memory reference, and the segment the pane looks through.
  S18. **RED**: the same compile failure, and the goto's own tests with
  it. **GREEN**: part of the fifty-two.
- **T8** — The breakpoints panel model: the list of the breakpoints and
  the watchpoints with their conditions. S19. **RED**: the tests did not
  compile — there was no `listing` and no `set_condition`. **GREEN**:
  the breakpoints test is 20 passed, 0 failed. The whole tree is **549
  passed, 0 failed**, and the clippy run is clean in every crate.
- **T9** — The conditions pane: the listing of the breakpoints and the
  watchpoints, and the editing of a condition in place. S20. **RED**:
  eleven of the new tests failed, because the pane did not open and the
  condition was not written. **GREEN**: the drawing test is 66 passed, 0
  failed; the whole tree is **563 passed, 0 failed**, and the clippy run
  is clean in every crate.
- **T10** — The console's window: the command line at the bottom, the log
  above it, and the key that opens it. S21.
- **T11** — The console's commands: the reference's own ten words, and
  the refusals that change nothing. S22-S23. **RED**: the new tests did
  not compile, because the console did not exist. **GREEN**: the console
  test is 15 passed, 0 failed; the drawing test is 72 passed, 0 failed;
  the whole tree is **584 passed, 0 failed**, and the clippy run is clean
  in every crate.
- **T12** — The redesign: the tokens, the sections, the glyphs, the focus
  and the states, with the design's own document as the brief. S24-S27.
- **T13** — The polish: the second round of the design, the narrow
  terminal and the empty space. S28-S31. **RED**: the five new tests
  failed, because the listing kept three rows, the console's row still
  carried the `▸`, the help's header was muted and the memory window
  stopped eight columns short of its rule. **GREEN**: the drawing test is
  89 passed, 0 failed; the whole tree is **601 passed, 0 failed**, and the
  clippy run is clean in every crate.
- **T14** — The keys that come with the core: `w` puts a watchpoint on the
  memory cursor, `F6` steps back and `F7` steps over — with the journal
  and the depth of the calls that the core does not have. S32-S36. Route:
  delegated, one bounded writer, one commit, one independent verifier.
  **DONE.** **RED**: the core's four tests did not compile, because the
  journal and the depth of the calls did not exist; and the five new
  TUI tests failed: `w` set no watchpoint, `F6` did not rewind the
  machine, `F7` did not step over the call, the console's `p` was still
  refused, and the help was cut after its ninth key. **GREEN**: the whole
  tree is **611 passed, 0 failed**, the clippy run is clean in every
  crate, and `cargo fmt --all --check` exits 0.
  **The corrections.** The watchpoint fires: the step path asks the model
  whether the write the instruction left hit one, and the machine stops
  there and the status says "the machine wrote SSSS:OOOO"; the step back
  puts the depth of the calls back with the rest of the machine; and the
  help's two columns keep the words that fit in them. The patch's help
  ends where the base's began, and the two entries that moved are `Esc`
  ("close a window", because the stack is one of three windows now) and
  `g` (short enough for its column). **RED** (corrections): the watchpoint
  did not fire and the machine ran on to the halt, the depth after a step
  back over a call was 1 and not 0, and the help had shortened the words
  that fit in its columns.
  **GREEN** (corrections): the whole tree is **614 passed, 0 failed** on
  `cargo nextest run`, the clippy run is clean in every crate,
  `cargo fmt --all --check` exits 0, and the headless runner is
  unchanged over the eighty-five reference programs.
  **The verifier's batch.** The step back over a halt left the debugger
  stuck: the machine was not halted any more and the debugger still said
  it was, so `F8` and `F5` did nothing. The debugger's `finished` follows
  the machine's own halt state after the rewind now. The step back also
  takes the address the machine was at off the list of what has run —
  the list holds no duplicates, so the last entry was not the one. And
  the console's own doc says that the two forms of an address it cannot
  read are what is missing, not two commands. **The probes**: the
  verifier's twenty-seven probes are in the tree now, in
  `crates/core/tests/journal.rs` and `crates/tui/tests/keys.rs`.
  **RED** (the verifier's batch): twenty-six of the probes passed and
  `the_step_back_over_a_halt_leaves_a_machine_that_can_be_stepped`
  failed — "the step back takes the machine out of the halt".
  **GREEN**: the whole tree is **642 passed, 0 failed** on
  `cargo nextest run`, the clippy run is clean in every crate and
  `cargo fmt --all --check` exits 0.
  **The native review's batch.** The journal's memory is bounded: the
  write log had no bound, and a guest that writes a megabyte in one
  instruction made the journal hold hundreds of megabytes. The bound is
  on the entries the journal holds in total, the restore undoes the
  last write first, the step over honours the breakpoints, the
  watchpoint's silence is proved, and the tests make a directory of
  their own for every run. **RED**: the step that writes more than the
  cap did not break the journal, the journal's total went past the
  bound, a byte the instruction wrote twice kept the intermediate value,
  the instruction that wrote nothing fired the watchpoint again, and the
  step over ran past a breakpoint. **GREEN**: the whole tree is **649
  passed, 0 failed** on `cargo nextest run`, the clippy run is clean in
  every crate, `cargo fmt --all --check` exits 0, and the headless
  runner is unchanged over the eighty-five reference programs.
  **The readability lens's batch.** The help's list is the screen's own:
  the production list is published and the tests read it, so a key that
  is added and not shown fails the test. The write log belongs to the
  journal: a machine with the journal off records nothing and its memory
  stays flat. The help falls back to one column below seventy-eight
  columns, because a key that is cut is a key that is lost. The two
  core test files say which guarantees they own, and the budget's two
  loops share the message its end is said in. **RED**: the help's test
  read a copy of the list, a key that could not be shown passed the
  test, the memory kept a log with the journal off, and the narrow
  terminal pushed the second column off the screen. **GREEN**: the whole
  tree is **652 passed, 0 failed** on `cargo nextest run`, the clippy run
  is clean in every crate, `cargo fmt --all --check` exits 0, and the
  headless runner is unchanged over the eighty-five reference programs.
  **The review's findings.** The step back takes the address that ran
  off the list of what has run, and not the address the machine was at;
  the step over runs the first instruction and stops at a breakpoint it
  reaches; the depth saturates; the restore refuses an address outside
  the address space; the log's door belongs to the journal; the help
  takes the room its layout needs; and the pre-step snapshot is taken
  only when the journal is on. **RED**: the undone instruction kept its
  grey mark, `F7` did nothing on a breakpoint address, a return without
  a call panicked the host, the restore indexed the memory blind, the
  help lost its last keys on a narrow screen, and the bulk write threw
  the recorded steps away without a word. **GREEN**: the whole tree is
  **677 passed, 0 failed** on `cargo nextest run`, the clippy run is
  clean in every crate, `cargo fmt --all --check` exits 0, and the
  headless runner is unchanged over the eighty-five reference programs.
  **The verifier's second round.** The help takes the whole area below
  the title and shows every key the screen can hold; the step back takes
  the failed instruction's address off the list of what has run; the help
  names `Backspace`; and a step that wrote too much leaves no log behind.
  The promise about a narrow help at twenty-four rows was mine and it
  was impossible: one or two keys cannot be shown at that size, and the
  log says so now. **RED**: the narrow help showed nineteen of the keys,
  the step back took the nop's mark off after a failed step, the help did
  not name `Backspace`, and the truncated step left sixty-five thousand
  entries in the log. **GREEN**: the whole tree is **690 passed, 0
  failed** on `cargo nextest run`, the clippy run is clean in every
  crate, `cargo fmt --all --check` exits 0, and the headless runner is
  unchanged over the eighty-five reference programs.
- **T15** — The guest's screen: the passive viewer of the text page, as
  a panel of the model and a takeover of the terminal. S37-S42. Route:
  delegated, one bounded writer, one commit, one independent verifier.
  **The unit.** The text page is a panel of the model — `Panels::of` reads
  it out of the machine's own memory through the segment the UI says, the
  page is bounded to the eight the hardware has, and the TUI holds no copy
  of it — and `x` takes the terminal over with it: every row but the
  status bar, where the machine's state is. The cells are drawn with the
  colours their attribute byte carries, in the guest's own sixteen, and
  what the terminal cannot hold is cut. The debugger's own keys keep
  working while the takeover is open — the takeover holds the whole
  screen, the listing's cursor is not on it, and so `F9` and the arrows
  act on nothing while it does. The files are
  `crates/debug/src/panels.rs`, `crates/debug/src/lib.rs`,
  `crates/debug/tests/screen.rs`, `crates/debug/tests/breakpoints.rs`,
  `crates/tui/src/app.rs`,
  `crates/tui/src/ui.rs`, `crates/tui/src/theme.rs`,
  `crates/tui/tests/screen.rs` and `design/DESIGN.md`. **RED**: the model
  had no screen panel at all — `Panels` had no `screen` field and `Ui` no
  page — and the TUI had no takeover: `theme::VGA` did not exist, `App`
  had no `screen_open`, and the takeover's own test did not compile.
  **GREEN**: the whole tree is **706 passed, 0 failed** on
  `cargo nextest run`, the clippy run is clean in every crate and
  `cargo fmt --all --check` exits 0.

## Log

- **L20 (2026-11-24).** The maintainer chose the cut: the guest's
  screen.
  - **The choice.** "La pantalla del invitado: visor pasivo de B800",
    chosen through the question tool, and closed the way T14 was closed:
    one unit, one commit, one independent verifier.
  - **Why this slice and not the adapter.** The core has no video
    adapter and no BIOS, and this slice does not add one: the memory is
    plain RAM, so the text page is read like any other memory and the
    guest's own writes to `B800:0000` are what it shows. The core's
    golden for the reference's own `1_sample` program shows the program
    writing "Hello, World!" straight to `B800:0002`, with no adapter in
    the way.
  - **What is not in this slice.** The graphics modes are not here, and
    `INT 10h` is not here: a program that calls it still stops where
    the interrupt took it, because there is no BIOS to vector to. The
    research's own word for the honest slice is a *passive viewer of the
    video page*, and that is what this is.

- **L19 (2026-11-24).** The verifier's second round on the corrected
  candidate: two blockers and three advisories.
  - **The promise about the narrow help was impossible, and it was mine.**
    I told the writer that at a narrow width and twenty-four rows every
    key of the list must be on the screen. It cannot be: the one-column
    layout needs a row per key and the section's header, and a
    twenty-four-row terminal has twenty-two rows below the title. One or
    two keys cannot be shown at that size, whatever the layout does.
    What the help does now: it takes the whole area below the title and
    shows every key the screen can hold, and **no key is cut in half** —
    at a width too narrow for two columns the one column is the only one
    that fits, and its keys are whole. At a narrow width the height of
    the screen, and not the layout, decides how many keys are on it.
    The promise `S36` and `S31` make is about eighty columns, and it
    holds there.
  - **The step back took the wrong mark off after a step that failed.**
    The instruction was attempted and the journal holds a record for it,
    so the address that goes off the list is the failed instruction's
    address and not the previous one's. The app remembers it on the fault
    path too.
  - **Backspace was bound and not named.** The handler binds it in the
    edit and in the goto, and the help now names it.
  - **A step that wrote too much left its log behind.** The truncated
    branch takes the writes too, so the log keeps nothing.
  - **The differential note was wrong.** The local gold holds eighty-five
    `.asm` examples, and that is what the differential reports.
- **L18 (2026-11-23).** The native review of the corrected candidate came
  back with findings from its risk and reliability lenses, and the
  biggest one was a regression.
  - **The step back took the wrong address off the list.** The list of
    what has run holds the address of the instruction that ran, and the
    rewind read the instruction pointer instead: it removed an address
    that was never there and left the undone instruction's mark. The app
    remembers the address that ran now.
  - **The step over was inert on a breakpoint address.** The check ran
    before the first instruction, so `F7` did nothing where `F5` leaves
    the machine. The check belongs after the first instruction: the place
    the user started from is not reached by the step over.
  - **The depth could overflow.** A return without a call drove the
    counter past its bound, and the host panicked. The depth saturates
    now, and the promise the core's own doc makes holds.
  - **The restore wrote at a caller's address.** The method refused an
    address outside the address space, and the log's door is the core's
    own: a caller cannot turn the journal's log off behind its back and
    leave the journal with empty write lists.
  - **The help's height did not follow its layout.** The overlay was
    sized for the two columns and the narrow layout is one column, so
    the tail of the keys was cut. The area the help takes is the area
    the layout it draws needs.
  - **The pre-step snapshot was taken with the journal off.** The
    headless runner paid for a copy of the registers on every step and
    never read it. The snapshot is taken only when the journal is on.
  - **Still open.** `executed` holds one entry per distinct address, so
    an address visited twice loses its mark entirely when the second
    visit is undone. The console's `p` output is not pinned by a test,
    and a step over that ended at a breakpoint or ran out of its budget
    says nothing in the console output: the reason only reaches the
    status bar. The step over's exit condition is the call depth, so a
    procedure that ends by jumping rather than returning never brings
    the depth back and the step over runs to its budget. And the core's
    `step` clears `stop_reason` and `halted` before every instruction,
    for every caller of the core, the runner included: that predates
    this unit, but the journal now restores both, so the two semantics
    meet.
- **L17 (2026-11-23).** The native review of `b208af3` came back with
  findings, and the biggest one was the guest's own amplification.
  - **The journal's memory is bounded.** The write log had no bound: a
    `rep movsw` with a count of 64K put a megabyte into one undo record,
    and the 256 records the journal keeps made that hundreds of
    megabytes. The bound is on the entries the journal holds in total —
    65 536 of them, about half a megabyte — and the oldest records go
    rather than the journal growing. A step that wrote more than the
    bound breaks the journal, because the machine's memory has moved
    past it and cannot be put back; the TUI says "the step was too big
    to undo".
  - **The restore undoes the last write first.** The first value a byte
    held is the one that must survive, so the writes are replayed in
    reverse order.
  - **The step over honours the breakpoints**, the way the run does, and
    it says so: "the machine stopped at a breakpoint".
  - **The watchpoint's silence is proved.** The core clears the last
    write at the start of every instruction — that line is the core's
    own — and a test now locks it down: an instruction that writes
    nothing fires nothing.
  - **This unit's own test helper makes a directory of its own.** The name
    carries the process id and a counter, the directory is made
    exclusively, and it goes when the test ends. The older helpers are
    another matter, and they are listed below.
  - **The help's width.** The two columns need seventy-eight columns,
    and a narrower terminal gets one column instead of losing keys: a
    key that is cut is a key that is lost.
  - **The older test helpers still share a directory.** The helpers of
    `crates/tui/tests/drawing.rs`, `crates/tui/tests/console.rs`,
    `crates/asm/tests/include.rs`, `crates/cli/tests/disasm.rs`,
    `crates/cli/tests/runner.rs` and `crates/core/tests/transport.rs`
    make their directory under the shared temporary one with a fixed
    name, create it without exclusivity and never remove it: another
    local user can pre-create it or plant a symlink in it, and two
    concurrent runs of the same test overwrite each other's program file.
    That weakness is older than this unit.
  - **Still open.** The watchpoint sees only the last write of an
    instruction: a `rep movsb` fires on its last byte alone, because the
    model's door takes one write. The step back does not rewind the
    devices behind the I/O bus, and the reference flags such operations
    instead of reversing them. A broken journal has no way back: the
    steps after the oversized one are undoable, but the journal refuses
    them, and the write log keeps being paid for a journal nobody can
    use. A rewind over an instruction that wrote a byte also discards a
    write made **after** it — the debugger's own edit of a memory byte —
    with nothing said. And the help, which takes the whole area below the
    title, covers the status bar: the machine's state is in the status
    bar and nowhere else, so with the help open that fact is on the
    screen nowhere until the help is closed.

- **L16 (2026-11-23).** The maintainer chose the cut: the keys that come
  with the core.
  - **The choice.** "Las teclas con el núcleo (w, F6, F7)", and for the
    closing: "Una unidad, un commit, un verificador".
  - **Why the core changes.** `F6` is the reference's step back
    (`features-and-ux.md` DBG-3) and it is journal-based: the machine goes
    back to the state it had before the instruction, which means the bytes
    that instruction wrote have to go back too. `Memory` kept one
    `last_write` and nothing else, so the core now keeps a journal of the
    steps it can undo. `F7` is the reference's step over (DBG-2), which
    runs "a procedure or interrupt call" to its end, so the core keeps the
    depth of the calls.
  - **The console's `p`.** The refusal it carried — "the machine keeps no
    call tracking" — is no longer true once the core keeps the depth of
    the calls, and the reference's own command is in its list, so `p`
    steps over too.
  - **The risk.** The journal changes the memory's write path, which every
    instruction uses: **Risk: item 3**. The independent verifier is part of
    this cut and is not optional.
  - **The verification.** An independent verifier read the whole plan and
    probed it: **S32, S34, S35 and S36 are met**, and **S33 was unmet** with
    one blocker — the step back over a `HLT` left the TUI's own `finished`
    flag set, so the debugger was stuck. The core already put the halt
    state back; the TUI's flag now follows the machine's. The recheck of
    that blocker is green, and the verifier's twenty-seven probes are in
    the tree.
  - **The differentials.** The eighty-five reference programs of the
    headless runner are byte-identical against the base, the console's
    twenty-three commands print the same words **except `p`**, which this
    cut turns from a refusal into the reference's step over, the drawn
    screens are identical at three sizes and eight states **except the
    help**, which this cut redraws in two columns with three new keys,
    and the gate over the 646000 captures is no slower.
  - **Still open.** The guest's screen; T17 (packaging and CI); T18 (the
    GUI); the `#MEM=` that the two frontends ignore; and the watchpoint,
    which sees only the **last** write of an instruction — a `rep movsb`
    fires on its last byte alone, because the model's door takes one write
    and changing that is a change of the debug crate's model, not of the
    TUI.

- **L15 (2026-11-21).** The maintainer chose the keys — with the core —
  and then stopped: "Las teclas que faltan + núcleo pero no prosigas,
  solo guarda la respuesta en memoria engram". The work is not started;
  the answer is kept here and in the mirror.
  - **What was decided.** `w` puts a watchpoint (the model's door is
    already there), `F6` steps back and `F7` steps over — and the last
    two enter **with the changes to the core they need**: the machine's
    history for the step back and the tracking of the calls for the step
    over. They are not a TUI-only change.
- **L14 (2026-11-21).** The polish: the second round of the design, and
  the narrow terminal.
  - **The listing is the panel a debugger lives in**, and it keeps what it
    needs: at 80x24 it went from three rows to five, and the strip takes
    what the listing can spare — in two columns, when one column does not
    fit.
  - **The listing asks the model for its rows** (`App::panels_with`), so a
    tall screen is filled with instructions and not with emptiness: at
    180x50 the band holds 26 and at 140x40 it holds 17.
  - **One glyph, one meaning.** `▸` means "here" and nothing else, and
    the console's caret is the terminal's own inverted cell.
  - **The help overlay is a section and not a box**, with the screen's own
    words.
  - **The memory window's width follows its bytes** — eight at eighty
    columns with their printable side beside them, thirty-two at a hundred
    and forty — and the columns it does not need go where they are used.
  - **Still open.** The help's list of keys is cut at 80x24 (twenty keys,
    ten rows: a two-column help needs shorter words), and the memory
    cursor's row is the model's sixteen bytes while the drawn rows follow
    the width (eight, sixteen or thirty-two). The guest's screen, whose
    research is done, is still the last piece of the TUI.
- **L13 (2026-11-21).** The design pass, the last unit of the TUI.
  - **The TUI was captured and looked at**, which is the only way to judge
    it: `crates/tui/examples/screenshot.rs` renders the TUI into the
    TestBackend and writes an SVG, and `rsvg-convert` makes a PNG. The
    captures are at 80x24, 140x40 and 180x50.
  - **What the capture found.** P0: every panel is framed with a box —
    five cards on one screen — and the memory window's cursor is
    invisible unless the window has the focus, so the user cannot find
    where the window is. P1: a state is colour alone (the machine's
    instruction, a breakpoint, a watchpoint); the focus is invisible in
    three of the four sections; two glyphs mean "here" (`>` in the
    listing and `►` in the source strip); the source strip has no header
    and floats. P2: the header is a boxed card; the labels are not dim;
    the status bar puts the volatile data on the left. P3: the ANSI
    palette on black is harsh, and the bottom band's sections inherit
    the registers' empty rows.
  - **The design's document** is `design/DESIGN.md`: the tokens, the
    shape and the named rules. The colours' meanings are the research's;
    the values are this project's.
  - **The words stay.** The redesign changes the frame and not the
    vocabulary, so the tests that pin the texts keep pinning them.
  - **The GREEN** is the twelve new tests and **596 passed, 0 failed**,
    with the clippy run clean in every crate. The before and after are
    side by side at 80x24, 140x40 and 180x50.
  - **The polish's findings**, from looking at the after: at 80x24 the
    listing keeps three rows because the strip takes the rest of the
    band; on a tall screen the listing's area is empty below the model's
    sixteen instructions; the console's caret is a second `▸` for another
    meaning; the help overlay is still a box; and at eighty columns the
    memory window is wider than the bytes it shows.
  - **The maintainer decided** (2026-11-21): the step back (`F6`) and the
    step over (`F7`) **enter with the changes to the core they need** —
    the machine's history and the tracking of the calls — when the keys
    are built. They are not a TUI-only change.
  - **Still open.** The guest's screen, whose research is done and whose
    honest slice is a passive viewer of the video page.

## Log

- **L12 (2026-11-21).** The debug console is landed, and with it the TUI
  has every panel the research's layout draws.
  - **The commands are the reference's own words**, read out of
    `re/docs-txt/debug.txt`: `c`, `i`, `iw`, `o`, `ow`, `d`, `t`,
    `r` and `rf`. The research's inventory says twelve; the reference's
    own document says ten, and the document wins.
  - **Every parameter is in hexadecimal and every value is printed in
    hexadecimal**, the reference's own rule: "All output values are in
    hex. All input parameters must be in hex. The h suffix is not
    required."
  - **The text the console prints is ours.** The reference documents the
    commands and not their output, so the strings are this project's, and
    the registers and the flags are printed as the panels draw them — the
    project grows one format and not two.
  - **Three commands are refused honestly**: `p` (the machine keeps no
    call tracking), the effective addresses (`d bx+5`) and the symbols
    (`d lion L 10`) — this project has no symbol table.
  - **A refusal changes nothing**: the command is read before anything is
    touched, and the machine, the breakpoints and the counters are left
    exactly as they were.
  - **Still open.** The guest's screen, whose research is done and whose
    honest slice is a passive viewer of the video page; and the design
    pass over the whole TUI, at the end, as the maintainer asked.
- **L11 (2026-11-21).** The conditions pane is landed: the breakpoints and
  the watchpoints are drawn with their segment, their offset and their
  condition, and the condition under the cursor is edited in place.
  - **The model was already there**, so this slice was the TUI's alone.
  - **The condition is the reference's own expression**, read by the
    evaluator the breakpoints already use, and a condition that does not
    read is refused in the evaluator's own words and the old one stays.
  - **The pane is a window**, like the stack: it takes the listing's
    place, and only one window holds it at a time.
  - **Still open.** The debug console; the guest's screen, whose research
    is running; and the design pass over the whole TUI, at the end.
- **L10 (2026-11-21).** The fifth slice is landed: the editing in the
  panes, and the goto with labels and segments.
  - **The two halves did not touch a file**, so they were built together:
    the TUI's own hub (the keys, the focus and the edit state) and the
    debug crate's breakpoints module.
  - **The edit state is an enum now**, because what is edited is a
    register, a flag or a byte, and the machine's three widths are not
    the same width: sixteen bits, one bit, eight bits.
  - **The goto reads the reference's own way**: a name of the program's
    own names first, then a memory reference `SSSS:OOOO`, and then a
    bare hexadecimal offset. The name comes first because it is the most
    specific of the three.
  - **The stack's key moved from `4` to `5`**: the four persistent panes
    take the research's own numbers, and the stack is a window and not a
    pane of the layout.
  - **The slice was two units and a third of documentation**: the TUI's
    hub and the breakpoints' model, each with its own commit, because
    they do not touch the same file.
  - **Still open.** The guest's screen; the debug console; the pane that
    draws the conditions the model now lists; and the design pass over
    the whole TUI, at the end.
- **L9 (2026-11-21).** The fifth slice, chosen by the maintainer through
  the question tool: **the editing in the panes, and the goto with labels
  and segments**.
  - **The choice.** "Editar en los panes + goto con labels", and for the
    closing: "Lo que no se toque, en paralelo".
  - **The parallel window here is narrow**, and the reason is the TUI's
    own hub: the keys, the focus and the edit state live in one file, so
    two features that add a key cannot be built at the same time. What
    does not touch it — the model's write doors — is built apart.
  - **The stack's key moves from `4` to `5`**, because the four
    persistent panes take the research's own numbers and the stack is a
    window and not a pane of the layout.
  - **Still open.** The guest's screen; the debug console; the
    stop-on-condition window; and the design pass over the whole TUI, at
    the end, as the maintainer asked.

- **L8 (2026-11-21).** The fourth slice, chosen by the maintainer through
  the question tool: **the stack and the source strip**.
  - **The choice.** "Pila + source strip (en paralelo)", and for the
    design: "Funciones primero, diseño al final".
  - **The two halves do not touch the same file**, so they are built
    together: the source map lives in the assembler and in the debug
    crate's own module, and the stack pane lives in the panels model and
    in the TUI.
  - **The design comes at the end**, once every panel is in: one pass
    over the layout, the colours, the keys and the help, with a visual
    proof.
  - **Still open.** The guest's screen; the memory's own editing; the
    segment the memory pane looks through; the labels in the goto; the
    flags' own editing; the debug console.
- **L7 (2026-11-21).** The third slice, chosen by the maintainer through the
  question tool: **the TUI's memory window and its goto**.
  - **The choice.** "TUI: ventana de memoria + goto", and for the cleanup
    that went with it: "En la misma unidad, commit propio".
  - **The cleanup went first, in its own commit** (`8115bb7`): the clippy
    warning in `crates/asm/src/assemble.rs` is gone, `assembler.md` counts
    the examples correctly (`eighty-five`), `HANDOFF.md` has one heading
    for the assembler's work instead of two, and `emu8067-foss.md` says
    that `AUDIT-1-old` is closed, because the code settled it and the row
    was left open.
  - **Still open.** The stack, the source strip and the guest's screen;
    the memory's own editing; the segment the memory pane looks through;
    and the labels in the goto, which need a symbol table this project
    does not have yet.
- **L6 (2026-11-19).** The second slice is built: the registers panel, the
  flags, and the register under the cursor editable in place.
  - **The model grew and the renderer did not.** `panels::Register` names
    the thirteen registers the panel shows and `panels::Flag` the nine
    flags; `Registers::cells` is the ordered list the renderer draws, and
    `Registers::flag` reads the flags word. The write is `Register::write`,
    the other side of the same door.
  - **The RED was a compile failure**: the eleven new tests did not
    compile, because neither the model nor the TUI's state existed. The
    GREEN is **491 passed, 0 failed**, and the clippy run is clean for the
    two crates that changed. The one clippy warning in the tree is
    pre-existing and lives in `crates/asm`.
  - **The value is the reference's own language.** `1234h`, `4660` and
    `101b` are the same value, `BX+1` is an expression, and a value that
    does not fit in sixteen bits is refused with "value out of range" — a
    truncation would be a lie about what the register holds.
  - **The focus.** `Tab` moves it, `1` and `2` take the disassembly and
    the registers, and the arrows move the cursor of the pane that has
    it. The disassembly's own cursor keeps its meaning when the
    disassembly has it, and a test says so.
  - **The middle of the band is empty on purpose.** The research's layout
    puts the memory window between the registers and the flags, so the
    window lands there without moving anything.
  - **Risk: none.** The change names no high-risk item. It writes the
    machine's registers when the user says so, which is the debugger's
    whole job, and the two crates it touches are the TUI and the panel
    model, whose shapes are unchanged: `Ui` and `Registers` keep every
    field they had, and the new types are additions.
  - **The native assessment of the commit**: `medium`, and `under_budget` —
    so no review is due, and the boundary stays pending until a later
    commit reaches the budget. The change names no high-risk item, so it
    gets no independent verifier either: the focused test and the suite
    are the whole of its verification.
  - **Still open.** The memory window, the stack and the source strip; the
    flags' own editing; the `g`/`b`/`w` prompts; the step-back (`F6`) and
    the step-over (`F7`); and the guest's screen, which the research says
    has to be planned from the start because the TUI is genuinely weak
    there.

- **L5 (2026-11-19).** The second slice, chosen by the maintainer through
  the question tool:

  > El corte más pequeño y coherente de L4. Dibuja el panel de registros
  > (AX/BX/CX/DX/SI/DI/BP/SP, segmentos, IP) y las banderas en el layout de
  > §4.4, y añade la tecla `e` para escribir un valor encima del registro
  > bajo el cursor. El núcleo ya expone set_word/set_byte/set_segment.

  - **The model grows the two things the renderer needs**, and the renderer
    grows nothing: the panel model is the product, so the registers come
    out of it as an ordered list of cells (the register, and the value it
    holds) and the flags come out of it as their nine names and their
    values. The write is the other side of the same door, in the debugger's
    crate: `Register::write`.
  - **The value is typed in the reference's own language.** The evaluator
    that already reads the breakpoint conditions reads this one too, so
    `1234h`, `4660` and `101b` are the same value and `BX+1` is an
    expression. A value that does not fit in sixteen bits is refused: the
    register holds sixteen bits and a value of `12345h` is a mistake, not
    a truncation.
  - **The focus.** `Tab` moves it, `1` and `2` take the disassembly and the
    registers, and the arrows move the cursor in the pane that has it. The
    disassembly's own cursor keeps its meaning when the disassembly has it.
  - **Still open.** The memory window, the stack and the source strip; the
    flags' own editing; the `g`/`b`/`w` prompts; the step-back (`F6`) and
    the step-over (`F7`); and the guest's screen, which the research says
    has to be planned from the start because the TUI is genuinely weak
    there.

- **L1 (2026-11-10).** The first slice: the shell and the disassembly.
  - **The crate.** `crates/tui` (`emo8067-tui`), with ratatui 0.30.2 — the
    version the research settled (`features-and-ux.md` §4.3) and the one the
    crates.io has.
  - **The loader** lives in the debugger's crate, not in the TUI: a program
    is loaded at its origin in the segment 0000 and the segments follow it,
    which is the reference's own boot template. The TUI asks for a machine
    and gets one.
  - **The drawing**: the title with the program and whether it is running,
    the disassembly with the cursor and the colours, the status line, and
    the help overlay.
  - **The keys**: `F5` runs to the next breakpoint, `F8` steps, `F9` sets
    a breakpoint at the cursor, the arrows move the cursor, `?` is the help
    and `q` quits.
  - **Still open.** The panels of the research's layout that are not the
    disassembly; the step-back (`F6`) and the step-over (`F7`), which the
    reference has and the core does not; the mouse; the `g`/`b`/`w`/`e`
    prompts; and the guest's screen, which the research says has to be
    planned from the start because the TUI is genuinely weak there.
  - **L4 (2026-11-10).** The next slice is chosen: **the panels the TUI
    still lacks** — the registers panel with its editing in place, the
    memory window scrolling over 1 MiB, the stack and the source strip. The
    model is in `emo8067-debug`; the work is drawing it and making it
    navigable. Two things the research settles for it: the memory panel is
    a **window** and not a buffer — it owns a cursor address and renders
    `rows × 16` bytes around it, and there is no `Vec<u8>` of a megabyte
    anywhere in the UI — and the panels are jumped to, not scrolled: `g
    0100` centres the window, and there is no scrollbar over a megabyte.
  - **L2 (2026-11-10).** The verification of L1, and what it changed.
    - **A blocker: the TUI loaded programs differently from the runner.**
      The loader in the debugger's crate kept only the boot template's
      defaults and ignored what the program declares, so a program with
      `#cs=2000#` or `#ss=500#` — the reference's own `robot.asm` has one —
      ran under a **different machine** in the TUI than in the headless
      runner. The loader now follows the reference's `.binf` model: the load
      segment and the load offset have their own lines, apart from `CS` and
      `IP`, each falls back to the other when the other is not given, and
      the register initialisers say the same thing. Two tests hold it down.
    - **The step count is the count of steps**, not the count of distinct
      addresses: the research's status line reads `STEP 1 243`.
    - **The cursor never leaves the listing**, and `F9` on a line that is
      not there sets nothing.
    - **The refusals are in the assembler's own words.** The assembler
      gained a `Display` for its errors, so the TUI says "file not found: …"
      where it used to say the debug dump of the error.
    - **A failure is a failure.** A missing file and a terminal that cannot
      be had both exit with 2, where they exited with 0.
    - **The budget is reported.** `F5` that runs out of its hundred
      thousand instructions says so in the status line.
    - **Still open.** The two load models - the headless runner's, with its
      flags on top, and the loader's - live in different crates, and they
      agree today because the loader was written from the runner's rules.
      They should be one.
  - **L3 (2026-11-10).** The recheck of L2 found the same blocker still
    half-open, and one thing I had claimed without measuring.
    - **The loader still dropped `SI`, `DI` and `BP`.** The reference's
      `.binf` file lists them beside the byte registers — "EF12 ; SI",
      "3456 ; DI", "7890 ; BP" — and a program that declares them ran under
      a different machine in the TUI than in the runner: the same store
      landed at a different physical address. The loader honours the three
      index registers now, and a test holds them down.
    - **The exit code was 1, not the 2 that was claimed.**
      `ExitCode::FAILURE` is 1; the runner's is 2, and the TUI's is 2 now.
    - **Still open.** `#MEM=` — the memory the reference's `.binf` file can
      carry — is ignored by both frontends.
