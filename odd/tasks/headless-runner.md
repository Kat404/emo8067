# T12 — the headless batch runner and the `.asm` harness

Objective: a CLI that assembles a program and runs it headless, and a harness
that runs the reference's own eighty-five examples and pins what each one
leaves behind. Branch: `main`. Delivery: one commit per task, each leaving the
tree green. Test runner: `cargo test --workspace`, plus the examples in
`re/extracted/app/examples/` (local gold, never committed).

Written 2026-10-22, after the assembler closed (T13) and made the criterion
reachable: T12's exit criterion was unreachable before, because assembling
`.asm` was T13's work.

## Why this and not more

The core is checked against hand-written programs and against hardware
captures. Both are narrow in the same way: they test one behaviour at a time,
on programs this project wrote. The reference's own eighty-five programs are
not that: they are eighty-five programs somebody else wrote, in the reference's
dialect, doing real things. Running them is the only way to see what the
emulator does to code that was not written for it — and a state that does not
change when the core changes is the cheapest regression harness there is.

The plan's own words for T12: *"A CLI that assembles and runs a file and
prints registers, flags and memory. Exit criterion: every example in
`re/extracted/app/examples/` produces a deterministic, checked-in golden
state. This is what makes T13..T16 safe to change."*

## Specs

- **S1** — The runner assembles and runs a `.asm`. `emo8067 run <file.asm>`
  assembles the file and runs the bytes: the image is loaded at the program's
  origin (its `ORG`), `CS:IP` starts there, and `SS`, `DS` and `ES` default to
  the origin as well, because the assembler makes one flat program. The
  existing options still override them. A file that is not `.asm` is still
  loaded raw, as today. The include path is searched: the file's own directory
  first, then the directories the caller names (`--include`, repeatable); the
  reference's library lives in `re/extracted/app/inc/`, one directory above
  the examples, so it has to be named.
- **S2** — The runner prints the registers, the flags and where it stopped,
  as it does today. The **golden text** is that state plus the memory that is
  not zero, page by page, as hex: the state says what the processor holds, and
  the pages say what the program wrote. It is one artefact that pins both the
  assembler's bytes (the image is in the pages) and the emulator's behaviour
  (the state and the writes).
- **S3** — The harness runs every example in the local gold and compares each
  one's golden text with the recorded one. It names every example whose state
  changed, and how many matched. **The goldens live with the examples**, which
  are gitignored because they come from a proprietary binary; the harness
  stands aside when the examples are not there, the way the assembler's gate
  does. The plan's wording says "checked-in golden state"; that is not
  possible for the same reason the examples cannot be committed, and the
  decision of 2026-10-19 ("local gold, never committed") governs.
- **S4** — The run is deterministic: no time, no randomness, no device
  unless one is asked for. A program that stops because it asked for a
  service the emulator does not have (an `INT` with no handler installed) is
  a normal end, and the runner says which interrupt it was. A program whose
  **handler is installed** is not cut short: the interrupt is taken and the
  program continues, because a runner that stops a program at its own
  interrupt is broken.

## Tasks

- **T1** — **DONE.** The runner assembles and runs a `.asm` (S1), and the
  golden text is the state plus the pages that are not zero (S2). The load
  model is the reference's own: the load segment is `0000` by default and
  the load offset is the program's `ORG`, with `CS:IP` set to the same
  values and `SS = DS = ES` following the load segment. `--load-seg SEG`
  overrides the segment; `--include DIR` names the include path; `--golden`
  prints the pages.
- **T2** — **DONE.** The harness (S3): it runs every example in the local
  gold, compares the golden text with the recorded one, and names what
  changed. It stands aside when the gold is not there. The goldens live in
  `re/extracted/app/goldens/`, beside the examples, gitignored.

## Log

- **L1 (2026-10-22).** The load model is the reference's, and the
  reference states it in its own boot template
  (`re/extracted/app/inc/3_boot_template.txt`): *"it loads this data into
  memory at 0000:7c00h and starts the execution from the first byte"* and
  *"default configuration for .binf file: load segment: 0000, load offset:
  7c00, same values are set to CS and IP registers accordingly"*. The
  `.COM` template uses the same model with `org 100h` as the load offset.
  So a program is loaded at its `ORG` in the segment `0000`, `CS:IP` are
  set to the same values, and the segments follow the load segment. A
  label is an `ORG`-based offset inside its segment, so with the load
  segment `0000` a label's value is also its physical address, and the
  data references land on the program's own bytes.
  **This corrects two earlier measurements.** The first version of this
  plan's L1 loaded every example at the runner's raw defaults, and the
  first implementation made the origin the segment. Both are conventions
  of our own: the first breaks the programs' data references (a program
  that counts a string read a zero), and the second puts `timer.asm`'s
  boot sector at `0x83C00` instead of `0x7C00`. The reference's model is
  the one above, and the labels are right under it — proved by a program
  that reads a string back through its label and counts five characters.
  **The tally over the eighty-five examples, under the reference's rule:**
  **twenty** run until the step budget is spent, **sixty-four** stop at an
  interrupt the emulator has no service for (thirty-one at `INT 10h`,
  thirty-two at `INT 21h`, one at `INT 15h`), and **one** halts
  (`simple_io.asm`, the port-I/O demo). So the goldens are of two kinds:
  twenty are fat — a whole program's execution, its loops, its string
  handling, its arithmetic — and sixty-four are thin: the setup code, the
  buffer, and the call. A thin golden still pins the assembler's bytes
  and the first steps of the emulator, but it is thin, and the plan says
  so rather than let "every example has a golden state" read like "every
  example is covered".
  **A BIOS layer would make them fat, and it is not in this unit.** The
  guide's own open question 1 says the register conventions for every
  `INT 21h` function but the five it could verify are unknown from a
  reachable public source, so a DOS layer would be guesswork. The
  reference's own `INT` behaviour is a separate unit, with its own
  evidence.

- **L2 (2026-10-22).** What the runner does with an interrupt, and what is
  still open.
  **The rule.** The core takes the interrupt before the runner can look at
  it: it pushes `FLAGS`, `CS`, `IP` and vectors the CPU through the table
  at `0000:0000`. The runner then reads the vector: four bytes per vector,
  the offset at `n*4` and the segment at `n*4+2`. A vector of zeros means
  no handler, and the run stops there — the program asked for a service
  the emulator does not have, and the runner says which one. A vector that
  is not zero means a handler is installed: the interrupt is taken and the
  program continues, because a runner that stops a program at its own
  interrupt is broken.
  **A program that sits on the interrupt table is the reference's own
  behaviour, not a defect.** A program with no `ORG` is loaded at physical
  zero, which is the interrupt table, and it overwrites it — the
  reference's boot template says exactly this: *"you may not use dos
  interrupts for boot record code, unless you manually replace them in
  interrupt vector table"*. No heuristic, no refusal; the golden pins
  what happens.
  **Still open.**
  - The **build directives** are not implemented: `#cs = 500#`, `#ds=`,
    `#ss=`, `#ip=` and the `#make_bin#`/`#make_com#` family are in the
    assembler's S5 and not in its code. `robot.asm` and its neighbours
    carry them, so the runner's default load segment `0000` is not the
    segment the reference would use for them. `--load-seg` is the knob
    until the directives land.
  - `--load SEG:OFF` (the physical address) and `--load-seg SEG` (the
    segment) are two knobs; whether the first should move the segments
    too is a decision the maintainer has not taken.
  - The **exit code for an unhandled `INT`** is still 1. Whether a
    program that asked for DOS and got none is a failure is the
    maintainer's call, and the plan does not take it.
  - The harness costs about **ten seconds** (eighty-five subprocesses,
    twenty of them running a hundred thousand instructions), down from
    thirty-four before it was run in parallel.

