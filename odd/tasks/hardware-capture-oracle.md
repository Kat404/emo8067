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

## Why this and not more tests

The captures exist, they are free, and they are the only source this project
has that carries a flag's value. `SingleStepTests/8086` is Daniel Balsom's
hardware-generated suite: 2,000 tests per opcode, each one a complete machine
state, the instruction's bytes, and the state the chip left behind. The
README says what the chip did, and the machine's own words are worth more
than a table transcribed by a tired agent at the end of a long session.

The catch is size: 160 MB of gzipped JSON, and no dependencies allowed in the
crate. So the captures are not vendored. They are taken apart by a script
into a small text file, and the script is in the repository so the file can
be regenerated rather than trusted.

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
  decoration, and this one is a gate over 400 real cases.
- **S5** — The megabyte of memory is not part of the comparison. A case
  carries the bytes the instruction touched, recovered from the bus cycles,
  and nothing else.
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
- **T5** — **TODO.** The rest of the opcode space: the other 198 opcodes,
  the group forms, the string operations, the shifts, the divides with their
  exceptions. Each one that disagrees is a finding, and the findings are the
  point.
- **T6** — **TODO.** The captures as the flag oracle for the `X` rows, so
  that the flag contract stops asserting nothing for `ADD`, `ADC`, `SUB`,
  `SBB`, `CMP`, `DAA`, `DAS`, `CMPS` and `SCAS`.

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
