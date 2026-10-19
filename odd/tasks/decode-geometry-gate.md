# Decode geometry gate (Fase 1)

Objective: close the one blind spot this project still has — **what an
instruction consumes and what it touches** — as opposed to what it computes.
Branch: `main`. Delivery: one commit per task, each leaving the tree green.
Test runner: `cargo test --workspace`, plus `python3 odd/research/verify_opcodes.py`.

Written 2026-10-15, after the session in which eight defects surfaced. Four were
invisible to every test in the repo because they were invisible to tests written
in the same vocabulary as the code. This unit is the first of a three-phase plan;
its acceptance criterion is at the bottom, and it is a list of things that
already broke here rather than a hypothetical.

## Why this and not more tests

261 tests, and fourteen of the hand-assembly errors made while writing the
program corpus were *decode geometry*: an inverted `disp16`, `BD` where `BE`
belonged, a program one byte short so a `jmp` landed past its end. A
badly assembled program does not fail its own test — it fails to mean anything,
silently. No existing gate measures consumed length: `every_first_opcode_byte_
the_8086_defines_decodes` proves a byte is not a hole, and
`every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole` proves a
`/digit` is either an instruction or a named one. Neither looks at bytes consumed.

## Specs

- **S1** — For every encoding the manual defines, the emulator consumes exactly
  the number of bytes the manual's operand format implies, counting: the opcode
  byte, a ModR/M byte when the encoding has a register-or-memory operand, a
  displacement when MOD requires one, and the immediate when the format names
  one. Source: `documentation-guide.md` §1.3, whose mnemonics carry the format
  (`i8`, `i16`, `i8:sx`, `m8`, `m16`, `sreg`, `short`, `near`, `far`).
- **S2** — Every such encoding **executes** without faulting, and writes only the
  registers and flags the manual's §1.4 row names for it. An encoding the 8086
  does not have must be refused, not executed: the 80186-only forms are refused
  today for `8C` with a register destination and for `8F` with a non-zero
  `/digit`, and nothing checks the rest.
- **S3** — The declared length in the table is a *claim from the manual*, written
  out by hand, not computed from the byte sequence the test feeds. If the table
  and the emulator disagree, that disagreement is the finding, and it is resolved
  by reading the manual rather than by editing whichever side is inconvenient.
- **S4** — The sweep is the coverage proof. Every documented encoding is executed
  by the gate itself, so no instrumentation is added to the core and no second
  list of encodings exists that could rot without being run.
- **S5** — Every gate in this file must be shown to fail. A gate nobody has seen
  die is decoration. Each task therefore ships with the mutant it kills.

## Tasks

- **T1** — **DONE in `086c3bf`.** Length table and gate for every encoding that
  consumes an immediate and no displacement: `04`–`3D`, `80`–`83` at MOD=00,
  `A0`–`A3`, `A8`/`A9`, `B0`–`BF`, `C2`, `CB`, `C6`/`C7` at MOD=00, `CD`, `D4`/`D5`,
  `E4`–`E7`, `E8`, `E9`, `EB`, `F6`/`F7` at MOD=00, `9A`. Fifty-six rows.
  Mutants killed: `JMP short` reading a `disp16`, `TEST AX,i16` reading one
  immediate byte, `RET i16` dropping its argument, `CALL near` reading a
  `disp8`.
- **T2** — The same gate extended to displacements: MOD=01 adds one byte and
  MOD=10 adds two, across every group that has a memory form. Route: inline.
  Mutant: MOD=10 fetches a `disp8`.
- **T3** — The execution half of S2: every defined encoding retires without
  faulting and leaves untouched what it is documented not to touch, and the
  80186-only forms are refused. Route: inline. Mutant: `8C` with a register
  destination starts executing.
- **T4** — Record the acceptance result in this document: each of the eight
  defects from this session, mapped to whichever task or phase kills it.
- **T5** — **DONE.** The `AAA` masking question, which the handoff carried as
  open and possibly wrong. Route: inline. Two defects, both against the
  manual: `AAA`/`AAS` masked `AL` only inside the adjustment branch, and
  `DAA`/`DAS` never wrote `AF` at all. Mutants killed: the mask moved out of
  the branch, and the `AF` write removed.

## Log

- **L1 (2026-10-15).** Eight defects this session: a string-source segment
  override discarded, `REP`-negates-`MUL`/`IDIV` dismissed as fiction and then
  implemented, two bugs inside that implementation that 220 tests could not see,
  `A8`/`A9` run as `CMP` for two units against real silicon, `LAHF`/`SAHF` on each
  other's opcodes, the 16-bit REG field naming five of eight registers wrongly,
  and one test that proved nothing until a mutation survived it. Four gates exist
  now; none of them would have caught the first, second or fourth.
- **L2 (2026-10-15).** T1 landed in `086c3bf`, and the three ways an encoding's
  length can be observed were not obvious in advance. A plain instruction leaves
  `IP` past itself. A relative jump needs its displacement accounted for: the
  first version exempted control transfers from the check, and a `JMP short`
  reading a `disp16` **survived** that gate. `INT` and far `CALL` own `IP` — it
  goes to the handler — so the length has to be read off the pushed return
  address. `RET` pops its own return address, so `IP` is not its length either,
  and the stack moves by the pops plus the immediate's value instead, which
  measures the immediate from the other side. `EA` is excluded on purpose: a far
  jump leaves no trace of how far it read, so no observation separates a
  five-byte read from a six-byte one; its length is pinned by its `9A` sibling.

- **L3 (2026-10-15).** Two of the four first mutants proved nothing because they
  changed nothing — one was a comment, one was a string that did not match — and
  a fifth failed to compile. Three attempts were needed to test the claim each was
  meant to test. This is the third time this session that an inert mutant was
  read as a surviving one, so the rule stands: **a mutant is evidence only once
  you have seen it fail to compile or visibly change the behaviour under test.**
  The check costs nothing, which is exactly why it is not optional.

- **L4 (2026-10-15).** The table was written from `documentation-guide.md` §1.3
  and agreed with the emulator on all fifty-six rows on its first run. That is
  the expected result when both sides come from the same transcription, and it
  is why the mutants matter more than the first green run: agreement here is not
  evidence, it is two readings of one document. What the gate buys is that code
  and document can no longer drift apart silently, and that the next change to
  either has something to fail against.

- **L5 (2026-10-15, audit of Phase 1).** An independent audit of the gates
  returned *holes found*, with two blockers of its own and two more in
  `flag_contract.rs`. All four are fixed, and each fix was verified by replaying
  the audit's own patch.

  1. **`PushesReturn` was a tautology.** The arm returned the *declared* length
     into the variable the shared assertion compared, so for the `CD`, `9A` and
     `E8` rows that assertion compared the table with itself. The pushed return
     address was the only real check. The arm now says so, and states the
     destination it must land in.
  2. **`PopsReturn` could not see the immediate's width.** The row's immediate
     had a low byte equal to its whole value, so a decoder that read one byte
     released the same number. The immediate is now `0102h`. This is the same
     mistake as the `E8` row below, and it is the most useful thing the audit
     found: **a row whose bytes cannot distinguish two implementations is not
     evidence for either.**
  3. **A row count bounded below cannot notice deletions.** `>= 50` is now `== 55`,
     pinned. Writing the number as a guess first, and having the gate catch that
     the table has 55 rows and not the 72 written, is the cheapest demonstration
     of why.
  4. **`flag_contract.rs` could be switched off by editing the file it reads.**
     Turning the `MOV` row from nine `—` marks into nine `X` marks deletes every
     assertion about `MOV`, `LEA`, `RET`, `CALL`, `PUSH` and `POP`, and the
     suite stayed green. A new test counts assertable marks in §1.4 and pins it,
     so weakening the oracle now fails loudly. Separately, `row_marks` searched
     the whole document for the first line starting with an instruction name, so
     a row inserted above the §1.4 heading could hijack it; the search is now
     confined to §1.4.

  **One correction to the audit.** Its first blocker — a `CALL` that reads a
  `disp8` and then nudges `IP` one byte forward, passing all 264 tests — is not
  an escape. With the row's immediate as written (`0003h`), that mutant produces
  the same pushed address, the same target and the same final `IP` as the
  correct implementation: it is an *equivalent* mutant, which no observation can
  distinguish. The fix was the same as blocker 2: give the row an immediate whose
  high byte is not zero, so reading one byte lands somewhere else. The gate now
  kills it, and the distinction is recorded because "equivalent mutant" and
  "escaped mutant" read identically in the output.

  **Still open from the audit, and now the plan's own backlog.**
  - 22 Group 1 `(opcode, /digit)` rows missing at MOD=00, including `F6 /1` and
    `F7 /1`, which §1.4 ‡8 documents as real 8086 instructions with **zero**
    executions anywhere in the suite.
  - 32 conditional-jump and 4 loop rows carrying a `short` displacement. Partly
    closed here: nine of them now have rows.
  - `X` and `U` marks assert nothing, so for `ADD`, `ADC`, `SUB`, `SBB`, `CMP`,
    `DAA`, `DAS`, `CMPS` and `SCAS` the flag gate proves only that D, I and T are
    untouched.
  - Nothing in the repository asserts what an instruction *touches*: a `MOV m8,AL`
    writing through `ES` instead of `DS` passes everything. That is T3.
  - In `programs.rs`, the padding bytes of both search programs are never
    executed, the backwards-copy program asserts pointers but not content, and
    `run()` never checks where a program halted.

- **L6 (2026-10-19).** The maintainer answered four questions, and the answers
  are the plan's constraints from here on.

  1. **The oracle stays the manual; the hardware captures come in phase 2.**
     The geometry — how many bytes an encoding consumes, and what its operand
     format implies — exists only in the manual, and the captures record flags
     and not operand formats, so they cannot close T2 or T3 on their own.
  2. **The 86 example programs are local gold, not repository content.** They
     live in `re/extracted/app/examples/`, which `.gitignore` excludes because
     it is derived from a proprietary binary. They may be run and compared
     against, and neither they nor their golden states go into the tree.
  3. **`8F` with `mod=11` stays open.** The structural argument says a register
     `POP`; no source states it, and the captures record `8F` by `/digit` only.
  4. **The `AAA` question is settled against the manual, and the core was
     wrong.** The handoff said `aaa` masks `AL` to its low nibble and that
     "Intel's pseudocode has no such mask". Both halves of that are false.
     The manual's own page 2-35 says the opposite in prose — "AAA ... changes
     the contents of register AL to a valid unpacked decimal number; the
     high-order half-byte is zeroed" — and Intel's SDM writes the mask as a
     step of its own, outside the branch (Volume 2A, page 3-19):
     `AL := AL AND 0FH;` after the `IF`. The rule the handoff quoted, `AL >
     99h`, belongs to `DAA`, not `AAA`. So the mask is right, and it was
     **unconditional** in the document and **conditional** in the code: `AL =
     35h` with `AF` clear must end as `05h`, and it ended as `35h`. The same
     reading found the second defect: `DAA`/`DAS` never wrote `AF`, although
     Table 2-21 gives both an `X` in that column and the prose says `DAA`
     "updates AF, CF, PF, SF and ZF".

  **One divergence left open, and it is narrow.** The SDM tests the *old* `AL`
  against `99h` in `DAA`'s second rule, where the classic 8086-era pseudocode
  tests the `AL` the first rule produced. The two differ only when the first
  rule fired and the old value was `94h`..`99h` with `AF` set. The 1979 manual
  has no rule text, the SDM's own worked example contradicts its own operation
  block, and the SDM describes later processors rather than the 8086, so the
  code was left as it is and the divergence is recorded here as debt. Settling
  it needs silicon or a source that says which the 8086 does.

