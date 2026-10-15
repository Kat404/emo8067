# Brief — U6: the remaining 8086 opcodes

**Superseded in part.** This brief is kept as the record of what was believed when it was written, and the body below is unchanged. One of its claims was disproved by this project's own later work; reading it as current guidance will reintroduce a defect.

1. **"`0x84` and `0x85` carry an open question" (~lines 33-35).** It says silicon agrees with the reading that `84`/`85` are an 80186 extension. It does not. Both are real 8086 instructions, and the manual's printing is right. The project applies them and tests them. Settled in `ec669a4`, which confirmed the resolution from two primary sources against the real microcode rather than from prose.

You are implementing one unit of **emo8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`. Work inside the allowed edit surfaces below and nowhere else.

## Skills to load before work
- Native `skill` tool, id `ponytail` — the smallest change that fully works. No speculative extras.

## How to read the code
Use `codegraph_explore` as your primary index and search, **not** grep and Read loops. One call returns verbatim source plus call path and blast radius. Useful queries:
- `step dispatch ExecError mov_group string_op shift_group`
- `decode_modrm Operand Reg Base mod_bits offset_of`
- `load_far_pointer pop_to_memory xchg_memory`

Fall back to grep only for files CodeGraph does not index, such as markdown under `odd/`.

## What you are implementing

Eleven opcodes the 8086 has and this core does not yet decode. The gate is the ignored sweep at the end of `crates/core/tests/regression.rs`, whose `missing` list is exactly this set.

| Opcode | Instruction | Notes |
|---|---|---|
| `84 /r` `85 /r` | `TEST r/m8, r8` and the word form | The AND that discards its result. Sets `ZF` `SF` `PF`, clears `CF` and `OF`. `AF` is undefined: leave it alone. |
| `9A ptr16:16` | `CALL far, immediate pointer` | Push `CS` **then** the return IP, then load `CS:IP` from the four immediate bytes, low word first. |
| `9B` | `WAIT` | No effect, no flags. One byte. |
| `CA iw` `CB` | `RETF` and `RETF iw` | Pop IP, then CS, then add `iw` to SP for the `CA` form. |
| `CE` | `INTO` | Executes `INT 4` **only if** `OF` is set. One byte, and it may change flags because it is an interrupt. |
| `D4 ib` | `AAM` | Divide AL by the immediate, quotient into AL, remainder into AH. `AF` is undefined on an 8086; leave it. Divide by zero raises the divide error. |
| `D5 ib` | `AAD` | The reverse: `AL = AH*imm + AL`, then AH = 0. Same `AF` rule and the same divide-by-zero fault. |
| `D7` | `XLAT` | `AL = DS:[BX + AL]`, signed-8-bit index. No flags. |
| `EA ptr16:16` | `JMP far, immediate pointer` | Load `CS:IP` from four immediate bytes, pushing nothing. |
| `F0` | `LOCK` | A prefix. On an 8086 it does nothing to the arithmetic; it is valid only immediately before a read-modify-write instruction, and the manual does not define the timing. Implement it as consuming the next instruction byte and executing that instruction unchanged. If that is more machinery than the rest of the unit, say so and leave it for U7 — do not invent semantics. |

### `0x84` and `0x85` carry an open question

`documentation-guide.md` §1.3 marks `84`/`85` with `‡3`: the 1979 printing lists them as defined, while standing project instruction says they are an 80186 extension. The project's convention is that the manual wins unless silicon disagrees, and here silicon agrees with the "80186" reading for most sources. **Report what you implement and why; do not silently settle the erratum.** A test that asserts the opcode decodes is acceptable either way.

## Read this first

`odd/research/documentation-guide.md` §1.3 is the opcode map and §1.4 the flag table, both transcribed from the Intel 8086 Family User's Manual. §1.3 is authoritative for what exists, §1.4 for which flags move. Cells marked as errata follow real silicon; that rule is settled.

**If you are unsure about a flag, an encoding, or an instruction's behaviour, look it up.** Use Context7 (`resolve-library-id` then `query-docs`, one concept per call) or `webfetch` on a primary source, and name the source in your report. Do not answer from memory — the last brief written for this project carried a wrong LAHF/SAHF formula and the implementer had to deviate from it deliberately.

**Do not touch** `Memory::read_u16`. Whether a word at offset `FFFFh` reads across the bus or re-reads the segment base is an unresolved audit item (`AUDIT-1` in the feature document). Leave it exactly as it is.

## Method

- **Test first.** Write the failing test, run it, watch it fail for the right reason, then implement. One test per opcode family.
- Where an instruction writes flags, set them in the test first and assert the outcome. Where it writes none, set flags first and assert they survived — "does not modify" is only meaningful when something was there.
- Where a flag is undefined on an 8086, assert the implementation **leaves it untouched**. That is a positive requirement, not an absence of one.
- Follow the house style: a doc comment naming the opcode, and a comment only where the code cannot say it.
- When an opcode works, remove its entry from the `missing` list in the ignored sweep. The sweep is the gate for T11: un-ignoring it before the list is empty is the point of it existing.

## Verification you must run and report

- `cargo test --workspace` — exact pass and fail counts.
- `cargo clippy --workspace --all-targets` — warning and error counts.
- `cargo fmt --check` — must exit 0. It is currently green and must stay green; a commit that trades one red file for another is a defect.
- `cargo test -p emo8067-core --test regression -- --ignored` — report whether it still fails and on which opcode. It must still fail while work remains; if it passes, the list was incomplete.
- For each opcode: the byte sequence and what you observed.

If anything fails, say so plainly and leave the tree green or report exactly what is red. Never fake a passing run.

## Allowed edit surfaces
crates/core/src/exec.rs
crates/core/src/modrm.rs
crates/core/src/flags.rs
crates/core/src/regs.rs
crates/core/src/memory.rs
crates/core/tests/regression.rs
The section ends here.

## Rules
- English, neutral register in comments and text. No persona.
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`.
- `unsafe` is forbidden crate-wide.
- Keep the change to what these opcodes need. No refactoring for its own sake.
- If you find a defect outside this scope, report it; do not fix it. In particular do not "fix" `read_u16`.

If you make important discoveries or decisions, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.