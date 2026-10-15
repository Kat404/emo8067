# Brief — U1: re-transcribe the 8086 opcode map and flag table from the manual

You are transcribing the Intel 8086 Family User's Manual into the project at `/home/josel/Proyectos/emo8067`. Accuracy is the entire point of this task: the previous transcription was wrong in at least seven places and an instruction was implemented against it wrongly, corrupting memory silently.

**Deliverable:** rewrite sections `## 1. Opcode reference` (§1.3 and §1.4) of `odd/research/documentation-guide.md` in place. You may create `odd/research/verify_opcodes.py` alongside it. Nothing else in the repository may change.

## Skills to load before work
- None required. This is transcription and verification, not library use.

## The source is already local

- `/tmp/opencode/manual/intel8086.pdf` — 748 pages, scanned, no text layer issues with `pdftotext -layout`.
- `/tmp/opencode/manual/out.txt` — already extracted with `pdftotext -layout intel8086.pdf out.txt`, 3.7 MB.
- `pdftotext` and `pdfinfo` are on PATH.

**Table 2-21 "Instruction Set Reference Data"** is the authoritative per-instruction reference: operands, clocks, transfers, bytes, coding example, and a `Flags` column written as `ODITSZAPC` with `X` for defined and `U` for undefined directly beneath each letter. It starts around line 4700 of `out.txt` and runs for many pages. Table entries are alphabetical by mnemonic.

The 256-entry opcode map is in a separate summary table further on — search `out.txt` for the instruction set summary or for a table listing opcodes in hex with their mnemonics and the `MOD`/`REG`/`R/M` groupings. Group 1 (opcode `00`-`3F` and the `80`-`83` extension) and Group 2 (`D0`-`D3`) have their own digit tables.

## What to produce

### §1.3 — the full 256-entry opcode map

One row per `0x00`-`0xFF`, sixteen cells each. Each cell is the instruction for that opcode, or `—` when the 8086 does not define it. Group entries (`Group 1`, `Group 2`, `Group 5`) name the group and refer to the digit table rather than guessing.

Requirements:
- Exactly sixteen cells per row, always. This is the specific defect that broke the previous version: a short row shifts every later opcode by one.
- `A8`/`A9` are `CMP` against an immediate, **not** `TEST`. The previous version had this wrong.
- `0F` is `POP CS`, not unused.
- `F2` is `REPNE`, `F3` is `REP`, and `F4` is undefined on the 8086.
- `EB` is the short **intra**-segment jump. The 8086 has no short intersegment jump.
- `90` is `XCHG AX,AX`, which is the `NOP`.
- `C4`/`C5` are `LES`/`LDS`.
- `86`/`87` are `XCHG` with a register.
- Note in prose that `84`/`85` `TEST r/m,r` and `C0`/`C1` shift-by-immediate are 80186 extensions, absent from the 8086.

### §1.4 — exact flag effects

One row per instruction, columns `O D I T S Z A P C`, each cell `X` or `—`, transcribed from Table 2-21 only. Where Table 2-21 writes `U`, that is `undefined`, and the row must say so. `—` means the flag is not affected at all; `U` means the instruction leaves it in an architecturally undefined state. These are different and the distinction matters to the implementation.

Resolve each multi-form instruction to the set of flags that holds across its forms, and say so in a note when the forms differ — `INC`/`DEC` leave `CF` alone while `ADD`/`SUB` with a flag destination write it.

### `odd/research/verify_opcodes.py`

A small script that reads `odd/research/documentation-guide.md`, parses the §1.3 table, and **exits non-zero** when any row does not have exactly sixteen cells. It is a structural guard against the exact class of defect that happened, so it must be able to fail. Keep it under 80 lines and dependency-free.

## Ground truth for cross-checking

`crates/core/src/exec.rs` was independently verified and is **correct** on these points where the old guide was wrong: `A8`/`A9` are `CMP AL/AX, imm8/imm16` with no ModR/M byte; `F2`/`F3` are `REPNE`/`REP`. When your reading of the manual disagrees with the code, the manual wins — but say so explicitly in the report rather than silently editing.

Known gaps in the code, which the transcription should cover but the code does not yet implement: `86` `XCHG r/m8,r8`, `8F` `POP r/m16`, `9E` `LAHF`, `9F` `SAHF`, `C4` `LES`, `C5` `LDS`, `CE` `INTO`, `D4` `AAM`, `D5` `AAD`, `FE /0,/1`, `FF /0,/1,/3,/5,/6`, `F6/F7 /0,/2,/3`, and the segment override prefixes `26` `2E` `36` `3E`. Do not implement them; transcribe them.

## Allowed edit surfaces
odd/research/documentation-guide.md
odd/research/verify_opcodes.py
The section ends here.

## Rules
- English, neutral register. Technical artifact, not a chat reply.
- Transcribe; do not infer from memory and do not carry anything over from the existing text without checking it against `out.txt`. Every cell must be traceable to a line in the manual.
- Keep every other section of `documentation-guide.md` intact — sections 2 through 7, Open questions and Sources. Only §1.3 and §1.4 are being rewritten.
- `/tmp/opencode/manual/` is scratch. Leave the PDF and `out.txt` there; do not copy them into the repository.
- If a cell in the scan is ambiguous, mark it `ambiguous` with the page reference rather than guessing.

If you make important discoveries, decisions, or fix bugs, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.