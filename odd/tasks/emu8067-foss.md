# emu8067 — FOSS cross-platform Intel 8086 emulator

Objective: reimplement emu8086 as a clean-room, free-software, cross-platform Intel 8086 emulator + assembler/disassembler, using RE of `emu8086.exe` only as a behavioral reference (opcode semantics, flag rules, UI/UX and edge cases), never as source to copy.
Branch: `main` (not a git repo yet — see T0). Delivery: `ask-on-risk`. Test runner: TBD per language choice (T1).

## Specs

- **S1** — "me gustaría hacer ingeniería inversa a @emu8086.exe debido a que emu8086 no está disponible para Linux además de tener un desarrollo bastante opaco y poco definido de dónde rayos debo descargar el ejecutable en el caso de usuarios de Windows."
  Scope: extract the real emulator binary from the Inno Setup installer and characterize it. Status evidence: the artifact is an Inno Setup 5.3.7 stub (overlay `zlb\x1a`, 3 085 508 bytes past file offset `0x22a00`); the emulator itself is compressed in that overlay.

- **S2** — "El propósito final sería dar una opción FOSS multiplataforma usando a emu8086 como base además de poder llegar a mejorarlo."
  Scope: deliverable is a FOSS multiplatform (Linux/macOS/Windows) emulator that matches emu8086's observable behavior and can exceed it. "FOSS" = OSI-approved license; "multiplatform" = native binaries for ≥3 OSes from one codebase.

- **S3** — Clean-room constraint (user decision, 2026-10-15): "Reimplementación clean-room desde specs del 8086".
  Acceptance: the emulation core is written from Intel 8086/8088 public specifications and executable tests, never transliterated from decompiled pseudocode. RE output is admissible only as (a) behavioral test vectors, (b) opcode/flag rule cross-checks, (c) UI/UX and feature-gap inventories.

- **S4** — "Tengo ghidra 12.1.2 instalado dentro de mi PATH."
  Correction: `ghidra` resolves at `/usr/bin/ghidra -> /opt/ghidra/ghidraRun`, but `analyzeHeadless` is **not** on PATH. Acceptance: use `/opt/ghidra/support/analyzeHeadless` (Ghidra 12.1.2 DEV, build 20260629, JDK 27).

## Tasks

- **T0** `S1` Scaffold workspace layout (`re/`, `odd/`, `src/`), init git. Route: inline. Status: partial — `re/`, `odd/` done; git not initialized.
- **T1** `S1` Extract installer payload with `innoextract`; identify the real emulator PE. Route: inline. Status: **done** — target `re/extracted/app/emu8086.exe`, sha256 `dba99f7c…55e9`, VB6 native-compiled, 2008-12-21.
- **T2** `S1` Ghidra headless analysis: function inventory, string/D Delphi form resources, emulator-core decompilation into `re/out/`. Route: inline. Status: **done** — 1538 funcs + strings on the VB6 exe, 200 funcs + 20 decompilations on `diasm.dll`; see `re/FINDINGS.md`.
- **T3** `S1,S3` Behavioral inventory: feature list, opcode coverage, flag semantics, error messages, UI flows, file formats. Route: exploratory worker. Status: **done** — see `re/FINDINGS.md` (37 forms, 31 modules, ModRM table, print formats, x87 table, evaluator, `.binf`, `.inf`, device buses).
- **T4** `S2` Language + toolchain decision. Route: user decision. Status: **done** 2026-10-15 → **Rust** (core + ratatui + egui), **The Unlicense**, wine isolated in **Podman** for golden tests.
- **T5** `S2,S3` Core CPU: fetch/decode/execute, all 8086 opcodes, FLAGS semantics, segment/offset rules, memory map. Route: inline or delegated. Status: pending.
- **T6** `S2` Assembler + disassembler round-trip. Route: pending.
- **T7** `S2` Debugger, step/breakpoints/registers/memory UI parity with S3 inventory. Route: pending.
- **T8** `S2` Multiplatform packaging + CI. Route: pending.

- **T9** `S1` Three parallel research tracks, then unify. Route: delegated (2 background + 1 foreground), unified by parent with mini verifications.
  - **T9a** Documentation and guides. Delegate → `odd/research/documentation-guide.md` (939 lines, 25 sources). **done** — verified by parent: Intel manual URL returns HTTP 200 `application/pdf`; ratatui 0.30.2 / egui 0.36.2 / eframe 0.36.2 / proptest 1.12.0 confirmed against the crates.io API.
  - **T9b** Feature surface, UI/UX, TUI vs GUI, inherited vs added value. Delegate → `odd/research/features-and-ux.md` (710 lines, 133 features, 16 gaps). **done** — verified by parent: feature-ID count matches the totals table; every cited `docs-txt` file exists.
  - **T9c** REA of the Softonic-repacked build vs the 2010 build. Delegate → `odd/research/softonic-build-comparison.md` (524 lines). **done** — verified by parent: `app/emu8086.exe` and `funcs.tsv` hashes identical across both builds; Security Directory empty on both installers.
  - **T10** Unify T9a..T9c into the ODD plan. Route: inline. **done** — see `## Unified plan` below.

## Unified plan

Three reports, 2 173 lines, cross-checked by the parent. What they collectively settle:

**RE phase is closed.** Track 3 proved the Softonic repack carries zero code change: 275 binaries byte-identical, `funcs.tsv` hash `8a4ed1fd…` reproduced from an independent Ghidra pass, 11 differing bytes in the Inno stub, all 43 file differences confined to documentation text and four images. No further reverse engineering of this product is warranted.

**The spec now has a citable primary source.** The Intel 8086 Family User's Manual (Oct 1979) is publicly reachable as a scanned PDF on Internet Archive Bitsavers. That replaces "write from memory and from the reference product" with a real oracle: the 256-entry opcode map, the per-instruction flag table and the `INT n`/`IRET` sequence are all transcribed from it. `documentation-guide.md` §1.4 becomes the source of truth for flag tests.

**There is no shortcut on crates.io.** No mature 8086 core exists; every actively maintained x86 crate targets 32/64-bit mode instead of real mode, and the closest hobby projects are single releases from 2019 and 2023. Writing `crates/core` from the manual is therefore also the only route that does not bet the project on abandoned code. The dependency-free core is load-bearing, not stylistic.

**eframe 0.36 broke its own API.** The `App::update(ctx, frame)` signature was replaced by `App::ui(ui, frame)` plus `App::logic`. Most published egui tutorials no longer compile. The GUI estimate must assume greenfield work against 0.36.2, not a tutorial walkthrough.

**UI decision: TUI primary, GUI deferred.** The product's whole value surface is text; ratatui 0.30.2 keeps the dependency budget honest where eframe pulls wgpu; a terminal is the only frontend that works over SSH and in CI. `features-and-ux.md` §4.3.

**Reordered build sequence** (from `features-and-ux.md` §6, cross-checked against the research):

- **T11 — complete `crates/core` against the manual.** All 256 opcodes, flag semantics driven by `documentation-guide.md` §1.4, `MUL`/`DIV`/`IMUL`/`IDIV` with their undefined-flag and divide-error cases, the segment and interrupt machinery. Exit criterion: the 86 shipped `.asm` examples run headless to the same halt state as the reference reports.
  - **T11a** ALU groups, stack, `MUL`/`IMUL`/`DIV`/`IDIV`, `CBW`/`CWD`, interrupt sequence. **done** — commits `d727abb` `fdc2bdb` `e723306` `ad1c649`.
  - **T11b** `MOV` in all eight encodings, `XCHG`, `LEA`. **done** — commit `bd7c57f`.
  - **T11c** Control transfer: `JMP` near/short/indirect, the sixteen `Jcc`, `CALL` near/indirect, `RET` and `RET iw`, `INT n`, `INT 3`, `IRET`. **done** — commit `571ee01`, 78 tests.
  - **T11e** **Repair the opcode map in `odd/research/documentation-guide.md`.** Found 2026-10-15: §1.3 row `0x80` is short — it is missing `0x87` and every entry from `0x88` through `0x8F`, which is exactly where `MOV r/m,r`, `MOV r,r/m`, `MOV Sreg`, `LEA` and `POP r/m` live. Row `0xA0` labels `A8`/`A9` as `TEST r/m,imm` when hardware implements `CMP r/m,imm`. Both rows contradict the manual and contradict `crates/core`, which is checked against real encodings. The map is this project's chosen oracle, so it has to be re-transcribed from the manual pages before it is trusted for anything built on top of it. Until then, `crates/core` is the authority for opcode allocation, not the guide.
  - **T11d** Shifts and rotates, string group with `DF`/`REP`, `IN`/`OUT`, segment transfers, `DAA`/`DAS`/`AAA`/`AAS`. Route: inline. Status: pending.
  - **Ordering correction (2026-10-15):** T12's exit criterion as originally written — "the 86 shipped `.asm` examples run headless" — was unreachable, because assembling `.asm` is T13. T12 is therefore split: the runner itself is testable immediately against hand-assembled bytes, and the example corpus becomes the exit criterion only after T13. U3 (runner) is built before U4 (assembler).
- **T12 — headless batch runner and `.asm` test harness.** A CLI that assembles and runs a file and prints registers, flags and memory. Exit criterion: every example in `re/extracted/app/examples/` produces a deterministic, checked-in golden state. This is what makes T13..T16 safe to change.
- **T13 — assembler.** MASM-style syntax, the full directive set, the 13-rule `STRICT_SYNTAX` matrix recovered in `re/FINDINGS.md`, macro expansion, `#make_com#`/`#make_exe#`/`#make_bin#`/`#make_boot#`, and the `.binf` load-state format.
- **T14 — disassembler.** Must reproduce the canonical output recovered in `re/FINDINGS.md`: hex as `0%02X%02Xh`, logical addresses as `SSSS:OOOO`, and the x87 mnemonic table that the reference decodes for display without executing.
- **T15 — device and interrupt bus.** Replace the drive-root `C:\emu8086.io` file with an in-process trait plus a framed socket transport. This removes the administrator-rights requirement entirely rather than relocating the file. Only traffic lights, stepper motor and robot have documented port maps; the other four shipped devices have none and need a decision (T9b GAP).
- **T16 — TUI debugger.** Panelled layout: disassembly, registers plus flags, memory hex/ASCII with virtual scrolling over 1 MiB, stack, breakpoints, variables. Stop-on-condition evaluator first, because it is already specified down to its error strings in `re/FINDINGS.md`.
- **T17 — packaging and CI.** Cross-compile to Windows and macOS, ship binaries per platform.
- **T18 — GUI, only after T16 is stable.** egui 0.36.2, `App::ui` + `App::logic`, not the tutorial API.

**Decisions taken 2026-10-15 (maintainer):**

1. **Spec-pure, not reference-compatible.** Where the architecture manual and the reference product's observable behaviour diverge, this project implements the manual. Applied to both open behaviours: a 16-bit word is read from `EA+1` masked to 20 bits rather than from a re-wrapped offset, and `PUSHF` reads FLAGS bits 12-15 as one. Pinned by `the_high_byte_does_not_return_to_the_segment_base` and `push_reads_the_undefined_bits_as_one`.
2. **The project is `emo8067`**, matching the repository directory. The core crate is `emo8067-core` (`crates/core/`), leaving the bare name free for a future CLI binary.
3. **Editor: minimal, and only if time permits.** A cursor, insert and save inside the TUI. The reference's roughly 70 hotkeys, syntax colouring, macro editor and symbol-table viewer are not inherited. Ship plain `.asm` files with external tooling first.
4. **Devices: in-process trait plus socket transport.** Reimplement the three with a documented port map — traffic lights (port 4), stepper motor (port 7), robot (ports 9/10/11) — as in-process `Device` implementations, and expose a framed socket transport so third-party devices written against the old `emu8086.io` file protocol keep working. The remaining five (`Simple`, `LED_Display`, `Thermometer`, `Printer`, `VGA_STATE`) have no documented port map and are declared unsupported rather than guessed at.

**Superseded concerns, kept for the record:**

1. **Spec-pure or reference-compatible?** Two behaviours are marked *inferred* and the two answers diverge. (a) A 16-bit word straddling the top of the bus: `crates/core` currently fetches each byte through its own `segment:offset`, so it wraps; the reference may read the pair as contiguous. (b) `PUSHF` reading FLAGS bits 12-15 as one — architecturally correct for an 8086, but undetermined for the reference. Pick one per behaviour and say so in the core.
2. **Crate naming.** The research used `emu8067`, `emu8086-core` and `emu8066-core`. The shipped crate is `emu8086-core`; decide before anything is published.
3. **The source editor.** Whether to inherit the reference's roughly 70-hotkey editor at all, or ship assembler input as plain files with external tooling.
4. **The seven VB6 device executables.** Drop them, or shim them over the new socket transport for compatibility.

**Known gaps carried forward:** the word "breakpoint" appears zero times across all 34 reference documentation files even though `frmTrapping` proves the mechanism exists, so breakpoint syntax is a design choice rather than a recovered fact. `innoextract` cannot expose the installer's compiled script, so `[Run]`/`[Registry]`/`[Tasks]` differences between the two installers are unknown. osdev.org returns HTTP 403 to programmatic fetches, making it a manual-reading source only. The BIOS/DOS service coverage in `documentation-guide.md` §3 is the thinnest section and will need the manual's Appendix A before the DOS layer is trustworthy.

## Log

- **L1 (2026-10-15, verbatim):** "Hola Minimax, me gustaría hacer ingeniería inversa a @emu8086.exe debido a que emu8086 no está disponible para Linux además de tener un desarrollo bastante opaco y poco definido de dónde rayos debo descargar el ejecutable en el caso de usuarios de Windows. Tengo ghidra 12.1.2 instalado dentro de mi PATH. El propósito final sería dar una opción FOSS multiplataforma usando a emu8086 como base además de poder llegar a mejorarlo."
- **L2 (2026-10-15) Evidence:** `emu8086.exe` sha256 `175061d9f03a56de25e867fd4aaf16925e6fb03afdfb3b403c05bda49ea716e7`, 3 227 332 bytes. PE32 GUI i386, timestamp 2010-01-06 03:56:24, linker 2.25 (Borland). Sections end at file offset `0x22a00`; the remaining 3 085 508 bytes are an overlay beginning `zlb\x1a`. Resource and string evidence: `name="JR.Inno.Setup"`, `Inno Setup Setup Data (5.3.7) (u)`, `Inno Setup Messages (5.1.11) (u)`, `FastMM Borland Edition (c) 2004 - 2008`. Conclusion: this file is an **installer**, not the emulator. The Delphi stub is Inno Setup boilerplate with no emulator logic to recover.
- **L3 (2026-10-15) Correction to S4:** `ghidra` is on PATH but `analyzeHeadless` is not; only `/opt/ghidra/support/analyzeHeadless` exists. Env `GHIDRA_INSTALL_DIR=/opt/ghidra`.
- **L4 (2026-10-15, user decision):** base = clean-room reimplementation from 8086 specs; RE yields behavior only, never copied code. `innoextract` to be installed by the user from `extra/innoextract`.
- **L5 (2026-10-15) Blocker:** T1 cannot start until `innoextract` is on PATH. — cleared 01:09, `innoextract` 1.9 installed.
- **L6 (2026-10-15, user decisions):** stack = **Rust** (`emu8086-core` sin deps + TUI ratatui + GUI egui/eframe); licencia = **The Unlicense** (unlicense.org); diff-testing = **wine dentro de Podman rootless**, sin tocar el host. Podman 6.1.4 verificado rootless con `quay.io/fedora/fedora:41`.
- **L7 (2026-10-15) RE phase closed.** Deliverable in `re/FINDINGS.md`. Key results: the target is Visual Basic 6 native-compiled (2.2 MB) plus `diasm.dll` (MSVC) for the disassembler and the stop-on-condition evaluator. Recovered the full VB6 class map, the ModRM effective-address table, the `0%02X%02Xh` print formats, the x87 display mnemonic table, the evaluator diagnostics, and the two device buses. Not needed for a clean-room build: the decompiled pseudocode itself — the Intel 8086 User's Manual plus the 86 shipped `.asm` examples are the authoritative source.
- **L9 (2026-10-15) Second build acquired.** `rea-bins/emu8086v408.zip` from Softonic (sha256 `d56d6e42…e17afd`, 3 131 331 B) contains only `ReadMe.txt` (75 B, "This is EMU8086 version 4.08 / Repack for Windows 10 / November 25, 2015") and `setup.exe` (3 220 050 B, sha256 `a1b36dc6…397db`). `setup.exe` is again Inno Setup 5.3.7 with a `zlb\x1a` overlay, so it is a **third-party repack of the same 4.08**, not a new upstream release. `rea-bins/emu8086.exe` is byte-identical to the original 2010 installer (`175061d9…16e7`); the user moved it, nothing changed. This is the supply-chain angle behind the user's "where do I even download this" complaint and gets its own REA track (T9c).
- **L10 (2026-10-15) Tooling.** CodeGraph is initialised at the repo root; `codegraph_explore` is the primary route for semantic code search, ahead of grep/Read. `.codegraph/` is gitignored. Ghidra headless lives at `/opt/ghidra/support/analyzeHeadless` with `GHIDRA_INSTALL_DIR=/opt/ghidra`; `rea/scripts/EmuDump.java` is the dump script (`funcs` / `strings` / `decomp <regex> [n]` / `top <n>` / `callees <regex>`).
- **L11 (2026-10-15) Track results.** T9c: the Softonic repack is the same binary, so the RE phase is closed. T9a: the Intel 8086 Family User's Manual is on Internet Archive and becomes the authoritative opcode and flag source; no mature 8086 core exists on crates.io, so writing the core is also the only route not bet on abandoned code; eframe 0.36 replaced `App::update` with `App::ui` + `App::logic`. T9b: 133 features to inherit, 16 gaps, TUI primary and GUI deferred. Parent verification: Intel PDF HTTP 200 `application/pdf`; crates.io versions ratatui 0.30.2 / egui 0.36.2 / eframe 0.36.2 / proptest 1.12.0 confirmed; `app/emu8086.exe` and `funcs.tsv` hashes identical across both reference builds.
- **L12 (2026-10-15) Correction retracted.** The earlier claim that `analyzeHeadless` was absent from `PATH` was wrong; the package installs it as `ghidra-analyzeHeadless`. Recorded in `odd/tooling.md` with the lesson: verify a negative before asserting it.
- **L8 (2026-10-15) Note on FASM:** FASM 1.64 ships inside the emu8086 package under its own BSD-like license, which forbids relicensing *FASM's own code* (explicitly including to the GPL). It may be redistributed unmodified with its notices intact as a separate binary; our code is Unlicense. Decision deferred to T6 — we may reimplement the assembler instead and drop the dependency.