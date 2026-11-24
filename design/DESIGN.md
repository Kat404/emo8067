# emo8067 — the TUI's design

The design of the terminal debugger: the tokens, the shape and the rules.
The colours' **meanings** are the research's (`features-and-ux.md` §4.4);
this document settles the **values**.

## Overview

The terminal debugger for an 8086 machine: the product's primary face,
drawn from the panel model `emo8067-debug` holds. The people who use it
are reading an old machine's memory, and the north star is **a debugger
that reads like an IDE**: dense, calm, and every fact in one place.

## Colors

The roles, and where each one appears. The values are the tokens in
`crates/tui/src/theme.rs`.

| Role | Value | Where |
|---|---|---|
| canvas | `#11141a` | the screen's ground |
| surface | `#161b22` | the header's and the bottom band's ground |
| rule | `#2b3138` | the thin rules between the sections |
| text primary | `#c3ccdc` | the values, the instruction's text |
| text secondary | `#8b95a5` | the labels, the addresses |
| text muted | `#616b7a` | what has run, the gutter's addresses |
| accent | `#56b6c2` | the focus, and the instruction the machine is at |
| warning | `#d6b352` | a breakpoint |
| error | `#e06c75` | a watchpoint, and a refusal |
| success | `#98c379` | a value that was written |

**The One Fact Rule.** A fact is on the screen once: the program's name is
in the header and nowhere else, and the machine's state is in the status
bar and nowhere else.

**The Colour Rule.** The research's meanings are kept: cyan is the current
`IP`, grey is what has run, yellow is a breakpoint and red is a
watchpoint. The values are the calm ones above, not the terminal's own
ANSI ones.

**The Guest's Palette.** The guest's screen has a palette of its own: the
sixteen colours the VGA's palette holds. The first eight are the
foreground's and the last eight the background's — the bright half of the
first eight, not a shade of the seventh. They are the guest's colours and
not the debugger's, and they appear on the guest's screen and nowhere
else.

| | Colour | Value | | Colour | Value |
|---|---|---|---|---|---|
| 0 | black | `#000000` | 8 | dark grey | `#555555` |
| 1 | blue | `#0000AA` | 9 | light blue | `#5555FF` |
| 2 | green | `#00AA00` | 10 | light green | `#55FF55` |
| 3 | cyan | `#00AAAA` | 11 | light cyan | `#55FFFF` |
| 4 | red | `#AA0000` | 12 | light red | `#FF5555` |
| 5 | magenta | `#AA00AA` | 13 | light magenta | `#FF55FF` |
| 6 | brown | `#AA5500` | 14 | yellow | `#FFFF55` |
| 7 | light grey | `#AAAAAA` | 15 | white | `#FFFFFF` |

## Type roles

| Role | Attribute | Where |
|---|---|---|
| title | bold + accent | the focused section's header |
| value | normal | the register's value, the instruction's text |
| label | dim (text secondary) | the register's name, the flag's name |
| meta | dim italic | the file and the line in the source strip |

**The Role Rule.** A label is dim and a value is not, everywhere, and the
one thing that matters in a section is the only bright thing in it.

## Layout

The bands, top to bottom:

1. **The header** — one row: the product, the program's name, and the
   state.
2. **The main band** — the listing, or the window that takes its place
   (the stack, the conditions, the console). Under it, the source strip.
3. **The bottom band** — three sections side by side: the registers, the
   memory window and the flags.
4. **The status bar** — one row.

The registers take 22 columns, the memory window takes the width its
bytes need, and the flags take what is left. The bytes a row of the
memory window holds follow the width the band gives it, in powers of two
so the row addresses stay the memory's own: eight at eighty columns,
sixteen at a hundred and twenty, thirty-two at a hundred and forty. The
printable side is drawn beside them when the columns are there for both.

**The Takeover.** The guest's screen takes the screen: `x` opens it, and
it takes every row but the status bar, where the machine's state is and
nowhere else. The page is drawn from its top-left corner, and the part of
it the terminal cannot hold is cut. It is a takeover and not a section:
no header of its own. The debugger's own keys keep working while it is
open — the machine still runs, still steps and still halts. The keys that
act on the listing do not: the takeover holds the whole screen and the
listing's cursor is not on it, and a cursor nobody can see is not one to
move or to set a breakpoint at.

## Surfaces

The canvas is the ground. The header and the bottom band are one step
above it (the surface colour). The sections are separated by a thin rule
— a horizontal one between the bands, a vertical one between the bottom
band's sections — and by nothing else. **No panel is framed with a box.**

**The Rule Rule.** Two sections are separated by a rule, and only by a
rule. A box is a card, and a card per fact is noise.

## Glyphs

One family, geometric:

| Glyph | Meaning |
|---|---|
| `▸` | here: the listing's cursor, and the source line the machine is at |
| `●` | the machine is running |
| `○` | the machine is halted |
| `◆` | a breakpoint |
| `◇` | a watchpoint |
| `─` `│` | the rules |

**The Glyph Rule.** One family, one meaning per glyph: `▸` always means
"here", and nothing else ever means "here".

## Components

- **The header**: the product dim, the program's name in the value
  colour, the state with its glyph. Nothing else.
- **The listing**: the address in the gutter (text secondary), the bytes
  muted, the instruction's text in the value colour, and the instruction
  the machine is at with `▸` and the accent. A breakpoint carries `◆` and
  a watchpoint `◇`.
- **The source strip**: the line's number in the gutter, the source's own
  text, and the machine's line with `▸` and the accent.
- **The registers**: the name dim, the value normal, and the register
  under the cursor with the accent.
- **The memory window**: the row address in the gutter, the bytes in hex,
  the printable side beside them, and the cursor's byte marked — always,
  and with the accent when the section has the focus.
- **The flags**: the name dim, the value normal, and the flag under the
  cursor with the accent.
- **The guest's screen**: the text page the guest wrote to, cell by
  cell — the character, and the foreground and the background its
  attribute byte carries, in the guest's own colours.
- **The status bar**: the hints on the left, the volatile data on the
  right, divided by `·`.

## Do's and Don'ts

- **Do** keep the density: one row per fact, and a debugger shows many.
- **Do** mark a state with a glyph and a colour together.
- **Don't** frame a panel with a box.
- **Don't** spend a blank row on hierarchy.
- **Don't** show the same fact twice.
- **Don't** encode a state with colour alone.
- **Don't** mix glyph families.
- **Don't** wrap a value in a fixed rail: truncate it with `…`.
