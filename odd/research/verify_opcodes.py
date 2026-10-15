#!/usr/bin/env python3
"""Structural guards for the transcribed 8086 tables in documentation-guide.md.

Section 1.3 is the 256-entry opcode map and 1.4 the per-instruction flag
table, both transcribed from the Intel 8086 Family User's Manual. An earlier
transcription of 1.3 lost a cell from one row, every later opcode shifted by
one, an instruction was built from the wrong row, and the emulator corrupted
memory silently. These checks fail when a row is malformed again.

They are structural only. They cannot tell whether a cell says the *right*
thing, only whether the grid has the shape a reader will assume it has. A
green run here means the table is well formed, not that it is correct.

Run:  python3 odd/research/verify_opcodes.py
"""

import re
import sys
from pathlib import Path

GUIDE = Path(__file__).resolve().parent / "documentation-guide.md"
OPCODES_PER_ROW = 16
FLAG_COLUMNS = ("O", "D", "I", "T", "S", "Z", "A", "P", "C")
# What the manual's Table 2-18 legend allows in a flag cell. X is defined, U
# undefined, 0 and 1 are a forced value, R is restored, and the dash means
# the flag is not touched at all.
FLAG_VALUES = frozenset("—X01UR")  # noqa: RUF001
OPCODE_ROW_LABEL = re.compile(r"\*\*([0-9A-F]{2})\*\*")


def section(text: str, heading: str, until: str) -> str | None:
    """Return the text between two headings, or None if the first is absent."""
    try:
        start = text.index(heading)
    except ValueError:
        return None
    try:
        return text[start : text.index(until, start)]
    except ValueError:
        return None


def rows_of(grid: str) -> list[list[str]]:
    """The pipe rows of every table in `grid`, cells stripped, outer pipes off."""
    out = []
    for raw in grid.splitlines():
        line = raw.strip()
        if line.startswith("|"):
            out.append([cell.strip() for cell in line.strip("|").split("|")])
    return out


def check_opcode_map(grid: str | None) -> list[str]:
    """Every row must carry sixteen cells, and the grid must cover 00..F0 once."""
    if grid is None:
        return ["section 1.3 not found"]

    problems: list[str] = []
    seen: dict[str, int] = {}
    for cells in rows_of(grid):
        match = OPCODE_ROW_LABEL.fullmatch(cells[0])
        if not match:
            continue  # header row, separator, or some other table in the section
        label = match.group(1)
        seen[label] = len(cells) - 1
        if len(cells) - 1 != OPCODES_PER_ROW:
            problems.append(
                f"row {label}: {len(cells) - 1} opcode cells, "
                f"expected {OPCODES_PER_ROW}"
            )
        for offset, cell in enumerate(cells[1:]):
            if not cell:
                problems.append(f"row {label}: cell +{offset:X} is empty")

    duplicates = [label for label in seen if list(seen).count(label) > 1]
    for label in sorted(set(duplicates)):
        problems.append(f"row {label}: duplicate opcode row")

    expected = {f"{nibble:X}0" for nibble in range(OPCODES_PER_ROW)}
    for label in sorted(expected - set(seen)):
        problems.append(f"missing opcode row: {label}")

    return problems


def check_flag_table(grid: str | None) -> list[str]:
    """Every flag row needs all nine columns and only legend characters."""
    if grid is None:
        return ["section 1.4 not found"]

    problems: list[str] = []
    rows = 0
    # A flag row is: instruction, nine flags, a note, and a page reference.
    width = len(FLAG_COLUMNS) + 3
    for cells in rows_of(grid):
        if len(cells) != width:
            continue
        name = cells[0].strip("*` ")
        if not re.fullmatch(r"[A-Z][A-Z0-9_]{0,7}", name):
            continue
        rows += 1
        flags = cells[1 : 1 + len(FLAG_COLUMNS)]
        for column, cell in zip(FLAG_COLUMNS, flags, strict=True):
            mark = cell.strip("*` ")
            if mark not in FLAG_VALUES:
                problems.append(
                    f"{name}: flag {column} has {cell!r}, which is not in the legend"
                )

    if rows == 0:
        problems.append("no flag rows found in section 1.4")
    return problems


def main() -> int:
    """Check both tables and report every problem found, not just the first."""
    text = GUIDE.read_text(encoding="utf-8")

    problems = check_opcode_map(section(text, "### 1.3", "### 1.4"))
    problems += check_flag_table(section(text, "### 1.4", "\n## 2."))

    if problems:
        print("FAIL: the transcribed 8086 tables are malformed", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    print(
        f"OK: {OPCODES_PER_ROW} rows x {OPCODES_PER_ROW} cells "
        f"= {OPCODES_PER_ROW * OPCODES_PER_ROW} opcode cells, flag table well formed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
