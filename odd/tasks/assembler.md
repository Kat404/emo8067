# The assembler (T13)

Objective: an assembler that takes the reference's MASM-style source and
produces the bytes. Branch: `main`. Delivery: one commit per task, each
leaving the tree green. Test runner: `cargo test --workspace`, plus the
examples in `re/extracted/app/examples/` (eighty-six of them, local gold, never
committed).

Written 2026-10-19, after the device bus closed. This is the one large piece
of construction the project has left, and it is the piece the rest of the
toolchain waits on: T12's exit criterion is the examples running, and running
them means assembling them.

## Why this and not more

Twenty-one of the tests in this repository are hand-encoded, and every
hand-encoding is an opportunity to encode the wrong thing. The project has
already paid for that twice: a `disp16` written the wrong way round, and a
program one byte short so a `jmp` landed past its end. An assembler is the
answer to both, and it is also what makes the rest of the plan safe to
change.

The reference's own grammar was recovered verbatim into
`re/FINDINGS.md`: the operand-type signatures, the thirteen `STRICT_SYNTAX`
checks, the accepted prefixes, the numeric literal rules and the register
names. That is the specification, and it is the same document the emulator is
checked against.

## Specs

- **S1** — The assembler is driven by the **same document the emulator is
  checked against**: `odd/research/documentation-guide.md` §1.3, read at
  compile time, the way `flag_contract.rs` reads §1.4. The opcode map is not
  written out a second time in the assembler, so the two cannot drift.
- **S2** — MASM-style source: the instruction mnemonics of §1.3, the
  operand forms of §1.3 (`r/m8`, `r16`, `i8`, `i16`, `m8`, `m16`, `sreg`,
  `short`, `near`, `far`), and the reference's numeric literals: `&HFF`,
  `1234h`, `0123456789$` (MASM's radix suffix), decimal, and the
  `zero prefix must be added to a HEX value` rule.
- **S3** — The reference's addressing: `[BX]`, `[SI]`, `[BP+SI]`,
  `[BX+DI]`, `[BP]` with `SS` as its default segment, `[disp16]`, the
  segment overrides `cs: ds: es: ss:` and `.cs: .ds: .es: .ss:`, and the
  two legality checks the reference made — `[BX] cannot go with [BP]` and
  `[SI] cannot go with [DI]`.
- **S4** — The operand types are checked the way the reference checked them:
  an 8-bit register and a 16-bit register do not match, a segment register
  and an immediate do not match, `CS` and `IP` cannot be the destination of a
  `MOV`, memory to memory is not allowed, and a port number is not an
  immediate. The thirteen checks are listed verbatim in `re/FINDINGS.md`.
- **S5** — The directives: `ORG`, `DB`, `DW`, `DD`, `EQU`, `LABEL`, `SEGMENT`,
  `END`, `PROC`/`ENDP`, `DUP`, `INCLUDE`, `RADIX`, `MACRO`/`ENDM`, and the
  build directives `#make_bin#`, `#make_com#`, `#make_exe#`, `#make_boot#`,
  `#start=…#`, `#mem=addr,bytes-…#` and the register initialisers
  `#cs= #ds= #es= #ss= #ip= #sp= #si= #di= #bp=`.
- **S6** — Macros expand, and a macro may call another; the reference
  reported a pass count and refused a recursive definition, so the assembler
  counts its passes and refuses a definition that would not terminate.
- **S7** — The errors are the reference's, verbatim where the reference
  printed one: `not enough operands for …`, `first operand can be AL or AX`,
  `second operand cannot be AL`, `segment registers cannot go together!`,
  `mem to mem`, `memory to memory is not allowed for 8086:`. A message that
  says which line and which instruction is worth more than one that does not.
- **S8** — The examples assemble: all eighty-six of them, and the bytes the
  assembler produces are the bytes the reference produced. The examples are
  local gold and are not committed; the goldens are.

## Tasks

- **T1** — **DONE.** The crate (`crates/asm`, no dependencies) and the
  reader: the opcode map out of §1.3, read at compile time, into one form per
  opcode — the mnemonic and the operand form, verbatim. The reader also
  settled a question the plan had wrong: the map has **one cell per opcode**,
  sixteen opcodes to a row, not eight columns to an opcode. The manual's
  Table 4-13 has eight columns; the guide's transcription flattened them, and
  the `/digit` of a group lives in the guide's group tables, not in the grid.
  The group forms are T2's work.
- **T2** — **DONE, the ModR/M half.** The byte and its three fields, with
  the `REG` field naming the 16-bit registers in the 8086's order
  (`AX CX DX BX SP BP SI DI`), and the 8-bit field unable to reach
  `AH CH DH BH` — which the 8086 can name only through a register operand.
  **DONE, the group forms.** The three group tables of the document - Group
  1, Shift and Group 2 - read into ninety-six forms: six opcodes, four and
  two, each with eight digits. A group is chosen by the *operation* the
  source asked for, not by the digit: `ADD` in Group 1 is the digit whose
  cell says `ADD`, which is how the reference chose it too. The digit then
  goes in the `REG` field, so `ADD BL,1` is `80 C3 01h`.

  **DONE, the matcher.** A form from the document plus the operands the
  source gave, and the bytes. Which operand goes in the `REG` field and
  which in the `R/M` field is the instruction's business, not the field's,
  so the matcher decides it: the `r8`/`r16` operand always names `REG`,
  and the `r/m` one always names `R/M`. The forms that are not encoded
  yet — a group's `/digit`, and a memory operand until the addressing
  modes land — say so instead of guessing.
- **T3** — **DONE, the addressing modes.** A memory reference, taken
  apart: the base and the index, the displacement, the R/M field they name
  (Table 4-10's eight forms), the MOD that says how wide the displacement
  is, the direct form with no register at all, and the default segment:
  anything involving `SP` or `BP` is relative to `SS`, everything else to
  `DS`. The reference's two refusals are refused too — a base of `BX` with
  an index of `BP`, and `SI` with `DI`.
  **DONE, the reference's own syntax.** A reference as the source wrote
  it: `[BX+SI+4]`, `WORD PTR [1234h]`, `BYTE PTR DS:[BX]`. The size and the
  segment override come before the brackets, and a sign in front of the
  number belongs to the number: `BX-4` is BX and minus four. Still to
  come in this task: the matcher using all of it, so that an instruction
  with memory comes out as bytes.
- **T4** — **DONE, the literals and the line.** The literals are below.
  **DONE, the line.** A source line, taken apart: the mnemonic, the operands
  as the source wrote them, and the prefixes that came before - the repeat
  prefixes, `LOCK`, and the segment overrides, which may carry a colon. An
  operand may hold a comma inside brackets, so the line splits on the commas
  that are not inside any, and a semicolon starts a comment.
- **T4** — **DONE, the literals.** Four ways to write a number, all of
  them from the reference's own diagnostics: `&HFF`, `1234h`, `16$FF` (MASM's
  radix: the base, then the `$`, then the digits) and plain decimal. A hex
  value has to begin with a digit or it reads as a name, so `FFh` is refused
  and `0FFh` is not; and a value that will not fit in sixteen bits is
  refused rather than truncated. Still to come in this task: the source
  parser that turns a line into operands, and the prefixes.
- **T5** — **TODO.** The type checks, the thirteen of them, with the
  reference's messages.
- **T6** — **TODO.** The directives, the macros and the build directives.
- **T7** — **TODO.** The eighty-six examples, and the goldens.

## Log

- **L2 (2026-10-19).** The bet of S1 was checked before anything was
  written: §1.3 was parsed as a table and the shape of it measured. Sixteen
  opcode rows, sixteen cells in each, ninety-five distinct operand forms,
  and **no cell that would not parse**. The table is regular enough to be
  read by a program, which is what S1 needs; the cost of the parser is a
  known cost now rather than a hope.

- **L1 (2026-10-19).** The maintainer chose the assembler after the device
  bus closed. The design that this document settles, and that the next
  session should not re-litigate: **the assembler reads the same document
  the emulator is checked against**. §1.3 is the opcode map, transcribed
  from the manual; the flag contract already reads §1.4 at compile time;
  so the assembler can read §1.3 the same way, and the instruction table
  exists once in the project instead of twice. The cost is a parser for
  the table's prose, and the benefit is that an assembler bug and a
  transcription bug cannot both be true and cancel out.
