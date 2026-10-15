# Technical research request: 8086 default-segment rules, and three adjacent unresolved questions

You are researching the Intel 8086/8088 architecture to resolve a specific, unresolved correctness question in an emulator. **I am not asking for code.** I am asking for rules precise enough to implement and test, each backed by a citable source.

---

## 0. What this is for, and the ground rules

I maintain `emo8067`, a clean-room, free-software Intel 8086/8088 emulator written in Rust. "Clean-room" means: **we never copy or transliterate another emulator's code.** We may study observable behaviour, and we read the Intel manual and published microcode analyses as our authority. When this request says "the manual", it means the *Intel 8086 Family User's Manual* (October 1979) and the *MCS-86 Assembly Language Reference Guide*.

**Evidence hierarchy I want you to use, in this order:**

1. The Intel 8086/8088 documentation itself, quoted with a section or page reference.
2. Published microcode or die-level analysis (for example Ken Shirriff's work at righto.com, Andrew Jenner's 8086 microcode disassembly at reenigne.org), quoted with the specific micro-instruction listing or bit pattern where it settles the point.
3. Everything else — and I want to know which bucket each claim fell into.

**I do not want an answer from memory.** If you are recalling something rather than reading it, say so explicitly. Every factual claim should carry: the claim, the source, and the exact wording or listing that supports it. If sources disagree, say they disagree and present both — do not pick a winner for me unless the evidence is decisive.

Where a question below turns out not to be decidable from available sources, say **"not decidable from available sources"** rather than guessing. That answer is genuinely useful to me; a confident wrong answer is not.

---

## 1. The primary question: the default segment for BP-based addressing

### What I currently implement

My decoder has one default segment, and it passes `DS` for **every** base register:

```
[BP]      reads DS:[BP]
[BP+SI]   reads DS:[BP+SI]
[BP+DI]   reads DS:[BP+DI]
```

A reviewer flagged this: on real 8086 silicon, **BP-based memory operands default to SS, not DS.** I have not been able to find a primary statement of this that I trust, so I have left the code alone and stopped relying on it.

### What I need

**Q1.1** — Does an 8086 memory operand whose base is `BP` default to segment `SS` when no segment override prefix is present? State it with the manual's own words and a page or section reference if one exists.

**Q1.2** — Is the rule expressed in the manual as a *default segment per base register* (a table), or is it stated some other way? Quote it.

**Q1.3** — What is the **complete default-segment table** for an 8086 memory operand, covering every base the 8086 can encode: `[BX]`, `[BX+SI]`, `[BX+DI]`, `[BP]`, `[BP+SI]`, `[BP+DI]`, `[SI]`, `[DI]`, and the direct `disp16` form? For each, give the default segment.

**Q1.4** — **AX and DX.** There is a well-known 8086 exception where `AX`, `DX` and the implied accumulator operands default to `DS` even in a context that would otherwise suggest `SS`. Confirm or refute this precisely:
- Do `MOV AX, [BP+SI]` and `MOV DS:[BX], AX` really default to `DS` on an 8086?
- Which base registers actually trigger the SS default, and is the trigger the *register*, or the *absence of a displacement*, or something else?
- If `AX`/`DX` do not get the SS default, is the rule better described as "BP triggers SS" or as something narrower?

**Q1.5** — The 8086 has an encoding quirk relevant to this. `[BP]` with no displacement is normally encoded as `mod=01, rm=110` with a `disp8` of `0x00`, not as `mod=00, rm=110` (which is the direct `disp16` form). Does that affect the default segment? Specifically: is `[BP+0]` encoded as `mod=01, rm=110, disp8=0` still an SS-defaulted operand?

**Q1.6** — Is the BP→SS default a property of the **8086 specifically**, or does the 8088 differ? Does it change in the 80186, 80286 real mode, or 386 real mode?

**Q1.7** — When a **segment override prefix** is present on a BP-based operand, the override obviously wins. Confirm, and confirm the override applies to the BP-based operand's segment and not to anything else (the register itself, the displacement, the instruction length).

**Q1.8** — Does the BP→SS default apply to **`LEA`**? `LEA` takes an address and performs no memory access. If the answer is that the default segment is irrelevant for `LEA`, say so; if the manual still assigns one, say which.

**Q1.9** — Does it apply to **`XLAT`** (`D7`), which uses an implicit `DS:[BX+AL]`? What is the base segment there?

**Q1.10** — Is there a case where `BP` is used as a memory base and the operand should **not** default to SS? For example, is there an encoding where `BP` as a base means `DS` by definition?

### Evidence I already have, for you to confirm or refute

Ken Shirriff's article on the 8086's undocumented instructions contains a hedged sentence along the lines of *"data references indirected through the BP register typically refer to the stack"* and attributes a behavioural difference to the 8088's 8-bit bus. I want to know whether that is the best-supported statement available or whether something firmer exists, and whether the underlying mechanism is a documented architectural rule or a consequence of how the hardware happened to decode.

---

## 2. Adjacent question: does the stack segment default apply to anything else?

**Q2.1** — Is `SS` the default for any operand **not** involving `BP`? In particular, does `PUSH`/`POP` of a register or memory operand read/write through `SS` regardless of `DS`? I currently read `SS` directly for those and ignore any override on the pushed value's *source* — confirm that `SS` is not itself overridable for the stack, and say explicitly whether a segment override prefix before `PUSH` has any effect at all on an 8086.

**Q2.2** — For the string operations, the destination segment is `ES` and is famously not overridable. Confirm for 8086. Is the *source* segment (`DS`) overridable on `MOVS`, `CMPS`, `LODS`, `SCAS`?

**Q2.3** — For `LES`, `LDS`, `POP Sreg`, `CALL`/`JMP` indirect: confirm which segment the memory operand reads through, and whether an override applies.

---

## 3. Three unresolved items from the same emulator

These are separate questions but from the same architecture, and I'd like them answered in the same pass.

### 3.1 A straddling word at the top of the bus

**Q3.1.1** — When a 16-bit operand is at offset `FFFFh` within a segment, how does the 8086 fetch its second byte?

There are two plausible models and I have implemented the first one, on reasoning rather than on a source:

- **Model A (what I implemented):** the effective address is computed once as `(segment << 4) + offset` in a 20-bit adder, and the second byte comes from `physical + 1`, masked to 20 bits. So `SS:FFFFh` reads bytes at physical `(SS<<4)+0xFFFF` and `(SS<<4)+0x10000` (wrapped to 20 bits).
- **Model B:** the adder wraps the **16-bit offset** to `0000h` and the segment base is re-added, so the second byte comes from the segment base, i.e. physical `(SS<<4) + 0`.

Which is correct on real 8086 hardware? Quote the manual. This is the famous "can an 8086 word operand cross a 64 KiB boundary within a segment" question, and I want the documented answer rather than a folklore one.

**Q3.1.2** — If Model B is correct, what is the general rule for `physical = (segment << 4) + offset` when the sum exceeds `FFFFFh`? Is the 20-bit truncation applied to the final sum, or does the offset wrap first?

**Q3.1.3** — Does the answer differ between the 8086 and the 8088?

**Q3.1.4** — Is there an authoritative statement about why the 8086's 640 KiB DOS limit exists, phrased in terms of this behaviour?

### 3.2 Opcode 8F: does it take mod=00 only?

My project has a genuine contradiction between two of its own sources:

- The opcode map cell says `8F | POP r16/m16` — implying all memory forms.
- A prose note in the same document says *"`8F` likewise takes `MOD 000` only"*, and my decoder rejects `mod=01` and `mod=10` on that basis.

**Q3.2.1** — On an 8086, which encodings of `8F` actually execute? Is `8F` `mod=00` only, or does it accept `mod=01` and `mod=10` like other memory operands?

**Q3.2.2** — What is the ModR/M structure of `8F`? If the `reg` field is the opcode extension and `rm` is the operand, does `mod` behave normally?

**Q3.2.3** — If `8F mod=11` exists, what does it do? (On the 80186 it is `POP r/m16` with a register operand; on an 8086 it may be something else or nothing.)

### 3.3 A gate that cannot see a routed prefix

This is a question about the 8086's decoder rather than about my code, but the answer determines what my test harness can assert.

**Q3.3.1** — I have a test that walks all 256 first opcode bytes and asserts none reports "unknown opcode". It passed even when four byte values were being routed to a completely wrong handler, because those bytes produced a *different* error rather than "unknown opcode".

Is there a documented notion of the 8086 **raising any kind of trap or exception for an unknown opcode**? I believe it does not, but I want the documentation. If the 8086 has no invalid-opcode trap at all, then an emulator's "unknown opcode" is entirely a policy decision, not hardware behaviour — confirm or refute that.

**Q3.3.2** — When the 8086 executes a byte it does not define, does it reliably do *something*, or is the behaviour unspecified/electrical? Quote the manual or a microcode reference.

---

## 4. What I need back

For **each** question, in this format:

- **Answer** — one or two sentences.
- **Source** — document name, section or page, and the quoted wording or the specific microcode listing / bit pattern.
- **Confidence** — one of: *documented in Intel's manual* / *documented in a published microcode or die analysis* / *widely stated in secondary sources only* / *not decidable from available sources*.
- **If secondary or undecidable**, say what would settle it: a specific page, a specific figure, or a specific experiment.

Then, separately:

- **A summary table** of the default segment per base register, so I can implement it directly and write tests against each row.
- **An implementation-ready statement** of the BP rule in one or two sentences, phrased so I can turn it into a doc comment without interpretation.
- **A list of every place my current description of the 8086 that you can correct**, if you find one.

## 5. Constraints

- Do not write emulator code for me. I want the rules, not an implementation.
- If you are relying on recollection rather than a source you actually read in this session, **mark that claim**. I will treat unmarked recollections as unverified.
- If you cannot fetch a source, say which and why, rather than substituting a weaker one silently.
- Where two reputable sources disagree, present both and say which is better supported and why.

---

## 6. Context you may want

For orientation only; none of this is something to take on faith:

- The emulator models a flat 1 MiB physical space formed by `(segment << 4) + offset` truncated to 20 bits.
- Registers: `AX BX CX DX SI DI BP SP`, segment registers `ES CS SS DS`, `IP`, `FLAGS` with `CF PF AF ZF SF OF TF IF DF`.
- The decoder keeps a pending segment override from a `26/2E/36/3E` prefix and currently defaults everything else to `DS`.
- Instruction length and address computation follow the standard ModR/M rules; the emulator distinguishes the `disp16` direct form from register-mode operands.