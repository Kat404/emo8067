# The CI — the tree's own witness

Objective: the project's gates run on every push, and not only the ones the
maintainer remembers to run. The same three commands every unit is closed
with — `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` and `cargo nextest run --workspace` — plus
the release build, the declared floor, the two other platforms the README
promises, the transcription gates, and the deep gate over every capture.
Branch: `feat/ci` (from `feat/gui`'s tip — L4 says why). Delivery: one work
unit, one commit. Test runner: `cargo nextest run --workspace`.

**Risk: item 5** — the unit touches delivery and nothing else: a workflow
that runs the project's own commands on GitHub's runners, and a deep gate
that downloads 160 MB from a third party. No line of the tree's behaviour
changes, and the tree's own tests are the evidence: the workflow's commands
are the commands the maintainer already runs by hand.

Written 2026-11-26, after the maintainer's request (L1).

## Why this and not more

Every unit of this project so far has been closed with the three gates run
by hand, and every one of them found something: the `LAHF`/`SAHF`
transposition, the 80186's register order, the `A8`/`A9` case. The gates
work. What has never existed is the tree checking itself, so a red suite is
found at the end of a unit instead of at the push that caused it.

The maintainer's notice is the reason the matrix is as wide as it is: the
minutes are free for a public repository, so there is no reason to run one
platform, one toolchain, or one gate.

What the workflow adds over the hand run, and nothing else:

- the **floor** (1.95) is checked on its own, and nothing on the
  maintainer's machine checks it — the machine's toolchain is 1.100;
- **macOS and Windows** are built and tested, and the README promises both;
- the **646 000-capture gate** runs against a pinned copy of the captures;
- the **transcription gates** run on every push, not only when somebody
  remembers that the tables were edited.

## Specs

- **S1** — The everyday gate is the maintainer's own three commands,
  unchanged: `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`, and `cargo nextest run
  --workspace`. The release build (`cargo build --workspace --release`) is
  in it as well, because the release profile is not the debug one: thin
  LTO and `panic = "abort"` are in it.
- **S2** — The declared floor is a job of its own: the workspace builds and
  the suite passes on the **floor the workspace declares**, and the job
  reads that declaration out of `Cargo.toml` instead of naming a version —
  so it follows the claim when the claim moves (the GUI raised it from
  1.85 to 1.95) and nobody has to remember to edit the workflow. Clippy is
  not run there: the floor is a promise about what compiles, and a lint set
  belongs to the toolchain that runs it.
- **S3** — The build's system packages are named, and they are the GUI's:
  `pkg-config`, `libwayland-dev`, `libwayland-bin` and `libxkbcommon-dev`.
  eframe's Wayland backend links the Wayland client and cursor libraries,
  and the Wayland bindings generate their code with the `wayland-scanner`
  **program**. The X11 path needs no package (it loads `libX11` at run
  time) and neither does OpenGL (EGL and GLX are loaded at run time too).
  See `odd/tasks/gui.md` L1, which recorded this before anything detected
  it. They are installed on every Linux run, so the GUI is checked the
  moment it lands, and they cost nothing before it.
- **S4** — The matrix covers the three platforms the README promises. Linux
  is the gate's own runner; Windows and macOS build the release binaries
  and run the suite, and they do not fail fast against each other.
- **S5** — The transcription gates run in CI too:
  `odd/research/verify_opcodes.py` (the shape of the opcode grid and the
  flag table) and `odd/research/test_extraction_contract.py` (the two
  rules the capture extraction depends on). Both are standard library only
  and both exit non-zero when they find something.
- **S6** — The deep gate is its own workflow: all **646 000** captures,
  against the commit the committed oracle names as its source
  (`e71c68d…`), on a weekly schedule, on every push to `main`, and on
  demand. The extraction is pinned through `CAPTURES_BASE`, because the
  captures live on a `main` that moves and a gate that follows it is not a
  gate.
- **S7** — The advisories job is **informational**: it reports, and it does
  not stop the pipeline. The lock file's 411 crates are scanned by hand in
  every unit; this is the same scan, every push.
- **S8** — The workflow needs no secret, no self-hosted runner and no
  credential: `permissions: contents: read`, and the only things it
  fetches are the toolchain, the tools, and the captures.
- **S9** — The program runs, not only its tests: the CLI's usage, and the
  TUI's screenshot example, which draws a screen with no terminal behind
  it.

## Tasks

- **T1** `S1`-`S5`, `S7`-`S9` — the everyday gate, in
  `.github/workflows/ci.yml`: the gate, the floor, the matrix, the
  transcription gates, the advisories. Route: inline. Status: **done**.
- **T2** `S6` — the deep gate, in `.github/workflows/hardware-gate.yml`, and
  the pinned capture base in
  `odd/research/extract_capture_oracle.py`. Route: inline. Status:
  **done**.
- **T3** — the plan's mirror, the handoff, and the commit. Route: inline.
  Status: **done**.

## Log

- **L1 (2026-11-26)** — the maintainer's request, verbatim: "Prosigamos con
  el desarollo del proyecto además de recuperar las directivas de
  desarrollo. Acabo de darme cuenta que los minutos de CI/CD en remoto son
  gratuitos para proyectos públicos así que además de hacerme question
  tool para seguir construyendo emo8067 también analiza, razona y crea un
  workflow completo para GitHub Actions cubriendo lo necesario para saber
  si nuestro programa compila y está bien, o lo que creas necesario."
- **L2 (2026-11-26)** — the decisions this unit rests on, and the facts
  they came from.
  - **The tree's own gates are the workflow.** The three commands are the
    maintainer's standing ones (the directive "test runner:
    `cargo nextest run --workspace`, never `cargo test`"), so the workflow
    cannot disagree with the hand run about what the gate is.
  - **`cargo test` is not used anywhere.** The workspace has **no Rust
    doctest**: every fenced block in a doc comment is `text` or `asm`
    (35 + 30 + 5 across the crates, all of them in the crates' own
    comments and none of them Rust). nextest does not run doctests, so
    nothing is lost by not running `cargo test` — and when a Rust doctest
    does appear, the workflow needs a doctest step, which is the one thing
    this unit leaves out on purpose.
  - **The floor is read, not named.** `Cargo.toml` says
    `rust-version = "1.95"` on this branch, and the floor moves: the GUI
    raised it from 1.85, and the next raise would be another edit. The job
    takes the version out of the declaration, so the check follows the
    claim. The machine's own toolchains are 1.97.1, 1.99.1 and 1.100 —
    **1.95 is not among them**, so the floor job is one of the two parts
    of this unit that no local run can prove (the other is the Windows and
    macOS matrix).
  - **The deep gate is fast in Rust and slow in Python.** The 646 000 cases
    run in **76 seconds** (the local `cargo nextest run --workspace`
    measured it), so the cost of the deep gate is the extraction: 160 MB of
    gzipped captures in, 214 MB of text out. The workflow's timeout is 90
    minutes for that reason, and the gate's own Rust run is not the
    bottleneck.
  - **The everyday suite does NOT cover the deep gate in CI.** The gate
    stands aside when `odd/research/capture_oracle_full.txt` is absent,
    and the file is generated and gitignored. The local run covers it
    because the file is on this machine (224 MB, generated 2026-10-23);
    the CI's run does not, which is exactly why the deep gate is its own
    workflow.
  - **The captures are pinned.** The committed oracle's header names
    `source-commit: e71c68d215a6bb8c356bd4cb3842de3bef345ca9`, the local
    full oracle names the same commit, and that commit's capture files are
    reachable (HTTP 200). The workflow therefore sets `CAPTURES_BASE` to
    that commit, so the CI generates the oracle the oracle was made of.
    The script's default stays `main`, because the maintainer's own local
    runs may want fresh captures.
  - **The advisories job is informational on purpose.** The one advisory
    the lock file touches is `RUSTSEC-2022-0048` (`xml-rs` 0.8.29, an
    "unmaintained" notice withdrawn on 2023-05-04), and `xml-rs` is a
    build-dependency of `gl_generator`, so it is in no binary. A build-only
    notice is information; it is not a reason to stop the pipeline.
  - **The Windows and macOS jobs are the untested part, and so is the
    floor job.** This project has only ever been built on Linux, and
    nothing in the tree is platform-specific outside `#[cfg(unix)]`
    (`crates/core/src/transport.rs` is the only file that touches
    `std::os::unix`, and it is guarded). The first run on either platform
    is a finding about the project, not a guess about the workflow. The
    floor job is the other one: the machine has no 1.95 installed.
  - **The repository is public** (`api.github.com/repos/Kat404/emo8067`:
    `visibility: public`), which is the maintainer's reason for the whole
    unit.
- **L3 (2026-11-26)** — the local evidence, before the workflow was
  written: `cargo fmt --all --check` exits 0, `cargo clippy --workspace
  --all-targets --all-features -- -D warnings` is clean, and
  `cargo nextest run --workspace` is **903 passed, 0 failed, 0 skipped**
  (the deep gate among them, at 76.8 seconds). The workflow's commands are
  therefore known green on this tree, and a red run on a push means
  something changed.
- **L4 (2026-11-26)** — **why the branch is cut from `feat/gui`'s tip and
  not from `main`.**
  - **The tree has to stay green.** The eighty-five examples' goldens are
    local gold, and they were recorded against the BIOS's tree: the golden
    says the program stopped at `F400:01C0` — the BIOS's own `INT 16h`
    handler — after the step budget, with the BIOS data area filled in.
    On `main`'s tree the same examples stop at `STOP took INT 0x16` after
    seventeen instructions, because the BIOS is not there yet: **85 of 85
    changed**. So `main` shows that one gate red **today**, before the CI
    exists, and a branch cut from `main` would inherit it. It is not the
    CI's doing and it is not this unit's to fix: the goldens belong to the
    newer tree, and re-recording them on `main` would throw them away.
  - **The first run proves the whole workspace, the GUI included.** The
    GUI's build has never been built on Ubuntu — the wayland package list
    in S3 has never been exercised anywhere but in the plan's log — and a
    CI cut from `main` would not build the GUI at all.
  - **The cost, and the way around it.** The CI reaches `main` after the
    stack (the keyboard, the graphics modes, the GUI, the CI), so the open
    pull requests are unchecked until then. One commit is all it takes to
    go the other way: cherry-pick it onto `main` and the checks are there
    at once — the only conflict is `odd/HANDOFF.md`, which every unit
    edits.
- **L5 (2026-11-26)** — **the first run, and what it found.** The run on
  the branch's push (`45920068767`): the Gate (linux, stable) **success**,
  the Floor (rust 1.95) **success**, the Portability (macos-latest)
  **success**, The transcribed tables **success**, the Advisories
  **success** — and the Portability (windows-latest) **failure**.
  - **The Windows failure was not a workflow's bug: it was the project's.**
    `crates/cli/src/main.rs` called
    `emo8067_core::transport::socket::DeviceServer::bind`, and that whole
    module is behind `#[cfg(unix)]` (`crates/core/src/transport.rs:133`),
    so the tree did not compile on Windows at all: `error[E0433]: cannot
    find `socket` in `transport``. The core's own doc says why the socket
    is unix: "Unix only, because that is where a device on the same
    machine can reach it; a device on another machine wants a TCP socket,
    which is the same frames over a different pipe." The README promises
    Linux, macOS and Windows, and only Linux had ever been built.
  - **The maintainer chose**: fix it at once, as a short unit — the option
    behind `#[cfg(unix)]` and the CLI refusing it with a clear message
    elsewhere. That is `b8a007f`, with a test of both halves of the
    contract (where the socket is, the bus is served on it; where it is
    not, the option is refused, says why, and makes no socket) and a test
    of the usage's half (the option is in the usage where it works and out
    of it where it does not).
  - **Two promises are proved now that never were**: the declared floor
    (**1.95**) builds the tree and passes the suite, and the tree builds
    and passes the suite on **macOS**. The Windows half is the Windows
    job's to say, once the fix is in.
  - **The matrix earned its keep in six minutes**: it found a defect that
    a Linux-only CI would have carried for the life of the project.
- **L6 (2026-11-26)** — the branch was rebased onto `feat/gui`'s tip, so
  the CI carries the GUI and every run proves the whole workspace: the
  GUI's build on Ubuntu (the wayland package list of S3 has never been
  exercised anywhere but in a log), and the GUI on macOS and on Windows
  too.
