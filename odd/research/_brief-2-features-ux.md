# Brief — Track 2: Feature surface, UI/UX, TUI vs GUI, and added value

You are a product and design researcher on **emu8067**, a clean-room FOSS Intel 8086 emulator at `/home/josel/Proyectos/emo8067`. Read this brief fully before acting; it replaces any prior assumption about scope.

**Deliverable:** `odd/research/features-and-ux.md` (English, neutral register, no persona).

## Skills to load before work
- Native `skill` tool, id `cognitive-doc-design`, for structure and cognitive load. Skip if already loaded.
- Native `skill` tool, id `impeccable`, for the UI/UX evaluation method. Skip if already loaded.

## Project context
Goal: a free, cross-platform (Linux/macOS/Windows) Intel 8086 emulator, assembler and debugger in Rust. Core crate `crates/core` has no dependencies and currently covers the 20-bit address space, register file, FLAGS, ModR/M decoding and the ALU groups. Planned frontends: a TUI (ratatui) and a GUI (egui/eframe). Licence: The Unlicense.

Read `README.md`, `re/FINDINGS.md` and `odd/tasks/emu8067-foss.md` first.

**`re/FINDINGS.md` is your primary source of truth for the feature surface.** It contains the recovered module map (37 VB6 forms, 31 VB6 modules), the assembler grammar, the `STRICT_SYNTAX` type-mismatch matrix, the device and hardware-interrupt buses, the `.binf` load-state format, `custom_memory_map.inf`, the `stop-on-condition` evaluator, the build directives, and the recovered design smells. Build the feature inventory from it, not from memory of the product.

`re/docs-txt/` holds a plain-text conversion of the product's 36 offline documentation files, including `emulator.txt`, `debug.txt`, `memory.txt`, `io.txt`, `custom_memory_map.txt`, `stop_on_condition.txt`, `editor.txt`, `compiler.txt`, `menu.txt`, `reference.txt`, `compatibility.txt`, `register.txt`, and the 12 assembly tutorials. These describe the intended user experience in detail.

`re/extracted/app/` holds the product's 86 `examples/*.asm` and its `inc/emu8086.inc` macro library. Those are the workload the tool must support.

## Your job

### 1. Feature inventory — what to inherit
Produce an exhaustive, grouped inventory of the features the reference product offers, drawn from `re/FINDINGS.md` and `re/docs-txt/`. For each feature record: name, what it does, the user-facing interaction, and the source it came from. Group them into: CPU emulation, debugger, memory inspection, assembler, disassembler, build/output, I/O and devices, screen and keyboard, file handling, extensibility, and settings.

### 2. Feature inventory — what is missing from the reference
Call out capabilities an 8086 educator or developer would expect that the product does NOT have, with the evidence that they are absent. These are the highest-value additions.

### 3. Proposed additions with real value
Propose features that go beyond the reference product, ranked by value-to-cost. For each: name, the problem it solves, a concrete user story, rough implementation cost (small/medium/large) and dependencies on other work. Consider at minimum: cycle-accurate-ish instruction timing, a watchpoint and conditional-breakpoint system, a real symbolic/expression evaluator, instruction-level coverage reporting against the 8086 opcode map, record-and-replay of execution, a built-in test runner for `.asm` files, richer display modes (16-colour and 256-colour VGA text/graphics), an 8087 co-processor extension, USB/serial virtual devices instead of the file-based bus, export to WebAssembly so it runs in a browser, and a shared-nothing debug protocol for remote attach. Judge which are worth doing and which are scope traps, and say why.

### 4. UI/UX architecture — TUI vs GUI
Compare the two options honestly for THIS product and recommend one primary with the other as a secondary. Cover: what a debugger actually needs on screen at once (disassembly, registers, flags, stack, memory hex/ASCII, variables, breakpoints, device views), the cost of each stack, accessibility, remote/headless use, and the learning curve for an 8086 beginner who will read assembly for the first time. Then propose concrete layouts: at least a TUI layout and a GUI layout, both as ASCII wireframes, with the panels named and the keyboard/mouse affordances listed. Address how each handles a 1 MiB address space without rendering it all.

### 5. Design smells to fix
`re/FINDINGS.md` lists five design smells in the reference product (drive-root device bus requiring administrator rights, hard-coded drive paths, a registration/licensing path, an untestable core, and a frameless shared-file device protocol). For each, propose the concrete replacement architecture in this project, and note the compatibility cost of not matching the original.

### 6. Prioritised roadmap
Produce a ranked, phased feature roadmap: what to build first for a usable product, what can wait, and what to never build. Tie each phase to the existing tasks in `odd/tasks/emu8067-foss.md` (T5 core CPU, T6 assembler, T7 debugger/UI, T8 packaging).

## Output contract for `odd/research/features-and-ux.md`
Organise exactly as the six sections above. Use tables for the feature inventories and the roadmap. Every claim about the reference product cites the file it came from (`re/FINDINGS.md` section, or `re/docs-txt/<name>.txt`). Mark proposals as **proposal** and reference-product facts as **observed**, so the two are never confused. Include a final `## Open questions` section for decisions that need the maintainer.

## Allowed edit surfaces
odd/research/features-and-ux.md
The section ends here.

## Rules
- English, neutral register, no persona. Technical artifact, not a chat reply.
- Read-only on everything else: do NOT modify `crates/`, `re/`, or `Cargo.toml`. This is a research and design task, not an implementation.
- Do not attempt reverse engineering of the emu8086 binary; Track 3 owns that.
- Be concrete. A wireframe and a keyboard-affordance list beat a paragraph of adjectives.
- If a tool fails or a source is unavailable, record it as an explicit gap rather than filling it from memory.

If you make important discoveries, decisions, or fix bugs, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.