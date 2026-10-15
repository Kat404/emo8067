# Brief — U5: complete the segment and stack group

You are implementing one unit of **emo8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`. Work inside the allowed edit surfaces below and nowhere else.

## Skills to load before work
- Native `skill` tool, id `ponytail` — the smallest change that fully solves it. No speculative extras.

## How to read the code
Use the `codegraph_codegraph_explore` MCP tool as your primary index and search, **not** grep and not Read loops. One call returns the verbatim source plus the call path and the blast radius. Useful queries:
- `decode_modrm Operand Reg Base mem::read_u16 offset_of`
- `push pop inc_dec write_operand`
- `step dispatch ExecError`

Fall back to `grep`/`Read` only for files CodeGraph does not index, such as the markdown under `odd/`.

## What you are implementing

Six opcodes that the 8086 has and this core does not yet decode. They are the segment and stack group.

| Opcode | Instruction | Notes |
|---|---|---|
| `8F /0` | `POP r/m16` | Only `/0` exists on an 8086. `50..5F` already pops registers; this pops to memory. |
| `86 /r` | `XCHG r/m8, r8` | `xchg_memory` already handles `86` and `87`, but the dispatcher only routes `87`. |
| `9E` | `LAHF` | `AH = AH&0F \| FLAGS&15`, i.e. SF ZF AF PF CF land in AH. No flags change. |
| `9F` | `SAHF` | `SF ZF AF PF CF = AH&0F`. No flags change. |
| `C4 /r` | `LES r16, m` | Loads a far pointer: offset from the word at the operand, segment from the next word. Sets **no** flags. |
| `C5 /r` | `LDS r16, m` | Identical to `LES` except the segment goes to DS. Sets no flags. |

For `LES` and `LDS` the far pointer lives at the effective address: the low word is the offset, the high word is the segment. Read the high word through `read_u16(mem, regs, <same address>, 16)` at offset+2 — i.e. build a second `Operand::Mem` with `disp + 2`, or add a small helper. Do not add a general abstraction for this.

`LAHF`/`SAHF` exist because a code generator emitted them; they are 8086 instructions and the 256-opcode sweep expects them.

## Read this first

`odd/research/documentation-guide.md` §1.3 is the transcribed opcode map and §1.4 the flag table, both from the Intel 8086 Family User's Manual. §1.3 is authoritative for what exists; §1.4 for which flags move. Note the marked errata in that document: where the printed manual and real silicon disagree, **silicon wins**, and the document marks those cells.

If you are unsure about a flag or an encoding, use Context7 (`resolve-library-id` then `query-docs`, one concept per call) or `webfetch` on a primary source, and say which source you used. Do not answer from memory.

## Method

- **Test first.** Write the failing test before the code, run it, watch it fail for the right reason, then implement. One test per opcode family.
- Every opcode here touches **no** flags except none of them: `LAHF`, `SAHF`, `LES`, `LDS`, `POP r/m16` and `XCHG` all leave FLAGS exactly as they were. Each test must set flags first and assert they survived, because "does not modify" is only meaningful when something was there.
- Follow the house style in the file: a doc comment naming the opcode and what it does, and a comment only where the code cannot say it.
- After implementing, remove the corresponding entries from the ignored to-do sweep at the end of `crates/core/tests/regression.rs` — `the_encodings_that_are_still_missing_are_listed_here`. If that test then fails, the list was wrong and something still does not decode; find out why before removing an entry.

## Verification you must run and report

- `cargo test --workspace` — report the exact pass/fail counts.
- `cargo clippy --workspace --all-targets` — report warning and error counts.
- `cargo fmt --check`
- For each of the six opcodes, the byte sequence that reproduces it and what you observed.

If anything fails, say so plainly and leave the tree green or report exactly what is red. Never fake a passing run.

## Allowed edit surfaces
crates/core/src/exec.rs
crates/core/src/regs.rs
crates/core/src/modrm.rs
crates/core/tests/regression.rs
The section ends here.

## Rules
- English, neutral register in comments and commit text. No persona, no jargon.
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`.
- `unsafe` is forbidden crate-wide; do not add it.
- Keep the change to what these six opcodes need. No refactoring for its own sake.
- If you discover a defect outside this scope, report it in your final response; do not fix it.

If you make important discoveries or decisions, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.