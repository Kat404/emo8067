# The GUI — the debugger's second face

Objective: the debugger gets a window. The same panels the TUI draws, in a
window a person can click, with the guest's screen drawn as real pixels —
because the panel model is already the product and ratatui and egui are two
renderers of it. Branch: `feat/gui` (from `feat/graphics-modes`'s tip). Delivery:
five work units, one commit each, one independent verifier. Test runner:
`cargo nextest run --workspace`.

**Risk: item 3 and item 5** — the unit adds the heaviest dependency the
project has ever taken (eframe pulls winit and wgpu), and it moves shared
state out of the TUI crate into the debug crate, which every front end
consumes.

Written 2026-11-25, after the maintainer chose the unit: "T18: la GUI".

## Why this and not more

The project's own research recommended the opposite — "TUI primary, GUI
secondary" (`odd/research/features-and-ux.md` §4.3) — and the maintainer
chose the GUI anyway. That is the decision, and this unit is its shape.

The research's rule still governs what a GUI here is: "the GUI is secondary
and deferred, and it should be built as a *view layer over the same panel
model the TUI defines*, not as a separate design. If the panel model is
wrong, a GUI makes it wrong twice as expensively. **The panel model is the
product; ratatui and egui are two renderers of it**" (§4.3). The crate
already says so: `emo8067-debug`'s own doc reads "the TUI and the GUI are two
renderers of the same model", and `Panels::of(cpu, ui)` reads "Nothing here
reads a terminal: the same call serves the TUI and the GUI".

The reference solved the same problem with **37 forms** (`re/FINDINGS.md:21`),
most of them small modal viewers, and the research's own lesson is to invert
that: "That is the main UI lesson to invert" (§4.2).

## The three decisions, the maintainer's own words

- **"La GUI de depuración, al lado de la TUI"** — the debugging GUI,
  beside the TUI. The reference is a debugger first: its screen window is a
  passive viewer of the guest's page, and the TUI's existing surface covers
  every panel the reference has **except** the source *editor* and the build
  pipeline — and this project has neither.
- **"egui 0.36.2 y el MSRV sube a 1.95"** — the version the plan already
  names (`odd/tasks/emu8067-foss.md:75`), with `App::ui` + `App::logic`
  and **not** the tutorial's `App::update`, which does not compile on 0.36
  (`odd/research/documentation-guide.md:834`-`:851`). The workspace's
  declared floor rises from 1.85 to 1.95, which is honest: ratatui 0.30.2
  already declares 1.88, so the floor was already below the TUI's.
- **"La GUI primero"** — T17 (packaging and CI) waits.

## Specs

- **S1** — The GUI is a **second renderer of the same panel model**, not a
  second design. The machine is read for the screen in exactly one place,
  `Panels::of(cpu, ui)` (`crates/debug/src/panels.rs:471`-`:501`), and the
  GUI draws what the TUI draws. The GUI adds no second reading of the
  machine's memory, no second disassembly, and no second evaluator.
- **S2** — The GUI is its own crate, `emo8067-gui`, and it depends on
  `emo8067-core`, `emo8067-debug` and `emo8067-asm` — **and on nothing
  else of the project's own**. `emo8067-core` and `emo8067-asm` keep their
  empty `[dependencies]`: "the core never formats a string, never opens a
  socket, and never knows a window exists"
  (`odd/research/features-and-ux.md:604`-`:607`). The GUI is a consumer of
  the core, never its dependency.
- **S3** — The toolkit is `egui`/`eframe` **0.36.2**, with eframe's default
  features **off** (`default-features = false`), so eframe's enabled
  features are exactly `default_fonts`, `wayland`, `wgpu` and `x11` — and
  not `default`. **eframe's `accesskit` feature stays off**, because the
  accessibility story this project has is the TUI's
  (`odd/research/documentation-guide.md:869`-`:873`); egui 0.36.2 depends
  on the accesskit *crate* unconditionally, so the crate is in the tree and
  the platform bridge is not. `arboard` and `smithay-clipboard` arrive with
  egui-winit's `clipboard` feature, which is eframe's own choice and not
  trimmable without patching eframe. The workspace's `rust-version` rises
  to **1.95**, the version egui 0.36.2 declares. The entry point is
  `eframe::run_native` and the app is `App::ui(ui, frame)` plus
  `App::logic` — **never** `App::update`.
- **S3a** — **The floor's rise is not a metadata change.** Clippy applies
  what the declared floor allows, so at 1.95 two lints it suppressed at
  1.85 become live (`collapsible_if`, which suggests let-chains, and
  `manual_is_multiple_of`, which suggests `is_multiple_of`). The sites
  that trip them are the project's own, and the project writes them the
  way the new floor wants them written — the lints are **not** muted
  workspace-wide.
- **S4** — The window carries the debugger's panels: the disassembly, the
  registers and the flags, the memory window, the stack, the conditions,
  the console and the source strip. That is the TUI's set, and the TUI's
  set is the reference's set minus the editor and the build pipeline.
- **S5** — **The guest's screen is drawn as real pixels.** In the graphical
  mode the window shows the framebuffer's 320×200 pixels, one pixel to a
  pixel, with the DAC's own colours — the one thing the GUI does better than
  the TUI, which draws blocks of pixels coloured with the average of the
  block (`odd/research/features-and-ux.md:333`). In the text modes it is
  the character grid, as the TUI draws it.
- **S6** — **Every panel is one click away.** No keymap is needed to reach
  a panel: that is the research's own argument for the GUI — "the dock at
  the bottom right: variables, stack, devices and the guest screen are one
  click away without a keymap. That is the accessibility and discoverability
  argument for the GUI" (§4.2). The GUI keeps the TUI's keyboard shortcuts
  as well; it does not replace them.
- **S7** — **The state the two front ends share moves into
  `emo8067-debug`.** Today it does not: the per-pane cursors live in the
  TUI's `App` (`crates/tui/src/app.rs:208`-`:266`), the console's **parser**
  lives in the TUI crate (`crates/tui/src/console.rs:238`-`:270`), and the
  edit, goto and condition prompts are the TUI's own windows. The GUI needs
  all three, so they move to the crate the panel model already lives in —
  **moved, not duplicated**.
- **S8** — The console is the reference's: its ten words and the
  expression evaluator that goes with them
  (`crates/debug/src/expr.rs:494`-`:516`). The GUI parses the same words
  and refuses the same five refusals (`re/FINDINGS.md:76`).
- **S9** — The GUI's actions mirror the TUI's: step, step back, step over,
  run to the next breakpoint, the breakpoints, the watchpoints, the
  conditions, the edit, the goto, the console and the guest's screen. The
  **guest's keys reach the guest** through the BIOS's own door — the same
  `push_key` the TUI uses — and not by writing the machine's memory.
- **S10** — The core is untouched: the GUI reads the machine through
  `Cpu`'s public surface (the registers, the memory, the history, the stop
  reason, the BIOS's video state) and nothing else.
- **S11** — The tree stays green, and the TUI keeps working: the GUI is a
  new crate and a new binary; the runner and the CLI do not change (S12),
  and the TUI's own tests keep passing — except where S7 moves the console
  and the cursors, which move with their tests.
- **S12** — The headless runner and the CLI do not change. The GUI is a
  front end; the runner must keep working over SSH and in CI, which is the
  research's own reason for the TUI being primary
  (`odd/research/features-and-ux.md:324`-`:334`).
- **S13** — The docs move with the change: `design/DESIGN.md` gains the
  window's shape and says that the panel model is the product, and the
  handoff's "Where the tree stands" carries the new front end.

## Out of scope

- **The source editor and the build pipeline.** The reference's shape
  (b) needs both, and this project has neither: the assembler has no
  `#make_bin#`, `#make_com#`, `#mem=...#` or `#directive=` yet
  (`odd/HANDOFF.md:756`-`:764`), and the `.symbol`, `.debug` and `.binf`
  sidecars do not exist.
- **The reference's 37 forms**, and the multi-window arrangement the
  research calls "exactly the failure mode to avoid" (§4.2).
- **Accessibility.** egui renders to a canvas and a screen reader sees an
  unlabelled bitmap; the research is blunt that "accessibility work is a
  separate project" (§4.2). The TUI keeps that story.
- **T17**, packaging and CI. The maintainer chose the GUI first.

## Tasks

- **T1** — The crate, the dependency and the first window. S2, S3, S10:
  the workspace gains `crates/gui` (`emo8067-gui`), egui/eframe 0.36.2
  with eframe's defaults off, the MSRV at 1.95, `eframe::run_native`, and
  a window that draws the disassembly from `Panels::of`. The core and the
  assembler keep their empty dependency lists.
- **T2** — The shared state. S7, S8: the per-pane cursors, the console's
  parser and the prompts' state move into `emo8067-debug`, with their
  tests; the TUI uses them from there.
- **T3** — The panels. S4, S6: the registers and the flags, the memory
  window, the stack, the conditions and the source strip get their widgets,
  and every panel is reachable without a keymap.
- **T4** — The guest's screen. S5: the 320×200 framebuffer as real pixels
  with the DAC's colours, and the text modes as the character grid.
- **T5** — The actions and the keys. S9, S12, S13: the debugger's
  actions, the guest's keys through the BIOS's door, the docs, and the
  handoff.

## Log

- **L2 (2026-11-25).** T1 is in: the crate, the dependency and the first
  window.
  - **The crate.** `crates/gui`, the package `emo8067-gui`, depending on
    `emo8067-core`, `emo8067-debug` and `emo8067-asm` — and on nothing else
    of the project's own; the core's and the assembler's dependency lists
    are still empty, and a test says so. The toolkit is egui/eframe
    **0.36.2** with eframe's default features off: eframe's enabled features
    are exactly `default_fonts`, `wayland`, `wgpu` and `x11`. The entry
    point is `eframe::run_native`, and the app is `App::logic` +
    `App::ui`; **`App::update` does not exist in the code**, because eframe
    0.36 replaced it and the widely copied form does not compile there.
    The workspace's floor is **1.95**.
  - **The window** draws the disassembly from `Panels::of(cpu, ui)`, and
    the drawing's logic is a **pure function** over the panel model, so the
    tree's tests exercise it **without a display** — and no `egui_kittest`
    was added. The window opened on this machine's Wayland session, loaded
    the program and ran.
  - **The floor's rise was not a metadata change** (S3a). Clippy applies
    what the declared floor allows, so at 1.95 the lints it suppressed at
    1.85 became live: `collapsible_if` (which suggests let-chains, stable
    in edition 2024 since 1.88) and `manual_is_multiple_of` (which
    suggests `is_multiple_of`, stable since 1.87). **The sites they lit up
    were the project's own, in the assembler, the core, the debug crate,
    the TUI and the CLI**, and the project now writes them the way the new
    floor wants them written. The lints are **not** muted workspace-wide:
    that would hide the signal for the new code too.
  - **The evidence**: the tree is green with the new crate in it, clippy is
    clean everywhere, and `cargo tree` says the GUI's own features are the
    four above. The verifier's own correction to the implementer's
    expectation is worth recording: the disassembler prints `MOV AX, 01234h`,
    not the lowercase `mov ax, 1234h` the source listings show, and the
    test now pins the disassembler's real output.
  - **What the crate brings that the project did not have**: 221 crates in
    the lock file, none removed, and the `accesskit` crate — which egui
    depends on unconditionally — even though eframe's accesskit *feature*,
    the platform bridge, is off.

- **L1 (2026-11-25).** The unit is chosen and its shape is settled.
  - **The decision**, the maintainer's own words: "T18: la GUI"; "La GUI de
    depuración, al lado de la TUI"; "egui 0.36.2 y el MSRV sube a 1.95";
    "La GUI primero".
  - **What the map found.** The panel model is already the product and
    already public: `emo8067-debug` re-exports `Cell`, `Flag`,
    `MemoryView`, `Panels`, `Register`, `Registers`, `ScreenCell`,
    `ScreenView`, `StackView`, `Ui` and `SourceView`
    (`crates/debug/src/lib.rs:26`-`:29`), and `Panels::of` reads "Nothing
    here reads a terminal: the same call serves the TUI and the GUI". The
    TUI's crate says the same: "the GUI will draw the same struct"
    (`crates/tui/src/lib.rs:7`-`:9`).
  - **The reference's own shape**: a VB6 program with 37 forms and 31
    modules (`re/FINDINGS.md:21`, `:23`) — a debugger first (its screen
    window is a passive viewer, `frmScreen` dumps the video mode's state,
    `frmMemory` watches an address, `frmFLAGS` edits the flags,
    `frmStopOnCondition` halts on an expression) and **no** breakpoints and
    **no** watch command at all (GAP-3 and GAP-5,
    `odd/research/features-and-ux.md:250`, `:252`). Those two are this
    project's own additions, and a GUI here inherits them.
  - **The toolkit's cost, measured from the plan's own notes**: eframe
    pulls winit and wgpu, "the single heaviest dependency this project
    could take" (§4.2); egui 0.36.2 needs Rust 1.95 against the
    workspace's declared 1.85, and ratatui 0.30.2 already needs 1.88, so
    the floor was already below the TUI's. egui and eframe are **not** in
    the machine's cargo cache (winit 0.30.13, wgpu 27.0.1 and glow 0.16.0
    are), so the first build fetches them; the machine has network.
  - **The version trap, already written down**: eframe 0.36 replaced
    `App::update(ctx, frame)` with `App::ui(ui, frame)` + `App::logic`,
    and "the widely copied form does not compile on 0.36"
    (`odd/research/documentation-guide.md:834`-`:851`).
  - **What the unit has to invent**: the widget layer; the per-pane cursor
    state (today in the TUI's `App`); the console's parser (today in the
    TUI crate); and the guest's key injection (through the BIOS's door,
    not by writing memory).
