# The two open silicon questions — `8C`/`8E` with a displacement, and the carry of a rotate by more than one

Objective: let the chip close the two questions the handoff still lists as
open. Branch: `main`. Delivery: one commit per task, each leaving the tree
green. Test runner: `cargo test --workspace`.

**Risk: item 3** — the rotate group's carry and overflow live in
`crates/core/src/exec.rs`, shared code that the runner, the CLI, the programs
gate and the compatibility gate all consume, so the change is reviewed on its
own and carries an independent verification.

Written 2026-10-23. Continues `odd/tasks/hardware-capture-oracle.md` (T5 and
T7), whose oracle is the evidence. The master plan's numbering runs T11..T18,
so this unit takes no number of its own.

## Why this and not more

The captures from a real Intel P80C86A-2 are on disk and both questions are
inside them: `8C.json.gz` and `8E.json.gz` hold 2000 cases each, with every
`MOD` value represented, and `C0.json.gz` holds 2000 cases whose immediate
count runs from 0 to 255. This unit asks the chip the two questions and
records what it says. Nothing else moves.

## Specs

- **S1** — `8C`/`8E` with a displacement. The manual prints `(not used)` for
  `8C MOD 1--` and `8E MOD 1--`, and §1.3's note says so; the core executes
  every `MOD`. The captures have `8C` and `8E`, and the set's README says the
  `reg` field's high bit is ignored — the `mod` field is another question, and
  the hardware can answer it.
  **The rule.** The emulator's behaviour for `8C`/`8E` with `MOD=01` and
  `MOD=10` must be what the chip does, in every observable: the register or
  the memory that is written, the segment that is read, the bytes that are
  touched, the flags, and the number of bytes the instruction consumes. If the
  chip refuses the form, the emulator refuses it; if the chip runs it, the
  emulator runs it. Where the chip and the 1979 manual's print disagree, the
  manual's print is marked as an erratum in the same commit.
- **S2** — The carry of a rotate by more than one. Six of the 1533 cases
  disagree, all of them the carry of a rotate by more than one, and all of
  them unexplained: the captures agree with the rule the code already had in
  every case they cover. The gate names the family: `"the rotate's carry and
  overflow"`.
  **The rule.** The six cases end up explained by a rule, and the emulator
  implements the chip's rule. A divergence that survives is recorded with the
  chip's rule as the truth and a reason, never as a shrug.
- **S3** — The extraction's own header lies about the opcodes it skipped. The
  generated oracle says:
  `# opcodes with no captures: 0F 26 2E 36 3E 80 81 82 83 9B D0 D1 D2 D3 F0 F1 F2 F3 F4 F6 F7 FE FF`
  and `D0`..`D3`, `80`..`83`, `F6`, `F7`, `FE` and `FF` are in the oracle as
  their `/digit` forms (`op D0.0` .. `op D0.7`). The line says what it means —
  no flat file — or a reader takes it as "the hardware has no cases for
  these" and stops looking.
- **S4** — The oracle holds the cases the two questions need: the `8C`/`8E`
  forms with `MOD=01` and `MOD=10`, and the rotates with a count above one.
  The extraction keeps eight cases per opcode plus one per flag direction, and
  the selection has to keep enough of them to answer S1 and S2. The count the
  gate pins (`1533`) moves with the oracle, and the new count is pinned
  exactly, not bounded.

## Out of scope

- The unsigned `DIV` under `REP`: the captures do not cover prefixes.
- The microcode ROM across the 8086 steppings: not decidable.
- Any change to a §1.3 cell: the assembler reads that table at compile time,
  so a cell that changes changes the assembler. If a cell has to change, this
  unit says so in the log and the parent takes it as its own unit.
- The stale bookkeeping in `odd/HANDOFF.md`, `odd/tasks/disassembler.md` and
  `odd/tasks/emu8067-foss.md`: the parent's unit, not this one.

## Tasks

- **T1** — The extraction grows to cover S1 and S2, and its header stops
  claiming that the dotted-only opcodes have no captures (S3, S4). The oracle
  is regenerated with the script, never by hand.
- **T2** — The emulator's `8C`/`8E` with a displacement matches the chip
  (S1).
- **T3** — The rotate's carry and overflow match the chip, and the six cases
  are explained by a rule (S2).
- **T4** — The gate keeps its shape: the pinned case count moves to the
  oracle's real size, `KNOWN_DIVERGENCES` loses the families that no longer
  disagree and keeps the ones that do, with the chip's rule as the reason
  (S2, S4).

## Log

- **L1** (2026-10-23) — The maintainer's request, verbatim: "¿En dónde nos
  habíamos quedado en el desarrollo de emo8067?" and, after the report:
  "Excelente, hazme question tool para decidir el próximo paso".
- **L2** (2026-10-23) — The maintainer's choice, verbatim: "Hardware: 8C/8E y
  carry del rotate", route "Delegar a worker + verificador".
- **L3** (2026-10-23) — The state this unit starts from: `cargo test
  --workspace` 417 passed, 0 failed; HEAD `6e28bc8`; the hardware gate
  passes with exactly six known divergences, all of them in the `D0`..`D3`
  families; `odd/research/.captures/` holds the raw captures (174 MB,
  gitignored) and `odd/research/capture_oracle.txt` holds the 1533-case
  oracle, both tracked by the project except the raw captures.
- **L4** (2026-10-23) — The measurements this plan starts from, taken from the
  captures on disk: `8C` has 2000 cases and `8E` has 2000 cases, with every
  `MOD` value represented. **L9 corrects the per-`MOD` counts** of this
  entry, and the `C0` claim in it was wrong: `C0`/`C1` are `RET imm16` and
  `RET` on an 8086 — the chip's own name for them is `retn` — so there are
  no rotate-by-immediate captures at all. The rotates are `D0`..`D3`.
- **L5** (2026-10-23) — **S1 settled.** The P80C86A-2 runs `8C` and `8E` with
  `MOD=01` and `MOD=10` as ordinary word-memory forms. `8C` writes the
  selected segment as two little-endian bytes at the effective address;
  `8E` reads the word there into the selected segment. The `reg` field's
  high bit is ignored, no flag changes, and the displacement is consumed.
  The captures include `26 8C 70 3B` (MOD=01: SS=`4716h` becomes
  `FEFF4h=16h,FEFF5h=47h`, IP `C500h` to `C504h`), `3E 8C 89 78 E4`
  (MOD=10: CS=`06CCh` becomes `213A8h=CCh,213A9h=06h`, IP `769Ch` to
  `76A1h`), `3E 8E 63 64` (MOD=01: the word `D5EAh` at `2214Ah` enters
  ES, IP `A5BDh` to `A5C1h`), and `8E 83 EC 87` (MOD=10: the word `6363h`
  at `096A3h` enters ES, IP `BCD3h` to `BCD7h`). **Erratum:** the 1979
  Table 4-13 print of `(not used)` for the non-zero `MOD` forms is wrong
  about the chip; `documentation-guide.md` already records the same
  erratum.
- **L6** (2026-10-23) — **S2 settled.** For a multi-bit rotate the P80C86A-2
  takes CF from the edge of the **final result**: ROL uses bit 0 and ROR uses
  bit 15. Its OF is the sign change in the final one-bit step. This differs
  from the old "bit that left" rule exactly when the count is a whole number
  of operand widths. The six former divergences are `3E D2 83 6B 8A` (ROL
  byte, CL=8, value `03h`, CF 1 to 1), `D2 88 F3 45` and `3E D2 4B 77`
  (ROR byte, CL=48, values `57h` and `2Dh`, CF 0 to 0), `2E D2 4F 51` (ROR
  byte, CL=8, value `E2h`, CF 1 to 1), `36 D3 C6` (ROL word, CL=48, value
  `9C56h`, CF 1 to 0), and `D3 CC` (ROR word, CL=48, value `BE6Eh`, CF 1 to
  1). The overflow rule is visible in `D2 C0` (AX=`E461h`, CL=8,
  FLAGS `F497h` to `FC97h`), `D3 C8` (AX=`289Ch`, CL=40, `F006h` to
  `F807h`), and `D2 D0` (AX=`068Bh`, CL=28, `F003h` to `F803h`).
- **L7** (2026-10-23) — **T1/S3/S4 done.** The extraction now selects the
  `8C`/`8E` displacement forms and multi-bit rotates explicitly, defaults
  to the whole opcode space, and distinguishes a missing flat file from an
  opcode with no capture at all. Running `python3 odd/research/extract_capture_oracle.py`
  regenerated the oracle with **2689** cases; the gate pins that count
  exactly.
- **L8** (2026-10-23) — **T2/T3/T4 done.** The emulator implements the
  chip's rotate rule and the segment moves' displacement forms. The
  hardware gate has no known divergences left; a future one must name the
  chip's rule rather than be skipped silently.
- **L9** (2026-10-23) — **Correction to L4, left in place as history.**
  L4's per-`MOD` counts came from reading byte 1 as the ModR/M byte. A
  segment-override prefix is byte 1 in those captures, so that parser
  counted prefix bytes as `MOD=00`. Reading the byte after the actual
  opcode gives `8C`: 0: 525, 1: 494, 2: 474, 3: 507; and `8E`: 0: 480,
  1: 526, 2: 500, 3: 494. L4's numbers are exactly the byte-1 parse.
  This does not change S1-S4 or the chip's rule, but the measurements in L4
  must not be used as evidence.
- **L10** (2026-10-23) — The independent verification (`gentle-ai-verify`, read
  only, the repository never mutated). Verdict: S1 met, S2 met with advisories,
  S3 met, S4 met, **no blockers**. It re-derived the chip's rules from the raw
  captures, not from this project's oracle: 4000 `8C`/`8E` cases and 32000
  rotate cases checked on value, CF and OF, 0 disagreements; the old "bit
  that left" rule is wrong in 516 of 30687 rotate cases, and a count masked to
  five bits is wrong in 3780 values and 2003 carries; the regenerated oracle
  is byte-identical to the tracked one; and the gate bites — restoring the old
  rule in a scratch copy fails 6 of 2689 cases, masking the count fails 26,
  and deleting one `8C MOD=01` case fails the coverage test. The fixes this
  log carries came out of it: the code comment that claimed the 8086 does not
  mask the count (the manual's print says it does, and this part's captures
  say it does not — silicon wins, the print is marked), the capture byte
  string `2E D2 C0` that is really `D2 C0`, the test named after the old
  rule, and this entry's own `C0` claim.
- **L11** (2026-10-23) — What the verification could not close, and one hole
  it found. (1) The gate compares registers and flags but never memory: the
  `8C` write and the rotate's write-back are pinned only by the hand-written
  regression tests. The oracle carries no final memory, so closing that means
  changing the extraction's own shape. (2) No capture has a count above 62:
  the captures' harness masks CL to six bits, so the emulator's behaviour for
  a count of 255 is consistent with the derived rule but unproven by hardware.
  (3) The gate's `modrm()` helper finds the opcode byte by searching the
  case's bytes for the opcode value: no 8086 prefix byte is `8C` or `8E`, so
  it is safe today, but for an opcode that is also a prefix byte it would
  misfire, and it would panic if the opcode were the last byte. (4) A gate
  over every capture, not only the 2689 the oracle keeps, was written during
  the verification and is not in the repository: it needs the 174 MB capture
  set, so it is a research gate and not a unit test.
