# Brief — AUDIT-2 and AUDIT-4: close the 256-opcode gate honestly

You are working on **emo8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`. Two tracked audit items in `odd/tasks/emu8067-foss.md` describe the problem; this brief tells you how to close them.

## Skills to load before work
- Native `skill` tool, id `ponytail` — the smallest change that fully works.

## Tooling
- **`codegraph_explore` is your primary index and search**, not grep/Read loops. Good queries: `step dispatch ExecError`, `ret ret_far jmp_far lock`, `decode_modrm Operand mod_bits`.
- **Context7** (`resolve-library-id` then `query-docs`) for library questions.
- **webfetch** for anything about the 8086 itself. **Never answer a flag or an encoding question from memory.** Two written specifications in this project have been wrong and had to be corrected by their implementers.

## Background: what the two audit items are

The gate for T11 ("all 256 opcodes decode") is currently an **ignored** test in `crates/core/tests/regression.rs` whose `missing` list U6 emptied. An empty list means the loop body never runs, so the test asserts nothing: **the gate stopped gating at the moment it looked closed.** That is AUDIT-2.

AUDIT-4 is that the replacement hole list must not be taken naively from the manual's "(not used)" cells. Independent verification found that on real 8086 silicon:

> Ken Shirriff, *Undocumented 8086 instructions, explained by the microcode* (righto.com), names `C0`, `C1`, `C8`, `C9` and `F1` among the 8086's holes, yet also explains that these are not traps: `C0`/`C1` behave as `RET imm16` / `RET`, `C8`/`C9` as `RETF imm16` / `RETF`, and `F1` as a second `LOCK` prefix. The microcode decodes them.

So "not in the documented opcode map" is not the same as "does nothing". **If that is right, five more opcodes are missing from the core and T11 is not finished.**

## Your job

### Part 1 — settle the hole list from primary sources

Determine, and cite with a URL, **which of the 256 opcode bytes the 8086 genuinely leaves without an instruction**, as distinct from those it decodes with an undocumented meaning. Use Shirriff's article and Andrew Jenner's microcode disassembly (reenigne.org) as the primary sources, cross-checked against `odd/research/documentation-guide.md` §1.3.

Produce a definitive list, split into:
- **holes** — the byte does not decode, or at least the 8086 has no documented behaviour; and
- **undocumented but real** — the byte decodes and does something, with what.

Then **state whether `C0`, `C1`, `C8`, `C9` and `F1` belong in the second list.** If they do, that is five more instructions and you implement them (below).

### Part 2 — implement whatever belongs in the second list

For `C0`/`C1`/`C8`/`C9`, reuse the existing `ret`/`ret_far` helpers: they are the same code as `C2`/`C3` and `CA`/`CB` with the immediate/without-immediate difference. For `F1`, treat it as `F0` does.

If your research says any of the five does **not** decode on an 8086, do not implement it and say why with the citation.

### Part 3 — replace the gate

Replace the ignored to-do sweep in `crates/core/tests/regression.rs` with a test that probes **all 256 opcode bytes** against the fixed hole list from Part 1, and fails with the offending bytes named. It must be a live gate, not an ignored one:

- remove `#[ignore]`, and
- the test must fail today for anything still undecoded, and pass when nothing is.

Bytes that are defined but not reachable by probing the opcode byte alone (the segment and BCD opcodes, where the byte is only half an instruction) may be excluded, but say exactly which and why.

### Part 4 — append the U6 regression tests

`crates/core/tests/regression.rs` is missing regression coverage for U6's eleven opcodes. Write one test each, derived from `documentation-guide.md` §1.4 rather than from what the implementation happens to do. Cover especially: `9A` pushes CS before the return IP; `CE` moves nothing at all when `OF` is clear; `D4` takes the divide error on a zero immediate and takes its flags from the **remainder**; `D5` clears `AH` even when the sum passes `FF00h`; `D7` uses a **signed** index; `84`/`85` leave `AF` alone.

## Two things that are known and must not be repeated

- **A test helper that sets every status flag including `DF` silently reverses the 8086 stack**, because `PUSH` and `POP` honour `DF`. Independent verification produced six false positives this way. If a helper sets flags, either exclude `DF` or set it deliberately and say so.
- **`AUDIT-1` is open and out of scope**: `Memory::read_u16` and how a word at offset `FFFFh` is addressed. Do not touch it.
- **`AUDIT-3` is open**: whether `8F` takes `mod=00` only. Do not touch that guard and do not write a test that pins it.

## Verification you must run and report

- `cargo test --workspace` — exact counts.
- `cargo clippy --workspace --all-targets` — warning and error counts.
- `cargo fmt --check` — must exit 0.
- The new gate, run alone, with its output.
- For each opcode you implemented or ruled out: the byte sequence, the observed result, and the source that settles it.

Never fake a passing run. If something fails, say so plainly and leave the tree green or report exactly what is red.

## Allowed edit surfaces
crates/core/src/exec.rs
crates/core/src/modrm.rs
crates/core/src/flags.rs
crates/core/src/regs.rs
crates/core/tests/regression.rs
The section ends here.

## Rules
- English, neutral register.
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`. If a doc needs changing to match, report the change instead of making it.
- `unsafe` is forbidden crate-wide.
- No refactoring for its own sake.
- If you find a defect outside this scope, report it; do not fix it.

If you make important discoveries or decisions, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.