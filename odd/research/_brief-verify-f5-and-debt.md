# Brief — verify F5, and clear the §1.3 documentation debt

**Superseded in part.** This brief is kept as the record of what was believed when it was written, and the body below is unchanged. Three of its claims were disproved by this project's own later work; reading them as current guidance will reintroduce a defect.

1. **The permutation probe, "Part 1" item 1 (~line 11).** It asks whether the same covered instruction reaches the same final state behind every permutation of `{F0, F1, 26, 2E, 36, 3E, F2, F3}`. That is no longer the project's claim: order **does** matter between two segment overrides, because the last one wins. The permuted test was also vacuous, because the override was discarded on the SI side. `fe16b6c` replaced it.
2. **The `F1` sign flag question, "Part 1" item 2 (~line 12).** It asks what the multiply does with the `F1` sign flag still set, and that question turns out to be the right one. Andrew Jenner, from microcode extracted off an 8086 die: "Using the REP or REPNE prefix with a MUL or IMUL instruction negates the product. Using the REP or REPNE prefix with an IDIV instruction negates the quotient." The multiply and divide microcode flips its internal F1 flag once per negative sign in the inputs, and a REP prefix is what loads that flag. Implemented in `ab12c21`. (This brief's premise was not false; an earlier banner here called it void, and was itself wrong.)
3. **The `CX = 0` expectation, "Part 1" item 3 (~line 13).** It says a `REP` with `CX = 0` must do one iteration. It does not. The printed manual, Vol. 2 p. 2-41, says the string instruction is not executed at all, and so does the microcode: the `RPTS` sub-routine at ROM address `0x112` is the *first* micro-instruction of all four string routines and jumps straight to `RNI` on a zero count, before any memory access and without decrementing `CX`. The emulator runs zero iterations, and the evidence agrees with the manual rather than contradicting it, so this is not an erratum. Pinned by `rep_with_a_zero_count_runs_no_iterations` in `314f493`.

Two jobs. The first is verification. The second is documentation debt that verification of the previous units has already proven wrong.

## Part 1 — verify F5

F5 made `REP`/`REPNE` a latch set by `step`, alongside the segment override, so prefix order stopped mattering. Read the commits the implementer made in `crates/core/src/{exec.rs,lib.rs}` and `crates/core/tests/regression.rs`, plus `/home/josel/Proyectos/emo8067/odd/research/_brief-f5-rep-latch.md` for the intended behaviour.

Check, highest risk first:

1. **Is prefix order genuinely irrelevant now?** Build a probe that runs the same covered instruction behind every permutation of `{F0, F1, 26, 2E, 36, 3E, F2, F3}` and compares the full final state — `IP`, all registers, FLAGS, and a hash of memory. Report any permutation that differs and from which prefix.
2. **Is a `REP` before a non-string instruction ignored rather than rejected?** The 8086 has no invalid-instruction detection circuitry, so it cannot object. Verify `F3 90`, `F3 F7 E1` and similar do not error, and say what the multiply does with the F1 sign flag still set — that one was declared out of scope, so confirm it is *unregressed* rather than fixed.
3. **Repeat counts.** `CX` must decrement once per iteration **including the last**, a `REP` with `CX = 0` must do one iteration, and `CMPS`/`SCAS` must stop at the first `ZF` mismatch for their latch. Probe the boundary cases, not just the middle.
4. **Latch lifetime.** Neither latch may survive its instruction, including after a fault that returns `Err`. Probe a faulting covered instruction followed by one that would show the latch if it leaked.
5. **`dispatch`'s match is still exhaustive over `u8`.** Delete one arm in a `/tmp` copy and confirm you get `error[E0004]`. If the prefix arm lost a byte, that is a defect.
6. Each of the four commits should leave the tree green. Check `git log` and confirm none is a half-finished state.

## Part 2 — clear the §1.3 debt

`odd/research/documentation-guide.md` §1.3 is the project's opcode map, adopted as the authority the decoder is written against. Independent verification and microcode evidence have since shown several cells wrong. This part is documentation, not code.

Rewrite the affected cells and notes **in place**, keeping the file's existing structure and the row/cell discipline its guard enforces (`python3 odd/research/verify_opcodes.py` must still pass, and it requires exactly sixteen cells per row). Do not add rows, drop rows, or restructure the table.

Cells and claims known to be wrong or unresolved:

| Cell | What the document says | What the evidence says |
|---|---|---|
| `60`–`6F` | `(not used)`, with a note saying they are | Aliases of `70`–`7F`; the condition is the low four bits. Group Decode pattern `011?????` runs the same microcode. |
| `C0`, `C1`, `C8`, `C9` | `(not used)` | `C0` = `RET imm16` like `C2`; `C1` = `RET` like `C3`; `C8` = `RETF imm16` like `CA`; `C9` = `RETF` like `CB`. Microcode pattern `1100?0?0` runs for all four. |
| `D6` | `(not used)` | `SALC`: `AL = CF ? FFh : 00h`, writes no flag. |
| `F1` | `(not used)` | A second `LOCK`. Decoded in logic, and the Group Decode ROM gives it the same outputs as `F0`. |
| `A8`, `A9` | `TEST AL/AX, imm`, footnote ‡5 | Real silicon executes `CMP`. The 1979 printing says `TEST` in both Table 4-12 and Table 4-13; the project settled on silicon and recorded the erratum. The footnote must now say **settled**, not open. |
| `84`, `85` | footnote ‡3, marked as an unresolved conflict with a do-not-implement note | Both are real 8086 instructions. Jenner's microcode carries a top-level `TEST rm,r` routine handling exactly two opcodes, and Shirriff's hole enumeration omits them while naming `80`–`87` as a byte/word-selected decode family. The footnote must record the evidence and drop the do-not-implement instruction. |
| `82` | discrepancy note about which digits are valid | On the 8086 the Immed group latches the second byte's bits 5–3 into the ALU X register, so `82 /1`, `82 /4` and `82 /6` run as OR, AND and XOR even though Table 4-13 prints them as not used. Also, only `81` is treated as 16-bit by the L8 condition, so `83` is the sign-extending byte-immediate/word-register form. |
| second-byte holes | not addressed | The `/digit` holes are settled: `F6 /1` and `F7 /1` are `TEST r/m, imm`; `D0`–`D3 /6` is `SETMO`; `FE /7` and `FF /7` are `PUSH r/m8`; `FE /6` reaches the same `PUSH rm` microcode. `FE /2`–`/5` run CALL and JMP microcode that depend on bus width, so they are not meaningful in byte mode. `8F` takes `mod=00`, `01` and `10` with `reg=000`; `mod=11` is undefined for it. |

Cite Ken Shirriff's articles (righto.com) and Andrew Jenner's disassembly (reenigne.org) for each corrected cell, with the specific microcode listing or bit pattern. Where a claim rests only on a secondary source, say so. Where two sources disagree, present both.

Do **not** touch §2, §3 or the `## Sources` list beyond adding references the new cells cite. Do not change §1.4's flag table; a separate decision owns that.

## Constraints

- Read-only on `crates/`. Part 2 writes only to `odd/research/documentation-guide.md`.
- `odd/research/_brief-u7-segment-prefixes.md` and `odd/research/_brief-u6-remaining-opcodes.md` still contain claims this work overturns — report them; do not edit those briefs.
- Use `codegraph_explore` for anything about `crates/`.
- Run `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check`, both gates in `crates/core/tests/regression.rs`, and `python3 odd/research/verify_opcodes.py`.
- A green suite is not evidence. This project's tests have carried wrong expectations repeatedly; trust the manual and the microcode.

## Allowed edit surfaces
odd/research/documentation-guide.md
The section ends here.

## Rules
- English, neutral register.
- Never answer an 8086 question from memory; cite or fetch.
- Report only on `crates/`. Fix nothing there.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.