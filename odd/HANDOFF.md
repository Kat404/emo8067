# Session handoff — emo8067

Written 2026-10-15 at the end of a long session. Everything below is a claim with a source; anything marked **UNRESOLVED** is a real open item, not a reminder.

## What the project is

`emo8067` — a clean-room, free-software Intel 8086/8088 emulator, assembler and debugger in Rust, intended for Linux/macOS/Windows. Licence: The Unlicense. Pure-Rust core with no dependencies and `#![forbid(unsafe_code)]`.

**Clean-room rule:** reverse engineering establishes *observable behaviour and specifications* only. No code is ever copied or transliterated from another emulator. When this project's own research and the silicon disagree, **silicon wins**, and the manual's own print is preserved and marked as an erratum.

## Where the tree stands

```
cargo test --workspace   5 + 114 + 93 + 0 passed, 0 failed
cargo clippy             0 warnings, 0 errors
cargo fmt --check        exit 0
HEAD                     314f493
```

Two gates, both live, both proved to fail when broken:

| Gate | Covers |
|---|---|
| `every_first_opcode_byte_the_8086_defines_decodes` | all 256 first opcode bytes, `probed == 256` literal, no exclusions |
| `every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole` | 96 `(opcode, /digit)` pairs, `96`, `5` and `91` as literals, cross-checked in both directions |

Plus `python3 odd/research/verify_opcodes.py`, which guards the transcription of the manual into `documentation-guide.md` §1.3 and §1.4: exactly sixteen cells per opcode row, every flag cell inside the Table 2-18 legend. Verified by mutation, not just by passing.

## Completed units

`U1` opcode map re-transcribed · `U2` reject 80186-only encodings · `U3` repair the shipped core (6 defects) · `U4` group 1 on memory · `U5` segment and stack group · `U6` the remaining opcodes · `U7` segment override prefixes · `AUDIT-1` word wrap · `AUDIT-3` `8F` mod · `AUDIT-5` prefix recursion · `AUDIT-7` `BP` → `SS` · `F5` `REP` as a latch.

The core covers the whole 8086 instruction set including 24 undocumented opcodes the manual prints as `(not used)`. The 8086 has **no opcode holes**: all 256 first bytes decode.

## Open items, in the order they should be taken

### 1. CLOSED — string operations now honour the segment override on their source side

**Closed in `fe16b6c`.** `string_op_once` in `crates/core/src/exec.rs` read the `SI` side through `DS` unconditionally; it now reads `operand_seg(cpu)`, and the `shortcut:` comment that admitted the divergence is gone. The manual is explicit that the *only* thing a segment override cannot reach is the **destination** of a string instruction: Vol. 1, "the only exception is the destination operand of a string instruction which must be in the extra segment", and Vol. 2 p. 2-112 uses `REP MOVS DESTINATION, ES:SOURCE` as its worked example. `STOS` and `SCAS` are still ES-only; `MOVS`, `CMPS`, `LODS` now obey the override.

Introduced by `5bdb235` (U7), before the F5 baseline, and fixed after the F5 baseline.

The vacuous `every_permutation_of_the_prefix_bytes_reaches_the_same_state` is gone. It permuted all four segment overrides and still agreed, because the override could not influence anything. In its place: a test that puts one override on the `SI` side of `MOVS`/`CMPS`/`LODS`, a test that pins last-override-wins, a guard that `STOS`/`SCAS` stay in `ES`, and the order-independence claim kept over `F0 F1 F2 F3` only, the four bytes whose latches come from independent decode columns. Suite after the fix: 215 green (5 + 114 + 96).

### 2. CLOSED — `CX = 0` with `REP`: the manual was right, and it is not an erratum

The code runs **zero** iterations, following the printed manual, Vol. 2 p. 2-41. Microcode settles it in the manual's favour: the `RPTS` sub-routine at ROM address `0x112` is the **first** micro-instruction of all four string routines and jumps straight to `RNI` on a zero count — before any memory access, and without decrementing `CX` (Shirriff, *The microcode and hardware in the 8086 processor that perform string operations*). No source was found for the "silicon runs one iteration" folklore. `rep_with_a_zero_count_runs_no_iterations` stays as it is.

The second test this item said would have to flip alongside it, `a_rep_string_op_with_a_zero_count_executes_nothing`, **does not exist**; there is only one.

### 3. CLOSED except one item — the four gaps, checked against the microcode

- **`POP rmw` / `8F mod`.** The `mod=00/01/10` behaviour was already right, but the *reason* recorded in `pop_to_memory` was wrong: it credited a per-routine table that does not exist. The real reason is structural — the Translation ROM dispatches `mod`/`rm` and computes the effective address before any instruction microcode starts, and the `POP rmw` entry at `0x040` has no branch at all, so no instruction can reject a `mod` value. Rewritten in the code. **`mod=11` and `reg != 0` stay refused and are now marked OPEN**, not settled: the microcode's `M` pseudo-register predicts a register destination, but the only source that appeared to confirm it never mentions `8F` (see item 4a).
- **Intel errata for `A8`/`A9`.** None exists — Intel published no errata for the 8086/8088 family. Worse for us: the microcode points the **other** way from our decision. A dedicated `TEST A,i` routine of two opcodes sits at `0x09c` (AND, discard to `no dest`), while `alu A,i` at `0x018` already covers its full sixteen and ends in `SIGMA -> M`, which would overwrite `AL`/`AX` with the difference. **Decision: keep `CMP`**, recorded in `documentation-guide.md` ‡5 as a documented divergence with the 30-second experiment that would settle it (`B0 05` / `A8 03`, then look at `AL`). Confidence ~0.75; the chain needs a stage-1 opcode count we could not re-verify.
- **The `FF` group's latch interactions.** No defect. Jenner: the group opcodes `F6`/`F7`/`FE`/`FF` have no top-level microcode instruction at all, so nothing in that group reads the `REP` latch — the project's "read by nobody" answer was right *for this group*, and the code comment claiming it generally has been corrected, because `MUL`/`IMUL`/`IDIV` do read it.
- **Silicon `REP` with `CX = 0`.** See item 2. Closed, and the manual is correct.

### 3a. NEW, and it reverses a decision this project had already made

**`REP`/`REPNE` negates the product of `MUL`/`IMUL` and the quotient of `IDIV`.** Jenner, from microcode extracted off an 8086 die: *"Using the REP or REPNE prefix with a MUL or IMUL instruction negates the product. Using the REP or REPNE prefix with an IDIV instruction negates the quotient."* The multiply and divide microcode flips its internal `F1` flag once per negative sign in the inputs, and a `REP` prefix is what loads that flag.

This is the item the F5 brief got **right** and this project dismissed as fiction. The dismissal rested on `mul_group` "reading no flags" — which confuses *the FLAGS register* with *the internal `F1` latch*. Implemented in `ab12c21`. Two banners that called the claim void were themselves wrong and are corrected in the same commit.

**`ab12c21` shipped two defects, both found by independent verification, not by the suite.** `u16::overflowing_neg`'s overflow flag is set exactly when the value is *not* zero, while the carry out of `!lo + 1` is 1 exactly when it *is* zero — the two are opposites, so every product with a zero low half came back un-negated (769 of 65,536 byte-sized cases). And `/7` returns `(quotient, remainder)` from one path, so the shared two-word negation complemented the remainder in every non-zero case (24,256 of 32,768). The four tests that commit added used `3 * 5` and asserted only `AX` for `IDIV`, so **not one of them caught either bug**, and a claim that three of four fail on revert was false: two do. Corrected in the next commit, with the exhaustive byte-sized multiplies and every non-faulting byte-sized `IDIV` that caught both.

The lesson is this project's sixth instance of the same shape, and the sharpest yet: the suite was green over 458,753 wrong word-sized products and 24,256 wrong remainders. What caught it was a verifier building its own probes against an independent reference.

### 4. CLOSED — stale briefs, marked rather than rewritten

Each contained a claim this work overturned. Reading one as current guidance would reintroduce a defect, so each file now carries one banner at the top naming the superseded claims, what is actually true, and the commit where it was settled. **The bodies are unchanged** — the stale claims stay as the record of what was believed, which is why the banners exist instead of edits. Four files, 21 inserted lines, 0 deletions:

- `odd/research/_brief-f5-rep-latch.md` — `CX = 0` does one iteration (manual and microcode say zero); `F3` negates an `IDIV` result (**the project had this backwards**: it negates the *product* of `MUL`/`IMUL` and the *quotient* of `IDIV` — see item 3a); the eight-prefix-byte permutation claim.
- `odd/research/_brief-u6-remaining-opcodes.md:33-35` — silicon does not agree with the "80186 extension" reading of `84`/`85`; both are real 8086 instructions.
- `odd/research/_brief-u7-segment-prefixes.md` — `F2`/`F3` are prefix latches now, not dispatcher arms; `8F` takes `mod=00`, `01` and `10`.
- `odd/research/_brief-verify-f5-and-debt.md` — **not in the list above when this item was written**: the same three false claims appear again at ~lines 11-13. A worker found it while doing the other three.

Still open, and named by the work that did the banners: `documentation-guide.md` §1.3 footnote ‡3 and log entry L14 are marked "now the wrong documents" by `ec669a4`. Untouched.

### 4a. A Deep Research report that over-claimed, and what survived it

`odd/research/_prompt-microcode-closure.md` asked six questions and labelled almost every answer **(a) verified**. Three of its claims were checked by hand; two did not survive:

- **Q6 was a method check for self-consistent arithmetic, and it failed its own check**: it wrote `208 + 7 + 8 + 32 = 256`. It is **255**.
- **Q2's load-bearing citation does not contain the claim.** The report cited Shirriff's *Undocumented 8086 instructions* as the source for "`8F C0` executes `POP AX`" and "`reg != 0` is ignored". The article has no section on `8F` at all. Its structural argument is in there — the hardware substitutes the pseudo-register `M` with a register when there is no effective address — but that is an inference, not a verified statement.
- The same author's own caveat, dropped by the report: *"my analysis is based on transistor-level simulation and examining the microcode; I haven't verified the behavior on a physical 8086 processor."* Everything labelled (a) from that work inherits it.

What did survive is the one link the open `A8`/`A9` decision needed: Jenner's **public** routine table lists `alu r,i 16` and `TEST A,i 2`. Eight ALU operations times two widths is exactly sixteen, and `A8`/`A9` are the only accumulator-against-immediate pair left over, so the manual and the microcode now agree and this project is the divergent one. Not yet acted on: the chain is being independently refuted before anything flips.

The lesson matches every other one in this file. The report was *more* confident than the sources allowed, and the two places to check a research output are its arithmetic and its citations — both of which were checkable in under a minute.

### 5. `odd/research/_prompt-audit7-and-friends.md` is the single most valuable document here

A research prompt whose answers closed **four** audit items, two of which reversed decisions the parent had already committed. Reuse it as the template for any future architecture question to another model: it demanded a source hierarchy, asked claims to be labelled by how they were verified, and explicitly made *"not decidable from available sources"* a valid answer. That framing is why it found two things the parent and two verifiers had both missed.

## Five defects, one cause

Every architecture bug this project shipped came from the same place — **reasoning where a citation was required**:

| Defect | What was assumed |
|---|---|
| word wrap | the 8086 carries from the offset into the segment base |
| `PUSH`/`POP` following `DF` | if string operations move one way, the stack does too |
| `LEA` width | `8D` is odd, so bit 0 means byte |
| `CA`/`CB` immediate | the pairing was swapped |
| `BP` default segment | one default segment for every base |

Not one was found by the test suite. Every one was found by a verifier, a microcode disassembly, or a sourced research answer. **The green suite proves the tests pass, not that the emulator is right.**

## Three tooling hazards

1. **Do not splice Rust source with a Python script that counts braces.** Three attempts in this repo corrupted the file; one deleted an entire function body and left a signature with no parameter list. Use the `edit` tool, or a single targeted replacement whose `old` text was just read from disk, and run `cargo build` immediately.
2. **`edit` silently reverts six pre-existing lines** in `crates/core/src/exec.rs` to pre-rustfmt style. Run `cargo fmt` before every commit and confirm `cargo fmt --check` exits 0.
3. **Never `git checkout` a file a delegated agent just wrote but has not committed.** It destroyed a unit once; recovery meant resuming the agent's session. The agent now commits in stages, and each stage leaves the tree green.

## How this project got its findings

Every external finding arrived as a hypothesis to reproduce, never an instruction to execute — and each reviewer has reported at least one false claim:

- `A8`/`A9` reported as `TEST` — **false**, it is `CMP`. Not applied.
- My own U5 brief's `LAHF`/`SAHF` formulas — **false**. The implementer deviated deliberately and was right.
- My own F5 brief's `CX = 0` and `F3`-negates-`IDIV` — **both false**, caught by the implementer and the verifier.

Subagent discipline that worked: brief by file path, allowed edit surfaces enumerated, atomic commits with the gate in the same commit, and "report defects outside your scope, do not fix them".

## Next session, in order

1. ~~Fix the string-source segment override (item 1)~~ — done in `fe16b6c`; its replacement permutation test had to be written from scratch, because the F5 verification report it referred to is in neither memory nor the repo.
2. ~~Decide item 2 with a primary source~~ — done; microcode settles `CX = 0`, and it is not an erratum.
3. ~~Correct the five stale briefs (item 4)~~ — done; four files carry banners, and a sixth document turned out to be wrong.
4. `T12`: the headless runner already exists at `crates/cli` (binary `emo8067`, with `--trace` and `--dump`); what remains is running the 86 shipped `.asm` examples, which needs `T13`, the assembler.

## Files worth reading first

| Path | What |
|---|---|
| `odd/tasks/emu8067-foss.md` | specs, every decision, the U1..U8 plan, audit items, and the log |
| `odd/tooling.md` | environment facts: Ghidra paths, what `analyzeHeadless` needs, `innoextract` |
| `odd/research/_prompt-audit7-and-friends.md` | the research prompt template |
| `re/FINDINGS.md` | the behavioural contract recovered from the reference implementation |
| `odd/research/feature*`, `softonic-build-comparison.md` | feature inventory and the repack analysis |