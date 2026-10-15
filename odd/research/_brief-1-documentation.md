# Brief — Track 1: Documentation and guides

You are a technical researcher on **emu8067**, a clean-room FOSS Intel 8086/8088 emulator at `/home/josel/Proyectos/emo8067`. Read this brief fully before acting; it replaces any prior assumption about scope.

**Deliverable:** `odd/research/documentation-guide.md` (English, neutral register, no persona).

## Skills to load before work
- Native `skill` tool, id `context7-mcp`, and follow its steps: always `resolve-library-id` first, then `query-docs` with one concept per call.

## Project context
Goal: a free, cross-platform (Linux/macOS/Windows) Intel 8086 emulator, assembler and debugger in Rust. The core crate `crates/core` has no dependencies; planned frontends are a TUI (ratatui) and a GUI (egui/eframe). Licence: The Unlicense.

Read `README.md`, `odd/tasks/emu8067-foss.md` and `re/FINDINGS.md` first. `re/FINDINGS.md` is the behavioural spec recovered from the reference implementation. `re/docs-txt/` holds a plain-text conversion of that product's 36 offline documentation files, including `8086_instruction_set.txt` and `8086_bios_and_dos_interrupts.txt`.

`crates/core/src/{memory,regs,flags,modrm,exec}.rs` currently implement the 20-bit address space, the register file, FLAGS, ModR/M decoding and the ALU groups. You do not need to change them.

## Track A — the 8086/8088 itself (the authoritative spec)

- **Canonical opcode reference** covering all 256 opcode bytes: mnemonic, operand encoding, ModR/M reg and r/m meaning, immediate size, instruction-length rules, and **exact flag effects** (which of CF/PF/AF/ZF/SF/OF are defined, modified, or undefined per instruction). Undefined cases matter: on an 8086, AF after a logical operation, and all of PF/AF/ZF/SF/OF after `MUL`/`IMUL`/`DIV`/`IDIV` overflow, are not architecturally defined. Source preference, in order: Intel 8086/8088 User's Manual and the 8086 Family User's Manual; public mirrors of the Intel manual; the OSDev wiki. Cite a URL per block of claims.
- **8086-specific hardware behaviour** an emulator must reproduce exactly: the 20-bit bus and `segment:offset` wrap; the non-writable FLAGS bits; `PUSHF` reading bits 12-15 as one; only 8 addressing modes existing in `mod=00` (no `(disp16)` alias without a 186); `AH`/`CH`/`DH`/`BH` unencodable in a ModR/M `reg` field; a word straddling the wrap; 1 MB with no memory protection or paging.
- **BIOS and DOS interrupt behaviour** a bare-metal 8086 emulator needs: interrupt vector table at `0000:0000`, the `INT n` / `IRET` sequence and what it pushes, and the commonly implemented services (video `INT 10h`, keyboard `INT 16h`, DOS `INT 21h` file and console functions). Use `re/docs-txt/8086_bios_and_dos_interrupts.txt` only as a cross-check; cite the public source for every claim.

## Track B — the Rust ecosystem for the remaining build

Use Context7 for current, real API facts. Never from memory.

- `ratatui` — current version, layout and constraint model, widget set, maintenance status.
- `egui`/`eframe` — current version, immediate-mode model, what a debugger UI needs (split panes, hex grid, virtual scrolling across 1 MiB), shipping cost.
- Cross-compiling a Rust binary from Linux to Windows and macOS: target triples, what works out of the box, what needs extra tooling (`*-pc-windows-gnu`, `xwin`/`cargo-xwin`, `osxcross`).
- Testing: property-testing an emulator core (`proptest` or `arbitrary`), and how to build a differential-testing harness that drives a reference binary over a pipe.
- Any crate that already models 8086 or x86 semantics worth evaluating instead of reinventing. Search for it, state clearly whether it exists, and judge whether it helps or distracts.

For every library claim record: crate name, current version, the specific fact, and the documentation URL. If Context7 has no entry, say so and use `webfetch` on the official docs. Never guess a version number or an API.

## Output contract for `odd/research/documentation-guide.md`
Sections: `## 1. Opcode reference`, `## 2. 8086 hardware behaviour to reproduce`, `## 3. BIOS and DOS services`, `## 4. Rust stack: versions and decisions`, `## 5. Cross-compilation and packaging`, `## 6. Testing strategy`, `## 7. Existing 8086/x86 crates to evaluate`, `## Sources`.

Every factual claim carries an inline citation `[N]` resolving in `## Sources`. Mark claims **verified** / **inferred** / **unknown**. Include `## Open questions`. Prefer tables over prose in the opcode reference.

## Allowed edit surfaces
odd/research/documentation-guide.md
The section ends here.

## Rules
- English, neutral register, no persona. Technical artifact, not a chat reply.
- Do not modify anything under `crates/`, `re/`, or `Cargo.toml`. Research only.
- Do not attempt reverse engineering of the emu8086 binary; Track 3 owns that.
- Be exhaustive but organised.
- If a tool fails or a source is unavailable, record it as an explicit gap rather than filling it from memory.

If you make important discoveries, decisions, or fix bugs, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1–5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.