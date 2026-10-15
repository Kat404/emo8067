# Deep Research prompt — closing the 8086 microcode questions

Written 2026-10-15. Paste the whole block below as a single research request and
return only the research output. This is the second use of the framing in
`_prompt-audit7-and-friends.md`, which is what closed four audit items the first
time; read it first if you have not.

## Standing rules of the project these questions come from

- **Clean-room.** Reverse engineering establishes observable behaviour and
  specifications only. No code is ever copied or transliterated from another
  emulator. Cite behaviour, never reproduce code.
- **Silicon outranks the printed manual.** When the two disagree, the chip wins
  and the manual's printing is filed as a known erratum.
- **No hardware is available.** There is no physical 8086 or 8088 on the desk, so
  no question may be answered by "run it and see". Everything must come from
  documentation, die photography, microcode listings, the patents, and existing
  microcode-accurate implementations.
- **A claim marked true and then found false is worse than an open question.**
  This project has already shipped one of those, so every answer below must state
  what would falsify it.

## Method you are required to follow

1. **Declare a source hierarchy** before answering, and rank the sources you
   actually used. For this subject the order is roughly: a microcode listing
   extracted from a die photo > a patent listing > Intel's own manuals >
   microcode-accurate emulator source that documents its evidence > emulator
   source without evidence > forum or blog prose.
2. **Label every single claim** with exactly one of:
   - **(a) verified** — you read a primary source, and you give its URL or
     document reference and, where relevant, the listing line or offset.
   - **(b) reported** — you found it stated, but you did not verify the source.
   - **(c) not decidable** — the available sources cannot settle it. Say what
     *would* settle it.
3. **"Not decidable" is a required and welcome answer.** Four of the questions
   below already have that answer and this research exists to convert some of
   them into (a). A confident wrong answer is the single worst outcome.
4. **Do not smooth over a chain of inference.** If a conclusion needs three links
   and you could only verify two, say so and give the confidence.

---

## Q1 — The stage-1 opcode count, which is the one link holding an open decision

The project's microcode-based reading of `A8`/`A9` rests on this chain: there is a
dedicated `TEST A,i` routine of two opcodes, the shared `alu A,i` routine is
already full at sixteen opcodes, so `A8`/`A9` cannot be in it. **The count of
sixteen comes from a transcription of the stage-1 decoder ROM that is not in any
public repository** — it is inside `8086_microcode_v2.zip` on reenigne.org, which
has never been downloaded.

Answer:
- Is the stage-1 ROM content, or a transcription of it, available anywhere
  reachable? Look for the US patent 4363091 (the 8086 patent, which documents the
  stage-1 decoder structure), the translation ROM contents published in any
  microcode emulator, and any fork or re-upload of `8086_microcode_v2.zip`.
- If you can obtain it: what is the exact set of opcodes the `alu A,i` routine at
  ROM `0x018` covers, and what opcodes does the `TEST A,i` routine at `0x09c`
  cover?
- If you cannot obtain it, is there any other independent way to establish that
  `alu A,i` is full at sixteen and that `A8`/`A9` route to `TEST A,i`?

This question matters more than it looks: it is the difference between an
emulator that is right and an emulator that is confidently wrong about a
documented opcode pair.

## Q2 — What the chip actually does for `8F` with `mod=11` and with `reg != 0`

`8F` is `POP r/m16`. This project accepts `mod=00`, `01` and `10` and refuses
`mod=11` and any `reg != 0`. The accepted half is right for a structural reason:
the Translation ROM dispatches `mod`/`rm` and computes the effective address
before any instruction's microcode starts, and the `POP rmw` entry at ROM `0x040`
contains no branch at all, so no instruction can reject a `mod` value.

Answer, for the real chip:
- What does `8F` with `mod=11` do? The microcode writes to the pseudo-register
  `M`, which the hardware substitutes with a register when there is no effective
  address, so `POP r/m16` to a register is the structural prediction. Is there any
  source that states this?
- What does `8F` with `reg != 0` do? There is no group substitution of the low
  three opcode bits for `8F`, so the prediction is that it executes `POP rmw`
  anyway. Is there any source that states this, or any undocument-opcode survey
  that tested it?
- Does either case raise anything? The 8086 is documented as having no
  invalid-instruction detection, so the interesting question is what it does
  *silently*, not whether it faults.

## Q3 — Is the microcode ROM identical across 8086 steppings?

Every piece of microcode evidence in this project comes from an **8086-2** die. The
8086's stepping history is not benign: there is a widely reported interrupt bug in
the earliest 8086s, and Andrew Jenner explicitly notes that early 8086s are
collector items he would not destroy for a destructive test.

Answer:
- Is there any evidence, dated, that the microcode ROM changed between steppings?
  Distinguish a functional bug fix from a microcode change.
- Is there a documented revision history of the 8086 that names silicon changes?
- If the ROM did change, which of these findings would be at risk: `REP` negating
  a `MUL`/`IMUL` product, `REP` negating an `IDIV` quotient, the `RPTS` zero-count
  exit, the segment-override latch being loaded rather than combined, and the
  absence of top-level microcode for the `F6`/`F7`/`FE`/`FF` group?

## Q4 — The `REP` negation rule set, in full

The project's implemented rule is narrow: `REP` or `REPNE` in front of `MUL`,
`IMUL` or `IDIV` negates the result. The source sentence is "Using the REP or
REPNE prefix with a MUL or IMUL instruction negates the product. Using the REP or
REPNE prefix with an IDIV instruction negates the quotient."

Answer, from the microcode, for **each** of these:
- Does the **byte** form behave like the word form? (`F6 /4`, `F6 /5`, `F6 /7`)
- Does **unsigned `DIV`** really have no effect? The project asserts it does not,
  because the unsigned path has no sign flag to invert. Confirm or refute.
- Does `REP` in front of **`CBW`/`CWD`** do anything? They are sign operations.
- Does `REP` in front of **`AAA`/`AAS`/`AAM`/`AAD`/`DAA`/`DAS`** do anything?
- Does `REP` in front of **`LOOP`, `LOOPZ`/`LOOPE`, `LOOPNZ`/`LOOPNE`** do
  anything? The `F1ZZ` micro-operation that governs early `SCAS`/`CMPS` exit is
  also used by these, but a report suggests the prefix has no effect on them
  because that path XORs with `ZF` instead. Confirm or refute, and say what the
  XOR implies.
- Does `REP` in front of **`CBW`-adjacent sign helpers or `RCL`/`RCR`/`SHL`** do
  anything?
- Is the negation of the **result** of a product that overflows still observable
  in the defined registers, and do `CF`/`OF` depend on the pre-negation or
  post-negation magnitude?

## Q5 — Low priority, and flagged as probably not worth the effort

Answer briefly; a short "not decidable and not worth it" is an acceptable answer.

- Does `LOCK#` actually assert in front of an `F6`/`F7`/`FE`/`FF` group
  instruction that is not lockable? The project models no bus timing, so this
  cannot change any behaviour here. Confirm that reading of its irrelevance.
- Does the repeat prefix delay interrupt recognition for the duration of the
  prefix, as the Group Decode ROM's prefix output appears to? Again unobservable
  in a core with no cycle model. Confirm it is unobservable rather than merely
  unimplemented.

## Q6 — Method check, not a fact question

Two of this project's earlier architecture findings came from a 16-cell opcode
grid transcription where a row contradicted its own prose three lines below it.
For every table, listing or listing excerpt you use in answering above:
- Is the table's own arithmetic self-consistent — do the counts, offsets and row
  lengths add up to what they claim?
- Where a source's prose and its own listing disagree, which one did the source
  author treat as authoritative elsewhere in the same document?

## Required output format

For each question, in order:
- the answer, or the honest "not decidable"
- the evidence, with URL or document reference and, where applicable, the ROM
  address, listing line or offset you are citing
- the label (a), (b) or (c) on every claim
- your confidence, and explicitly which link in any chain of inference you could
  not verify

Then a single closing section listing every claim you could not settle, with what
would settle it. Then a `## Key Learnings` block of 1-5 numbered items, each a
standalone factual sentence of at least 4 words.

Return the research output only. Do not propose code changes; report the evidence.