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
  **DONE, the matcher uses it.** A memory operand brings its own MOD and
  its displacement: `MOV [BX+SI],CL` is `88 08h` and `MOV [BX+4],BX` is
  `89 5F 04h`. The R/M field names a register or a memory reference, and
  a memory reference is the one that brings the displacement.
- **T4** — **DONE, the literals, the line and the whole thread.** Four ways
  to write a number, all of them from the reference's own diagnostics:
  `&HFF`, `1234h`, `16$FF` (MASM's radix: the base, then the `$`, then the
  digits) and plain decimal. A hex value has to begin with a digit or it
  reads as a name, so `FFh` is refused and `0FFh` is not; and a value that
  will not fit in sixteen bits is refused rather than truncated. A source
  line, taken apart: the mnemonic, the operands as the source wrote them,
  and the prefixes that came before - the repeat prefixes, `LOCK`, and the
  segment overrides, which may carry a colon. An operand may hold a comma
  inside brackets, so the line splits on the commas that are not inside any,
  and a semicolon starts a comment. And the whole thread, end to end: a
  line's name, the form the document's table gives it, the operands, and the
  bytes - `MOV AX,BX` is `89 D8h`, `ADD BL,1` is `80 C3 01h`, `MOV [BX+SI],CL`
  is `88 08h`. A mnemonic can name many forms, so the operands pick the one;
  a group's cell names the operation rather than a form, so the group's own
  table is searched too.
- **T5** — **DONE, the type checks.** The reference's own words, one for
  each refusal it made: `8 bit and 16 bit register`, `16 bit and 8 bit
  register`, `16 bit register and 8 bit address`, `8 bit address and 16 bit
  register`, `8 bit register and 16 bit address (add 'b.' before ...)`,
  `16 bit address and 8 bit register (add 'b.' before ...)`, `second operand
  is over 8 bits!`, `cannot use segment register with an immediate value`,
  `segment registers cannot go together!`, `first operand cannot be an
  immediate value`, `CS cannot be modified directly (use far JMP or CALL)`,
  `IP cannot be modified directly (use JMP or CALL)`, `cannot get value of
  IP register (use POP after CALL)`, `wrong use of segment register` and
  `memory to memory is not allowed for 8086:`. The two wordings of a
  mismatch belong to the position: the first operand says it one way and
  the second says it the other. The checks that need no form - two segment
  registers, memory to memory, a segment with an immediate, `CS`, `IP` -
  come first, because their answer does not depend on which instruction the
  source meant; the width checks need the form, so they run against each
  candidate, and the complaint that counts is the one from the form that
  matched the operands.
- **T6** — **DONE, the directives and the program.** The reference's own
  list of them: `LISTING` `LABEL` `PROC` `END` `ENDS` `ENDP` `DUP` `FAR`
  `NEAR` `EQU` `ORG` `DB` `DW` `DD` `STACK` `NONE` `SEGMENT` `ASSUME`
  `MACRO` `ENDM` `COMMENT` `PAGE` `TITLE` `NAME` `INC`. Most of them matter
  to the assembler and not to the bytes: the segments, the procedures and
  the listing are the shape of the source, not of the program. The rest say
  where the program is (`ORG`), what it holds (`DB`, `DW`, `DD`), and what
  its names mean (`EQU`, `LABEL`, and the label written as `name:`). A
  program is assembled from its lines: the origin, the bytes, and the names
  with the addresses they are at. A constant is folded into the operand
  that names it.
  **DONE, the jumps to a label.** A program is walked twice: the first time
  the names are collected and the addresses counted, the second time the
  bytes are made, and by then a jump knows where it is going. A jump's size
  does not depend on where it goes, so the first walk can count it with a
  target of zero. `JMP label` is the near form and the conditional jumps
  are the short one, as the table gives them. Still to come in this task:
  the macros (`MACRO`/`ENDM`).
- **T7** — **TODO.** The eighty-six examples, and the goldens.

## Log

- **L3 (2026-10-19).** A false alarm, caught before it cost anything: the
  guide's row `60` of §1.3 looks like a copy of its row `70`, and the
  assembler duly gave `JE` the opcode `64h`. But the guide is right and the
  hardware agrees: the captures of a real P80C86A-2 show `60h` running `JO`
  (the IP moves by two and then by the displacement), and the guide's
  footnote ‡8 says why - the microcode for the conditional jumps runs for
  the whole bit pattern `011?????`, so `60h`-`6Fh` alias `70h`-`7Fh`, and
  Table 4-13 prints them as `(not used)`. The emulator's decoder has known
  this all along (`crates/core/src/exec.rs`: "they alias `70`-`7F`").
  What was wrong was the assembler, which took the alias for the canonical
  opcode; it now writes `74h`, the opcode people write, and the alias row
  is left out of the candidates.

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
