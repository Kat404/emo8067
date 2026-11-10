# Session handoff — emo8067

Written 2026-10-15 at the end of a long session, and kept current since: the
2026-10-19 work is folded into it, and the 2026-10-23 silicon and full-capture
units with it, and the 2026-11-10 unit that closed the three families the
full-capture gate left.
Everything below is a claim with a source; anything marked **UNRESOLVED** is
a real open item, not a reminder.

## Where the 2026-11-10 work is

The three families the full gate left are **closed**, and the gate over every
capture is green with **no disagreement at all**:

- **The tens correction of `DAA` and `DAS`** — the chip applies it when the
  carry was set, or when the old `AL` was over `99h` **and** the `AF` flag
  was clear or the old `AL` was over `9Fh`. The 4000 `DAA`/`DAS` captures
  cover every one of the 256 values of `AL` with the flag set and with it
  clear, and the rule fits all 4000 of them with no mismatch. The manual's
  prose is looser than the chip, and the chip wins.
- **`AAD` with a zero immediate** — the chip does **not** fault: it computes
  the result, leaves `AH` clear and the low byte untouched, and clears `CF`
  and `AF`. `AAM` with a zero immediate still faults, as the manual says.
- **`IDIV r/m8`** — the **magnitude** of the quotient has to fit in seven
  bits, so a quotient of `-128h` takes the type 0 interrupt. The guide's print ("does not fit in
  the destination") is an erratum for the byte form, and the guide now says
  so. **The word form is not covered by any capture**: no capture in the set
  has a dividend that can produce a quotient of `-32768h`, so `IDIV r/m16`
  still follows the print.

The gate's three family pins are at **zero**, so a family that grows again
fails the gate the way the count it had did. The push is still the
maintainer's decision.

## Where the 2026-11-10 assembler work is

The debt the assembler plan left is closed, and the length table has the
three families it was missing:

- **The glued segment override** (`cs:mov ax,1`) keeps its prefix. It was
  worse than refused: the single-line assembler dropped the override
  silently, so the program read `DS:` where it wrote `CS:`.
- **`PUSHA` and `POPA`** are the eight-word sequences the reference's own
  documentation gives. They were seven words each way, so the `SP` was
  never on the stack, and a `POPA` on its own left the stack two bytes
  off.
- **The second `LOCK`** needed no code: the reference's own binary carries
  the string `LOCK-OPCODE:F0`, so its assembler writes `F0h` twice over,
  exactly as this one does.
- **The register initialisers** (`#cs=1234#` and the rest) are read, and
  the program starts where it says. Three of the reference's own examples
  declare one, and they were running in the wrong state.
- **The length table** has `8F`, `C4`/`C5` and `FF /2`–`FF /5`: fourteen
  rows, 191 encodings in all.

## Where the 2026-10-23 work is

Two units landed, both routed through a worker and an independent verifier,
and both decided by the captures of a real P80C86A-2:

- **The two silicon questions** — `odd/tasks/silicon-questions.md`, closed in
  `49b9d27`. The `8C`/`8E` forms with a displacement, which the chip runs as
  ordinary word-memory forms, and the carry of a rotate by more than one,
  which the chip takes from the edge of the final result. The oracle grew to
  2689 cases and the known-divergence list is empty.
- **The gate over every capture** — `odd/tasks/full-capture-gate.md`, closed in
  `f3a216f`. The gate compares the chip's **memory** as well as its registers
  and flags, and a research gate runs all 646 000 captures. It found
  `PUSH SP` pushing the wrong value — the chip pushes the pointer it had
  already decremented — and it left **36** disagreements in three families,
  which are in "Still open" below with the chip's rule for each.

The verification was in two rounds and it earned its keep: the first round
found a family that was misclassified and a silicon rule that was simply
false, both written with total confidence by the implementer.

**The four commits of 2026-10-23 are not pushed.** `main` is four commits
ahead of `origin/main`, and the push is the maintainer's decision.

**The next unit is not chosen.** The open work is in "Still open" below and in
the two plans above.

## What the project is

`emo8067` — a clean-room, free-software Intel 8086/8088 emulator, assembler and debugger in Rust, intended for Linux/macOS/Windows. Licence: The Unlicense. Pure-Rust core with no dependencies and `#![forbid(unsafe_code)]`.

**Clean-room rule:** reverse engineering establishes *observable behaviour and specifications* only. No code is ever copied or transliterated from another emulator. When this project's own research and the silicon disagree, **silicon wins**, and the manual's own print is preserved and marked as an erratum.

## Where the tree stands

```
cargo test --workspace                  438 passed, 0 failed
cargo clippy --workspace --all-targets   0 warnings, 0 errors
cargo fmt --all --check                 exit 0
python3 odd/research/verify_opcodes.py   OK: 16 rows x 16 cells
the examples' gate      85 of 85 assemble; the runner's gate pins the state
                         each of the 85 leaves behind
HEAD                     d40dc28 (docs), f3a216f (the gate and PUSH SP),
                         25cef28 (docs), 49b9d27 (the rotate and the 8C/8E);
                         every commit is GPG-signed and none is pushed
```

Eight gates. Seven run in the everyday tree; the eighth is the research gate
over every capture, and it stands aside when its generated oracle is absent.
The first two are the ones that were proved to fail when broken:

| Gate | Covers |
|---|---|
| `every_first_opcode_byte_the_8086_defines_decodes` | all 256 first opcode bytes, `probed == 256` literal, no exclusions |
| `every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole` | 96 `(opcode, /digit)` pairs, `96`, `5` and `91` as literals, cross-checked in both directions |
| `the_word_register_field_names_the_register_intel_table_4_9_says` | all eight `B8`..`BF` encodings, plus the same field in a ModR/M byte. Written against instruction bytes, not the API, because the API is where the encoding mistake hid |
| `every_encoding_consumes_exactly_what_the_manual_says` | 177 encodings: 55 that carry an immediate and 122 that carry a displacement, each with the length the manual's operand format implies, stated by hand. Plain instructions by where `IP` lands, relative jumps by their displacement, `INT`/far `CALL` by the pushed return address, `RET` by how far the stack moved. Phase 1 of `odd/tasks/decode-geometry-gate.md` |
| `an_instruction_touches_only_what_it_is_documented_to_touch` | one instruction of every family, in `crates/core/tests/touched.rs`: the registers it writes, the memory it writes, and every other register, every other byte of the megabyte and every flag the guide marks `—` checked not to have changed. The flags it may change are read out of §1.4, not typed by hand |
| `the_flags_come_out_the_way_a_real_8086_leaves_them` | 2689 cases from 245 opcodes, out of `SingleStepTests/8086`, a real Intel P80C86A-2: the machine state, the instruction, the memory the chip left, and the flags the chip left. Every address in the oracle's `finalmem` line is compared, so both the bytes the chip wrote and the bytes it left alone have to hold the chip's values. The one exception is the undefined flags word a divide fault pushes. The flags the chip leaves undefined are masked out of both sides, because "undefined" means the chip left whatever was there; the flags of a divide fault are not compared — the cause is the divide, not the interrupt — while `INT 0` keeps its defined flags and is compared. The count is pinned exactly, and the known-divergence list is empty: no case in this committed sample disagrees with the emulator |
| `every_capture_comes_out_the_way_the_chip_leaves_it` | every one of the 646000 captures, from the generated 214 MB `capture_oracle_full.txt`: the memory, the registers and the flags. It is a research gate, not a unit test: it stands aside when the generated file is absent, so `cargo test --workspace` never depends on a file outside the repository. The 36 disagreements are pinned in three families, and a divergence outside those families fails the gate |
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
3. **DONE, and it is phase 2 of the plan: gate against silicon.** `SingleStepTests/8086` ships per-opcode captures from a real P80C86A-2 and a per-opcode `metadata.json` naming the flags each opcode leaves undefined. The gate is `crates/core/tests/hardware_flags.rs`: it takes each opcode's undefined-flag mask out of `metadata.json`, runs the case, compares the flags the chip defined, and checks the document in the other direction too - a §1.4 row marked `—` must be a flag the chip left alone. 2689 cases from 245 opcodes, the count pinned exactly, the known-divergence list empty. **What was deliberately not done is the last part of the original claim:** §1.3 is still an authority and not a derived document, because the manual stays the second witness and the gate checks it instead of replacing it. That is the weighing the decision was about, and it landed on keeping the manual. `odd/tasks/hardware-capture-oracle.md`, T6.

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
| `8F` with `mod=11` | **CLOSED — the chip pops into the register** | The captures name the forms "pop dx", "pop si", "pop bp" and "pop bx", and they pop with every `/digit`. The refusal and the `/digit` check are both withdrawn |
| `8F` with `reg != 0` | refused | **settled in our favour**: the captures record `8F` `/1`–`/7` as `"status": "undefined"`, so refusing matches hardware rather than contradicting it |
| Does unsigned `DIV` (`/6`) negate under `REP`? | left alone as a policy | the captures do not cover prefixes. Needs a `F3 F6 /6` case, or the `8086_microcode_v2.zip` listing |
| Is the microcode ROM identical across 8086 steppings? | not decidable | needs a 1978 Stepping 1 die, which is a collector item. Affects all five microcode findings if the ROM changed |
| `SHL`/`SHR`: does silicon update S, Z, P or preserve them? | **CLOSED — silicon updates them** | The captures show the shifts writing the arithmetic flags from the result, and the project had already believed the manual's prose over the table. The captures' `metadata.json` records the *undefined* flags, and the per-opcode files carry the initial and final state, which is what settled it |
| `AAA`/`DAA` against the manual's pseudocode | **CLOSED — the core was wrong twice** | The manual (Oct 1979, page 2-35) says `AAA` zeroes the high-order half-byte of `AL`, and Intel's SDM (Vol 2A, page 3-19) writes the mask as its own step, outside the branch. The core masked only inside the branch, and `DAA`/`DAS` never wrote `AF` at all, although Table 2-21 gives both an `X` in that column. Both fixed in `8dcc9da`, with the mutants shown to fail |
| Does `DAA`'s second rule test the old `AL` or the adjusted one? | **CLOSED — the chip says the old one** | The hardware settles it: `DAA` on `AL = FAh` leaves `AL = 60h` with the carry set, which is the old value testing true. Intel's SDM writes the same (`IF ((old_AL > 99H) or (old_CF = 1))`). The 1979 manual has no rule text and the classic pseudocode says the adjusted value; the captures in `crates/core/tests/hardware_flags.rs` decide it |
| `PUSH SP` | **CLOSED — the chip pushes the stack pointer it had already decremented** | The 1979 manual says to push the pointer before the push; the P80C86A-2 pushes the decremented pointer. `push sp` with `SS:SP = A291:6068` leaves `6066h` at physical `A8976h`. The print is an erratum against the silicon. Fixed in `f3a216f`, in both the `50h`-`57h` dispatch and the `FF /6`/`FF /7` push |
| `DAA`/`DAS` tens correction | **CLOSED — the chip's rule, and it is stricter than the manual's** | The chip applies the tens correction when the carry was set, or when the old `AL` was over `99h` **and** the `AF` flag was clear or the old `AL` was over `9Fh`. The 4000 `DAA`/`DAS` captures cover every one of the 256 values of `AL` with the flag set and with it clear, and this rule fits all 4000 with no mismatch. `daa()` in `crates/core/src/exec.rs` runs it, and the family pin is at zero |
| `AAD` with a zero immediate | **CLOSED — the chip does not fault** | It computes the result, leaves `AH` clear and the low byte untouched, and clears `CF` and `AF`. `AAM` with a zero immediate still faults, as the manual says. `aad()` in `crates/core/src/exec.rs` runs it, and the family pin is at zero |
| `IDIV r/m8` | **CLOSED — the magnitude of the quotient has to fit in seven bits** | A quotient of `-128h` takes the type 0 interrupt, although it fits in the signed byte the quotient goes to. The guide's "does not fit in the destination" is an erratum for the byte form. **The word form is unproven**: no capture has a dividend that can produce `-32768h`, so `IDIV r/m16` still follows the print |
| The 16-bit `REG` field | **CLOSED — the 80186's order had been implemented** | The 8086's Table 4-9 gives `AX CX DX BX SP BP SI DI`; the 80186 moved the 16-bit registers to make room for `AH CH DH BH` in the 8-bit field, and every modern reference quotes that later table. The 1979 manual's own table and a real P80C86A-2 agree with each other: `MOV SP,AX` is `8B E0`. The core, the guide's note and every hand-assembled program had followed the later order. Fixed in `98fc6f3`, with thirty-eight hand-assembled tests re-encoded |
| `8C` with a register destination | **CLOSED — it is an 8086 instruction** | The project refused it as an 80186 encoding, on the strength of the manual printing `(not used)`. The chip runs it: `8C DF` is `MOV DI,DS`, with the segment landing in the register. The refusal is withdrawn, and so is the guide's claim that `8C` and `8E` take `MOD=000` only — the chip runs all four `MOD` values for both |
| `8C`/`8E` with a displacement | **CLOSED — the chip runs both forms** | The manual prints `(not used)` and §1.3's note says so; the core executes every `MOD`. The captures have `8C` and `8E`, and the set's README says the `reg` field's high bit is ignored — the `mod` field is another question, and the hardware can answer it. **It answered in `49b9d27`:** the P80C86A-2 runs `8C` and `8E` with `MOD=01` and `MOD=10` as ordinary word-memory forms. `8C` writes the selected segment as a little-endian word at the effective address, `8E` reads that word into the selected segment, the `reg` field's high bit is ignored, no flag changes, and the displacement is consumed. The 1979 print's `(not used)` is an erratum for those two forms. `odd/tasks/silicon-questions.md` |
| `8C`/`8E` with a displacement | **CLOSED — and the core was right to run them** | The manual prints `(not used)` for `8C MOD 1--` and `8E MOD 1--`, and §1.3's note says so; the core executes every `MOD`. Refusing them would be a change on a document's print rather than on evidence about the chip, and `MOV DS,AX` is a bread-and-butter 8086 instruction. The microcode's `MOV Sreg` entry would say whether the address unit computes an effective address for them. **The captures answered it instead, in `49b9d27`: the chip runs them, the core already did, and the print is the erratum** — see `odd/tasks/silicon-questions.md` |
| The length table's missing families | **CLOSED — fourteen rows, 191 encodings** | `8F` (`POP r/m16`), `C4`/`C5` (`LES`/`LDS`, which read a far pointer through the R/M field) and `FF /2`–`FF /5` (the indirect calls and jumps) are in the gate's table now. The indirect ones needed what the gate did not have: their length is not in `IP`, which is where they went — the near call is measured through the return address it pushes, and the jumps through the address the pointer was read from. `9e2252b` |
| The flag rows marked `X` and `U` | **CLOSED — the captures are the oracle now** | `X` is value-dependent and `U` is undefined, so the flag contract asserts nothing for them: for `ADD`, `ADC`, `SUB`, `SBB`, `CMP`, `DAA`, `DAS`, `CMPS` and `SCAS` it proves only that `D`, `I` and `T` are untouched. Two of the eight defects from the last session lived in that hole. The touches gate claims which flags *may* change, not what they become. Closing it means asserting values, which means the operand values, which means the gate has to become a specification rather than a consistency check. **That is what phase 2 became:** the oracle gate asserts the chip's own flag values for every opcode it covers, and checks the document in the other direction. `odd/tasks/hardware-capture-oracle.md`, T6 |

## The hardware oracle

`SingleStepTests/8086` — Daniel Balsom's hardware-generated suite for the
Intel P80C86A-2 — is now the project's oracle for what a flag *becomes*. It
is 160 MB of gzipped JSON, so it is not vendored: a script in
`odd/research/extract_capture_oracle.py` takes it apart into text files.
The committed oracle, `odd/research/capture_oracle.txt`, holds **2689** cases
from **245** opcodes, and `crates/core/tests/hardware_flags.rs` runs that
sample against the emulator. The full capture set is a separate generated
oracle, `odd/research/capture_oracle_full.txt`, with **646000** cases:
`python3 odd/research/extract_capture_oracle.py --all` writes it, it is
**214 MB**, and it is gitignored rather than committed.
`crates/core/tests/hardware_full.rs` is the research gate over it. It
stands aside when the generated file is absent, so the everyday test run
never depends on a file outside the repository. The first slice was the
eight ALU groups: 400 cases, 58 opcodes.

By the end of the silicon unit it had already paid for itself seven times,
and every one of them by disagreeing with this project:

- the 16-bit `REG` field was the 80186's order, not the 8086's;
- `DAA`'s second rule tests the old `AL`, not the adjusted one;
- `8C` with a register destination is an 8086 instruction;
- `INC`/`DEC` overflowed on the wrong side, and so did the rotates;
- `POP r/m16` pops into a register, and ignores its `/digit`;
- `XLAT` indexes with an unsigned `AL`;
- `C6`/`C7` ignore their `/digit`.

Ten more came out of closing the two families that were open: the shift
group's overflow, the count of `CL` (the 8086 does not mask it to five
bits, which is what the later processors do), and the I/O bus (a port is
not a memory address; with nothing attached to it a read returns all
ones). **Six of the 1533 cases still disagree, all of them the carry of a
rotate by more than one, and all of them unexplained — that was the state
at `6e28bc8`, and it is wrong now.** The committed sample holds 2689 cases
from 245 opcodes instead of 1533, and **no case in that sample disagrees
with the emulator any more**: the six cases are explained, and the chip's rule is
the one the emulator runs. A rotate by more than one takes its carry from
the edge of the **final result** — `ROL` from bit 0, `ROR` from bit 15,
and `RCL`/`RCR` carry on through the extra bit — and its overflow is the
sign change of the final one-bit step. The old "the bit that left" rule is
wrong in 516 of 30687 rotate cases. A count of zero is a complete no-op.
The suspicion that the extraction writes the wrong memory for those cases
was tried, and **the audit is dead**: an audit of the effective address
against the chip's own bus cycles is wrong itself, since it disagrees with
the chip in the files that pass the gate too. **The stronger conclusion
that the extraction's memory agrees with the chip's own RAM list was
wrong, though.** The old recovery read a byte at an odd address from the
low half of the sixteen-lane bus field and put it one address too high,
and it recovered a write-only byte as if it had been read. A write carries
the value the chip wrote, not the value that was there before it, so that
byte must not be handed to the emulator as the answer. The corrected
recovery matches every capture's own `ram` list; against it, the old
recovery differed in **118778** of the **646000** initial-memory cases and
in **55796** of the final-memory ones.
`odd/research/test_extraction_contract.py` pins both rules. The plan is
`odd/tasks/full-capture-gate.md`.

**The other question the same captures closed.** The P80C86A-2 runs `8C` and
`8E` with `MOD=01` and `MOD=10` as ordinary word-memory forms: `8C` writes
the selected segment as a little-endian word at the effective address,
`8E` reads that word into the selected segment, the `reg` field's high bit
is ignored, no flag changes, and the displacement is consumed. The 1979
manual's `(not used)` print for those two forms is an erratum. The plan
is `odd/tasks/silicon-questions.md`.

The `X` and `U` rows of §1.4 are no longer a hole. The oracle gate
compares every flag the chip defined, and it checks the document in the
other direction too: a row that marks a flag with an em dash says the
instruction does not touch it, and the captures say whether the chip
agreed. That check passes on all 2689 cases, so every flag the document
calls untouched is one the chip left alone.

`odd/tasks/hardware-capture-oracle.md` is the plan for the committed sample:
T5 covers the whole opcode space, T6 makes the captures the flag oracle and
checks the guide against them, and T7 closes the rotate carry. The gate
pins the sample's count exactly and names no divergence.
`odd/tasks/full-capture-gate.md` is its companion: it owns the final-memory
comparison, the research gate over all **646000** captures, and the findings
that gate leaves. **The full set agrees with the emulator in every case**: the
36 disagreements the three families held are closed, and a divergence outside
those families still fails the research gate.

## The I/O bus

A port is not a memory address. The 8086 has a separate I/O bus, and the
reference implementation made it a 64 KB file at `C:\emu8086.io` with byte N
of the file behind port N, so reaching a device meant a writable file at the
root of the drive. The core now has the bus in the process: a `Device` trait,
a bus that hands each port to the device that owns it, and a shared handle so
a device can be watched from outside the emulator while it runs. A port
nobody owns reads as all ones and swallows writes, which is what the bare
chip does and what the captures confirm.

The three devices with a documented port map are in: traffic lights on 4,
stepper motor on 7, robot on 9/10/11, with their protocols in
`re/docs-txt/io.txt`. The bus also speaks a framed protocol over a unix
socket, so a device can live in another process: the CLI takes
`--device-socket PATH`, and the frame is `type(1) port(2) value(1)` with a
one-byte reply. The server is served between the emulator's instructions. A
`SPACE` frame returns the flat 64 KB view of the port space, the way the
old file showed it — but it is a view and not the bus's storage: the chip
wins, so a device that writes a port byte does not reach the program.

## Decisions of 2026-10-19, and what they settle

| Question | Decision | Why it matters |
|---|---|---|
| What is the oracle for the flags? | **The manual first; the hardware captures come in phase 2** | The captures record flags and not operand formats, so they cannot close the geometry half of the phase. `documentation-guide.md` §1.4 stays the oracle until phase 2 says otherwise |
| What happens to the 86 example programs? | **Local gold, never committed** | They live under `re/extracted*/`, which is gitignored because it is derived from a proprietary binary. Running them is allowed; versioning them is not |
| `8F` with `mod=11` | **CLOSED — the chip pops into the register** | The captures name the forms "pop dx", "pop si", "pop bp" and "pop bx", and they pop with every `/digit`. The refusal and the `/digit` check are both withdrawn |
| `AAA`/`DAA` against the manual | **The core is corrected, not the manual** | Two defects: the mask was conditional and `AF` was never written. The manual's page 2-35 and the SDM's Volume 2A agree, and the SDM's `AL := AL AND 0FH` sits outside the branch |

## Next session, in order

1. ~~Fix the string-source segment override (item 1)~~ — done in `fe16b6c`; its replacement permutation test had to be written from scratch, because the F5 verification report it referred to is in neither memory nor the repo.
2. ~~Decide item 2 with a primary source~~ — done; microcode settles `CX = 0`, and it is not an erratum.
3. ~~Correct the five stale briefs (item 4)~~ — done; four files carry banners, and a sixth document turned out to be wrong.
4. ~~Decide the oracle question~~ — decided 2026-10-19: the manual stays the oracle and the hardware captures come in phase 2. See "Decisions of 2026-10-19" above. ~~**Phase 2 is now the next unit of the plan**~~ — **phase 2 is done**: the captures are the oracle now (see "The hardware oracle" below), and the plan is `odd/tasks/hardware-capture-oracle.md`.
5. **Keep growing the program corpus.** `crates/core/tests/programs.rs` has thirty programs — the two BCD ones landed with the `AAA`/`DAA` fix — and twenty-eight and found the REG-field defect on its first execution, which is the highest yield per line of anything in this repo. Batches two and three added: the direction flag moving pointers backwards, `INT`/`IRET`, a far `CALL`/`RET`, a signed `XLAT` index, writing a segment register and indexing through it, `LOOP`'s counting rule, `CBW`/`CWD`, an overflowing byte multiply, a shift loop, an XOR checksum, a program reading its own instruction stream, `REP` with a zero count, nested `CALL`s, self-modifying code written through `DS` and executed through `CS`, two segments at one offset, a linear search in both outcomes, and `REPE CMPSB` over two buffers. **Two rules for every new program.** Mutate the exact rule its name claims — one of the twenty-eight proved nothing until a mutation survived it. And never write an expected value from memory: the packed-BCD program was cut rather than guessed, which is how the `AAA` masking question in "Still open" was found, and it is written now — the touches gate found four of its own expectations wrong the same way, every one of them a sentence remembered rather than computed. Hand-assembly errors so far, all caught because the emulator disagreed: `BE` for `SI`, `BB` for `BX`, `46` for `INC SI`, `5E` for `POP SI`, `disp16` byte order, `89 C6` read as a memory store when `mod=11` makes it a register move, and a program one byte short.
6. ~~**`T13`, the assembler.**~~ — **DONE and verified.** The crate `crates/asm` (no dependencies) reads §1.3 of `documentation-guide.md` at compile time, the way the flag contract reads §1.4, so the instruction table exists once in the project instead of twice. The plan is `odd/tasks/assembler.md` (ten specs, seven tasks, log L1-L8).
   - **The examples.** The gate `crates/asm/tests/examples.rs` assembles the reference's own eighty-five examples — local gold, gitignored, never committed — and **all eighty-five assemble**. What it took: four dialect defects (a word's trailing edge is the *first* character of what follows it; `ENDM` is read as a word, because the reference writes `<name> ENDM`; `offset` is a *term* of an expression; and there was no `include`), the reference's own workaround for a conditional jump that is too far (the opposite instruction and a big jump, five bytes; `JCXZ` becomes `OR CX,CX` and two jumps, seven bytes), and the emission of the prefixes the source writes (the bytes come out of the map, not out of a list).
   - **Verified independently.** A read-only verifier rebuilt the differential from scratch at every commit and checked all 284 conditional jumps itself: 270 in reach, 14 rewritten, every rewrite landing exactly on its label. No blockers; three advisories, all named in `odd/tasks/assembler.md`'s L8.
   - **The goldens cannot be had yet.** The reference is a PE32 Windows program and there is no wine here, so its assembler cannot be run to produce the bytes to compare against. The gate is therefore "they assemble"; the byte-for-byte comparison waits for a way to run the reference.
   - **Still open in the assembler**, all named in L7 and L8: the glued form `cs:mov ax,1` is still refused; `PUSHA`/`POPA` carry the prefix but the reference's own expansion does not; the map's second `LOCK` cell is `F1h` and the assembler writes `F0h` twice; the `E9h` displacement is sixteen-bit signed and the reference past 32767 bytes is unknown.
   - **A finding from the captures.** The 8086's 8-bit R/M field reaches all eight byte registers — the captures show `88 F4` running `MOV AH,DL` — and the 1979 print says otherwise. The chip wins; the guide's legend is marked in `odd/tasks/assembler.md`'s log.
7. ~~**`T12`**~~ - **DONE.** The runner assembles and runs a `.asm` (the reference's own load model, from its boot template: the load segment `0000`, the load offset the program's `ORG`), and the harness runs the eighty-five local examples and pins the state each one leaves behind. The goldens live in `re/extracted/app/goldens/`, gitignored with the examples they come from.
8. ~~**The unit chosen by the maintainer on 2026-10-22: emit the prefixes.**~~ — **DONE and closed, and it had already landed on 2026-10-21, the day before this item was written.** `0a4ae80` emits the prefix bytes out of the guide's own map — `REP/REPE/REPZ` is `F3`, `REPNE/REPNZ` is `F2`, the first `LOCK` is `F0` — before the opcode, and `fdaea50` pins the bytes of every prefix the map gives, in the order the source wrote them. **The claim that the assembler parsed a prefix and threw it away is true of 2026-10-21 and wrong now**: `rep movsb` assembles to `F3h A4h`, and six of the eighty-five examples carry a prefix, eleven instructions in all. The plan is `odd/tasks/assembler.md` (S10, L7 and L8).
9. ~~**The unit chosen by the maintainer on 2026-10-23: the two silicon questions.**~~ — **DONE and closed in `49b9d27`**: the `8C`/`8E` displacement forms and the carry of a rotate by more than one, both settled by the P80C86A-2's own captures. The oracle holds 2689 cases from 245 opcodes, the gate pins that count exactly, and the known-divergence list is empty. The plan is `odd/tasks/silicon-questions.md`.
10. ~~**The unit chosen by the maintainer on 2026-10-23: the gate over every capture.**~~ — **DONE and closed in `f3a216f`.** The oracle now carries the chip's final memory and the gate compares it; the corrected extraction recovery matches every capture's own `ram` list; and `crates/core/tests/hardware_full.rs` runs all **646000** captures. The same commit fixes `PUSH SP`: the chip pushes the stack pointer it had already decremented, so the 1979 manual's print is an erratum against the silicon. The full gate leaves **36** disagreements in three families — `DAA`/`DAS` (**28**), zero-immediate `AAD` (**6**) and `IDIV r/m8` (**2**) — all named in "Still open" above. The plan is `odd/tasks/full-capture-gate.md`.
11. ~~**The unit chosen by the maintainer on 2026-11-10: the three families the full gate left.**~~ — **DONE and closed.** The tens correction of `DAA`/`DAS`, the zero-immediate `AAD` and the seven-bit quotient of `IDIV r/m8` are all the chip's rule now, the family pins are at zero, and the gate over all 646000 captures has **no disagreement left**. The plan is `odd/tasks/full-capture-gate.md` (L17).
12. ~~**The unit chosen by the maintainer on 2026-11-10: the assembler's debt and the length table.**~~ — **DONE and closed.** The glued segment override keeps its prefix (`21099ac`), `PUSHA`/`POPA` are the eight-word sequences the reference's own documentation gives (`d7c1cd8`), the second `LOCK` needed no code because the reference's own string says `LOCK-OPCODE:F0` (`re/FINDINGS.md`), the register initialisers are read and the program starts where it says (`ed2e4b6`), and the length table got the three families it was missing (`9e2252b`). The plan is `odd/tasks/assembler.md` (L9).
13. **The next unit is not chosen.** What is left is in "Still open" above: `IDIV r/m16`'s `-32768h` quotient, which no capture covers; T12 a fondo, the eighty-five examples run with their own baseline pinned; the assembler's build directives (`#make_bin#`, `#make_com#`, `#mem=...#` and the `.binf` file itself), which are not written yet; **the operand's own segment override, which the assembler parses and drops** (`mov cs:temp1, si` comes out without its `2Eh`, and the reference's own documentation says the manual attachment is required when the data is not in `DS` — the bytes are wrong, and three of the reference's examples write it); an unknown `#directive=`, which the reference refuses and this ignores; and T16 (the TUI), T17 (packaging and CI) and T18 (the GUI, the last stage).

## Files worth reading first

| Path | What |
|---|---|
| `odd/tasks/emu8067-foss.md` | specs, every decision, the U1..U8 plan, audit items, and the log |
| `odd/tooling.md` | environment facts: Ghidra paths, what `analyzeHeadless` needs, `innoextract` |
| `odd/research/_prompt-audit7-and-friends.md` | the research prompt template |
| `odd/research/_prompt-microcode-closure.md` | the second use of that template; its answer over-claimed, see item 4a |
| `SingleStepTests/8086`, `v1/metadata.json` | **not in the repo, and the source the hardware oracle is built on.** Single-step captures from a real Intel P80C86A-2, recording per opcode which flags end up undefined. Free, no hardware, and it settles flag questions a transcription cannot. See "A second cause" above |
| `re/FINDINGS.md` | the behavioural contract recovered from the reference implementation |
| `odd/research/feature*`, `softonic-build-comparison.md` | feature inventory and the repack analysis |