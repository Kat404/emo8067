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

## Out of scope

- **The panels that are not the disassembly**: the registers, the memory
  window, the stack, the source strip and the guest's screen. Each is its
  own slice.
- **The debug console** and the reference's command vocabulary.
- **The GUI** (T18), which will draw the same struct.

## Log

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
