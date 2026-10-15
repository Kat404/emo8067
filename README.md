# emu8067

A clean-room, free-software Intel 8086/8088 emulator, assembler and debugger
that runs on Linux, macOS and Windows.

The original [emu8086](http://www.emu8086.com) is a capable 8086 teaching
emulator, but it is Windows-only, closed-source shareware behind a 14-day
trial, and there is no clear answer to where a Windows user should download
it from. This project exists to provide the same capability as free software
on every major platform, and then to go beyond it.

## Clean-room

Nothing in this repository is derived from decompiled third-party code.

Reverse engineering of the reference implementation was used **only** to
establish observable behaviour: which instructions exist, what the operand
syntax is, how flags are defined, what the device and interrupt buses do,
and what the user interface actually offers. The emulation core is written
from the public Intel 8086/8088 architecture documentation and from
executable tests.

The full reverse-engineering report, including the reference binary's
module map and behavioural contracts, lives in
[`re/FINDINGS.md`](re/FINDINGS.md). It is a specification document, not a
source of code.

## Layout

| Path | What it is |
|---|---|
| `crates/core` | The emulator core. No dependencies, no I/O, `#![forbid(unsafe_code)]`. |
| `re/FINDINGS.md` | Reverse-engineering report on the reference implementation. |
| `odd/tasks/emu8067-foss.md` | Feature specification and task log. |

## Status

Early. The core currently covers the 20-bit address space, the register
file, FLAGS, ModR/M decoding, and the arithmetic and logic instruction
groups with exact carry, parity, auxiliary-carry, sign, zero and overflow
semantics. Everything is unit-tested; there is no UI yet.

## Licence

This project is released into the public domain under
[The Unlicense](https://unlicense.org/). See [`LICENSE`](LICENSE).

### Note on FASM

If this distribution bundles the FASM assembler binary (`fasm/FASM.EXE`),
FASM is covered by its own license, Copyright (c) 1999-2010 Tomasz
Grysztar. That license permits redistribution in binary form provided the
copyright notice and conditions are reproduced in the documentation, and
forbids relicensing FASM's own code under different terms (explicitly
including the GPL). FASM is redistributed here unmodified and separate
from this project's source; this Unlicense applies only to the emu8067
source code, not to FASM.