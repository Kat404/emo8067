# Brief — F5: make REP a latch, so prefix order stops mattering

**Superseded in part.** This brief is kept as the record of what was believed when it was written, and the body below is unchanged. Three of its claims were disproved by this project's own later work; reading them as current guidance will reintroduce a defect.

1. **The `CX = 0` bullet, "Tests to add" (~line 56).** It says a `REP` with `CX = 0` does one iteration, as the hardware does. It does not. The printed manual, Vol. 2 p. 2-41, states: "If CX is 0, the string instruction is not executed, and control goes to the following instruction". The emulator runs zero iterations, and the microcode agrees with the manual: the `RPTS` sub-routine at ROM address `0x112` is the first micro-instruction of all four string routines and jumps straight to `RNI` on a zero count, before any memory access and without decrementing `CX`. Settled in `e03650c`, pinned by `rep_with_a_zero_count_runs_no_iterations` in `314f493`.
2. **The `IMUL`/`IDIV` bullet, "Out of scope" (~line 61).** It says a `REP` in front of `IMUL`/`IDIV` negates the result, and that `F3 F7 E1` computes a negated quotient. **The claim was right and this project marked it wrong.** Andrew Jenner, from microcode extracted off an 8086 die: "Using the REP or REPNE prefix with a MUL or IMUL instruction negates the product. Using the REP or REPNE prefix with an IDIV instruction negates the quotient" — the multiply and divide microcode flips its internal F1 flag once per negative sign in the inputs, and a REP prefix is what loads that flag. Only the worked example is wrong: `F7 E1` decodes as `MUL CX`, not `IDIV CX`, because the ModR/M reg field is bits 5-3 of `0xE1`, which is `100`, i.e. `/4`, so it negates the *product* in `AX:DX`. Implemented in `ab12c21`; this bullet is the one this brief got right for the wrong reason.
3. **The eight-prefix-byte bullet, "Tests to add".** It asks that a run of mixed prefixes in every permutation of `{F0, F1, 26, 2E, 36, 3E, F2, F3}` before the same instruction reach the same final state. That is no longer what the project claims: order **does** matter between two segment overrides, because the last one wins. `fe16b6c` replaced the old permutation test, which was vacuous, with tests that discriminate, and pins last-override-wins.

You are working on **emu8067**, a clean-room FOSS Intel 8086/8088 emulator in Rust at `/home/josel/Proyectos/emo8067`.

## Skills to load before work
- Native `skill` tool, id `ponytail` — the smallest change that fully works.

## Tooling
- **`codegraph_explore` is your primary index and search**, not grep/Read loops. Useful queries: `step prefix_effect dispatch`, `string_op compare push pop`, `Cpu seg_override Rep`.
- **Context7** (`resolve-library-id` then `query-docs`) for library questions.
- **webfetch** for anything about the 8086. **Never answer an encoding question from memory.** Four written specifications in this project have been wrong and had to be corrected by their implementers.

### Two tooling hazards, both learned the hard way here

1. **Do not delete or splice Rust code with a Python script that matches braces.** Three attempts in this repo corrupted the file that way — the last one deleted an entire function body and left `fn string_op_once` with no parameter list. Use the `edit` tool, or make one small targeted `python` replacement whose `old` text you have *just read from the file* and whose replacement you then verify compiles. After any edit, run `cargo build` before doing anything else.
2. **`edit` silently reverts six pre-existing lines in `crates/core/src/exec.rs` to pre-rustfmt style.** Run `cargo fmt` before committing, and confirm `cargo fmt --check` exits 0.

## The problem

`REP` (`F2`) and `REPE` (`F3`) are handled by a separate `repeat_prefix` helper that fetches the following byte itself and rejects anything that is not a string operation with `ExecError::BadRepeat`. That makes `REP` **order-sensitive** against the other prefixes:

| bytes | today | should be |
|---|---|---|
| `26 F3 A4` | works | works |
| `F3 26 A4` | `Err(BadRepeat(0xF3, 0x26))` | works — REP MOVSB through ES |
| `F3 90` | `Err(BadRepeat(0xF3, 0x90))` | runs as a NOP, latch ignored |

Ken Shirriff's analysis of the 8086 Group Decode ROM (righto.com) settles the design: column 31 indicates a `REP` prefix and sets the REP latch; column 32 indicates a segment prefix and **loads** the segment latches. They are independent latches set by independent columns, so their order cannot matter. The 8086 also has no invalid-instruction detection circuitry at all, so it cannot object to a `REP` before something that is not a string operation.

## What to build

1. **`Cpu` gains a REP latch** next to `seg_override`, holding a public `Rep` enum with `Rep` and `RepNe`. Same reasoning as the segment latch: not a register, no program can read it, dead once the instruction it covers retires.
2. **`prefix_effect` covers all six prefix bytes** and returns a typed effect — `Lock`, `Segment(Seg)`, or `Rep(Rep)` — instead of the current `Option<Option<Seg>>`.
3. **`step`'s prefix loop sets both latches**, and clears both on the way out including after a fault.
4. **The string operations honour the REP latch**: repeat while `CX` is non-zero, and for `CMPS`/`SCAS` stop at the first pair that fails the latch's condition (`REP` stops when `ZF` is clear, `REPNE` stops when `ZF` is set). Decrement `CX` once per iteration including the last, as the hardware does.
5. **Delete `repeat_prefix` and `ExecError::BadRepeat`.** A `REP` in front of a non-string instruction is simply ignored — that is what the hardware does, and refusing it would be the divergence.
6. **Update `dispatch`'s prefix arm** to list all six bytes so the match over `u8` stays exhaustive. That exhaustiveness is load-bearing: it turns a future hole into `error[E0004]` rather than a runtime surprise.

## Atomic commits

Commit as you go, so a failure costs one step rather than the unit. A sensible order, each leaving the tree green:

- **commit 1** — the `Rep` type and the `Cpu` latch, unused. Nothing else changes.
- **commit 2** — `prefix_effect` returning the typed effect, and `step` setting and clearing both latches. `REP` still dispatches to `repeat_prefix`, so behaviour is unchanged; only the latch is now populated. A test asserts the latch is set and cleared.
- **commit 3** — `string_op` honours the latch, `repeat_prefix` and `BadRepeat` deleted. This is the commit that changes behaviour.
- **commit 4** — the regression tests.

Each commit must leave `cargo test --workspace`, `cargo clippy --workspace --all-targets` and `cargo fmt --check` green. Use Conventional Commit messages in English.

## Tests to add

- `F3 26 A4` and `26 F3 A4` produce identical state, including which segment the destination went through.
- `F2 3E A4` and `3E F2 A4` likewise.
- `F3 90` executes and does nothing; it is not an error.
- A run of mixed prefixes in every permutation of `{F0, F1, 26, 2E, 36, 3E, F2, F3}` before the same instruction reaches the same final state.
- `REP` still repeats `CX` times and still stops at the first `ZF` mismatch for `CMPS`/`SCAS`; a `REP` with `CX = 0` does one iteration, as the hardware does.
- The pending latch does not survive the instruction: the next instruction sees no override and no repeat.

## Out of scope — report, do not fix

- **`REP` with `IMUL`/`IDIV` negates the result.** The F1 flag the multiply and divide microcode reuse as a sign flag is set by a REP prefix, so `F3 F7 E1` computes a negated quotient. That is a separate defect, still present, and this unit should not attempt it. Note it in your report.
- **The `FF` group** and the `BadRepeat`-free path have not been re-examined for other latch interactions.
- `StepInfo.opcode` for a `REP`ped string operation still reports the prefix byte rather than the covered one, while `LOCK` and the segment overrides now report the covered opcode. Making `REP` consistent with the others is a one-line change and is **in** scope if it is cheap; say whether you did it.

## Verification you must run and report

- `cargo test --workspace` — exact counts, per commit.
- `cargo clippy --workspace --all-targets` — warning and error counts.
- `cargo fmt --check` — must exit 0.
- Both gates in `crates/core/tests/regression.rs` run alone: `every_first_opcode_byte_the_8086_defines_decodes` and `every_modrm_digit_of_the_8086_is_an_instruction_or_a_named_hole`.

Never fake a passing run. If something fails, say so plainly and leave the tree green or report exactly what is red.

## Allowed edit surfaces
crates/core/src/exec.rs
crates/core/src/lib.rs
crates/core/tests/regression.rs
crates/core/tests/  (new files under it are fine)
The section ends here.

## Rules
- Do not touch `odd/`, `re/`, `crates/cli/`, or any `Cargo.toml`.
- `unsafe` is forbidden crate-wide.
- If you find a defect outside this scope, report it; do not fix it.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.