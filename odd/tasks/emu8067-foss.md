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
  - **T9a** Documentation and guides. Delegate → `odd/research/documentation-guide.md`.
  - **T9b** Feature surface, UI/UX, TUI vs GUI, inherited vs added value. Delegate → `odd/research/features-and-ux.md`.
  - **T9c** REA of the Softonic-repacked build vs the 2010 build. Delegate → `odd/research/softonic-build-comparison.md`.
  - **T10** Unify T9a..T9c into the ODD plan. Route: inline. Status: pending.

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
- **L8 (2026-10-15) Note on FASM:** FASM 1.64 ships inside the emu8086 package under its own BSD-like license, which forbids relicensing *FASM's own code* (explicitly including to the GPL). It may be redistributed unmodified with its notices intact as a separate binary; our code is Unlicense. Decision deferred to T6 — we may reimplement the assembler instead and drop the dependency.