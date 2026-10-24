# The hardware capture oracle (Phase 2)

Objective: let the chip say what the flags **become**. Branch: `main`.
Delivery: one commit per task, each leaving the tree green. Test runner:
`cargo test --workspace`, plus `python3 odd/research/verify_opcodes.py` and
`python3 odd/research/extract_capture_oracle.py --opcodes 00-3D`.

Written 2026-10-19, the day after Phase 1 closed. Phase 1 answered "how many
bytes does an instruction consume" and "what may an instruction touch". It
could not answer "what does a flag become", because the 1979 manual does not
say: the value depends on the operands, and every reference that quotes the
manual quotes the same `X` that asserts nothing. Two of the eight defects from
the 2026-10-15 session lived in that hole.

So the oracle stops being a document. It is a real Intel P80C86A-2.

**Update, 2026-10-23 — this plan now has a companion.** T5, T6 and
T7 below describe the committed sample as it stood before the full-capture
unit. This plan still owns that sample: T5 covers the whole opcode space,
T6 makes the captures the flag oracle and checks §1.4 against them, and T7
closes the rotate carry. `odd/tasks/full-capture-gate.md` owns the
final-memory comparison, the research gate over all **646000** captures,
and the three families still open (`DAA`/`DAS`, zero-immediate `AAD` and
`IDIV r/m8`). The specs, tasks and log below stay as the record of what
was believed when this unit ran; the stale claims are marked in place.

## Why this and not more tests

The captures exist, they are free, and they are the only source this project
has that carries a flag's value. `SingleStepTests/8086` is Daniel Balsom's
hardware-generated suite: 2,000 tests per opcode, each one a complete machine
state, the instruction's bytes, and the state the chip left behind. The
README says what the chip did, and the machine's own words are worth more
than a table transcribed by a tired agent at the end of a long session.

The catch is size: 160 MB of gzipped JSON, and no dependencies allowed in the
crate. So the captures are not vendored. They are taken apart by a script
into text files: a small committed sample and a generated file for every
capture. The script is in the repository so both can be regenerated rather
than trusted.

## Specs

- **S1** — The oracle is derived, not typed. Every case comes from the
  captures through `odd/research/extract_capture_oracle.py`, the script
  records the source, the version and the chip, and the generated file says
  it was generated. A hand-edited oracle is a second oracle, and the project
  has been bitten by that before.
- **S2** — The chip is the arbiter. Where the document and the chip disagree,
  the chip wins and the document's print is preserved and marked. Where the
  *transcription* and the chip disagree, the transcription is wrong and is
  corrected.
- **S3** — The gate compares what the chip defined. `metadata.json` says
  which flags an opcode leaves undefined, and those are masked out of both
  sides: "undefined" means the chip left whatever was there, and there is
  nothing to compare.
- **S4** — The gate must be shown to fail. A gate nobody has seen die is
  decoration, and this one is a gate over 400 real cases. **That was the
  shape when this unit ran; the committed gate now holds 2689 cases, and
  the companion research gate covers all 646000.**
- **S5 — STALE as of the full-capture unit.** The megabyte of memory is not
  part of the comparison. A case carries the bytes the instruction touched,
  recovered from the bus cycles, and nothing else. **The committed gate now
  compares the chip's final memory as well, through the oracle's `finalmem`
  line. The companion plan owns that shape.**
- **S6** — The coverage is counted and pinned, so a slice that quietly
  shrinks is a failure rather than a smaller green run.

## Tasks

- **T1** — **DONE.** The extraction script and the generated oracle. The
  script reads the captures, reconstructs the memory a case depends on from
  the bus cycles, and keeps the tests that exercise each defined flag in
  both directions — the captures are randomised, so the first few tests of
  an opcode rarely produce a carry, an overflow or a zero. 400 cases from the
  58 opcodes of the eight ALU groups that have captures.
- **T2** — **DONE.** The gate, `crates/core/tests/hardware_flags.rs`: it
  parses the generated file, runs each case and compares the flags, the
  registers the chip changed, and the ones it did not.
- **T3** — **DONE.** The 8086's REG field. The captures say `011` is `BX` and
  `100` is `SP`; the 1979 manual's Table 4-9 says the same; every modern
  reference says otherwise, because it quotes the 80186. The core, the
  guide's note, the gate that enforced the wrong order and the hand-assembled
  programs all followed the later table.
- **T4** — **DONE.** `DAA`'s second rule tests the value the low rule
  started from, not the value it produced. The 1979 manual has no rule text
  and the classic pseudocode says the adjusted value; the chip says the old
  one, and so does the SDM.
- **T5** — **DONE in `4cb26a5` and `e7bd116`.** The rest of the opcode
  space: 1533 cases from 233 opcodes, the group forms included. It found
  five more defects — the overflow of `INC`/`DEC` and of the rotates, the
  `POP` into a register, the unsigned index of `XLAT`, and the `/digit`
  that `C6`/`C7` ignore. **Seventy-one cases still disagreed then, in two
  families, and the gate named and counted them: the rotates' carry, and
  the port reads, where this emulator read memory at the port and the chip
  returned `FFh` because nothing is attached to it.** Both families are
  closed now — the port reads with the I/O bus, the rotates' carry in
  `odd/tasks/silicon-questions.md` — and the oracle holds **2689** cases
  from **245** opcodes, the count pinned exactly. **This task still covers
  the committed sample.** The generated oracle over every capture, and the
  research gate that runs it, belong to `odd/tasks/full-capture-gate.md`.
- **T6** — **DONE in `d9b5e96`.** The captures as the flag oracle for the `X`
  rows, and the document checked against the hardware in the other
  direction: a row that marks a flag with an em dash says the instruction
  does not touch it, and the captures say whether the chip agreed. The
  mapping from an instruction to its row comes from the captures' own
  disassembler, so this project keeps no list of its own. The check passes
  on all 2689 cases: every flag the document calls untouched is one the
  chip left alone, and the `X` rows are backed by the chip's numbers.
  **This task still covers the committed sample.** Its comparison now
  includes the chip's final memory; the all-capture research gate is in
  `odd/tasks/full-capture-gate.md`.
- **T7** — **DONE, and the six cases are closed too.** Three of the nine
  are closed: the negate writes the auxiliary carry (the borrow out of the
  low nibble, which the captures confirm in every case they cover), and a
  rotate by a whole number of revolutions still moves the carry. **Six
  remained, all of them the carry of a rotate by more than one, where the
  captures agreed with the rule the code already had in every case they
  cover — so those six were unexplained.** They are explained now, in
  `odd/tasks/silicon-questions.md`, which is where this plan's two open
  questions were carried to the chip. **The chip's rule:** a rotate by more
  than one takes its carry from the edge of the **final result** — `ROL`
  from bit 0, `ROR` from bit 15, and `RCL`/`RCR` carry on through the
  extra bit — and its overflow is the sign change of the final one-bit
  step. The old "the bit that left" rule is wrong in 516 of 30687 rotate
  cases. **The known-divergence list is empty: no hardware case disagrees
  with the emulator any more.**
  - **One lead was tried and it is dead.** The six cases come from the
    captures whose operand lives in memory, so the first suspicion was the
    extraction: that the memory state it writes for a case is not the memory
    the chip had. An audit of the effective address — computed from the
    encoding, against the address on the chip's own bus cycles — says the
    encoding's address and the chip's address disagree in 1058 of 1060 cases
    of `D2.0`, and in *every* memory form of `D0.0`, `00` and `8B` as well.
    Those last three files pass the gate, so the audit is what is wrong, not
    the chip and not the emulator. **The next session should not repeat it**:
    the address computation in that script is wrong, and the memory the
    extraction writes agrees with the chip's own RAM list.
  - **Update, 2026-10-23 — the extraction conclusion above was too
    broad.** The audit was wrong, but the old recovery was wrong too: a
    byte at an odd address sits in the upper half of the sixteen-lane bus
    field, and a write carries the value the chip wrote rather than the
    value that was there before. The corrected recovery matches every
    capture's own `ram` list. `odd/research/test_extraction_contract.py`
    pins both rules, and `odd/tasks/full-capture-gate.md` records the
    correction and the full set's remaining findings.

## Log

- **L1 (2026-10-19).** The maintainer chose Track A: the hardware captures
  as the oracle, after the manual. The captures were fetched, the format was
  read out of the files rather than assumed, and the first gate went green
  on 400 of 400 cases — **after** two defects the captures found on the way,
  and the second of them was the project's own "greatest defect".

  - **The 16-bit REG field was the 80186's, not the 8086's.** The project's
    guide transcribed Table 4-9 with `011` as `SP` and `111` as `BX`; the
    1979 manual prints two columns over the same eight slots, `AL CL DL BL
    AH CH DH BH` and `AX CX DX BX SP BP SI DI`, and the 80186 moved the
    16-bit registers to make room for the high bytes. The chip agrees with
    the manual: `MOV SP,AX` is `8B E0` and `ADD CX,SP` is `01 E1`, and
    neither is possible under the later order. The core followed the note
    rather than the table, and so did every hand-assembled program — the
    tests were written in the same vocabulary as the code, which is the
    failure this project keeps paying for.
  - **`DAA`'s second rule tests the old `AL`.** The 1979 manual has no rule
    text, the classic pseudocode tests the adjusted value, and the SDM
    tests `old_AL`. The chip settles it: `DAA` on `AL = FAh` leaves
    `AL = 60h` with the carry set, which is the old value testing true.
  - The gate's own first run failed 75 of 400 cases, and the failures were
    the register field: the emulator named the wrong register, so the
    operands were different and the flags came out different. That is the
    gate working, not the gate being wrong.
  - **The full space found five more**, and the pattern is the same one:
    every one of them is a place where this project believed a note, a
    prose rule or a structural argument instead of the chip. The overflow
    of `INC`/`DEC` and of the rotates, the `POP` into a register, the
    unsigned index of `XLAT`, and the `/digit` that `C6`/`C7` ignore.
    Three more came out of closing the two families that were open: the
    shift group's overflow, the count of `CL` (the 8086 does not mask it to
    five bits, which is what the later processors do), and the I/O bus (a
    port is not a memory address; with nothing attached to it a read
    returns all ones). Nine cases still disagree: the carry of a rotate
    by more than one, and the negate's carry.
  - **The rule of the rotates came from the captures, not from the manual.**
    The overflow is the top bit of the result against the top bit of the
    operand, and 2133 of 2133 hardware cases agree; the bit that left
    the top, which is what the code used, matches in half of them.

- **L2 (2026-10-23).** This plan's two open questions are closed, and the
  unit that closed them has its own plan: `odd/tasks/silicon-questions.md`,
  closed in `49b9d27`.
  - **The six cases T7 left unexplained are explained.** The emulator took
    the rotate's carry from the bit that left. The P80C86A-2 takes it from
    the edge of the **final result**: `ROL` from bit 0 and `ROR` from bit
    15, and `RCL`/`RCR` carry on through the extra bit. Its overflow is
    the sign change of the **final one-bit step**, not a comparison of the
    original operand with the final result. The two rules part company
    exactly when the count is a whole number of operand widths, which is
    what the six cases were about; the old rule is wrong in 516 of 30687
    rotate cases. A count of zero is a complete no-op, CF included.
  - **The `8C`/`8E` question T5 and T7 both left open is closed too.** The
    chip runs `8C` and `8E` with `MOD=01` and `MOD=10` as ordinary
    word-memory forms: `8C` writes the selected segment as a
    little-endian word at the effective address, `8E` reads that word into
    the selected segment, the `reg` field's high bit is ignored, no flag
    changes, and the displacement is consumed. The 1979 manual's
    `(not used)` print for those two forms is an erratum.
  - **The gate's shape.** The extraction covers the whole opcode space now
    and selects the cases the two questions need, so the oracle holds
    **2689** cases from **245** opcodes instead of 1533, and the gate pins
    that count exactly. `KNOWN_DIVERGENCES` is empty: no hardware case
    disagrees with the emulator any more.
