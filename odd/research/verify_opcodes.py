#!/usr/bin/env python3
"""Structural guard for the 256-opcode map in section 1.3 of documentation-guide.md.

An earlier transcription lost a cell from one row. Every later opcode then
shifted by one, the decoder built from the table was wrong, and the emulator
corrupted memory silently. This script fails when that happens again: any row of
the section 1.3 opcode grid that does not have exactly sixteen opcode cells, or
a grid that does not cover 0x00..0xFF exactly once each, exits non-zero.

Run:  python3 odd/research/verify_opcodes.py
"""

import re
import sys
from pathlib import Path

GUIDE = Path(__file__).resolve().parent / "documentation-guide.md"
EXPECTED_CELLS = 16
EXPECTED_ROWS = 16


def main() -> int:
    text = GUIDE.read_text(encoding="utf-8")

    start = text.index("### 1.3")
    end = text.index("### 1.4", start)
    grid = text[start:end]

    problems: list[str] = []
    rows = 0
    seen: dict[str, int] = {}

    for line in grid.splitlines():
        line = line.strip()
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        label = cells[0]
        if not re.fullmatch(r"\*\*[0-9A-F]{2}\*\*", label):
            continue  # header row or a non-grid table
        opcode = label.strip("*")
        rows += 1
        if len(cells) - 1 != EXPECTED_CELLS:
            problems.append(
                f"row {opcode}: {len(cells) - 1} opcode cells, expected {EXPECTED_CELLS}"
            )
        if opcode in seen:
            problems.append(f"row {opcode}: duplicate opcode row")
        seen[opcode] = len(cells) - 1
        for i, cell in enumerate(cells[1:]):
            if not cell:
                problems.append(f"row {opcode}: cell +{i:X} is empty")

    if rows != EXPECTED_ROWS:
        problems.append(f"{rows} opcode rows found, expected {EXPECTED_ROWS}")

    expected = {f"{v:X}0" for v in range(16)}  # rows are labelled 00, 10, ... F0
    missing = sorted(expected - set(seen))
    if missing:
        problems.append(f"missing opcode rows: {', '.join(missing)}")

    if problems:
        print("FAIL: section 1.3 opcode map is malformed", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1

    print(f"OK: {rows} rows x {EXPECTED_CELLS} cells = 256 opcode cells")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
