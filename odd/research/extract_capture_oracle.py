#!/usr/bin/env python3
"""Turn the SingleStepTests 8086 captures into a small oracle the suite can run.

The captures are a hardware test suite: Daniel Balsom's `SingleStepTests/8086`,
made on an Intel P80C86A-2 with an 8288 bus controller. Each file holds 2,000
tests for one opcode, and each test is a complete machine state, the
instruction's bytes, and the state the chip left behind.

They are the only source this project has that says what a flag *becomes*.
The 1979 manual says which flags an instruction may touch, and the SDM
repeats it; neither says what the flag is afterwards, because the answer
depends on the operands. That is the hole the `X` and `U` marks in
`documentation-guide.md` §1.4 leave open, and it is where two of the eight
defects from the 2026-10-15 session lived.

The captures are 160 MB of gzipped JSON, which is too much to vendor and too
slow to fetch from a test. So this script takes them apart and writes a
small text file: a handful of tests per opcode, with the machine state the
instruction needs, the bytes it read and wrote, and the flags the chip left.

Three things are worth knowing about the format, and all three are read out
of the files rather than assumed:

* The physical address of a transfer is the address on the **T1** cycle. The
  address on T3 is the same address shifted, because the 8086 multiplexes the
  address over the data bus. The value transferred is the **data bus** field
  of the T3 cycle.
* The `ram` list in a test holds the code window and some of the memory, but
  not all of it: a test whose operand lives elsewhere lists only the code.
  The missing bytes are recovered from the cycles, which carry the value the
  chip read.
* `metadata.json` says which flags are undefined after each opcode, and
  gives the mask to clear them. Undefined means the chip left whatever was
  there, so there is nothing to compare.

Run:  python3 odd/research/extract_capture_oracle.py --opcodes 00-3D
"""

import argparse
import gzip
import json
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
GUIDE = HERE / "documentation-guide.md"
CACHE = HERE / ".captures"
BASE = "https://raw.githubusercontent.com/SingleStepTests/8086/main"
# The commit the cases were taken from. The captures are a moving target, and
# a case that changed under the oracle has to be traceable to a revision.
SOURCE_COMMIT = "e71c68d215a6bb8c356bd4cb3842de3bef345ca9"

# The flags the chip's FLAGS word carries, and the ones the emulator models.
# Bits 12..=15 are undefined on an 8086 and read back as one through POPF, so
# they are masked out of every comparison.
FLAG_BITS = {
    "cf": 0x0001,
    "pf": 0x0004,
    "af": 0x0010,
    "zf": 0x0040,
    "sf": 0x0080,
    "tf": 0x0100,
    "if": 0x0200,
    "df": 0x0400,
    "of": 0x0800,
}
# The order `metadata.json` uses: o d i s z a p c.
METADATA_FLAG_ORDER = ["of", "df", "if", "sf", "zf", "af", "pf", "cf"]
# What the emulator's FLAGS word can hold: the nine flags plus the fixed one.
EMULATOR_MASK = 0x0FD5

REGISTERS = [
    "ax",
    "cx",
    "dx",
    "bx",
    "sp",
    "bp",
    "si",
    "di",
    "cs",
    "ds",
    "es",
    "ss",
    "ip",
]


def fetch(path: str, cache: Path) -> bytes | None:
    """Download `path` once and cache it, or None when it is not there.

    The test set leaves some opcodes out on purpose: `0F` (`POP CS`), `9B`
    (`WAIT`), `F4` (`HLT`) and the two `LOCK` bytes have no captures, and
    the README says why. A missing file is data, not a failure.
    """
    cache.parent.mkdir(parents=True, exist_ok=True)
    if not cache.exists():
        url = f"{BASE}/{path}"
        try:
            with urllib.request.urlopen(url, timeout=120) as response:
                cache.write_bytes(response.read())
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise SystemExit(f"could not fetch {url}: {error}") from error
        except Exception as error:  # noqa: BLE001 - the message is the report
            # A network hiccup is not a finding about the captures, so it is
            # retried and then reported as a file that is not there.
            last = error
            for _ in range(3):
                try:
                    with urllib.request.urlopen(url, timeout=120) as response:
                        cache.write_bytes(response.read())
                    last = None
                    break
                except Exception:  # noqa: BLE001, PERF203 - retried below
                    continue
            if last is not None:
                print(f"could not fetch {url}: {last}", file=sys.stderr)
                return None
    return cache.read_bytes()


def transfers(test: dict) -> list[tuple[int, int, int]]:
    """The memory transfers a test made: (address, bytes, value).

    The address comes from the T1 cycle and the value from the data bus on
    the T3 cycle, because that is where the chip puts the byte.
    """
    out = []
    cycles = test.get("cycles", [])
    for index, cycle in enumerate(cycles):
        if cycle[7] not in ("MEMR", "MEMW") or cycle[8] != "T1":
            continue
        address = cycle[1]
        # The data bus carries the byte on the T3 cycle. A word transfer
        # puts two bytes there, low byte first.
        value = 0
        width = 1
        for later in cycles[index : index + 4]:
            if later[8] == "T3" and later[6]:
                value = later[6]
                width = 2 if value > 0xFF else 1
                break
        out.append((address, width, value))
    return out


def memory_state(test: dict) -> dict[int, int]:
    """The bytes of memory the instruction depends on, and their first value.

    The `ram` list gives what the chip started with, and the cycles give
    what it read: a byte the test did not list is recovered from the value
    the bus carried.
    """
    memory = {address: value for address, value in test["initial"]["ram"]}
    for address, width, value in transfers(test):
        for offset in range(width):
            byte = (value >> (8 * offset)) & 0xFF
            memory.setdefault((address + offset) & 0xFFFFF, byte)
    return memory


def final_memory(test: dict, memory: dict[int, int]) -> dict[int, int]:
    """The memory after the instruction: the `ram` list of the final state."""
    final = dict(memory)
    for address, value in test["final"]["ram"]:
        final[address] = value
    return final


def flag_mask(metadata: dict, opcode: str) -> int:
    """The mask that clears the flags the chip leaves undefined.

    A group opcode's flags live under its `reg` entries, one per /digit,
    so `F6.4` reads the entry for digit four of `F6` and `F6` itself
    reads the top-level one.
    """
    key = opcode.upper()
    entry = metadata["opcodes"].get(key, {})
    if "." in key:
        base, digit = key.split(".")
        entry = metadata["opcodes"].get(base, {}).get("reg", {}).get(digit, {})
    mask = entry.get("flags-mask", 0xFFFF)
    return mask & EMULATOR_MASK


def is_divide_error(test: dict) -> bool:
    """Whether the chip took the type 0 interrupt instead of finishing.

    The divides that overflow or divide by zero push the flags and vector
    through the interrupt table, and the test set points that vector at
    `0400h` so the run can finish. The state the chip left is then the
    handler's, not the instruction's, and there is nothing to compare: the
    flags on the stack are undefined and the README says so.
    """
    return test["final"]["regs"].get("ip") == 0x0400


def select(tests: list[dict], keep: int) -> list[dict]:
    """The tests worth keeping: the first few, plus the ones that exercise a
    flag's value in both directions.

    The captures are randomised, so the first few tests of an opcode rarely
    produce a carry, an overflow or a zero result. One test for each
    direction of each defined flag is what turns the captures into an
    oracle about flag *values* rather than about flag *presence*.
    """
    chosen = []
    seen = set()

    def take(test: dict) -> None:
        key = test["test_hash"]
        if key not in seen:
            seen.add(key)
            chosen.append(test)

    for test in tests[:keep]:
        if not is_divide_error(test):
            take(test)

    for bit in FLAG_BITS.values():
        for wanted in (True, False):
            for test in tests:
                if is_divide_error(test):
                    continue
                flags = test["final"]["regs"].get("flags")
                if flags is None:
                    continue
                if bool(flags & bit) is wanted:
                    take(test)
                    break
    return chosen


def emit_case(out: list[str], test: dict, memory: dict[int, int]) -> None:
    """One test, in the format the Rust gate reads."""
    initial = test["initial"]["regs"]
    out.append("  case")
    out.append("    bytes " + " ".join(f"{byte:02X}" for byte in test["bytes"]))
    out.append(
        "    regs " + " ".join(f"{name}={initial[name]:04X}" for name in REGISTERS)
    )
    out.append(f"    flags {initial['flags']:04X}")
    out.append(
        "    mem "
        + " ".join(
            f"{address:05X}={value:02X}" for address, value in sorted(memory.items())
        )
    )
    final = test["final"]["regs"]
    changed = [name for name in REGISTERS if name in final]
    # The final state lists only what changed, so a missing `flags` means the
    # chip left the flags exactly as they were.
    final_flags = final.get("flags", initial["flags"])
    out.append(
        "    final flags=%04X %s"
        % (final_flags, " ".join(f"{name}={final[name]:04X}" for name in changed))
    )


def opcode_list(spec: str) -> list[str]:
    """`00-3D` or `00,04,91` or `all`."""
    if spec == "all":
        return [f"{n:02X}" for n in range(256)]
    out = []
    for part in spec.split(","):
        if "-" in part:
            low, high = part.split("-")
            out.extend(f"{n:02X}" for n in range(int(low, 16), int(high, 16) + 1))
        else:
            out.append(part.strip().upper())
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opcodes", default="00-3D", help="e.g. 00-3D, 00,04")
    parser.add_argument("--keep", type=int, default=8, help="tests kept per opcode")
    parser.add_argument("--out", default=str(HERE / "capture_oracle.txt"))
    args = parser.parse_args()

    metadata = json.loads(fetch("v1/metadata.json", CACHE / "metadata.json"))
    out = [
        "# The 8086 hardware flag oracle, generated from SingleStepTests/8086.",
        "#",
        "# Generated by odd/research/extract_capture_oracle.py. Do not edit by hand:",
        "# re-run the script instead. The provenance is below.",
        "#",
        f"# source: {metadata['url']} v1 (version {metadata['version']})",
        f"# source-commit: {SOURCE_COMMIT}",
        f"# cpu: {metadata['cpu_detail']}",
        "#",
        "# Each case is one instruction: the machine before it, the bytes it",
        "# touched, and the flags the chip left. The flag mask that goes with an",
        "# opcode clears the flags the chip leaves undefined, so those are not",
        "# compared.",
        "",
    ]
    total = 0
    skipped = []
    written = 0
    for opcode in opcode_list(args.opcodes):
        # A group opcode has one file per /digit, and the digit is part of
        # the instruction, so it is part of the oracle's name too.
        names = [f"{opcode}.json.gz"] + [f"{opcode}.{d}.json.gz" for d in range(8)]
        found = False
        for name in names:
            raw = fetch(f"v1/{name}", CACHE / "v1" / name)
            if raw is None:
                if name == f"{opcode}.json.gz":
                    skipped.append(opcode)
                continue
            found = True
            label = name.removesuffix(".json.gz")
            tests = json.loads(gzip.decompress(raw))
            kept = select(tests, args.keep)
            out.append(f"op {label}")
            out.append(f"  mask {flag_mask(metadata, label):04X}")
            for test in kept:
                memory = memory_state(test)
                emit_case(out, test, memory)
                total += 1
            out.append("")
            written += 1
    del found

    if skipped:
        out.insert(9, f"# opcodes with no captures: {' '.join(skipped)}")
        out.insert(10, "")
    Path(args.out).write_text("\n".join(out) + "\n", encoding="utf-8")
    print(
        f"wrote {total} cases from {len(opcode_list(args.opcodes)) - len(skipped)} "
        f"opcodes to {args.out}"
    )
    if skipped:
        print(f"no captures for: {' '.join(skipped)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
