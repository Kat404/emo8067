# Brief — U7: segment override prefixes

**Superseded in part.** This brief is kept as the record of what was believed when it was written, and the body below is unchanged. Two of its claims were superseded by this project's own later work; reading them as current guidance will reintroduce a defect.

1. **The `F2`/`F3` and `F0`/`F1` note, "Scope" (~line 51).** It says they already exist as separate arms in the dispatcher and must not be folded into the new mechanism. Superseded for `F2`/`F3`: they are prefix latches now, one mechanism with the segment overrides. Settled in `e03650c`.
2. **The AUDIT-3 item, "Open audit items" (~line 80).** It lists `8F` as `mod=00` only and forbids pinning either reading with a test. Superseded: `8F` takes `mod=00`, `01` and `10`, and the AUDIT-3 audit item is closed. Settled in `ed3845f`, which rewrote the test that pinned the old rule instead of deleting it.

You are working on **emu8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`. This is the last implementation unit before the core covers the whole 8086 instruction set.

## Skills to load before work
- Native `skill` tool, id `ponytail` — the smallest change that fully works.

## Tooling
- **`codegraph_explore` is your primary index and search**, not grep/Read loops. Queries that pay off: `step dispatch decode_from decode_modrm`, `Operand mem read_operand write_operand offset_of`, `mov_accumulator_offset segment_push_pop`.
- **Context7** (`resolve-library-id` then `query-docs`) for library questions.
- **webfetch** for anything about the 8086. **Never answer an encoding or flag question from memory.** Three written specifications in this project have been wrong and had to be corrected by their implementers.

## What you are implementing

The four segment override prefixes, which are the only reason the 256-byte gate currently excludes four bytes:

| Prefix | Overrides |
|---|---|
| `26` | ES |
| `2E` | CS |
| `36` | SS |
| `3E` | DS |

A prefix modifies the **next** instruction's memory operand to use the named segment instead of `DS`. It does not modify the instruction in any other way.

They are currently excluded from the gate as `NOT_AN_OPCODE_ALONE` in `crates/core/tests/regression.rs`, with the comment that the decoder has no prefix state. Once they work, **remove them from that exclusion list**: the gate then probes all 256 bytes with no exclusions, and `probed` must become the literal `256`.

## Requirements

### The decoder needs prefix state

`decode_from` and `decode_modrm` currently take a `default_seg` argument and every caller passes `Seg::Ds`. Prefixes are not per-instruction decoration — they bind to the instruction that follows, so the state has to live somewhere that a prefix byte and the instruction after it can both reach. Decide where that is; `Cpu` is the obvious home, but say why.

### Which instructions a prefix applies to

Apply the override to the **memory operand of the instruction that follows**, and only where that instruction actually addresses memory. Be careful about each of these and state your reasoning:

- `LEA` takes an *address*, so a segment override on `LEA` must **not** appear in the result. The code already discards the segment there; make sure a prefix does not change that.
- A register operand has no segment to override; a prefix before one is still consumed.
- `MOV Sreg, r/m16` and `MOV r/m16, Sreg` — which side does the override touch?
- `PUSH`/`POP` of a segment register, `PUSH r/m16` via `FF /6` and `FE /6`, `PUSH`/`POP` of a register: the stack segment is `SS`, not `DS`, and a prefix must not silently redirect a push to the data segment.
- `CALL`/`JMP`/`RET`/`RETF` do not take a memory operand in the encoding, so a prefix consumes and goes away.
- `INT n`, `LEA`, `AAM`, `AAD`, `XLAT`, `XLAT` reads through `DS`.

`XLAT` is a good stress case: its index register is `BX`, its base segment is `DS`, and a prefix should change the base segment only.

### Scope

**Implement the prefixes. Do not** implement a general multi-byte prefix chain, a `LOCK`/`REP` interaction rewrite, or any instruction not named here. One prefix byte, one instruction. If a test needs two prefixes in a row, decide explicitly whether the later one wins and say so.

Note `F2`/`F3` (REPNE/REP) and `F0`/`F1` (LOCK) already exist as separate arms in the dispatcher; do not fold them into the new mechanism.

## Part 2 — update the gate

- Remove `26`, `2E`, `36`, `3E` from `NOT_AN_OPCODE_ALONE` in `crates/core/tests/regression.rs`.
- Change the literal `probed` assertion from `252` to `256`.
- A probe of a prefix byte on its own must still work: the CPU has to consume the byte and the following instruction has to execute. Whatever the prefix binds to when nothing follows it must not read out of bounds.
- Prove the gate gates: break the prefix handling deliberately, show the gate fails and names the bytes, restore, show it passes. **A gate you have only ever seen green is not a gate** — this project's first one shipped vacuous for exactly that reason.

## Part 3 — regression tests

One test per prefix, and at least one that shows the override is *used* rather than merely parsed: a `DS:disp16` operand under `3E` should still read through `DS`, and the same operand under `26` should read through `ES` and therefore give a different answer from the same physical location mapped differently.

Also test, with the reasoning stated in each: `LEA` under an override, a register operand under an override, and `PUSH`/`POP` under an override.

**The trap that has already cost this project six false positives:** a test helper that presets every status flag *including* `DF` changes string behaviour. `DF` no longer moves the stack — it is fixed — but it still governs the string instructions. Do not set it casually.

## Verification you must run and report

- `cargo test --workspace` — exact counts.
- `cargo clippy --workspace --all-targets` — warning and error counts.
- `cargo fmt --check` — must exit 0.
- Both gates run alone, with their output.
- The break-and-restore sequence for the 256-byte gate.

Never fake a passing run. If something fails, say so plainly and leave the tree green or report exactly what is red.

## Open audit items — out of scope, do not touch, do not test
- **AUDIT-1**: `Memory::read_u16` and how a word at offset `FFFFh` is addressed.
- **AUDIT-3**: whether `8F` takes `mod=00` only. The project's sources contradict each other; do not pin either reading with a test.
- **AUDIT-5**: `LOCK`/`F1` re-enter the dispatcher recursively, so a long run of prefix bytes exhausts the host stack under `panic = "abort"`. Report, do not fix. Note this task adds a second kind of prefix, so say whether your design makes that worse or not.

## Allowed edit surfaces
crates/core/src/exec.rs
crates/core/src/modrm.rs
crates/core/src/memory.rs
crates/core/src/regs.rs
crates/core/src/lib.rs
crates/core/tests/regression.rs
The section ends here.

## Rules
- English, neutral register.
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`. If a document needs changing to match the code, report the change instead of making it.
- `unsafe` is forbidden crate-wide.
- **Commit your work when done**, with a Conventional Commit message in English, gate and implementation in the same commit. An earlier unit was lost to a stray `git checkout` because that separation had not been respected.
- If you find a defect outside this scope, report it; do not fix it.

If you make important discoveries or decisions, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.