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
- **S9** — A conditional jump whose target is out of the byte's reach is not
  refused: the reference replaces it with the opposite instruction and adds a
  big unconditional jump. Its own words: `re/extracted/app/version.txt`
  (v4.02) "short conditional jumps are replaced with the opposite instruction
  and one normal jump"; `documentation/asm_tutorial_07.html` "making total of
  5 bytes instead of just 2"; and `writebin.asm:65` "this instruction is
  assembled into OR CX, CX and two JMPs". The `70h`-`7Fh` row becomes the
  opposite opcode - `opcode ^ 1`, which is the opposite for every pair of
  that row - with a displacement of `+3`, and then `E9h` and the near
  displacement: five bytes. `JCXZ` becomes `OR CX,CX`, `JNZ` with a
  displacement of `+3`, and `E9h` and the near displacement: seven bytes.
  (`OR CX,CX` is `09h C9h`: the R/M field names `CX` as `001`, so the
  ModR/M byte is `11 001 001`. It is not `09h C8h`, which would be
  `OR AX,CX`.) The `LOOP` family keeps the refusal: the reference's table
  gives their companions, but the replacement for `LOOPE` and `LOOPNE` would
  decrement `CX` twice and test the wrong condition, and no example needs it.
- **S10** — The prefixes the source writes before the instruction are
  emitted as the bytes the guide's map gives them, in the order they were
  written: the repeat prefixes (`F3h` for `REP`/`REPE`/`REPZ`, `F2h` for
  `REPNE`/`REPNZ`), `LOCK` (`F0h`), and the segment overrides (`2Eh` CS,
  `36h` SS, `3Eh` DS, `26h` ES, which the map writes as `SEG CS:` and so
  on). Today the prefixes are parsed and thrown away, so `rep movsb`
  assembles to `A4h` without its `F3h`, and the gate cannot see it: it
  compares against a base that loses them too. The bytes come from the
  guide's own map and not from a list written out here, and the instruction
  grows by the bytes of its prefixes, so the layout has to settle again
  with them in it.

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
  are the short one, as the table gives them.
  **DONE, the macros.** `MACRO` and `ENDM`, with the names after the
  macro's name as its parameters. A call replaces them with the arguments,
  word by word - a whole word, never a part of a longer name. A macro may
  call another, and one that calls itself is refused with the reference's
  own words: "recursive MACRO definition is over 64".
- **T7** — **IN PROGRESS.** The eighty-five examples, and the goldens.
  **The gate.** `crates/asm/tests/examples.rs` reads the reference's own
  examples (local gold, gitignored) and assembles every one of them. It
  stands aside when they are not there, so the repository's own tests do not
  depend on files that are not in it. **Eighty-one of the eighty-five
  assemble.** The four that do not are listed in the gate itself, each with
  what it needs, and `odd/research/examples-pending.txt` holds the same list
  with the line and the error.
  **DONE, the four dialect defects.** They were not the dialect but four
  bugs, each of them found by reading the reference's own examples:
  a word's trailing edge is the **first** character of what follows it, so a
  macro parameter followed by a comma was never substituted; the reference
  writes a macro's end as `<name> ENDM`, the same shape as `<name> MACRO`,
  and reading `ENDM` as a whole line dropped the entire tail of every
  program that used the library's macros; `offset` is a **term** of an
  expression and not a whole operand, so `offset msg_end - offset msg` and
  `offset filename+2` are now read; and the `include` directive brings the
  library in textually, before the macros are read, with the file's own
  directory first and the caller's include directories after it.
  **DONE, the four that the reference itself rewrites.** `bin2dec.asm`,
  `micro-os_kernel.asm`, `mouse.asm` and `writebin.asm` each hold a
  conditional jump whose target is more than 127 bytes away. The reference
  does not refuse those: its own changelog says that "short conditional
  jumps are replaced with the opposite instruction and one normal jump", and
  its tutorial 07 says the same in prose - five bytes instead of two. The
  assembler now rewrites them the same way: the opposite opcode and a big
  unconditional jump for the `70h`-`7Fh` row, and `OR CX,CX` and two jumps
  for `JCXZ`. **All eighty-five examples assemble.** The layout is a fixed
  point reached over several walks, and the assembler refuses a program whose
  addresses do not settle. The `LOOP` family keeps the refusal, for the
  reason S9 gives.
  **Two more dialect defects, found by the same four.** `REP` was not a
  prefix (`rep movsb` parsed as the mnemonic `rep`), and a first word that
  already carried a colon was given another one, which lost the label of
  `nsp:mov [di-1], 0`. Both are fixed.
  **DONE, the prefixes.** The prefixes the source writes are now emitted as
  the bytes the guide's map gives them, in the order they were written: the
  repeat prefixes, `LOCK`, and the segment overrides. `rep movsb` is `F3h
  A4h` and not `A4h`. The bytes come out of the map - `REP/REPE/REPZ` is
  `F3`, `REPNE/REPNZ` is `F2`, the first `LOCK` is `F0`, and the overrides
  are the cells that say `SEG CS:` and so on - and not out of a list written
  here. The gate could not see this defect: it compares against a base that
  loses the prefixes too. Six examples carried one; the other seventy-nine
  are byte-identical to the base, and the eleven prefixed instructions are
  the eleven bytes the map gives. The dotted override (`.cs:`) was parsed
  as a label and lost; the recovered contract names it, so it is a prefix
  now.
  **The goldens are not possible yet.** The reference is a PE32 Windows
  program and there is no wine here, so its assembler cannot be run to
  produce the bytes to compare against. The gate is therefore "they
  assemble", and the byte-for-byte comparison waits for a way to run the
  reference.

## Log

- **L8 (2026-10-21).** An independent verification of the assembler's three
  commits, and the correction of a number I wrote down wrong.
  **The verdict: it holds.** No blockers. Three advisories.
  **The method.** The gate compares the examples against a base that is wrong
  in the same way, so the verifier wrote its own probes: it marked every
  source-level conditional jump in a copy of every example with a label, read
  each jump's address out of the program, and checked the encoding of all
  **284** conditional jumps itself. Two hundred and seventy are in reach -
  two bytes, the canonical opcode, landing on their label - and **fourteen**
  are rewritten, each with the opposite opcode, the `+3` displacement, the
  `E9h`, and the `E9`'s displacement landing exactly on the label it
  replaced. A whole-byte scan found no rewrite-shaped site that was not at a
  marked address. The differential was rebuilt from scratch at every commit:
  the 79 examples without a prefix are byte-identical, the six with one
  differ by the eleven prefix bytes and by the displacements that move
  because the addresses moved, and `string.asm` went from **zero bytes** to
  234 - the `ENDM` fix, and a program the gate had been calling a pass.
  **The correction.** L6 said "all nineteen replacements"; the verified count
  is **fourteen** (twelve of the five-byte form and two of the `JCXZ` form).
  The number came from a report that had not counted the way the verifier
  counts, and the difference matters: nineteen was a claim about the bytes
  that nobody had checked. The verifier's probes are now the repository's
  tests (`crates/asm/tests/prefix_bytes.rs` and
  `crates/asm/tests/jump_rewrite_bytes.rs`), so the claim is pinned.
  **The two advisories that are not already open.** The glued form
  `cs:mov ax,1` is still refused - pre-existing, identical at every commit,
  and already named in L7. And `lock lock xchg ax,bx` gives `F0h F0h`: the
  map's second `LOCK` cell is `F1h`, so a second `LOCK` is not the same
  byte as the first. No example writes two `LOCK`s, and S10 names `F0h`,
  so the assembler does what the spec says; the map's `F1h` cell is a hole
  of its own.

- **L7 (2026-10-21).** S10, the prefixes, and the hole the gate could not
  see.
  **The defect.** The prefixes were parsed and thrown away: `rep movsb`
  assembled to `A4h` without its `F3h`, and `repe cmpsb` to `A6h` without
  its `F3h`. Six of the eighty-five examples carried a prefix - eleven
  instructions in all - and every one of them had the wrong bytes. The gate
  could not see it, because the gate compares against a base that loses the
  prefixes too: the defect is invisible to a differential check against
  itself, and only a differential check against a *correct* assembler would
  catch it.
  **The bytes come from the map.** `REP/REPE/REPZ` is `F3`, `REPNE/REPNZ` is
  `F2`, the first `LOCK` is `F0` (`F1` is the same prefix a second time),
  and the segment overrides are the cells that say `SEG CS:`, `SEG SS:`,
  `SEG DS:` and `SEG ES:`. Nothing is written out in the assembler, which is
  S1's promise kept a second time.
  **A prefix moves the opcode.** So the instruction's size grows, the count
  pass counts it for free (it takes the size from the bytes it assembles),
  and a jump's displacement has to be measured from the **opcode's** address
  and not from the line's - which is also where the out-of-range
  replacement of L6 measures from now.
  **The proof.** Seventy-nine of the eighty-five examples are byte-identical
  to the base commit; the six that changed are exactly the six that carry a
  prefix, and the byte-level diff shows eleven insertions - one per prefixed
  instruction - and nothing else that is not a displacement.
  **The layout got easier.** The worst case is now four count walks where
  the base needed five, because a prefixed instruction is longer and pushes
  the addresses it does not reach.
  **Still open.** The glued form `cs:mov ax,1` - a segment override with no
  space - is still mangled by `label_first()`, and it is mangled in the base
  too; no example uses it, and it is a parsing change and not an emission
  one. `PUSHA`/`POPA` carry the prefix but the reference's own expansion
  does not (L5's divergence). The `E9h` displacement is still sixteen-bit
  signed, and the reference past 32767 bytes is unknown.

- **L6 (2026-10-21).** S9, the reference's own workaround for a jump that is
  too far, and the count pass that it forces.
  **The rule, from the reference's files.** `version.txt` (v4.02): "short
  conditional jumps are replaced with the opposite instruction and one normal
  jump". Tutorial 07: "making total of 5 bytes instead of just 2", and
  `writebin.asm:65`: "this instruction is assembled into OR CX, CX and two
  JMPs". So the `70h`-`7Fh` row becomes the opposite opcode (`opcode ^ 1`,
  which is the opposite for every pair of that row) with a displacement of
  `+3` and then `E9h`; and `JCXZ` becomes `OR CX,CX`, `JNZ` and `E9h`.
  `OR CX,CX` and `JNZ` are built by the assembler's own matcher, not written
  by hand - and that is what caught the mistake in S9's own text: `OR CX,CX`
  is `09h C9h`, because the R/M field names `CX` as `001`, and the `09h C8h`
  I had written would have been `OR AX,CX`.
  **The count pass.** A jump's size now depends on how far its target is, so
  the walk that counts the addresses has to know the size the walk that makes
  the bytes will produce. Two things make that work: a target the program has
  not declared yet stays a **name** in the count walk instead of becoming a
  zero (a zero would be a guess, and the guess would go through the range
  check), and the walks repeat until the names stop moving - five passes at
  the most for the examples, where the base needed three, because every
  replacement is three bytes longer and can push a later jump out of reach as
  well. A program whose addresses do not settle is refused.
  **The proof.** The 81 examples that already assembled are byte-identical to
  the base commit; only the four that now assemble differ, and their
  fourteen rewrites (twelve of the five-byte form and two of the `JCXZ`
  form) land exactly on their own label.
  **Still open.** The prefixes are parsed but never emitted: `rep movsb`
  assembles to `A4h` without the `F3h`. It is pre-existing, and emitting the
  prefix would change the bytes of five other examples, so it is its own
  unit. The `E9h` displacement is sixteen-bit signed, and the reference's
  behaviour past 32767 bytes is unknown.

- **L5 (2026-10-20).** Four bugs, not four dialects, and one wrong
  premise of mine that the reference's own files settled.
  **The bugs.** (1) `macros::replace_word` tested the **last** character of
  the rest of a line where a word's trailing edge is the **first** one, so
  a macro parameter followed by a comma was never substituted -
  `s_dcl DB sdat, 0` kept its parameter. (2) `ENDM` was read as a whole
  line, while the reference writes `<name> ENDM` the way it writes
  `<name> MACRO`; every line after a macro definition was therefore dropped
  from the program, which is why `writebin.asm` had no `clear_screen` and
  why `string.asm` assembled to **zero bytes** while the gate called it a
  pass. (3) `offset` is a term of an expression and not a whole operand, so
  the whole-operand shortcut swallowed `offset msg_end - offset msg` before
  the expression reader ever saw it. (4) There was no `include` at all; it
  is textual, it happens before the macros are read, and it looks in the
  file's own directory first and in the caller's include directories after.
  **The premise.** I had read the reference's binary and found its message
  "conditional jump is out of range: allowed values are from -128 to 127",
  and concluded that the four examples with an out-of-range conditional jump
  do not assemble in the reference either. The reference's own changelog
  (`re/extracted/app/version.txt`, v4.02) says the opposite: "automatic
  workaround for short relative jumps updated, to overcome jump limit of
  +/-127 bytes, short conditional jumps are replaced with the opposite
  instruction and one normal jump"; `documentation/asm_tutorial_07.html`
  says the same in prose, "making total of 5 bytes instead of just 2"; and
  `documentation/compiler.html` calls the error a bug of an older
  single-pass compiler. `writebin.asm:65` says it outright: "this
  instruction is assembled into OR CX, CX and two JMPs". The message is in
  the binary and the binary does not use it that way. The four refusals are
  ours, not the reference's, and the rewrite is T7's next unit.
  **A divergence worth naming.** `PUSHA`/`POPA` are emitted as the seven
  instructions they stand for (T6's decision: the 8086 has neither), while
  the reference emits `60h`/`61h`; our layout is therefore twelve bytes
  longer per expansion, which moves the distances a jump has to reach and
  will matter when the goldens arrive.

- **L4 (2026-10-20).** The dialect of the reference's own assembler, read
  out of its 85 examples and its macro library, and the places where the
  1979 print and the chip disagree.
  - The reference writes a macro as `PUTC MACRO char`: the **name comes
    first** and the word `MACRO` follows it. The guide's group tables
    carry the operation with a slash (`SHL/SAL`), so a name has to be read
    as a set, not as one word.
  - A label may stand where the instruction goes, with the instruction
    after it (`str1 db 'abc'`), and the label may be written with or
    without a colon.
  - `LOCAL` names the labels that belong to one expansion, and each
    expansion needs its own.
  - A name on its own is an address (`mov m, al` is the address of `m`),
    and `offset <label>` asks for that address as a value.
  - **The 8086's 8-bit R/M field reaches all eight byte registers, and the
    1979 print says otherwise.** The captures from a real P80C86A-2 show
    `88 F4` running `MOV AH,DL`, whose ModR/M byte has `100` in the R/M
    field. The guide's legend still says "AH, CH, DH and BH are reachable
    only through r8"; the print is what is wrong here, and the chip wins.

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
