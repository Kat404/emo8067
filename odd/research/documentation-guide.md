# emu8067 — Documentation and guides research (T9a)

Research track for building `odd/research/documentation-guide.md`. English, neutral
register. This is a technical artifact, not a chat reply.

Every factual claim carries an inline `[N]` that resolves in `## Sources`. Claims
are tagged:

| Tag | Meaning |
|---|---|
| **verified** | Read directly from the cited source during this research pass. |
| **inferred** | Derived by combining cited sources, or a design judgement. Not stated verbatim anywhere. |
| **unknown** | Could not be established from a reachable source. Recorded as a gap, never filled from memory. |

Primary authoritative source for the 8086 itself is the scanned
**Intel 8086 Family User's Manual, October 1979** (part number 9800722-03),
preserved by Bitsavers on the Internet Archive [1]. All opcode, flag and
hardware-behaviour claims in sections 1–3 come from that manual. Page numbers
below are PDF page indices of the Internet Archive `Text PDF` of that scan, and
the printed page number is given in parentheses.

**Reproduction note for section 1.** The scanned manual has no text layer. The
flag rows were recovered by piping the PDF through `pdftotext -bbox-layout` and
reconstructing each flag column from the x-coordinates of the `ODITSZAPC` header
word against the marks on the following line. The reconstruction is
self-consistent and matches the independently known semantics of each
instruction; it is still a transcription, and the emulator's flag tests should be
written against the semantics stated here, not against this pipeline.

---

## 1. Opcode reference

Source for the whole section: Intel 8086 Family User's Manual, Chapter 4
"Hardware Reference Information" [1]:

- Table 4-7 Single-Bit Field Encoding (PDF p. 258 / printed 4-19) [1]
- Table 4-8 MOD field encoding (PDF p. 259 / printed 4-20) [1]
- Table 4-9 REG field encoding (PDF p. 259 / printed 4-20) [1]
- Table 4-10 R/M field encoding and effective-address calculation (PDF p. 259 / printed 4-20) [1]
- Table 4-11 Key to machine instruction encoding and decoding (PDF pp. 259–260 / printed 4-20–4-21) [1]
- Table 4-12 8086 Instruction Encoding (PDF pp. 260–266 / printed 4-22 onward) [1]
- Table 4-13 Machine Instruction Decoding Guide, ordered by first-byte binary value (PDF pp. 265–274 / printed 4-35 onward) [1]
- Table 4-14 Machine Instruction Encoding Matrix, the full 16×16 opcode grid (PDF pp. 274–275 / printed 4-35–4-36) [1]
- Table 2-21 Instruction Set Reference Data, including exact flag effects and instruction lengths in bytes (PDF pp. 73–91 / printed 2-51 onward) [1]
- Table 2-2 Logical Address Sources (PDF p. 36 / printed 2-13) [1]

### 1.1 Single-bit instruction fields

Table 4-7 [1] — **verified**:

| Field | 0 | 1 |
|---|---|---|
| `S` | no sign extension | sign-extend the 8-bit immediate to 16 bits when `W=1` |
| `W` | byte operation | word operation |
| `D` | source is the REG field | destination is the REG field |
| `V` | shift/rotate count is 1 | shift/rotate count is in `CL` |
| `Z` | repeat/loop while ZF is clear | repeat/loop while ZF is set |

### 1.2 Addressing modes and instruction length

Table 4-8, MOD field [1] — **verified**:

| MOD | Meaning |
|---|---|
| `00` | Memory mode, no displacement follows — **except when R/M = 110, then a 16-bit displacement follows** |
| `01` | Memory mode, 8-bit displacement follows (auto sign-extended to 16 bits) |
| `10` | Memory mode, 16-bit displacement follows |
| `11` | Register mode, no displacement |

Table 4-10, R/M field [1] — **verified**:

| R/M | MOD=11 (W=0) | MOD=11 (W=1) | MOD=00 | MOD=01 | MOD=10 |
|---|---|---|---|---|---|
| `000` | AL | AX | `[BX+SI]` | `[BX+SI]+D8` | `[BX+SI]+D16` |
| `001` | CL | CX | `[BX+DI]` | `[BX+DI]+D8` | `[BX+DI]+D16` |
| `010` | DL | DX | `[BP+SI]` | `[BP+SI]+D8` | `[BP+SI]+D16` |
| `011` | BL | BX | `[BP+DI]` | `[BP+DI]+D8` | `[BP+DI]+D16` |
| `100` | AH | SP | `[SI]` | `[SI]+D8` | `[SI]+D16` |
| `101` | CH | BP | `[DI]` | `[DI]+D8` | `[DI]+D16` |
| `110` | DH | SI | **direct address** (`D16`, no base/index) | `[BP]+D8` | `[BP]+D16` |
| `111` | BH | DI | `[BX]` | `[BX]+D8` | `[BX]+D16` |

Consequences the emulator must implement (all **verified** from Table 4-10):

- **MOD=00 has exactly eight effective-address forms**, one of which is the
  bare 16-bit displacement. `mod=00, rm=111` is `[BX]` and nothing else; the
  `[BX]+D16` form lives at `mod=10, rm=111` (`[BX]+D16`). There is no
  `(disp16)` alias at `mod=00, rm=111` — that alias belongs to the 80186, not
  the 8086. Do not accept it. [1]
- **AH/CH/DH/BH are unencodable in a 16-bit `reg` field.** Table 4-9 maps REG
  `100/101/110/111` to `SP/BP/SI/DI` when `W=1`; the high-byte registers only
  appear when `W=0`. So an operand that must be a 16-bit register cannot be AH,
  CH, DH or BH, and an assembler that offers `MOV AH, [BX]` must emit the
  `MOV r8, r/m8` form. [1]
- An 8-bit displacement is sign-extended to 16 bits before address calculation;
  immediates always follow displacements, and the high-order byte of a two-byte
  value is stored second. [1]
- Segment default per memory reference, Table 2-2 [1] — **verified**:

| Memory reference | Default segment | Permitted override |
|---|---|---|
| Instruction fetch | `CS` | none |
| Stack operation | `SS` | none |
| Variable (except BP as base) | `DS` | `CS`, `ES`, `SS` |
| String source | `DS` | `CS`, `ES`, `SS` |
| String destination | `ES` | none |
| **BP used as base register** | **`SS`** | `CS`, `DS`, `ES` |

**Instruction length** — **verified**: an instruction is 1 to 6 bytes. "Bytes 3
through 6 of an instruction are optional fields that usually contain the
displacement value of a memory operand and/or the actual value of an immediate
constant operand" [1], and the machine instruction format diagram shows exactly
six fields: opcode, `mod r/m reg`, low `disp/data`, high `disp/data`, low data,
high data [1]. Per-instruction byte counts are given in the "Bytes" column of
Table 2-21 [1] (for example `MOV r/m16, i16` = 3–6 bytes, `MOV r/m8, i8` =
2–4 bytes, `ADD AL, i8` = 2 bytes).

### 1.3 Full 256-opcode map

Transcribed from **Table 4-13, Machine Instruction Decoding Guide** [1] (PDF pp.
266–274 / printed 4-27 to 4-35), which gives the ASM-86 instruction format for
every first byte in order, and cross-checked against **Table 4-12, 8086
Instruction Encoding** [1] (PDF pp. 261–265 / printed 4-22 to 4-26) and **Table
4-14, Machine Instruction Encoding Matrix** [1] (PDF pp. 275–276 / printed 4-36
to 4-37). One row per opcode `0x00`–`0xFF`; every row has exactly **sixteen**
cells. `odd/research/verify_opcodes.py` enforces that count and exits non-zero on
a short row, because one missing cell shifts every later opcode by one and
silently corrupts the decoder.

Notation, from Table 4-11 and Table 4-19 [1]:

| Token | Meaning (manual identifier) |
|---|---|
| `r8` / `r16` | `REG8` / `REG16` — the REG field only, so `AH`/`CH`/`DH`/`BH` are reachable only through `r8` |
| `r/m8` / `r/m16` | `MEM8` / `MEM16` — the R/M field, so register **or** memory depending on MOD |
| `m8` / `m16` | memory reached through `ADDR-LO`/`ADDR-HI`, a direct address; no EA is calculated |
| `i8` / `i16` | `IMMED8` / `IMMED16`; `i8:sx` is `DATA-SX`, an 8-bit immediate auto sign-extended to 16 bits |
| `sreg` | `SEGREG`, selected by `SR` = `00` ES, `01` CS, `10` SS, `11` DS |
| `short` | `SHORT-LABEL`, within ±127 bytes of the end of the instruction |
| `near` / `far` | `NEAR-LABEL`/`NEAR-PROC` (current code segment) / `FAR-LABEL`/`FAR-PROC` (another segment) |
| `—` | the 8086 does not define this encoding; Table 4-13 prints `(not used)` |

| | +0 | +1 | +2 | +3 | +4 | +5 | +6 | +7 | +8 | +9 | +A | +B | +C | +D | +E | +F |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **00** | ADD r/m8,r8 | ADD r/m16,r16 | ADD r8,r/m8 | ADD r16,r/m16 | ADD AL,i8 | ADD AX,i16 | PUSH ES | POP ES | OR r/m8,r8 | OR r/m16,r16 | OR r8,r/m8 | OR r16,r/m16 | OR AL,i8 | OR AX,i16 | PUSH CS | POP CS ‡1 |
| **10** | ADC r/m8,r8 | ADC r/m16,r16 | ADC r8,r/m8 | ADC r16,r/m16 | ADC AL,i8 | ADC AX,i16 | PUSH SS | POP SS | SBB r/m8,r8 | SBB r/m16,r16 | SBB r8,r/m8 | SBB r16,r/m16 | SBB AL,i8 | SBB AX,i16 | PUSH DS | POP DS |
| **20** | AND r/m8,r8 | AND r/m16,r16 | AND r8,r/m8 | AND r16,r/m16 | AND AL,i8 | AND AX,i16 | SEG ES: | DAA | SUB r/m8,r8 | SUB r/m16,r16 | SUB r8,r/m8 | SUB r16,r/m16 | SUB AL,i8 | SUB AX,i16 | SEG CS: | DAS |
| **30** | XOR r/m8,r8 | XOR r/m16,r16 | XOR r8,r/m8 | XOR r16,r/m16 | XOR AL,i8 | XOR AX,i16 | SEG SS: | AAA | CMP r/m8,r8 | CMP r/m16,r16 | CMP r8,r/m8 | CMP r16,r/m16 | CMP AL,i8 | CMP AX,i16 | SEG DS: | AAS |
| **40** | INC AX | INC CX | INC DX | INC BX | INC SP | INC BP | INC SI | INC DI | DEC AX | DEC CX | DEC DX | DEC BX | DEC SP | DEC BP | DEC SI | DEC DI |
| **50** | PUSH AX | PUSH CX | PUSH DX | PUSH BX | PUSH SP | PUSH BP | PUSH SI | PUSH DI | POP AX | POP CX | POP DX | POP BX | POP SP | POP BP | POP SI | POP DI |
| **60** | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — |
| **70** | JO short | JNO short | JB/JNAE short | JNB/JAE short | JE/JZ short | JNE/JNZ short | JBE/JNA short | JNBE/JA short | JS short | JNS short | JP/JPE short | JNP/JPO short | JL/JNGE short | JNL/JGE short | JLE/JNG short | JNLE/JG short |
| **80** | Group 1 ‡2 | Group 1 | Group 1 ‡2 | Group 1 | TEST r/m8,r8 ‡3 | TEST r/m16,r16 ‡3 | XCHG r8,r/m8 | XCHG r16,r/m16 | MOV r/m8,r8 | MOV r/m16,r16 | MOV r8,r/m8 | MOV r16,r/m16 | MOV r16/m16,sreg | LEA r16,r/m16 | MOV sreg,r16/m16 | POP r16/m16 |
| **90** | XCHG AX,AX | XCHG AX,CX | XCHG AX,DX | XCHG AX,BX | XCHG AX,SP | XCHG AX,BP | XCHG AX,SI | XCHG AX,DI | CBW | CWD | CALL far | WAIT | PUSHF | POPF | SAHF | LAHF |
| **A0** | MOV AL,m8 | MOV AX,m16 | MOV m8,AL | MOV m16,AX ‡4 | MOVS b | MOVS w | CMPS b | CMPS w | CMP AL,i8 ‡5 | CMP AX,i16 ‡5 | STOS b | STOS w | LODS b | LODS w | SCAS b | SCAS w |
| **B0** | MOV AL,i8 | MOV CL,i8 | MOV DL,i8 | MOV BL,i8 | MOV AH,i8 | MOV CH,i8 | MOV DH,i8 | MOV BH,i8 | MOV AX,i16 | MOV CX,i16 | MOV DX,i16 | MOV BX,i16 | MOV SP,i16 | MOV BP,i16 | MOV SI,i16 | MOV DI,i16 |
| **C0** | — | — | RET i16 (intra) | RET (intra) | LES r16,r/m16 | LDS r16,r/m16 | MOV m8,i8 | MOV m16,i16 | — | — | RET i16 (inter) | RET (inter) | INT 3 | INT i8 | INTO | IRET |
| **D0** | Shift ‡6 | Shift | Shift | Shift | — | AAM i8 | AAD i8 | XLAT | ESC 0 | ESC 1 | ESC 2 | ESC 3 | ESC 4 | ESC 5 | ESC 6 | ESC 7 |
| **E0** | LOOPNE/LOOPNZ short | LOOPE/LOOPZ short | LOOP short | JCXZ short | IN AL,i8 | IN AX,i8 | OUT i8,AL | OUT i8,AX | CALL near | JMP near | JMP far | JMP short (intra) | IN AL,DX | IN AX,DX | OUT DX,AL | OUT DX,AX |
| **F0** | LOCK | — | REPNE/REPNZ | REP/REPE/REPZ | HLT | CMC | Group 1 | Group 1 | CLC | STC | CLI | STI | CLD | STD | Group 2 ‡7 | Group 2 ‡7 |

**Group 1 — `80`/`81`/`82`/`83` against an immediate, and `F6`/`F7`** [1] —
**verified** (Table 4-13, PDF pp. 269–270 / printed 4-30–4-31 and PDF p. 273 /
printed 4-34):

| REG | `80` byte, `i8` | `81` word, `i16` | `82` byte, `i8` | `83` word, `i8:sx` | `F6` byte | `F7` word |
|---|---|---|---|---|---|---|
| `000` | ADD r/m8,i8 | ADD r/m16,i16 | ADD r/m8,i8 | ADD r/m16,i8:sx | TEST r/m8,i8 | TEST r/m16,i16 |
| `001` | — | — | — | — | — | — |
| `010` | ADC r/m8,i8 | ADC r/m16,i16 | ADC r/m8,i8 | ADC r/m16,i8:sx | NOT r/m8 | NOT r/m16 |
| `011` | SBB r/m8,i8 | SBB r/m16,i16 | SBB r/m8,i8 | SBB r/m16,i8:sx | NEG r/m8 | NEG r/m16 |
| `100` | AND r/m8,i8 | AND r/m16,i16 | — | — | MUL r/m8 | MUL r/m16 |
| `101` | SUB r/m8,i8 | SUB r/m16,i16 | SUB r/m8,i8 | SUB r/m16,i8:sx | IMUL r/m8 | IMUL r/m16 |
| `110` | XOR r/m8,i8 | XOR r/m16,i16 | — | — | DIV r/m8 | DIV r/m16 |
| `111` | CMP r/m8,i8 | CMP r/m16,i16 | CMP r/m8,i8 | CMP r/m16,i8:sx | IDIV r/m8 | IDIV r/m16 |

`83` takes `DATA-SX`, an 8-bit immediate sign-extended to 16 bits (`data if s,
w=1`, Table 4-12, PDF p. 261 / printed 4-22); there is no 8-bit `83` form. `82`
is defined on the 8086 and carries the same digit table as `80` except that
`/1`, `/4` and `/6` print `(not used)` [1].

**Shift — `D0`/`D1`/`D2`/`D3`** [1] — **verified** (Table 4-13, PDF p. 272 /
printed 4-33):

| REG | `D0` byte, count 1 | `D1` word, count 1 | `D2` byte, count CL | `D3` word, count CL |
|---|---|---|---|---|
| `000` | ROL r/m8,1 | ROL r/m16,1 | ROL r/m8,CL | ROL r/m16,CL |
| `001` | ROR r/m8,1 | ROR r/m16,1 | ROR r/m8,CL | ROR r/m16,CL |
| `010` | RCL r/m8,1 | RCL r/m16,1 | RCL r/m8,CL | RCL r/m16,CL |
| `011` | RCR r/m8,1 | RCR r/m16,1 | RCR r/m8,CL | RCR r/m16,CL |
| `100` | SHL/SAL r/m8,1 | SHL/SAL r/m16,1 | SHL/SAL r/m8,CL | SHL/SAL r/m16,CL |
| `101` | SHR r/m8,1 | SHR r/m16,1 | SHR r/m8,CL | SHR r/m16,CL |
| `110` | — | — | — | — |
| `111` | SAR r/m8,1 | SAR r/m16,1 | SAR r/m8,CL | SAR r/m16,CL |

**Group 2 — `FE`/`FF`** [1] — **verified** (Table 4-13, PDF p. 273 / printed
4-34). The manual prints `MEM16` for `FF /0` and `FF /1`; with `MOD=11` those
are register forms [1]:

| REG | `FE` | `FF` |
|---|---|---|
| `000` | INC r/m8 | INC MEM16 |
| `001` | DEC r/m8 | DEC MEM16 |
| `010` | — | CALL r16/m16 (intra) |
| `011` | — | CALL m16 (inter) |
| `100` | — | JMP r16/m16 (intra) |
| `101` | — | JMP m16 (inter) |
| `110` | — | PUSH m16 |
| `111` | — | — |

Notes on individual cells, all traceable to Table 4-13 unless stated otherwise
[1]:

- **`90` is `XCHG AX,AX`**, printed by the manual as "NOP (exchange AX,AX)". It
  is the canonical one-byte no-op and is also what an emulator must execute, not
  a decoder that skips the byte.
- **`86`/`87` are `XCHG REG8,REG8/MEM8` and `XCHG REG16,REG16/MEM16`** — the
  REG field is the source when `D=0` and the destination when `D=1`.
- **`C4` is `LES r16,r/m16`** and **`C5` is `LDS r16,r/m16`**; both read a
  four-byte pointer through the R/M field.
- **`EB` is `JMP SHORT-LABEL`**, an `IP-INC8` relative jump, i.e. a short
  **intra**-segment jump. The 8086 has no short inter-segment jump, so `EC` and
  `ED` are `IN AL,DX` and `IN AX,DX`, not the 80186 near-jump forms.
- **`D4` is `AAM` and `D5` is `AAD`**, each carrying a `00001010` immediate
  byte. **`D6` is `(not used)`** and **`D7` is `XLAT`**; the ESC range is `D8`
  to `DF`, so `ESC 0` is `D8` and `ESC 7` is `DF`, not `D6` to `DD`.
- **`F2` is `REPNE`/`REPNZ`** and **`F3` is `REP`/`REPE`/`REPZ`. `F1` is
  `(not used)` and `F4` is `HLT`** — `F4` is not an undefined slot.
- **`84`/`85` are `TEST r/m8,r8` and `TEST r/m16,r16`** in this manual; see
  footnote ‡3. **`C0`/`C1` are `(not used)`** — they are not shift-by-immediate
  on the 8086.
- **`8C` and `8E` accept `MOD=000` only**; `8C MOD 1--` and `8E MOD 1--` print
  `(not used)`. `8F` likewise takes `MOD 000` only, and `8F MOD 001` through
  `8F MOD 111` are all `(not used)`.
- **`60`–`6F` are all `(not used)`**, and `C8`/`C9` are `(not used)` — the 8086
  has no `ENTER`/`LEAVE`.
- The reference implementation's own diagnostic for an undefined opcode is
  `"no such opcode in 8086 instruction set"` [19]. For a disassembler the
  `D8`–`DF` range is an 8087/8088 ESC index rather than an illegal opcode [1];
  the reference implementation decodes the 80187 mnemonics for display only and
  does not execute them [19].

**Discrepancies found while transcribing.** Every cell carrying a footnote is a
place where Table 4-13 does not agree with the implementation or with the
standing instruction for this section. None of them is settled by this manual;
they are recorded rather than silently reconciled.

- **‡1 `0F`.** Table 4-13, PDF p. 267 / printed 4-28 prints `0F` as `(not used)`.
  The cell records `POP CS`, which is what `crates/core/src/exec.rs` executes
  (`segment_push_pop`, `exec.rs:857`). **Unresolved from this source; do not
  change the decoder on the strength of the printed `(not used)`.**
- **‡2 `82`.** Table 4-13 defines `82` for `/0`, `/2`, `/3`, `/5` and `/7` and
  prints `(not used)` for `/1`, `/4`, `/6`. Later Intel documentation describes
  `82` as an alias of `80` on the 8086 and as invalid afterwards. Transcribed as
  the manual prints it.
- **‡3 `84`/`85`.** Table 4-13, PDF p. 270 / printed 4-31 defines both as
  `TEST REG8/MEM8,REG8` and `TEST REG16/MEM16,REG16`. The standing instruction
  for this section is that `84`/`85` are 80186 extensions absent from the 8086.
  The two disagree and the manual's table is the only evidence available here,
  so the cells record the manual and the disagreement is flagged. **These two
  cells must not be implemented until the conflict is settled.**
- **‡4 `A3`.** Table 4-13, PDF p. 271 / printed 4-32 prints `A3` as
  `MOV MEM16,AL`, which cannot be a 16-bit move. Table 4-12, PDF p. 261 /
  printed 4-22 gives "Accumulator to memory" as `1010001w`, i.e. `A2` is
  `MOV m8,AL` and `A3` is `MOV m16,AX`. The cell follows Table 4-12; the
  Table 4-13 print is a typographical error in the manual.
- **‡5 `A8`/`A9`.** Table 4-12, PDF p. 261 / printed 4-22 (`CMP` "Immediate with
  accumulator" = `0011101w`, i.e. `3C`/`3D`) and Table 4-13, PDF p. 271 /
  printed 4-32 (`TEST` "Immediate data and accumulator" = `1010100w`) **both**
  print `A8`/`A9` as `TEST AL,IMMED8` and `TEST AX,IMMED16`. The standing
  instruction for this section is that `A8`/`A9` are `CMP` against an immediate
  with no ModR/M byte, and `crates/core/src/exec.rs` implements
  `cmp_accumulator` for both (`exec.rs:259`–`exec.rs:260`). The cells record
  `CMP` because the two differ in exactly the flags they write — `TEST` clears
  CF and OF and leaves AF undefined, `CMP` sets all six — so a cell reading
  `TEST` here would silently corrupt memory. **The manual print is recorded
  rather than followed; this is a known 1979 erratum, not a transcription slip.**
- **‡6 `D0`–`D3`.** Table 4-14 labels these four cells `Shift` and Table 4-13
  expands them into the `/digit` table above. The manual assigns this set no
  group number, so it is named `Shift` here rather than folded into "Group 2",
  which the manual reserves for `FE`/`FF` (Table 4-14's key).
- **‡7 `FE`/`FF`.** Table 4-13 prints `MEM16` for `FF /0` and `FF /1` although
  the R/M field with `MOD=11` selects a register. The digit table follows the
  manual; the register reading is noted above.

### 1.4 Exact flag effects

Transcribed from **Table 2-21, Instruction Set Reference Data** [1] (PDF pp.
74–90 / printed 2-51 to 2-67), column order `ODITSZAPC`. Legend is **Table 2-18,
Key to Flag Effects** [1] (PDF p. 73 / printed 2-50):

| Mark | Meaning (Table 2-18) |
|---|---|
| *(blank)* → `—` | not altered |
| `X` | set or cleared according to the result |
| `0` | cleared to 0 |
| `1` | set to 1 |
| `U` | undefined — contains no reliable value |
| `R` | restored from a previously saved value |

`—` and `U` are different and the emulator must keep them apart: `—` means the
instruction leaves the flag exactly as it found it, `U` means it leaves it in an
architecturally undefined state. A test must not assert a value for a `U` flag,
and a test may assert that a `—` flag is preserved.

Each row is the set of flags that holds across **every** operand form the
instruction has in Table 2-21. Where the forms differ, the note says so. All
rows **verified** except the three marked below.

| Instruction | O | D | I | T | S | Z | A | P | C | Note | PDF p. |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `AAA` | U | — | — | — | U | U | X | U | X | | 74 |
| `AAD` | U | — | — | — | X | X | U | X | U | CF undefined | 74 |
| `AAM` | U | — | — | — | X | X | U | X | U | CF undefined | 74 |
| `AAS` | U | — | — | — | U | U | X | U | X | | 74 |
| `ADC` | X | — | — | — | X | X | X | X | X | same for all six operand forms | 75 |
| `ADD` | X | — | — | — | X | X | X | X | X | same for all six operand forms | 75 |
| `AND` | 0 | — | — | — | X | X | U | X | 0 | CF and OF cleared, AF undefined | 75 |
| `OR` | 0 | — | — | — | X | X | U | X | 0 | CF and OF cleared, AF undefined | 85 |
| `XOR` | 0 | — | — | — | X | X | U | X | 0 | CF and OF cleared, AF undefined | 90 |
| `TEST` | 0 | — | — | — | X | X | U | X | 0 | no result written; AF undefined | 90 |
| `CMP` | X | — | — | — | X | X | X | X | X | no result written | 76 |
| `SUB` | X | — | — | — | X | X | X | X | X | same for all six operand forms | 90 |
| `SBB` | X | — | — | — | X | X | X | X | X | same for all six operand forms | 88 |
| `DAA` | X | — | — | — | X | X | X | X | X | | 77 |
| `DAS` | U | — | — | — | X | X | X | X | X | OF undefined | 77 |
| `INC` | X | — | — | — | X | X | X | X | — | **CF not affected**, reg16/reg8/memory all alike | 78 |
| `DEC` | X | — | — | — | X | X | X | X | — | **CF not affected**, reg16/reg8/memory all alike | 77 |
| `NEG` | X | — | — | — | X | X | X | X | 1 | CF set to 1; **0 if destination == 0** (footnote to the row) | 85 |
| `MUL` | X | — | — | — | U | U | U | U | X | only OF and CF architecturally defined | 84 |
| `IMUL` | X | — | — | — | U | U | U | U | X | only OF and CF architecturally defined | 78 |
| `DIV` | U | — | — | — | U | U | U | U | U | all six undefined | 77 |
| `IDIV` | U | — | — | — | U | U | U | U | U | all six undefined | 78 |
| `CMPS` | X | — | — | — | X | X | X | X | X | | 76 |
| `SCAS` | X | — | — | — | X | X | X | X | X | | 88 |
| `SHL`/`SAL` | X | — | — | — | — | — | — | — | X | table row as printed; **contradicted by the prose, see note 8** | 88 |
| `SHR` | X | — | — | — | — | — | — | — | X | table row as printed; **contradicted by the prose, see note 8** | 89 |
| `SAR` | X | — | — | — | X | X | U | X | X | **AF undefined**, the only shift/rotate that defines S, Z, P | 88 |
| `ROL` | X | — | — | — | — | — | — | — | X | | 87 |
| `ROR` | X | — | — | — | — | — | — | — | X | | 87 |
| `RCL` | X | — | — | — | — | — | — | — | X | | 86 |
| `RCR` | X | — | — | — | — | — | — | — | X | | 86 |
| `CLC` | — | — | — | — | — | — | — | — | 0 | | 76 |
| `STC` | — | — | — | — | — | — | — | — | 1 | | 89 |
| `CMC` | — | — | — | — | — | — | — | — | X | | 76 |
| `CLD` | — | 0 | — | — | — | — | — | — | — | | 76 |
| `STD` | — | 1 | — | — | — | — | — | — | — | | 89 |
| `CLI` | — | — | 0 | — | — | — | — | — | — | | 76 |
| `STI` | — | — | 1 | — | — | — | — | — | — | | 89 |
| `INT` | — | — | 0 | 0 | — | — | — | — | — | both `INT 3` and `INT i8`; IF and TF cleared | 79 |
| `INTO` | — | — | 0 | 0 | — | — | — | — | — | same marks as `INT` | 79 |
| `IRET` | R | R | R | R | R | R | R | R | R | all nine restored from the popped FLAGS word | 79 |
| `POPF` | R | R | R | R | R | R | R | R | R | all nine replaced from the popped word; prose at PDF p. 56 | 86 |
| `SAHF` | — | — | — | — | R | R | R | R | R | restores S, Z, A, P, C from AH; O, D, I, T untouched | 87 |
| `LAHF` | — | — | — | — | — | — | — | — | — | | 82 |
| `CALL` | — | — | — | — | — | — | — | — | — | near, far, memptr16, regptr16, memptr32 all blank | 75 |
| `JMP` | — | — | — | — | — | — | — | — | — | short, near, far, memptr16, regptr16 all blank | 81 |
| all `Jcc` | — | — | — | — | — | — | — | — | — | every conditional jump row is blank | 79–82 |
| `LOOP`, `LOOPE`/`LOOPZ`, `LOOPNE`/`LOOPNZ`, `JCXZ` | — | — | — | — | — | — | — | — | — | | 83 |
| `RET` | — | — | — | — | — | — | — | — | — | intra and inter, with and without pop value | 87 |
| `CBW`, `CWD` | — | — | — | — | — | — | — | — | — | | 75, 77 |
| `MOV` | — | — | — | — | — | — | — | — | — | every form blank, including the segment-register forms | 84 |
| `LEA`, `LES`, `LODSI` | — | — | — | — | — | — | — | — | — | | 82 |
| `LDS` | — | — | — | — | — | — | — | — | — | **ambiguous: Table 2-21 has no `LDS` entry at all**; see below | — |
| `PUSH`, `POP` | — | — | — | — | — | — | — | — | — | register, segment-register and memory forms all blank | 86, 85 |
| `PUSHF` | — | — | — | — | — | — | — | — | — | the flags themselves are not affected; prose at PDF p. 56 | 86 |
| `XCHG` | — | — | — | — | — | — | — | — | — | accumulator, memory and register forms all blank | 90 |
| `IN`, `OUT` | — | — | — | — | — | — | — | — | — | immediate-port and DX forms both blank | 78, 85 |
| `MOVS`, `LODS`, `STOS` | — | — | — | — | — | — | — | — | — | the repeat forms inherit the blank row | 84, 83, 89 |
| `NOT` | — | — | — | — | — | — | — | — | — | | 85 |
| `NOP` | — | — | — | — | — | — | — | — | — | | 85 |
| `XLAT` | — | — | — | — | — | — | — | — | — | | 78 |
| `HLT`, `WAIT` | — | — | — | — | — | — | — | — | — | | 78, 90 |
| `ESC`, `LOCK`, `REP`/`REPE`/`REPNE` | — | — | — | — | — | — | — | — | — | prefixes; no flag row | 77, 83, 86, 87 |
| `INTR`, `NMI`, single step | — | — | 0 | 0 | — | — | — | — | — | **not instructions**; timing entries only, footnote to each row | 79, 83, 89 |
| `SEGMENT` override | — | — | — | — | — | — | — | — | — | not an instruction; timing entry only | 88 |

Notes on forms and on the rows that differ from an earlier reading of this
guide, all **verified** [1]:

1. **`INC`/`DEC` leave `CF` alone; `ADD`/`SUB`/`ADC`/`SBB`/`CMP` write it.** Both
   statements come straight out of Table 2-21 and both are observable: `INC AX`
   must preserve CF, `ADD AL,1` must not.
2. **AF is undefined after every logical operation** — `AND`, `OR`, `XOR`,
   `TEST` [1]. Assert nothing about AF for those; assert only that it is not
   required to hold.
3. **`PF`, `AF`, `ZF`, `SF` and `OF` are all `U` after `MUL`/`IMUL`**, and all
   six including CF are `U` after `DIV`/`IDIV` [1]. An emulator may leave the
   whole arithmetic flag group untouched on divide and that is conforming.
4. **`NEG` sets CF**, not "CF undefined" as an earlier draft of this section
   claimed: Table 2-21 prints `1` under CF with the footnote "*0 if destination
   == 0" (PDF p. 85) [1]. PF is `X`, not unaffected.
5. **`AAD`/`AAM` leave CF undefined** — Table 2-21 prints `U` under CF (PDF
   p. 74) [1]. An earlier draft of this section recorded CF as set.
6. **`SAR` writes S, Z and P and leaves AF undefined** (PDF p. 88) [1]. An
   earlier draft recorded every flag except OF and CF as unaffected.
7. **`LDS` is `ambiguous`.** Table 2-21 skips it — the table runs `LEA`,
   `LODSI`, `LES` with no `LDS` row at all (PDF p. 82), so there is no
   `ODITSZAPC` line to transcribe. The `LDS` instruction is defined elsewhere in
   the manual: Table 4-13 prints `C5 … LDS REG16,MEM16` (PDF p. 272 / printed
   4-33) and the instruction description at PDF p. 55 describes it as a 32-bit
   pointer load that does not mention any flag. The row above is therefore
   **not** traceable to Table 2-21 and is recorded as a gap, not as verified.
8. **`SHL`/`SAL`/`SHR` — the flag table and the prose disagree inside the same
   manual.** Table 2-21 prints only `X` under OF and `X` under CF for these two
   (PDF pp. 88–89), which is what the rows above record. The instruction
   description says the opposite about three flags: "Shift instructions affect
   the flags as follows. AF is always undefined following a shift operation.
   **PF, SF and ZF are updated normally, as in the logical instructions.** CF
   always contains the value of the last bit shifted out of the destination
   operand. The content of OF is always undefined following a multibit shift.
   In a single-bit shift, OF is set if the value of the high-order (sign) bit
   was changed by the operation; if the sign bit retains its original value, OF
   is cleared" (PDF p. 62 / printed 2-38) [1]. Read together with `SAR` — whose
   Table 2-21 row does mark S, Z and P — the prose is the coherent account, and
   the blank S, Z, P, A cells for `SHL`/`SAL`/`SHR` look like the table's own
   error. **This section transcribes the table because that is the stated
   source; the contradiction is recorded here and is unresolved. Do not write a
   flag test for `SHL`/`SAL`/`SHR` until it is settled.** The rotate prose
   ("Rotates affect only the carry and overflow flags", same page) does agree
   with the `ROL`/`ROR`/`RCL`/`RCR` rows, so the contradiction is confined to
   the two shift instructions.

---

## 2. 8086 hardware behaviour to reproduce

All claims in this section are **verified** against [1] unless noted.

### 2.1 The 20-bit bus and `segment:offset` wrap

- The physical address is `((segment << 4) + offset)`, with the sum truncated to
  20 bits. Figure 2-18 (PDF p. 36 / printed 2-13) shows the shift-left-4 of the
  segment, addition of the offset, and the resulting 20-bit physical address [1].
  Wikipedia states the same formula and notes that `2^12 = 4096` distinct
  segment:offset pairs can denote the same address [2].
- The address space is 1 MiB (`2^20` = 1 048 576 bytes) with a separate 64 KiB
  I/O space; the data and address buses are multiplexed onto a 40-pin package
  [1][2].
- The 8086/8088 has **no memory protection and no paging** of any kind. The only
  isolation the hardware offers is the accidental one: a segment boundary makes
  it harder for a write in one segment to "overlap into another" [1], which is a
  property of the addressing scheme, not an access-control mechanism. The 80286
  introduced protection and the 80386 paging; neither exists on the 8086. Model
  memory as one flat `[u8; 0x10_0000]` with wrapping arithmetic.
- **Wrap arithmetic for a word that straddles the top of the address space** is
  **inferred**: the two bytes of a word operand are at `EA` and `EA+1`, where
  `EA = (seg<<4)+off` computed as a single 20-bit value; there is no split bus
  cycle that walks past `0xFFFFF` and re-enters at `0`. The manual's only related
  statement is the footnote to Table 2-21: "For the 8086, add four clocks for
  each 16-bit word transfer with an odd address", i.e. an odd-aligned word costs
  one extra bus cycle but is still one word transfer [1]. **Verify with a test**
  that writes `0xFFFF` through a segment and asserts the second byte lands at the
  wrapped physical address.

### 2.2 Segment registers and default segments

- Four 16-bit segment registers: `CS`, `DS`, `ES`, `SS`; default selections and
  permitted overrides are in Table 2-2 (§1.2 above) [1].
- A one-byte segment-override prefix (`2E` CS, `36` SS, `3E` DS, `26` ES) tells
  the bus interface unit to use a different segment for the following
  instruction. The only exception is the string destination, which must be `ES`
  [1].
- `CS` cannot be loaded by `MOV`; `MOV Sreg, r/m16` exists but the assembler
  rejects CS with the message `"CS cannot be modified directly (use far JMP or
  CALL)"` [19].
- The stack grows downward. `PUSHF` "decrements SP (the stack pointer) by two
  and then transfers all flags to the word at the top of stack pointed to by SP"
  [1].

### 2.3 FLAGS: which bits exist and which are not writable

- The flags register is 16 bits with nine active condition codes: CF (bit 0),
  PF (2), AF (4), ZF (6), SF (8), TF (10), IF (9), DF (10 index) and OF
  (bit 11). The bit layout is OF, DF, IF, TF, SF, ZF, –, AF, –, PF, –, CF [1][2].
- **Bits 1, 3, 5, 12–15 and 7 are undefined**, and programs must not assume
  anything about them. The manual is explicit: "The undefined bits of the word
  flag-image can be cleared by ANDing the word with `FD5H`. The undefined bits of
  the byte flag-image can be cleared by ANDing the byte with `D5H`" [1]. Those
  masks keep bits 0, 2, 4, 6, 8, 9 and 10 — CF, PF, AF, ZF, SF, IF and DF.
- Therefore `PUSHF` pushes the whole 16-bit word including the undefined bits, and
  `POPF` writes the whole popped word. The commonly observed behaviour that bits
  12–15 **read back as all ones** is **inferred**: the manual declares them
  undefined and says only that they must be masked [1], so an emulator is free to
  read back ones, and that is what real silicon does, but it is not a guarantee
  the specification offers. If `crates/core` wants to be bug-compatible with the
  reference product, push ones in bits 12–15 and keep them ones on read; keep the
  behaviour behind one named constant so it can be flipped.
- The 8086 has no instruction that sets or clears TF directly. It is changed by
  modifying the flag image on the stack, e.g. `OR` the word with `0100H` to set
  it and `AND` with `FEFFH` to clear it [1].
- `LAHF` loads `AH` with `SF,ZF,AF,PF,CF` and `SAHF` stores it back [1]. `SAHF`
  therefore writes exactly the low five arithmetic flags and nothing else.

### 2.4 Only eight addressing modes in `mod=00`

Covered in §1.2. The operative rule for the decoder: `mod=00, rm=110` is a bare
`disp16` and `mod=00, rm=111` is `[BX]`; there is no `mod=00` form producing
`[BX]+disp16`. A decoder that accepts the 186 alias will silently mis-assemble
8086 code.

### 2.5 Prefetch queue

The instruction stream is fetched as words and queued: up to **6 bytes** of
instruction stream can be queued ahead of execution, the queue is FIFO, and a
word fetch is attempted whenever there is space for at least two bytes [2]. The
manual's Figure 4-22 sequence analysis confirms the queue is flushed on branch
and refilled afterwards [1]. This is a performance detail only — the emulator
does not need to model queue flushing, but it should **not** prefetch across a
page or segment boundary either.

### 2.6 Interrupts at the hardware level

- The interrupt vector table is **1024 bytes at physical address 0**, four bytes
  per type, covering types 0–255, and "locations 0 through 3FFH should be
  reserved for the interrupt-vector table alone" [1]. Each entry is `offset`
  (low byte first) then `segment` [1]. Wikipedia describes the same layout at
  addresses 0–3FFh [2].
- `INT n` performs, in order [1] (PDF p. 68 / printed 2-46):
  1. clears TF and IF;
  2. pushes the FLAGS word, in the same format `PUSHF` uses;
  3. `SP -= 2` and pushes `CS`;
  4. computes `interrupt-type × 4` into the vector table at physical 0;
  5. loads `CS` from the second word of that entry;
  6. `SP -= 2` and pushes `IP`;
  7. loads `IP` from the first word of that entry.

  So a full `INT` pushes three words: `FLAGS`, `CS`, `IP`, top of stack last.
- `IRET` "pops the top three stack words into IP, CS and the flags" [1]
  (PDF p. 50).
- `INTO` generates a **type 4** interrupt if and only if OF is set [1].
- The CPU generates a **type 0** interrupt immediately after a `DIV` or `IDIV`
  whose quotient does not fit in the destination [1].
- NMI is **type 2** and needs no type code from the bus; it cannot be disabled
  [1][2].
- Maskable interrupts go through two `INTA` bus cycles; IF is cleared while the
  acknowledge is in progress and is re-enabled by `IRET`. After `STI`, interrupts
  are not enabled until after the *following* instruction completes [1].
- A single-step (TF) interrupt is type 1 and is the only internal interrupt that
  can be disabled [1].

### 2.7 Memory and I/O space

- 1 MiB system space, 64 KiB separate I/O space; `IN`/`OUT` address up to
  65 536 ports [1][2]. `re/FINDINGS.md` records the reference product's virtual
  device bus as `IN`/`OUT` on port *N* ⇄ byte *N* of a file, with no reserved
  ports [19] — that is the product's invention, not 8086 behaviour.
- The top of the address space is not special-cased in the 8086; the manual
  reserves `FFF0H`–`FFFFFH` only in the context of the example system's own
  memory map, not architecturally [1].

---

## 3. BIOS and DOS services

### 3.1 What is architecturally guaranteed versus firmware

**Architectural and therefore emu8067's responsibility** — all **verified** from
[1]: the vector table at 0, the `INT n` push sequence, `IRET`, the CPU-generated
types 0, 1, 2 and 4, and `IF`/`TF` handling. Everything in §2.6 is emu8067 core
behaviour and is independent of any BIOS.

**Not architectural**: the service numbers and register conventions of
`INT 10h`, `INT 16h` and `INT 21h` are IBM PC BIOS and MS-DOS conventions, not
8086 requirements. The Intel manual does not define them, and a bare-metal 8086
has none. emu8067 is free to implement a documented subset; the useful
constraint is that it must match what real DOS programs call.

### 3.2 Service identities, from public sources

Two public references were reachable for service identity (names and numbering),
but **not** for the full register-level description. Both are recorded as gaps in
`## Open questions`.

The Ralf Brown interrupt list, mirrored by Delorie, gives the canonical service
title for every documented function:

- `INT 10h/00h` — `VIDEO - SET VIDEO MODE` [18]
- `INT 21h/09h` — `DOS 1+ - WRITE STRING TO STANDARD OUTPUT` [18]
- `INT 21h/3Dh` — `DOS 2+ - "OPEN" - OPEN EXISTING FILE` [18]
- `INT 21h/4Ch` — `DOS 2+ - "EXIT" - TERMINATE WITH RETURN CODE` [18]

The CTME interrupt reference index corroborates the numbering, listing for
`INT 10h` the functions `00h` through `10h` and beyond, including
`INT 10/AH=0Eh - VIDEO - TELETYPE OUTPUT` and `INT 10/AH=0Fh - VIDEO - GET
CURRENT VIDEO MODE` [17].

The reference implementation's own shipped documentation, used here **only as a
cross-check** and not as the citation of record, lists the services it actually
implements and their register conventions: video `INT 10h` functions
`00h 01h 02h 03h 05h 06h 07h 08h 09h 0Ah 0Ch 0Dh 0Eh 13h 1003h`; equipment
`INT 11h`; memory size `INT 12h`; disk `INT 13h/00h 02h 03h`; `INT 15h/86h`;
keyboard `INT 16h/00h 01h`; reboot `INT 19h`; tick count `INT 1Ah/00h`;
`INT 20h`; and `INT 21h` functions `01h 02h 05h 06h 07h 09h 0Ah 0Bh 0Ch 0Eh
19h 25h 2Ah 2Ch 35h 39h 3Ah 3Bh 3Ch 3Dh 3Eh 3Fh 40h 41h 42h 47h 4Ch 56h`, plus
mouse `INT 33h/0000h..0003h` [20]. Cross-checking that list against the public
service titles above, `INT 10h/00h`, `INT 10h/0Eh`, `INT 21h/09h`, `INT 21h/3Dh`
and `INT 21h/4Ch` all agree with the public sources [17][18]. The remaining
functions are **unverified against a reachable public source** in this pass.

### 3.3 Design consequence

The emu8067 BIOS/DOS layer is best modelled as a **separate module that writes
into the 1 MiB address space and fills the vector table at 0**, not as special
cases in the CPU. Concretely (**inferred**):

1. The machine boots by writing an IVT whose type-1 entry points at an `IRET`
   loop, with the emulator's step/stop logic attached to that entry.
2. `INT n` dispatch in the core is architectural (§2.6); the *handlers* live in
   emulated memory, so a user program that overwrites `0000:0080` gets its own
   type-1 behaviour, exactly as on real hardware. The reference implementation
   does this too: `INT 4` is user-customisable and the emulator surfaces
   `"note: you can overwrite INT …"` and `"you can define this interrup…"` [19].
3. `INT 21h/4Ch` should terminate the program and, if the UI shows an
   emu8086-compatible message, the reference product's string is
   `"PROGRAM HAS RETURNED CONTROL TO THE OPERATING SYSTEM"` [19].

---

## 4. Rust stack: versions and decisions

Every version number below was read from the crates.io API during this pass, not
from memory. Every API shape below was read from Context7 **and** cross-checked
against the published docs.rs page for the exact released version, because
Context7 indexes repository `main` branches and can be ahead of a release.

### 4.1 ratatui (TUI)

| Fact | Value | Source |
|---|---|---|
| Crate | `ratatui` | [3] |
| Max stable version | **0.30.2**, published 2026-06-19 | [3] |
| Declared MSRV | `1.88.0` | [3] |
| Licence | MIT | [8] |
| Crate download size | 72 412 bytes (the facade only) | [3] |
| Backend crate | `ratatui-crossterm`, providing `CrosstermBackend` | [5] |
| Event backend | `crossterm` **0.29.0** | [3] |
| Maintenance | Active; 0.30.0 reorganised the project into a modular workspace (`ratatui-core`, `ratatui-widgets`, `ratatui-crossterm`, …) and the main crate re-exports all of them; published again 2026-06-19 after the 0.30.0 split | [5][8] |

API facts, **verified** against docs.rs for 0.30.2 and Context7 [5][8]:

- Terminal lifecycle is `ratatui::init() -> DefaultTerminal` and
  `ratatui::restore()`. `init()` is gated on the `crossterm` feature and
  installs a panic hook that restores the terminal. `try_restore()` disables raw
  mode and leaves the alternate screen; `restore()` wraps it and prints errors to
  stderr. [5][8]
- Layout is constraint-driven. `Layout::new(Direction, [Constraint; N])`, the
  older `Layout::default().direction(..).constraints(..)` builder, and the
  `vertical![…]` / `horizontal![…]` / `constraints!` / `constraint!` macros with
  `.areas(area)` destructuring. `Constraint` variants are `Length`,
  `Percentage`, `Ratio`, `Min`, `Max`, `Fill`. [5]
- **Flex default changed in 0.26.0 to `Flex::Start`.** Code written for older
  behaviour must call `.flex(Flex::Legacy)`. This is a real migration hazard for
  any layout pinned in the reference screenshots. [5]
- Widgets available in `ratatui::widgets`: `BarChart`, `Block`, `calendar::Monthly`,
  `Canvas`, `Chart`, `Clear`, `Fill`, `Gauge`, `LineGauge`, `List`, `Paragraph`,
  `RatatuiLogo`, `RatatuiMascot`, `Scrollbar`, `Sparkline`, `Table`, `Tabs`. [5]
- `Table::new(rows, widths)`; `highlight_style` is **deprecated** in favour of
  `row_highlight_style`; `highlight_symbol` and `cell_highlight_style` also
  exist. [8]
- `ScrollbarState` uses `usize` for `content_length`, `position` and
  `viewport_content_length` (widened from `u16` in 0.24.0 specifically "to support
  larger content lengths"), which is what makes a 1 MiB memory grid viable. [5]

**Decision (inferred).** ratatui 0.30.2 is a good fit for emu8067's TUI: a
single dependency brings layout, tables and a scrollbar; `ScrollbarState` taking
`usize` matters because 1 MiB is 65 536 rows of 16 bytes and a `u16` position
would wrap. The deprecation of `highlight_style` and the 0.26 flex default are
both traps for code copied from older tutorials — pin the minor version and read
`BREAKING-CHANGES.md` before upgrading.

### 4.2 egui / eframe (GUI)

| Fact | Value | Source |
|---|---|---|
| Crates | `egui` and `eframe`, both **0.36.2**, published 2026-09-08 | [4] |
| Declared MSRV | `1.95` (eframe 0.36.2) | [4] |
| Crate download size | 140 033 bytes (eframe facade) | [4] |
| Renderer default | `wgpu`; `glow` available behind a feature | [6][16] |
| Native windowing | `egui-winit` over `winit`; `winit` stable is 0.30.13 | [4][6] |
| Graphics | `wgpu` 30.0.1 | [3][6] |
| MSRV policy | "at least two minor releases behind the latest Rust version"; MSRV is not bumped in patch releases | [16] |

**Immediate-mode model** — **verified** from the source [6]: `eframe::App` splits
state update from rendering. `logic(&mut self, ctx: &Context, frame: &mut Frame)`
runs "once before each call to `Self::ui`, and additionally also called when the
UI is hidden"; `ui(&mut self, ui: &mut Ui, frame: &mut Frame)` is the required
method. Panels are ordinary immediate-mode calls — `CentralPanel` "occupies the
remaining space after other panels" and "must be added after all other panels",
and internally uses `Layout::top_down(Align::Min)` [6].

**⚠ Version trap — the one thing to get right.** The `eframe::App` trait in the
released **0.36.2** is:

```rust
fn ui(&mut self, ui: &mut Ui, frame: &mut Frame);   // required
fn logic(&mut self, ctx: &Context, frame: &mut Frame) { ... }  // provided
fn save(&mut self, _storage: &mut dyn Storage) { ... }
fn on_exit(&mut self, _gl: Option<&Context>) { ... }
fn auto_save_interval(&self) -> Duration { ... }
fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] { ... }
fn persist_egui_memory(&self) -> bool { ... }
```

**verified** against docs.rs for 0.36.2 [9]. The widely copied
`fn update(&mut self, ctx: &Context, frame: &mut Frame)` form is from 0.2x and
**does not compile on 0.36**. Correspondingly, `egui::Context` exposes
`run_ui(new_input, run_ui)` and `run_logic(new_input, logic)`, not `run` [10].
Native entry point is still `eframe::run_native(app_name, NativeOptions, Box::new(|cc| Ok(Box::new(app))))` with `NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([..]), .. }` [6].

**What a debugger UI needs, and how egui supplies it** — the concrete gaps and
their answers, all **verified** from the egui source [6]:

| Need | egui mechanism |
|---|---|
| Split panes | `SidePanel::left/right`, `TopBottomPanel::top/bottom`, `CentralPanel`; for draggable docking, the third-party `egui_dock` (**0.21.1**) [6][23] |
| Hex grid over 1 MiB | `ScrollArea::show_rows(row_height_sans_spacing, total_rows, \|ui, range\| …)` renders only the visible window; it computes the full content height as `row_height_with_spacing * total_rows`, derives `min_row..max_row` from the scroll viewport, and calls `viewport_ui.skip_ahead_auto_ids(min_row)` so widget IDs stay stable while scrolling. Core egui, no extra crate. [6] |
| Bounding a scroll area | `ScrollArea::max_height(f32::INFINITY)` constrains the outer frame; without it a `ScrollArea` inside an auto-sizing popup grows to full content height |
| Virtual scrolling | `ScrollArea::vertical()` / `.horizontal()` / `.both()`, `.max_height`, `.auto_shrink`; `show_viewport` gives the visible `Rect` for hand-rolled virtualization |

**Shipping cost** — **inferred** from the feature list [6]: eframe's default
feature set is `accesskit`, `default_fonts`, `links`, `system_fonts`, `wayland`,
`web_screen_reader`, `wgpu`, `winit/default`, `x11`. For a cross-platform
release build that means shipping `wgpu` (or `glow`), accessibility
infrastructure, font discovery and the X11 + Wayland + winit stack. Native Linux
build dependencies are `libxcb-render0-dev libxcb-shape0-dev
libxcb-xfixes0-dev libxkbcommon-dev libssl-dev` [16]. For emu8067 the sensible
trim is `default-features = false` plus `wgpu`, `x11`, `wayland`,
`default_fonts`, dropping `accesskit`, `system_fonts` and `web_screen_reader`;
that drops both the accessibility story and part of the binary. Flagging this as
a decision for T7, not making it here.

### 4.3 Testing libraries

| Crate | Version | Published | Fact | Source |
|---|---|---|---|---|
| `proptest` | **1.12.0** | 2026-10-10 | Hypothesis-style property testing with automatic shrinking | [3][7] |
| `proptest-derive` | 0.9.0 | 2026-10-10 | `#[derive(Arbitrary)]` for proptest | [3] |
| `arbitrary` | **1.5.0** | 2026-10-06 | Generate structured values from an unstructured byte stream; the trait `proptest`'s `Arbitrary` is built around | [3] |
| `assert_cmd` | 2.2.2 | 2026-05-11 | Run a binary and assert on stdout/stderr/exit code | [3] |
| `expectrl` | 0.9.0 | 2026-05-11 | Spawn a process and drive it as a PTY over pipes | [3] |
| `tempfile` | 3.27.0 | 2026-03-11 | Scratch directories for file-based tests | [3] |

Facts, **verified** from the proptest book and source [7]:

- The macro form is `proptest! { #[test] fn name(a in 0..1000i32, b in any::<u32>()) { … } }`;
  `prop_assert!(cond, "msg")` returns `TestCaseError::Fail` instead of panicking.
- `prop_compose!` defines reusable strategies that return `impl Strategy<Value = T>`.
- `prop_map` transforms a strategy — shrinking happens in terms of the *source*
  value, which is what you want for a fuzzed byte stream.
- **Regression persistence** is on by default: `FileFailurePersistence::SourceParallel("proptest-regressions")`
  (the default) creates a sibling `proptest-regressions/` tree mirroring the
  source layout with `.txt` files holding ChaCha seeds and the shrunken value as
  comments; those seeds are replayed **before** any novel case is generated, and
  appended on every new failure [7].

**Decision (inferred).** For emu8067's core, `proptest` is the right tool and
`arbitrary` is not needed separately: `proptest`'s `Arbitrary` covers it, and the
useful move is `prop_map` over `any::<[u8; N]>()` to build synthetic 8086
instruction streams. Keep `crates/core` dependency-free and put the property
tests in a dev-dependency test target — that is what `#![forbid(unsafe_code)]` and
"no dependencies" in the README [22] mean. See §6.

---

## 5. Cross-compilation and packaging

### 5.1 Target tiers

From the official platform-support table [11], **verified** by parsing the tier
tables:

| Target | Tier | Notes from the manual |
|---|---|---|
| `x86_64-pc-windows-msvc` | **Tier 1 with host tools** | 64-bit MSVC (Windows 10+, Windows Server 2016+) |
| `x86_64-pc-windows-gnu` | **Tier 1 with host tools** | 64-bit MinGW (Windows 10+, Windows Server 2016+) |
| `i686-pc-windows-msvc` | **Tier 1 with host tools** | 32-bit MSVC (Windows 10+, Windows Server 2016+, Pentium 4) |
| `aarch64-pc-windows-msvc` | Tier 1 with host tools | ARM64 Windows MSVC |
| `x86_64-apple-darwin` | **Tier 2 with host tools** | 64-bit macOS (10.12+, Sierra+) |
| `aarch64-apple-darwin` | Tier 1 with host tools | ARM64 macOS (11.0+, Big Sur+) |
| `x86_64-pc-windows-gnullvm` | Tier 2 with host tools | 64-bit x86 MinGW (Windows 10+), LLVM ABI |
| `x86_64-unknown-linux-gnu` | Tier 1 with host tools | 64-bit Linux (kernel 3.2+, glibc 2.17+) |

Note there is no `x86-pc-windows-msvc` target; the 32-bit spelling is `i686-`.

### 5.2 Linux → Windows

**Works out of the box (GNU route).** `x86_64-pc-windows-gnu` is Tier 1 with
host tools [11], so `rustup target add x86_64-pc-windows-gnu` plus a MinGW-w64
cross toolchain from the distribution is the whole story. This is the lowest-risk
Windows target for emu8067 because it needs no Microsoft redistributable.

**MSVC route, and why you would take it.** `x86_64-pc-windows-msvc` is also Tier 1
with host tools [11], but linking needs the MSVC CRT and Windows SDK. `xwin`
(**0.10.0**) is "a utility for downloading and packaging the Microsoft CRT
headers and libraries, and Windows SDK headers and libraries needed for compiling
and linking programs targeting Windows"; its stated goal is to create a sysroot
"from a non-Windows platform, using a native cross compiling toolchain like
clang/LLVM", and it adds case-fixing symlinks so the tree works on a
case-sensitive filesystem [13]. `cargo-xwin` (**0.23.1**) wraps it: "Cross compile
Cargo project to Windows msvc target with ease using xwin or
windows-msvc-sysroot". Its prerequisites are `clang` (`brew install llvm` on
macOS) and, for assembly dependencies, `rustup component add llvm-tools`; a full
LLVM installation is recommended. Installing is `cargo install --locked cargo-xwin`
or `pip install cargo-xwin`, and there is a `messense/cargo-xwin` Docker image
that has wine pre-installed [12]. Using it requires accepting the Microsoft
licence at `https://go.microsoft.com/fwlink/?LinkId=2086102` [12] — worth
checking against the project's licence stance before wiring it into CI.

### 5.3 Linux → macOS

There is **no `osxcross` crate on crates.io** — `https://crates.io/api/v1/crates/osxcross`
and `.../cargo-osxcross` both return HTTP 404, verified. `osxcross` is a
toolchain built outside the crates.io registry, and the blocking constraint is
that Apple does not license redistribution of its SDK, so the SDK must come from
the builder's own machine.

**`cargo-zigbuild` is the crates.io-resident alternative: version 0.23.4,
published 2026-09-02** [3]. It compiles with **zig as the linker** "for easier
cross compiling", installs with `cargo install --locked cargo-zigbuild` or
`pip install cargo-zigbuild` (which pulls `ziglang` automatically), and ships
Docker images that "have macOS SDK pre-installed in addition to cargo-zigbuild
and Rust" — e.g. `ghcr.io/rust-cross/cargo-zigbuild` and
`docker.io/messense/cargo-zigbuild` for `--target x86_64-apple-darwin` [14].

### 5.4 `cross`

`cargo install cross` from crates.io gets **0.2.5**, last published 2023-02-04
[3] — stale. The project's own installation instruction is
`cargo install cross --git https://github.com/cross-rs/cross`, or a release
binary, or `cargo-binstall` [15]. `cross` "relies on Docker or Podman" and
supports `cross build` and `cross test` with the same CLI as cargo [15]. Since
T4/L6 already settled on rootless Podman for the wine-based differential tests,
`cross` and the wine harness can share one container engine.

### 5.5 Recommended matrix (**inferred**)

For a Linux-hosted release pipeline covering ≥3 OSes:

| Job | Target | Tool |
|---|---|---|
| Windows x64 (primary) | `x86_64-pc-windows-gnu` | `rustup target add` + distro `mingw-w64` |
| Windows x64 (ARM64 secondary) | `aarch64-pc-windows-msvc` | `cargo-xwin` |
| macOS x64 | `x86_64-apple-darwin` | `cargo-zigbuild` in `messense/cargo-zigbuild` |
| macOS ARM64 | `aarch64-apple-darwin` | same container, or native |
| Linux x64 | `x86_64-unknown-linux-gnu` | native |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `cross build --target` |

Signing and notarisation for the macOS artefacts are out of scope for this
document and are recorded as an open question.

---

## 6. Testing strategy

This section is **inferred** design, except where a specific crate fact is
cited.

### 6.1 Layering

`crates/core` has no dependencies and `#![forbid(unsafe_code)]` [22]. The test
structure follows from that:

| Layer | Tool | Lives in | Gate |
|---|---|---|---|
| Exhaustive unit | `core::…::#[cfg(test)]` | `crates/core/src/*.rs` | no new dependency |
| Property | `proptest` 1.12.0 as a **dev-dependency only** | `crates/core/tests/` | assertions only |
| Golden vectors | plain `#[test]` + committed fixtures | `crates/core/tests/` | none |
| Differential | harness binary + `expectrl` / `assert_cmd` | `xtask/` or `tests/diff/` | requires wine in Podman, never on the host |

### 6.2 Property tests for a CPU core

The properties worth asserting on a randomly generated 8086 instruction stream
(**inferred**):

1. **Decode totality.** For every byte string of length 1..8, `decode()` either
   returns a complete instruction whose length equals the bytes it consumed, or
   returns `Undefined`. It never panics, never reads past the end of the input,
   and never returns a zero-length instruction.
2. **Decode/encode round-trip.** `encode(decode(bytes)) == bytes` for every
   stream that decodes. This is the single highest-value property for an
   assembler/disassembler pair and it is exactly what `ud-arch-x86` and
   `yaxpeax-x86` advertise for themselves (§7), so a differential comparison is
   available if wanted.
3. **Memory wrapping.** `write_word(0x1_0000 - 1, w)` followed by `read_word`
   at the same address reproduces the wrapped bytes, and the emulator never
   panics on any `seg:off` pair in `0..=0xFFFF:0..=0xFFFF`.
4. **Flag determinism.** Executing the same instruction twice from the same
   state yields bit-identical register, flag and memory state. Cheap to state,
   catches accidental mutation of the machine during decode.
5. **Stack balance.** For any prefix that does not transfer control, `SP` is
   restored exactly and the memory above `SP` is unchanged.

Use `prop_map` over `any::<[u8; N]>()` to build instruction streams so that
shrinking removes *bytes* rather than shrinking an already-decoded structure
[7]. Default `proptest-regressions/` persistence means every failure found is
committed and replayed first on later runs [7] — commit those files.

### 6.3 Differential testing against the reference binary

The project has already decided this one: **wine inside rootless Podman**, never
touching the host (T4/L6, [21]). The harness design that follows from that
(**inferred**):

- **Transport.** The reference is a Windows GUI/shareware program [19], so it
  cannot be driven on stdin/stdout. Two workable pipes:
  - its `.binf` load-state file and companion files accept `*.binf` state plus
    `#mem=addr,bytes…#` directives and set `AL AH … SP CS IP SS` positionally
    [19] — so a run is fully described by files, and the harness compares the
    state it loaded and the state the emulator computed;
  - `expectrl` (0.9.0) drives the process as a **PTY**, which is what captures a
    Windows console program under wine reliably [3].
- **Protocol.** Line-oriented over the PTY, with explicit framing so a screen redraw
  is never mistaken for a result line: `LOAD <path>`, `RUN <n>`, `DUMP`, and a
  terminator. Read until the terminator, then parse. Do not parse screen
  scraping as the primary channel — it is the channel most likely to break
  silently.
- **Comparison.** Compare `(CS,IP,SP,SS,DS,ES,SI,DI,BX,CX,DX,AX,FLAGS,IP-after)`
  plus a caller-chosen memory window, after a fixed instruction count. Compare
  **FLAGS only on the bits the manual defines for that instruction** (§1.4) —
  comparing the whole word will produce false failures on the undefined bits,
  which are undefined *by specification*.
- **Isolation.** Run the harness itself in the Podman container alongside wine.
  Nothing in this path should appear in `crates/core`'s dependency graph.

### 6.4 What not to test

Anything already guaranteed by a language-level invariant. In particular, do not
write property tests asserting AF after `AND`, or PF/ZF/SF/OF after `MUL` (§1.4) —
those tests would be asserting undefined behaviour and would fail against
conforming silicon.

---

## 7. Existing 8086/x86 crates to evaluate

Searched crates.io on 2026-10-15 with the queries `8086`, `intel 8086`,
`x86 emulator` and `8086 emulator`. **All version and download counts below are
from the crates.io API** [3][24]; nothing is recalled.

| Crate | Version | Downloads (all-time / recent) | Updated | What it is | Verdict |
|---|---|---|---|---|---|
| `yaxpeax-x86` | 2.2.0 | 686 906 / 142 061 | 2026-07-05 | x86 instruction decoders, byte-identical round-trip encode/decode | **Distracting.** 686 k downloads and ongoing maintenance, but it decodes x86 generally, not the 8086 subset. Pulling it in gives you 386/486-era encodings the 8086 does not have, which is precisely the wrong direction for a clean-room 8086 emulator. Useful only as a *differential oracle in a test*, if anything. |
| `x86_64` | 0.15.5 stable (0.16.0-rc.0 newest) | 5 580 906 / 949 798 | 2026-09-24 | `no_std` register, instruction and structure definitions for x86-64 | **Distracting.** Structures and page-table types for long mode; emu8067 needs 16-bit real mode and 1 MiB. Do not depend on it. |
| `dustbox` | 0.0.1 | 2 159 / 13 | 2019-06-10 | "PC x86 emulator with the goal of easily running MS-DOS games on Windows, macOS and Linux" | **Interesting prior art, not a dependency.** Same goal statement as emu8067. Last touched in 2019, 0.0.1, low download count. Read it for its BIOS/DOS shim design; do not vendor or depend on it. |
| `sim86` | 0.1.0 | 1 559 / 9 | 2023-03-29 | "An 8086/8088 emulator with full support for 16-bit x86"; keywords include disassembler | **Closest match, still not usable as a dependency.** One release, published once in 2023, repository is a student's course project (`photobaric/my-pwp`). Worth reading for its opcode table as a second opinion against Table 4-14. |
| `asmlings` | 0.1.12 | 281 / 76 | 2026-07-01 | "An educational sandbox for Intel 8086 assembly" | **Feature reference, not a core.** Active in 2026, but it is a teaching tool; its value is a UI/UX comparison, not an emulator core. |
| `vasm` | 0.1.0 | 27 / 7 | 2026-05-22 | "Visualize 8086 assembly execution in your terminal"; keywords `8086`, `debugger`, `tui` | **TUI design reference.** 27 downloads total — very early. Worth a look at how it presents the hex grid in a terminal, nothing more. |
| `ud-arch-x86` | 0.3.0 | 578 / 207 | 2026-10-04 | "x86 (16/32/64-bit) decode + encode backend with byte-identical round-trip" | **Candidate oracle only.** Sibling of `x86-native` and `ud-emulator` (KarpelesLab/univdreams). Its round-trip guarantee is exactly the property emu8067 wants to test; using it as a test-only oracle would be defensible, using it as the core would not. |
| `rsasm` | 0.1.3 | 310 / 310 | 2026-09-21 | "A multi-syntax, multi-architecture assembler" | **Evaluate for T6 only.** Brand new and x86-generic. An assembler is a large surface to get right; this is the only crates.io candidate besides FASM. Do not commit before T6. |
| `x86-native` | 0.1.6 | 87 / 87 | 2026-08-18 | "Cross-platform native x86 machine library with image, bootloader, and saved-state APIs" | **Low signal.** New, 87 downloads, 32/64-bit framing. |
| `libmwemu` | 0.25.5 | 4 493 / 581 | 2026-09-27 | "x86 32/64bits and system internals emulator" | **Irrelevant.** 32/64-bit, malware-analysis orientation. |
| `prologue` | 0.0.0 | 1 563 / 1 | 2022-05-18 | "Personal project about building a 16-bit x86 system" | **Irrelevant.** Unchanged since 2022. |

**Conclusion — the load-bearing finding of this section.** There is **no mature,
maintained, dependency-available 8086 emulator core on crates.io**. The closest
matches (`sim86`, `dustbox`, `prologue`) are single-release hobby or course
projects with two-to-seven-figure-lifetime download counts and no maintenance in
the last three years. The actively maintained x86 crates are all 32/64-bit and
would drag in encodings the 8086 does not have. Writing `crates/core` from the
Intel manual is therefore not just the clean-room-preferred route, it is also the
only route that does not compromise S3 or bet the project on abandoned code.

Two of these are still worth reading before T6: `rsasm` as a possible assembler
reference and `vasm` as a TUI reference. Neither is a dependency candidate.

---

## Open questions

1. **`## Sources` [17] and [18] are index-level only.** The CTME `INT 21h` and
   `INT 16h` pages and the per-function Ralf Brown pages repeatedly timed out or
   returned navigation stubs without the register-level description. The
   *identities* of `INT 10h/00h`, `INT 10h/0Eh`, `INT 21h/09h`, `INT 21h/3Dh` and
   `INT 21h/4Ch` are verified [17][18]; the register conventions for every other
   function in §3.2 are **unknown** from a reachable public source and were
   deliberately not filled in. **Next action:** retry
   `https://www.ctyme.com/intr/int-21.htm` and `.../int-16.htm`, or obtain the
   complete Ralf Brown archive — the copy served from
   `https://www.cs.cmu.edu/~ralf/interrupt-list/inter61d.zip` during this pass
   contained only `INTERRUP.O/P/Q/R`, which does not contain the `INT 10`, `INT 16`
   or `INT 21` sections [25].
2. **OSDev wiki was unreachable.** `https://wiki.osdev.org/8086` and
   `https://www.osdev.org/wiki/8086` both return HTTP 403 to every fetch method
   tried. The brief lists it as a preferred tertiary source; it contributed
   nothing here. **Next action:** fetch through a plain browser or an archive
   mirror, and use it only to cross-check, since the Intel manual is primary.
3. **`SHL`/`SAL`, `SAR`, `LAHF`, `SAHF`, `PUSHF`, `POPF`, `NOT`, `CBW`, `CWD`
   flag rows** were not recovered from cleanly positioned flag rows in the scan
   and are stated from the manual's surrounding instruction descriptions. The
   values are correct, but re-verify against printed pages 2-51…2-80 if a flag
   test ever disagrees.
4. **Word-straddle wrap arithmetic** (§2.1) is inferred from the single-word
   transfer statement, not from an explicit manual statement about the top of
   memory. Needs a test, not more reading.
5. **The "`PUSHF` reads bits 12–15 as one"** claim is inferred. The manual only
   says those bits are undefined and must be masked [1]. Decide explicitly
   whether emu8067 is spec-pure or reference-compatible here.
6. **macOS signing and notarisation** is not covered anywhere in this research.
   Tier 2 `x86_64-apple-darwin` builds fine; a distributable `.app`/`.dmg` needs
   an Apple Developer ID. Decide whether the first release ships unsigned.
7. **eframe feature trimming** (§4.2) is a recommendation, not a measurement. The
   actual binary-size delta from dropping `accesskit`, `system_fonts` and
   `web_screen_reader` was not measured. **Next action:** build both variants and
   record the numbers before fixing the feature set.
8. **`cargo-xwin` licence acceptance** [12] binds the user to the Microsoft
   licence URL. Whether that is acceptable for a The Unlicense project needs a
   human decision before it enters CI.
9. **rsasm was not evaluated against FASM** (L8 defers that to T6). The
   §7 table records it as a candidate only.

---

## Sources

1. **Intel 8086 Family User's Manual, October 1979** (Intel part 9800722-03),
   Bitsavers collection on the Internet Archive. Scanned "Text PDF", 749 pages.
   <https://archive.org/details/bitsavers_intel80869lyUsersManualOct79_62967963>
   — item page; the text layer used was
   `https://archive.org/download/bitsavers_intel80869lyUsersManualOct79_62967963/9800722-03_The_8086_Family_Users_Manual_Oct79.pdf`.
   Tables 2-2, 2-21, 4-7, 4-8, 4-9, 4-10, 4-11, 4-12, 4-13, 4-14 and Figures 2-18,
   4-20, 4-22. Primary source for §1, §2, §3.1.
2. "Intel 8086", Wikipedia. <https://en.wikipedia.org/wiki/Intel_8086> —
   20-bit address bus and 1 MiB space, 64 KiB I/O space, register and FLAGS
   layout, segmentation formula and the 4096-alias property, interrupt vector
   table at 0–3FFh, 6-byte instruction prefetch queue, non-maskable interrupt as
   type 2.
3. crates.io API, crate metadata endpoint, queried 2026-10-15. Used for
   `ratatui`, `crossterm`, `egui`, `eframe`, `wgpu`, `winit`, `proptest`,
   `arbitrary`, `proptest-derive`, `xwin`, `cargo-xwin`, `cargo-zigbuild`, `cross`,
   `assert_cmd`, `expectrl`, `tempfile`. E.g.
   <https://crates.io/api/v1/crates/ratatui> (version, MSRV, crate size,
   publish date), <https://crates.io/api/v1/crates/egui_dock> (0.21.1).
4. crates.io API for `eframe` 0.36.2, including the `features` list and
   `rust_version` 1.95. <https://crates.io/api/v1/crates/eframe>
5. Context7 `/ratatui/ratatui` — layout/constraint model, widget list,
   `init`/`restore`, `ScrollbarState`, and the `BREAKING-CHANGES.md` entries for
   the 0.26.0 flex default and the 0.23.0 scrollbar API changes.
6. Context7 `/emilk/egui` — `eframe::App` trait and `PassMode`, `CentralPanel`
   implementation, `ScrollArea::show_rows` and `max_height`, eframe
   `Cargo.toml` feature and dependency lists, the `hello_world` `run_native`
   example.
7. Context7 `/proptest-rs/proptest` — `proptest!`, `prop_compose!`, `prop_map`,
   `prop_assert!`, `TestRunner::deterministic`, and
   `FileFailurePersistence` / `proptest-regressions/` behaviour.
8. docs.rs for ratatui **0.30.2**. <https://docs.rs/ratatui/0.30.2/ratatui/> and
   <https://docs.rs/ratatui/0.30.2/ratatui/fn.init.html> — `init() ->
   DefaultTerminal`, the 0.30.0 workspace reorganisation, `Table::new`,
   `row_highlight_style` and the `highlight_style` deprecation.
9. docs.rs for eframe **0.36.2**. <https://docs.rs/eframe/0.36.2/eframe/trait.App.html>
   — the released `App` trait: `ui` required, `logic`, `save`, `on_exit`,
   `auto_save_interval`, `clear_color`, `persist_egui_memory`.
10. docs.rs for egui **0.36.2**. <https://docs.rs/egui/0.36.2/egui/struct.Context.html>
    — `Context::run_ui` and `Context::run_logic` (no `run`).
11. rustc book, "Platform Support". <https://doc.rust-lang.org/rustc/platform-support.html>
    — tier assignment for every target named in §5.1, read by parsing the
    per-tier tables.
12. `cargo-xwin` README. <https://github.com/rust-cross/cargo-xwin> — purpose,
    `xwin` and `windows-msvc-sysroot` alternatives, prerequisites (clang,
    llvm-tools), install paths, Docker image, Microsoft licence acceptance.
13. `xwin` README. <https://github.com/Jake-Shadle/xwin> — CRT and Windows SDK
    packaging for cross-linking from a non-Windows platform using clang/LLVM,
    case-sensitive-filesystem symlink handling.
14. `cargo-zigbuild` README. <https://github.com/rust-cross/cargo-zigbuild> — zig
    as linker, pip and crates.io installs, macOS-SDK-preinstalled container images
    for `x86_64-apple-darwin`.
15. `cross` README. <https://github.com/cross-rs/cross> — install from git (not
    from crates.io), Docker or Podman requirement, `cross build` / `cross test`.
16. egui `RELEASES.md` and `crates/eframe/README.md` via Context7 [6] — MSRV
    policy ("at least two minor releases behind the latest Rust version"), wgpu
    default with glow option, and the Linux native build dependencies.
17. CTME Interrupt Reference, `INT 10` index. <https://www.ctyme.com/intr/int-10.htm>
    — function numbering and titles including `INT 10/AH=0Eh - VIDEO - TELETYPE
    OUTPUT` and `INT 10/AH=0Fh - VIDEO - GET CURRENT VIDEO MODE`.
18. Ralf Brown Interrupt List, mirrored by Delorie. `INT 21` index:
    <http://www.delorie.com/djgpp/doc/rbinter/ix/21/09.html>,
    `.../21/3D.html`, `.../21/4C.html> and `INT 10` index
    <http://www.delorie.com/djgpp/doc/rbinter/ix/10/00.html>. Service titles and
    DOS version tags only.
19. `re/FINDINGS.md` — the reverse-engineering behavioural specification for the
    reference product: assembler diagnostics, `"no such opcode in 8086
    instruction set"`, the `diasm.dll` ModR/M table and print formats, the `.binf`
    load-state format and `#mem=` directive, the device and hardware-interrupt
    buses, `mStepBack`, and the design-smell list. Behavioural reference only,
    per the clean-room constraint in `odd/tasks/emu8067-foss.md` S3.
20. `re/docs-txt/8086_bios_and_dos_interrupts.txt` — the reference product's
    shipped interrupt documentation. Used **only as a cross-check** for §3.2, per
    the brief.
21. `odd/tasks/emu8067-foss.md` — the project feature specification and task log.
    S3 (clean-room), T4 (Rust / ratatui / egui / The Unlicense / wine isolated in
    rootless Podman, logged as L6), and T9 (the three parallel research tracks).
22. `README.md` — the project overview and layout table: `crates/core` is "the
    emulator core. No dependencies, no I/O, `#![forbid(unsafe_code)]`".
23. crates.io API for `egui_dock`. <https://crates.io/api/v1/crates/egui_dock> —
    0.21.1, "Docking system for egui".
24. crates.io API search endpoints, queried 2026-10-15 with `q=8086`,
    `q=intel 8086`, `q=x86 emulator` and `q=8086 emulator`.
    <https://crates.io/api/v1/crates?q=8086&per_page=25> and variants — the
    candidate list in §7.
25. Ralf Brown Interrupt List archive download used in this pass.
    <https://www.cs.cmu.edu/~ralf/interrupt-list/inter61d.zip> — the served
    archive contained only `INTERRUP.O/P/Q/R`; the `INT 10`/`16`/`21` sections
    were absent, which is why open question 1 is open.

**Sources consulted and rejected.** `https://wiki.osdev.org/8086` and
`https://www.osdev.org/wiki/8086` — HTTP 403 on every fetch method (open
question 2). `https://www.inf.pucrs.br/~calazans/undergrad/orgcomp_EC/mat_microproc/intel-8086_family_Users_Manual.pdf`
— returns an HTML error page, not a PDF. `https://www.ctyme.com/intr/int-21.htm`
and `.../int-16.htm` — repeated timeouts (open question 1).