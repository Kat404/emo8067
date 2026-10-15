# Session handoff — emo8067

Written 2026-10-15 at the end of a long session. Everything below is a claim with a source; anything marked **UNRESOLVED** is a real open item, not a reminder.

## What the project is

`emo8067` — a clean-room, free-software Intel 8086/8088 emulator, assembler and debugger in Rust, intended for Linux/macOS/Windows. Licence: The Unlicense. Pure-Rust core with no dependencies and `#![forbid(unsafe_code)]`.

**Clean-room rule:** reverse engineering establishes *observable behaviour and specifications* only. No code is ever copied or transliterated from another emulator. When this project's own research and the silicon disagree, **silicon wins**, and the manual's own print is preserved and marked as an erratum.

## Where the tree stands

```
cargo test --workspace   0 + 5 + 115 + 3 + 4 + 28 + 109 + 0 passed   (264)
cargo clippy             0 warnings, 0 errors
cargo fmt --check        exit 0
python3 odd/research/verify_opcodes.py   OK: 16 rows x 16 cells
HEAD                     086c3bf   (signed; every commit in this session is GPG-signed)
```

Two gates, both live, both proved to fail when broken:

| Gate | Covers |
|---|---|
| `every_first_opcode_byte_the_8086_defines_decodes` | all 256 first opcode bytes, `probed == 256` literal, no exclusions |
| `every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole` | 96 `(opcode, /digit)` pairs, `96`, `5` and `91` as literals, cross-checked in both directions |
| `the_word_register_field_names_the_register_intel_table_4_9_says` | all eight `B8`..`BF` encodings, plus the same field in a ModR/M byte. Written against instruction bytes, not the API, because the API is where the encoding mistake hid |
| `every_encoding_consumes_exactly_what_the_manual_says` | 56 encodings that carry an immediate, each with the length the manual's operand format implies, stated by hand. Plain instructions by where `IP` lands, relative jumps by their displacement, `INT`/far `CALL` by the pushed return address, `RET` by how far the stack moved. Phase 1 of `odd/tasks/decode-geometry-gate.md` |
| `every_flag_the_guide_says_is_untouched_really_is` | one representative instruction per §1.4 row, run with all nine flags set and again cleared; asserts `—` stays, `0` clears, `1` sets. Expectations read out of the document at compile time, so code and oracle cannot drift apart silently. Found the `LAHF`/`SAHF` transposition on its first run |

Plus `python3 odd/research/verify_opcodes.py`, which guards the transcription of the manual into `documentation-guide.md` §1.3 and §1.4: exactly sixteen cells per opcode row, every flag cell inside the Table 2-18 legend. Verified by mutation, not just by passing. **It cannot check whether a cell says the right thing** — see "A second cause" below.

## Completed units

`U1` opcode map re-transcribed · `F10` flag-contract gate, and the `LAHF`/`SAHF` transposition it found · `U2` reject 80186-only encodings · `U3` repair the shipped core (6 defects) · `U4` group 1 on memory · `U5` segment and stack group · `U6` the remaining opcodes · `U7` segment override prefixes · `AUDIT-1` word wrap · `AUDIT-3` `8F` mod · `AUDIT-5` prefix recursion · `AUDIT-7` `BP` → `SS` · `F5` `REP` as a latch · `F1` string-source segment override · `F8` `REP` negates `MUL`/`IMUL`/`IDIV` · `F9` `A8`/`A9` are `TEST` · `F4` four research briefs marked superseded.

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
- **Intel errata for `A8`/`A9`.** None exists — Intel published no errata for the 8086/8088 family, so there was never an erratum to file. **The question is now settled against us, by silicon.** `SingleStepTests/8086` (`v1/metadata.json`), captured on a real **Intel P80C86A-2 782003 J20 (C)1978**, records `A8`/`A9` with `AF` and only `AF` undefined — the signature of an `AND` — while `3C`/`3D`, the documented `CMP AL/AX, imm`, are recorded with no undefined flags at all. The 8088 capture agrees. That matches the 1979 manual, the 1983 ASM-86 manual, and Jenner's routine table, where `alu r,i` is full at sixteen and a dedicated two-opcode `TEST A,i` exists. **This project ran `A8`/`A9` as `CMP` for two units and was wrong**; see item 3b.
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

What did survive is the one link the `A8`/`A9` decision needed: Jenner's **public** routine table lists `alu r,i 16` and `TEST A,i 2`. Eight ALU operations times two widths is exactly sixteen, and `A8`/`A9` are the only accumulator-against-immediate pair left over, so the manual and the microcode agree and this project was the divergent one. The chain then went to an independent refutation, which could not break it and turned up hardware captures that settle it outright. See item 3b.

### 3b. `A8`/`A9` are `TEST`, decided by silicon, after this project ran them as `CMP` for two units

`SingleStepTests/8086` is single-step captures taken on a real **Intel P80C86A-2 782003 J20 (C)1978**, and its `v1/metadata.json` records which flags each opcode leaves undefined. It records `A8`/`A9` with `"flags": ".....a.."`, `AF` and nothing else — the signature of an `AND` — and `3C`/`3D`, the documented `CMP AL/AX, imm`, with **no** undefined flags, because a subtraction defines all six. The 8088 captures (Harris 80C88) agree. I fetched that file and read those entries directly.

So the 1979 manual printed the truth, the 1983 ASM-86 manual printed the truth, the microcode agrees, and **the only thing that ever said otherwise was a source this project adopted without checking.** `cmp_accumulator` is now `test_accumulator`: CF and OF cleared, `AF` left alone, and nothing written to `AL`/`AX`.

Two things make this the most instructive defect in the file. First, it is the same shape as the REP negation — a true claim dismissed by reasoning where a citation was required — and it happened **twice in one session**, in opposite directions. Second, this project's own record says a reviewer once reported `A8`/`A9` as `TEST` and the parent filed it as false; that reviewer was right, and the ground rule that every external finding is a hypothesis to reproduce had already been violated when the rejection was not reproduced, only argued.

The generalisable lesson: when two projects of evidence disagree and one of them is a named individual's measurement on real silicon, the measurement wins, no matter how well the other side is argued.

The lesson matches every other one in this file. The report was *more* confident than the sources allowed, and the two places to check a research output are its arithmetic and its citations — both of which were checkable in under a minute.

### 5. `odd/research/_prompt-audit7-and-friends.md` is the single most valuable document here

A research prompt whose answers closed **four** audit items, two of which reversed decisions the parent had already committed. Reuse it as the template for any future architecture question to another model: it demanded a source hierarchy, asked claims to be labelled by how they were verified, and explicitly made *"not decidable from available sources"* a valid answer. That framing is why it found two things the parent and two verifiers had both missed.

## Eight defects, one cause — and a second cause that arrived with them

Every architecture bug this project shipped came from the same place — **reasoning where a citation was required**:

| Defect | What was assumed |
|---|---|
| word wrap | the 8086 carries from the offset into the segment base |
| `PUSH`/`POP` following `DF` | if string operations move one way, the stack does too |
| `LEA` width | `8D` is odd, so bit 0 means byte |
| `CA`/`CB` immediate | the pairing was swapped |
| `BP` default segment | one default segment for every base |
| string-source segment override | a prefix that reaches a memory operand reaches every one of them |
| `LAHF`/`SAHF` transposed | `9E` and `9F` are adjacent, so swapping them changes nothing structurally — and two unit tests asserted the wrong opcode's behaviour, so code and test agreed |
| the 16-bit REG field | `Reg16`'s discriminants run in the order `words` wants, not the order Intel assigns the field, and `from_bits` was the identity on them. Five of eight registers were named wrong, and **twenty-one tests encoded the wrong table**, so the wrong answer was confirmed from both sides |

That last one is the shape worth remembering. It is not a slip in one place: a project's tests are usually written in the same vocabulary as its code, so when a shared assumption is wrong, the tests do not merely fail to catch it — they **assert it**. Twenty-one tests agreed with each other and with the emulator about a register encoding that no 8086 has ever used. Only running a whole program, whose bytes came from a table rather than from the API, could break that agreement.
| `REP` negates `MUL`/`IMUL`/`IDIV` | dismissed, because "the multiply reads no flags" conflated FLAGS with the internal `F1` latch |
| `A8`/`A9` | the same claim in reverse: a source said silicon runs `CMP` and that was adopted with no citation |

Not one was found by the test suite. Every one was found by a verifier, a microcode disassembly, a hardware capture, or a sourced research answer. **The green suite proves the tests pass, not that the emulator is right.**

The last two are the same mistake pointing opposite ways, in one session: a true claim rejected, then a false claim accepted. Both times the missing ingredient was a citation, and both times the deciding evidence was a **named individual's measurement on real silicon** that nobody had looked for.

## A second cause: the oracle was allowed to disagree with the code

`A8`/`A9` changed behaviour, the footnote changed, and the §1.3 opcode grid did not. Independent verification caught it: the grid is the project's declared oracle for everything built on top of it, so the next unit built against it would have reintroduced `CMP`.

**A behaviour change and the document that is this project's oracle for that behaviour are one unit of work.** A green suite cannot catch this, because the grid is not code: `verify_opcodes.py` checks that every row has sixteen cells and every flag cell is inside the legend, and its own docstring says it cannot tell whether a cell says the *right* thing. A transcription gate can only ever check structure.

Three ways to stop it, in increasing order of cost and decreasing order of independence:

1. **Cheapest, and worth doing now: make the cell and the behaviour one commit.** Every mnemonic change edits the §1.3 cell in the same commit. It is a convention, not a check, and conventions are what this project has already broken once.
2. **DONE — a semantic gate over the flag table.** `crates/core/tests/flag_contract.rs` reads the §1.4 rows out of the document and runs one representative instruction per row, asserting that a row marked untouched really is untouched, that `0` clears and `1` sets. Each case runs twice, all flags set and all cleared, so a write in either direction is caught. **It paid for itself on its first run: it found that `0x9E` and `0x9F` were swapped, so every `LAHF` and `SAHF` in every program was inverted, and 228 tests had missed it because the two unit tests covering those opcodes asserted the wrong opcode's behaviour.** Proved by mutation: three mutants killed (LAHF writing a flag, CLC no longer clearing CF, INC writing CF).
3. **NOT DONE, and it is a decision rather than a task: gate against silicon.** `SingleStepTests/8086` ships per-opcode captures from a real P80C86A-2 and a per-opcode `metadata.json` naming the flags each opcode leaves undefined. A gate can assert, for every opcode, that the emulator's undefined-flag set matches the hardware's — one JSON file, 256 opcodes, no hardware, and no transcription in the loop. This also closes several questions in "Still open" below, because the same data settles flag semantics rather than just naming them. It makes §1.3 a *derived* document rather than an authority, which changes how this project works. **Deliberately not started**, because option 2 already covers the class of defect that bit us and option 3 has to be weighed against losing the manual as an independent second witness.

## Three tooling hazards

1. **Do not splice Rust source with a Python script that counts braces.** Three attempts in this repo corrupted the file; one deleted an entire function body and left a signature with no parameter list. Use the `edit` tool, or a single targeted replacement whose `old` text was just read from disk, and run `cargo build` immediately.
2. **`edit` silently reverts six pre-existing lines** in `crates/core/src/exec.rs` to pre-rustfmt style. Run `cargo fmt` before every commit and confirm `cargo fmt --check` exits 0.
3. **Never `git checkout` a file a delegated agent just wrote but has not committed.** It destroyed a unit once; recovery meant resuming the agent's session. The agent now commits in stages, and each stage leaves the tree green.

## How this project got its findings

Every external finding arrived as a hypothesis to reproduce, never an instruction to execute — and each reviewer has reported at least one false claim:

- `A8`/`A9` reported as `TEST` — dismissed as **false**, on the reasoning that a later source said silicon runs `CMP`. **That dismissal was itself the error**: the source had no citation, and a real P80C86A-2 disagrees with it. Applied in the end, after silicon confirmed it.
- My own U5 brief's `LAHF`/`SAHF` formulas — **false**. The implementer deviated deliberately and was right.
- My own F5 brief's `CX = 0` and `F3`-negates-`IDIV` — **both false**, caught by the implementer and the verifier.

Subagent discipline that worked: brief by file path, allowed edit surfaces enumerated, atomic commits with the gate in the same commit, and "report defects outside your scope, do not fix them".

## Still open, and what each one would take

| Question | State | What would close it |
|---|---|---|
| `8F` with `mod=11` | refused by explicit policy, recorded as OPEN | the structural argument says register `POP`; no source states it. The hardware captures record `8F` by `/digit` only, so they do not cover `mod`. A `mod=11` case in `SingleStepTests` would settle it |
| `8F` with `reg != 0` | refused | **settled in our favour**: the captures record `8F` `/1`–`/7` as `"status": "undefined"`, so refusing matches hardware rather than contradicting it |
| Does unsigned `DIV` (`/6`) negate under `REP`? | left alone as a policy | the captures do not cover prefixes. Needs a `F3 F6 /6` case, or the `8086_microcode_v2.zip` listing |
| Is the microcode ROM identical across 8086 steppings? | not decidable | needs a 1978 Stepping 1 die, which is a collector item. Affects all five microcode findings if the ROM changed |
| `SHL`/`SHR`: does silicon update S, Z, P or preserve them? | code follows the manual's prose | the `metadata.json` cannot tell — it records unreliability, not change. The per-opcode capture files carry initial *and* final state and would settle it |
| `AAA`/`DAA` against the manual's pseudocode | **open, and possibly wrong** | `aaa` masks `AL` to its low nibble after the `+ 6` adjustment. Intel's pseudocode has no such mask: the second rule then tests `AL > 99h` on the *unmasked* value, which the mask destroys. Nothing here has checked it, and no end-to-end program covers it because the expected value could not be stated without guessing. Settling it needs the manual's `AAA` and `DAA` text |

## Next session, in order

1. ~~Fix the string-source segment override (item 1)~~ — done in `fe16b6c`; its replacement permutation test had to be written from scratch, because the F5 verification report it referred to is in neither memory nor the repo.
2. ~~Decide item 2 with a primary source~~ — done; microcode settles `CX = 0`, and it is not an erratum.
3. ~~Correct the five stale briefs (item 4)~~ — done; four files carry banners, and a sixth document turned out to be wrong.
4. **Decide the oracle question** in "A second cause" above — the cheap semantic gate first, and whether §1.3 becomes a derived document gated on the hardware captures. It is a decision before it is a task, and it changes what the next unit is allowed to trust.
5. **Keep growing the program corpus.** `crates/core/tests/programs.rs` has twenty-eight programs and found the REG-field defect on its first execution, which is the highest yield per line of anything in this repo. Batches two and three added: the direction flag moving pointers backwards, `INT`/`IRET`, a far `CALL`/`RET`, a signed `XLAT` index, writing a segment register and indexing through it, `LOOP`'s counting rule, `CBW`/`CWD`, an overflowing byte multiply, a shift loop, an XOR checksum, a program reading its own instruction stream, `REP` with a zero count, nested `CALL`s, self-modifying code written through `DS` and executed through `CS`, two segments at one offset, a linear search in both outcomes, and `REPE CMPSB` over two buffers. **Two rules for every new program.** Mutate the exact rule its name claims — one of the twenty-eight proved nothing until a mutation survived it. And never write an expected value from memory: the packed-BCD program was cut rather than guessed, which is how the `AAA` masking question in "Still open" was found. Hand-assembly errors so far, all caught because the emulator disagreed: `BE` for `SI`, `BB` for `BX`, `46` for `INC SI`, `5E` for `POP SI`, `disp16` byte order, `89 C6` read as a memory store when `mod=11` makes it a register move, and a program one byte short.
6. **`T13`, the assembler.** The one large piece of construction left, and it gates `T12`. The corpus now makes its value concrete: twenty-one tests had to be hand-re-encoded against Intel's table, which is exactly the work an assembler does once and for all.
7. **`T12`**: the headless runner already exists at `crates/cli` (binary `emo8067`, with `--trace` and `--dump`). Note that `re/` currently contains **no** `.asm` files — the 86 examples are not in the tree, so this needs either locating them in the extracted reference build or writing them.
8. Only then the packaging and UI decisions listed in `documentation-guide.md`'s open questions (T6 and T7), which cannot be settled while the pieces they depend on do not exist.

## Files worth reading first

| Path | What |
|---|---|
| `odd/tasks/emu8067-foss.md` | specs, every decision, the U1..U8 plan, audit items, and the log |
| `odd/tooling.md` | environment facts: Ghidra paths, what `analyzeHeadless` needs, `innoextract` |
| `odd/research/_prompt-audit7-and-friends.md` | the research prompt template |
| `odd/research/_prompt-microcode-closure.md` | the second use of that template; its answer over-claimed, see item 4a |
| `SingleStepTests/8086`, `v1/metadata.json` | **not in the repo, and the most valuable source this project has not been using.** Single-step captures from a real Intel P80C86A-2, recording per opcode which flags end up undefined. Free, no hardware, and it settles flag questions a transcription cannot. See "A second cause" above |
| `re/FINDINGS.md` | the behavioural contract recovered from the reference implementation |
| `odd/research/feature*`, `softonic-build-comparison.md` | feature inventory and the repack analysis |