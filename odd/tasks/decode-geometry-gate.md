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

- **T1** — Length table and gate for every encoding that consumes an immediate
  and no displacement: `04`–`3D`, `80`–`83` with MOD=00, `A0`–`A3`, `A8`/`A9`,
  `B0`–`BF`, `C2`, `C6`/`C7` with MOD=00, `CD`, `D4`/`D5`, `E4`–`E7`, `E8`/`E9`,
  `EA`, `EB`, `F6`/`F7` with MOD=00, `9A`. Route: inline. Mutant: one opcode
  stops fetching its immediate.
- **T2** — The same gate extended to displacements: MOD=01 adds one byte and
  MOD=10 adds two, across every group that has a memory form. Route: inline.
  Mutant: MOD=10 fetches a `disp8`.
- **T3** — The execution half of S2: every defined encoding retires without
  faulting and leaves untouched what it is documented not to touch, and the
  80186-only forms are refused. Route: inline. Mutant: `8C` with a register
  destination starts executing.
- **T4** — Record the acceptance result in this document: each of the eight
  defects from this session, mapped to whichever task or phase kills it.

## Log

- **L1 (2026-10-15).** Eight defects this session: a string-source segment
  override discarded, `REP`-negates-`MUL`/`IDIV` dismissed as fiction and then
  implemented, two bugs inside that implementation that 220 tests could not see,
  `A8`/`A9` run as `CMP` for two units against real silicon, `LAHF`/`SAHF` on each
  other's opcodes, the 16-bit REG field naming five of eight registers wrongly,
  and one test that proved nothing until a mutation survived it. Four gates exist
  now; none of them would have caught the first, second or fourth.