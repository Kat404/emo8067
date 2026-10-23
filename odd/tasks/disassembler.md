# T14 — the disassembler

Objective: a disassembler that turns raw machine code into the reference's own
canonical text. Branch: `main`. Delivery: one commit per task, each leaving the
tree green. Test runner: `cargo test --workspace`.

Written 2026-10-22, after T12 (the runner) and T13 (the assembler). This is the
unit the TUI's first panel needs: a debugger that shows code needs to read it.

## Why this and not more

The project has the instruction table once already: the assembler reads §1.3 of
`odd/research/documentation-guide.md` at compile time. A disassembler is the
same table read the other way, so it belongs in the same crate and reads the
same document — "the instruction table exists once in the project instead of
twice" is a promise the disassembler should keep, not break. The reference
itself keeps them together: the assembler is `MicroAsm.dll` and the
disassembler is `diasm.dll`, both reading the same map.

The reference's own output format is recovered verbatim in `re/FINDINGS.md` —
the print formats, the house style, the x87 table — so this unit has a
specification, not a guess.

## Specs

- **S1** — The mnemonic comes from the guide's §1.3, the same table the
  assembler reads, and the operands are rendered in the reference's own
  formats (`re/FINDINGS.md` §Operand/address print formats, verbatim):
  ```
  %s 0%04Xh                       mnemonic + absolute address
  %s%s[0%02X%02Xh]                bracket form, e.g. MOV [0A0B h], 1
  %s%s0%02X%02Xh                  bare 16-bit hex
  %s%s0%02X%02Xh:0%02X%02Xh       logical seg:offset
  %s%s0%Xh                        8-bit hex
  %s, [0%02X%02Xh]   %s, %s0%02X%02Xh   %s%s0%Xh, %s   %s, %s%s0%Xh
  %s, %s   [0%02X%02Xh], %s   %s0%02X%02Xh, %s   EXT ???
  ```
- **S2** — The house style: hex is `0%02X%02Xh` — a leading `0`, two-digit
  bytes, a lowercase `h` suffix — and a logical address always renders as
  `SSSS:OOOO`. The eight-bit form is `0%Xh`.
- **S3** — The `D8`–`DF` range is an ESC index and not an illegal opcode:
  the reference decodes the 80187 mnemonics **for display only** and does not
  execute them. The table is in `re/FINDINGS.md` §x87, with the fallbacks
  `F??? st%d` and `??? st%d`.
- **S4** — An undefined opcode says what the reference says: `"no such opcode
  in 8086 instruction set"`.
- **S5** — A BIOS stub is decoded as `BIOS DI`: the `FFFF` prefix in front of
  `CD` is the BIOS device interrupt, so `FFFFCD10` reads as interrupt `10h`
  and not as `int 0FFh`.
- **S6** — The runner disassembles. `emo8067 disasm <file> [options]` prints
  a listing, one instruction per line: the logical address, the bytes, and
  the reference's canonical text — the same shape as the TUI's disassembly
  panel (`SSSS:OOOO  BB BB  TEXT`, the shape in
  `odd/research/features-and-ux.md` §4.4). A `.asm` is assembled first, by
  the same rule `run` uses (the extension decides, `--include` names the
  include path, the load model is the reference's own `.binf` model), and a
  `.bin` is loaded raw. The options are where to start (`--at`), how many
  instructions (`--count`) and where to load a raw image (`--load`).
  **Why it matters.** A disassembler nobody can call is a library and not a
  tool, and this is the first panel the TUI will draw: the listing this
  prints is the panel the debugger will draw.

## Tasks

- **T1** — **DONE.** The decoder: the opcode from the guide's §1.3, the
  ModR/M from the reference's own effective-address table, the immediate and
  the displacement, and the text in the reference's formats (S1, S2, S4).
  `crates/asm/src/disasm.rs`: `disassemble(bytes, address) ->
  Result<Instruction, Error>`, with `Instruction { text, length }`, plus
  `disassemble_all` for a listing and `instruction_of(opcode)`.
- **T2** — **DONE.** The x87 range for display (S3) in
  `crates/asm/src/x87.rs`, the BIOS stub (S5), and the round trip: the bytes
  the disassembler prints assemble back to the same bytes.
- **T3** — **IN PROGRESS.** The runner learns to disassemble (S6): the
  `disasm` command, its listing and its options.

## Log

- **L1 (2026-10-22).** What the reference's disassembler is, from the
  recovered findings: a separate 200-function MSVC DLL (`diasm.dll`) that
  decodes raw machine code without source, with its own ModR/M table, its own
  print formats and its own x87 table. The features inventory names four
  things to inherit: the native disassembler (DIS-1), the house print format
  (DIS-2), the BIOS `DI` decoding (DIS-3) and the source-line mapping
  (DIS-4) — and **DIS-4 is not this unit**: it is a `.debug` sidecar, a file
  format, not a decoder.
  **The golden problem, stated plainly.** The reference's own disassembled
  output cannot be obtained here: the reference is a PE32 Windows program and
  there is no wine. So the expectations are ours, written from the formats in
  `re/FINDINGS.md` and the guide's §1.3, and the gate is that the text follows
  the house style and the round trip holds — not that it matches bytes we have
  never seen. The round trip is the strong half: the disassembler's text must
  assemble back to the same bytes, which is a property of the pair and not a
  claim about the reference.

- **L2 (2026-10-22).** What the decoder decided, and what it could not.
  **The house format is five characters wider than the document's example.**
  The recovered format string is `0%02X%02Xh` — a literal `0`, then the two
  bytes, then the `h` — so a sixteen-bit value renders as `01234h`, six
  characters. The document's own prose example says `MOV [0A0B h], 1`, four
  digits and a stray space. **The format string wins**, because it is the
  reference's own artifact and the example is the document's prose; the
  difference is one character in one function, and it is recorded here so
  nobody "fixes" it later.
  **Two forms the recovered list names are unreachable.** `%s0%02X%02Xh, %s`
  wants a sixteen-bit port, and the guide's `OUT` takes an eight-bit one
  (`OUT i8,AL`). `EXT ???` wants an escape opcode that is not the x87 range,
  and the guide's only escapes are `ESC 0`–`ESC 7`, which the x87 table
  answers for.
  **S4 cannot fire through the decoder.** The guide's map defines all 256
  first opcodes, so there is no undefined opcode to refuse; the message is
  kept, pinned by a test, and the ESC range is the only hole.
  **The x87 is names, not bytes.** The recovered list in `re/FINDINGS.md` is a
  prose sentence naming forty-six mnemonics, and it is tracked in git — so
  the crate reads it at compile time with `include_str!` and quotes the
  reference's own spellings. But the list carries **no opcode mapping**, and
  it is a partial recovery: no `FILD`, `FIST`, `FABS`, `FTST`, `FXAM`,
  `FLD1`, `FLDZ` and no control words. So the encoding-to-name map lives in
  the crate and the document supplies the spellings. The 80187
  transcendentals (`FSCALE`, `FRNDINT`, `FSINCOS`, `FSQRT`, `FYL2XP1`,
  `FPREM`, `FPREM1`, `FXTRACT`) are in the name list but their encodings are
  not recoverable from it, so they fall back.
  **The round trip: three hundred and twenty-four of three hundred and
  twenty-four.** The corpus is the guide's whole table — the 256 cells plus
  every operation of the three group tables — assembled, disassembled and
  assembled again: the same bytes, every time. The bridge between the
  reference's spelling and the assembler's reader is three places: the size
  prefix (`BYTE PTR` / `WORD PTR`, printed when the form's other operands do
  not fix the width), the `SHORT` qualifier on a short jump (the guide's
  `JMP short (intra)` and `JMP near` are otherwise indistinguishable to the
  assembler), and a **signed** displacement (`[BP-02h]`: the magnitude in
  the house form, the sign in front).
  **What cannot come back, and why.** The assembler always writes the *first*
  form in the guide's table whose operands match, so the encodings it never
  writes cannot round trip: the alias row `60h`–`6Fh` (it writes `70h`–
  `7Fh`), the `82h`/`83h` rows (it writes `80h`/`81h`), the direct-address
  forms `A0h`–`A3h` (it writes the r/m form, one byte longer), `FF /2`–`/5`
  (intra and inter take the same operands), and the `F1h` second `LOCK` (it
  writes `F0h`). A test names them, so a change is noticed.
  **The examples are not the round-trip corpus.** A program mixes code and
  data, and the data bytes are not instructions. The corpus is the guide's
  own table instead; the eighty-five examples still assemble (that gate is
  untouched).

