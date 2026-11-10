# The debugger's core (P3)

Objective: the part of the debugger that is not a picture — the expression
evaluator, the panel model and the breakpoints — as a library the TUI and
the GUI both draw from. Branch: `main`. Delivery: one commit per part, each
leaving the tree green. Test runner: `cargo test --workspace`.

## Why this and not more

The research's roadmap puts a debugger that is actually a debugger before
the TUI (`odd/research/features-and-ux.md` §6): P3 is the expression
evaluator, the watchpoints and the conditional breakpoints, plus **the panel
model as a pure function of the state**. The TUI (P4) and the GUI render
that model; neither of them draws a second one.

The reference's own words are the spec. Its expression language is in
`re/docs-txt/stop_on_condition.txt`, the evaluator itself is in
`diasm.dll` — the DLL that also holds the disassembler — and its five
refusals are in that DLL's strings (`re/out/diasm/strings.tsv`):
`expression too complex`, `bad operator in stack!`, `bad syntax`,
`missing an operand` and `bad expression`.

## Specs

- **S1** — The expression language is the reference's: a literal in decimal
  (`5`), in binary (`101b`) and in hexadecimal with its `h` suffix (`34h`);
  the registers of the 8086; a word of memory written `SSSS:OOOO`, whose two
  halves are hex and need no suffix; and the operators `~` (not), unary `-`,
  `*`, `/`, `%`, `+`, `-`, `<<`, `>>`, `&`, `^` and `|`.
- **S2** — The values are the reference's: 32-bit, with the registers and
  the memory zero-extended from their 16 bits, and a shift count masked to
  five bits, which is what the reference's evaluator does with its shift.
- **S3** — The five refusals are the reference's, word for word.
- **S4** — The panel model is a pure function of the machine's state and of
  a UI state struct: the disassembly, the registers with their flags, the
  memory as hex and characters, and the stack. Nothing in it draws, and
  nothing in it reads a terminal.
- **S5** — A conditional breakpoint is an expression: `b AX==0x1234 &&
  CS==0x0200` stops where the expression is true. The comparisons and the
  logical operators are this project's addition, because the reference's
  expression has none; the reference's own breakpoint syntax is not
  recovered anywhere — the word "breakpoint" appears in none of its
  thirty-four documentation files — so this part is a design choice and
  says so.
- **S6** — A watchpoint is an address: `w 0100:0004` fires on a write to
  it, and an access that cannot fire says why. The memory says where its
  last write landed, because a write of the value that was already there is
  still a write.

## Out of scope

- **The TUI itself** (P4, T16) and the GUI (T18). This is the model they
  render, not a renderer.
- **The debug command vocabulary** of the reference (`c i iw o ow d t p r
  rf` in `re/docs-txt/debug.txt`). It is the CLI's own front end, and it
  comes with the TUI, where the panels are.
- **The guest's screen.** The reference's VGA screen is a panel of its own
  (`frmScreen`), and the research says to plan its takeover from the start.
- **Reverse execution.** The reference has a journal (`mStepBack`), and it
  is a feature of its own.

## Log

- **L1 (2026-11-10).** The unit the maintainer chose: the debugger's core.
  - **The crate.** `crates/debug` (`emo8067-debug`), which depends on the
    core and on the assembler and on nothing else. The TUI, the GUI and the
    CLI are three consumers of one model, so the model is a library and not
    a module of the CLI.
  - **The evaluator** (`expr.rs`): the reference's language, its 32-bit
    values, its five refusals, and the comparisons and the logical
    operators this project adds. The shift count is masked to five bits
    because the reference's own evaluator masks it: its shift reads
    `(byte)count & 0x1f`.
  - **Two hexadecimal spellings.** The reference's `1234h` and the
    `0x1234` of the research's breakpoint examples are both read, because
    the research's own example is `b AX==0x1234 && CS==0x0200`.
  - **The panel model** (`panels.rs`): the disassembly, the registers, the
    memory and the stack, each a pure function of the machine and of a UI
    state struct. The disassembly comes from the assembler's own
    disassembler, in the reference's text.
  - **The breakpoints** (`breakpoints.rs`): a breakpoint with a condition,
    and a watchpoint that fires on a write.
  - **The memory remembers where it was last written.** The core's memory
    gained one field, and a watchpoint asks the memory, because a write of
    the value that was already there is still a write and comparing the
    memory before and after would not see it.
  - **Still open.** The reference's own "stop on condition" window works
    only while it is open, and this library has no window: the TUI decides
    when the conditions are checked. The reference's evaluator reads the
    registers and the memory of the machine it is attached to, and so does
    this one; what it does with a value it cannot read is the reference's
    business and is not recovered.
