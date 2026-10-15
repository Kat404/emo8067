# Brief — AUDIT-6: close the second-byte holes and gate the ModR/M digits

You are working on **emo8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`. `odd/tasks/emu8067-foss.md` tracks this as AUDIT-6.

## Why this exists

The project gates that all **256 first opcode bytes** decode, which is true and now proved. But a first-byte gate is structurally blind to a whole class of defect: **the `/digit` in the ModR/M byte**. Four second-byte holes are known and unimplemented:

| Encoding | Instruction | Current behaviour |
|---|---|---|
| `F6 /1`, `F7 /1` | `TEST r/m, imm8/imm16` | `UnknownOpcode`. Ken Shirriff's microcode shows `/1` is processed exactly as if it were `/0`. |
| `D0`–`D3 /6` | `SETMO` — load the operand with `FFh`/`FFFFh` | falls into `shift_left`, which is the **80186** `SAL`, not an 8086 instruction. |
| `FE /7`, `FF /7` | `PUSH r/m8`, `PUSH r/m16` | `UnknownOpcode` |

The `D0`-`D3` case is the dangerous one: it does not error, it quietly executes the wrong instruction.

## Part 1 — settle the whole group table, not just these four

Before implementing, establish the **complete, authoritative `/digit` table** for every ModR/M group the 8086 has, and record which digits are defined, which decode to something undocumented, and which are genuine holes *in that digit position*. Sources, in order of preference:

1. `odd/research/documentation-guide.md` §1.3 and the Group 1/2/5 tables it carries, transcribed from the Intel 8086 Family User's Manual.
2. Ken Shirriff, *Undocumented 8086 instructions, explained by the microcode* — <http://www.righto.com/2023/07/undocumented-8086-instructions.html>. His Group Decode ROM discussion is the authority on which digit patterns the 8086's own decoder runs microcode for.
3. Andrew Jenner's 8086 microcode disassembly — <https://github.com/reenigne/reenigne> — as an independent cross-check.

The groups that take a `/digit` on an 8086 are: `80`–`83` (Group 1, eight ALU ops), `F6`/`F7` (Group 3), `FE`/`FF` (Group 4), `C0`/`C1` (Group 2, but `80186` on an 8086), `D0`–`D3` (Group 2), and `D8`–`DF` (ESC). Confirm that list rather than trusting it.

`C0`/`C1` are a known trap: **they are not defined on an 8086**, they were aliases added for the 80186. Do not implement them. If you find the group actually does decode them, say so with the microcode evidence.

Report the table in your final response. It is the deliverable as much as the code is.

## Part 2 — implement what belongs in it

Implement every digit the research says the 8086 runs microcode for, and reject the ones it does not. Use the existing `alu_group_imm`, `mul_group`, `shift_group`, `ff_group`, `inc_dec`, `test`-style helpers and the `write_operand`/`read_operand` pair rather than adding new machinery.

Semantics to get right:

- **`F6 /1`, `F7 /1` `TEST r/m, imm`.** An AND whose result is discarded. Sets `ZF` `SF` `PF` from the **low byte**, clears `CF` and `OF`, leaves `AF` alone. Identical to `F6 /0`, `F7 /0` and to `84`/`85` apart from where the immediate comes from.
- **`D0`–`D3 /6` `SETMO`.** Store `FFh` (byte) or `FFFFh` (word) into the operand. The manual's own description is "operand ← `FFh`" with flags behaving as `OR r/m, -1`: `CF` clears, `OF` clears, `SF` follows the result, `ZF` set when the result is zero, `PF` from the low byte, `AF` unchanged. Confirm this against the source rather than trusting the parenthetical.
- **`FE /7`, `FF /7` `PUSH r/m`.** Push the operand. `FE /7` is the byte form, `FF /7` the word form. `SP` decrements unconditionally: an 8086 stack does **not** follow `DF`.

## Part 3 — build the `/digit`-level gate

This is the point of the exercise. Add a second gate, alongside the existing `every_first_opcode_byte_the_8086_defines_decodes` in `crates/core/tests/regression.rs`, that **enumerates every ModR/M group and every `/digit` in it** and asserts each one is either implemented with the right semantics or explicitly listed as a hole with a citation.

Requirements:

- **Pin the count literally.** Do not derive the expected number from the list of exclusions — that is the defect the first gate shipped with, and independent verification caught it by mutating the exclusion list and watching the gate stay green. Write the number out, or assert against a fixed table.
- **No `#[ignore]`.** It must be a live gate.
- Every excluded `(opcode, digit)` must carry a one-line reason naming the source that settles it, so a later reader does not have to re-derive it.
- The existing first-byte gate must keep passing, unmodified.

## Part 4 — verify the gate gates

Prove it, do not assume it. Break one digit deliberately, show the gate fails and names it, restore it, show it passes. Report that sequence. A gate you have only ever seen green is not a gate — the first one in this project shipped vacuous for exactly that reason.

## Part 5 — regression tests

One test per behaviour you implement, derived from the sources rather than from the implementation. Where a flag is undefined on an 8086, assert it is **left untouched** — that is a positive requirement.

**The trap that has already cost this project six false positives:** a test helper that presets every status flag *including* `DF` silently changes string behaviour and used to look like a stack reversal. `DF` now has no effect on the stack, but it still governs the string operations. Do not set `DF` casually in a helper.

## Open audit items — out of scope, do not touch, do not test

- **AUDIT-1**: `Memory::read_u16` and how a word at offset `FFFFh` is addressed.
- **AUDIT-3**: whether `8F` takes `mod=00` only. The project's sources contradict each other; do not add a test that pins either reading.
- **AUDIT-5**: `LOCK`/`F1` re-enter the dispatcher recursively, so a long run of prefix bytes exhausts the host stack. Report, do not fix.

## Verification you must run and report

- `cargo test --workspace` — exact counts.
- `cargo clippy --workspace --all-targets` — warning and error counts.
- `cargo fmt --check` — must exit 0.
- Both gates, run alone, with their output.
- The break-and-restore sequence from Part 4.

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
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`. If a document needs changing to match the code, report the change instead of making it.
- `unsafe` is forbidden crate-wide.
- **Commit your work when done**, with a Conventional Commit message in English. The gate and the implementation it proves belong in the same commit: an earlier unit was lost to a stray `git checkout` because that separation had not been respected.
- If you find a defect outside this scope, report it; do not fix it.

If you make important discoveries or decisions, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.