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

Two files come out of it, and both are generated:

    python3 odd/research/extract_capture_oracle.py
        -> odd/research/capture_oracle.txt       the sample, committed

    python3 odd/research/extract_capture_oracle.py --all
        -> odd/research/capture_oracle_full.txt   every capture

The full file is the whole capture set, 646 000 cases, and it is gitignored
beside the sample: `crates/core/tests/hardware_full.rs` is the gate over it
and stands aside when the file is not there, so `cargo test --workspace`
never depends on a file that is not in the repository.

Three things are worth knowing about the format, and all three are read out
of the files rather than assumed:

* The physical address of a transfer is the address on the **T1** cycle. The
  address on T3 is the same address shifted, because the 8086 multiplexes the
  address over the data bus. The value transferred is the **data bus** field
  of the T3 cycle.
* The `ram` list in a test holds the code window and some of the memory, but
  not all of it: a test whose operand lives elsewhere lists only the code.
  The missing bytes are recovered from the cycles, which carry the value the
  chip read. A write is not recovered: the bus carries the value the chip
  wrote, not the value that was there before it, and handing the emulator
  the value it is meant to work out would make the memory comparison say
  nothing.
* The bus has sixteen lanes and a byte sits in the lane its address
  selects: the low lanes for an even address, the high lanes for an odd
  one. A word transfer has both lanes at once, so its low byte is at the
  address and its high byte at the next one.
* `metadata.json` says which flags are undefined after each opcode, and
  gives the mask to clear them. Undefined means the chip left whatever was
  there, so there is nothing to compare.

Run:  python3 odd/research/extract_capture_oracle.py
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

# The capture set stores these opcodes only in their `/digit` forms.  The
# metadata also has per-digit entries for a few flat opcodes, so the file
# layout has to be named rather than inferred from the metadata alone.
GROUP_OPCODES = {
    "80",
    "81",
    "82",
    "83",
    "D0",
    "D1",
    "D2",
    "D3",
    "F6",
    "F7",
    "FE",
    "FF",
}


def is_gzip(path: Path) -> bool:
    """Whether a cached file is a gzip and not an error page."""
    try:
        with path.open("rb") as handle:
            return handle.read(2) == b"\x1f\x8b"
    except OSError:
        return False


def fetch(path: str, cache: Path) -> bytes | None:
    """Download `path` once and cache it, or None when it is not there.

    The test set leaves some opcodes out on purpose: `0F` (`POP CS`), `9B`
    (`WAIT`), `F4` (`HLT`) and the two `LOCK` bytes have no captures, and
    the README says why. A missing file is data, not a failure.
    """
    cache.parent.mkdir(parents=True, exist_ok=True)
    if cache.exists() and not is_gzip(cache):
        # A failed download leaves an error page behind, and the next run
        # would read it as the captures.
        cache.unlink()
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
                # A file that is not there (a 404) and a file the network
                # failed to deliver look the same from here, and the raw
                # host times out on paths that do not exist. Either way the
                # opcode is left out, the list of them is written into the
                # generated file, and the gate pins the number of cases, so
                # a hole in the oracle cannot pass unnoticed.
                print(f"could not fetch {url}: {last}", file=sys.stderr)
                return None
    return cache.read_bytes()


def transfers(test: dict) -> list[tuple[int, int, str]]:
    """The bytes a test moved over the bus: (address, value, bus status).

    The address comes from the T1 cycle and the value from the data bus on
    the T3 cycle, because that is where the chip puts the byte.

    The bus carries sixteen lanes and the byte sits in the lane its address
    selects: the low lanes for an even address, the high lanes for an odd
    one. A word transfer has both lanes at once, so the word's low byte is
    at the address and its high byte at the next one.
    """
    out = []
    cycles = test.get("cycles", [])
    for index, cycle in enumerate(cycles):
        if cycle[7] not in ("MEMR", "MEMW") or cycle[8] != "T1":
            continue
        address = cycle[1]
        value = 0
        # BHE is active low: clear means the high lanes are in use too, and
        # with an even address that is a word.
        bhe = 1
        for later in cycles[index + 1 : index + 4]:
            if later[8] == "T3":
                value = later[6]
                bhe = later[5]
                break
        if bhe == 0 and address % 2 == 0:
            out.append((address, value & 0xFF, cycle[7]))
            out.append((address + 1, (value >> 8) & 0xFF, cycle[7]))
        elif address % 2 == 1:
            out.append((address, (value >> 8) & 0xFF, cycle[7]))
        else:
            out.append((address, value & 0xFF, cycle[7]))
    return out


def memory_state(test: dict) -> dict[int, int]:
    """The bytes of memory the instruction depends on, and their first value.

    The `ram` list gives what the chip started with, and the cycles give
    what it read: a byte the test did not list is recovered from the value
    the bus carried. A byte the chip only wrote is left out, because the
    value on the bus is the one the chip wrote and not the one that was
    there before: the emulator has to work the write out itself.
    """
    memory = {address: value for address, value in test["initial"]["ram"]}
    for address, value, bus in transfers(test):
        if bus == "MEMR":
            memory.setdefault(address, value)
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


def select(tests: list[dict], keep: int, opcode: str) -> list[dict]:
    """The tests worth keeping for one opcode or `/digit` form.

    The captures are randomised, so the first few tests of an opcode rarely
    produce a carry, an overflow or a zero result. One test for each
    direction of each defined flag is what turns the captures into an
    oracle about flag *values* rather than about flag *presence*.

    The two open silicon questions need cases the random sample does not
    guarantee: both displacement forms of `8C`/`8E`, and rotates whose count
    is above one. Those are selected explicitly, so the answer cannot vanish
    because the first few cases happened not to contain it.
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

    if opcode in ("8C", "8E"):
        opcode_byte = int(opcode, 16)
        modes = set()
        for test in tests:
            if is_divide_error(test):
                continue
            at = test["bytes"].index(opcode_byte)
            mode = test["bytes"][at + 1] >> 6
            if mode not in modes:
                modes.add(mode)
                take(test)

    rotate_labels = {
        "D2.0",
        "D2.1",
        "D2.2",
        "D2.3",
        "D3.0",
        "D3.1",
        "D3.2",
        "D3.3",
    }
    if opcode in rotate_labels:
        for test in tests:
            if is_divide_error(test):
                continue
            if test["initial"]["regs"]["cx"] & 0xFF > 1:
                take(test)
                break

        # ROL and ROR expose the chip's carry rule when the count is a whole
        # number of operand widths. Keep one such case for each width.
        if opcode in {"D2.0", "D2.1", "D3.0", "D3.1"}:
            width = 16 if opcode.startswith("D3") else 8
            for test in tests:
                if is_divide_error(test):
                    continue
                count = test["initial"]["regs"]["cx"] & 0xFF
                if count > 1 and count % width == 0:
                    take(test)
                    break

    return chosen


def row_name(test: dict) -> str:
    """The §1.4 row this test belongs to, from the mnemonic the chip's own
    disassembler gave it.

    The captures name their instructions ("add cl, ah", "rol byte
    [ss:bp+di]"), and the first word of that is the instruction the
    document's §1.4 table has a row for. The name is the chip's, so the
    mapping needs no list of this project's own.
    """
    name = test.get("name", "").split()
    return name[0].upper() if name else ""


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
    # The memory the chip left behind: the initial list plus the bytes it
    # wrote. The gate compares every address this line names.
    out.append(
        "    finalmem "
        + " ".join(
            f"{address:05X}={value:02X}"
            for address, value in sorted(final_memory(test, memory).items())
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
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--opcodes", default="all", help="e.g. all, 00-3D, 00,04")
    parser.add_argument("--keep", type=int, default=8, help="tests kept per opcode")
    parser.add_argument(
        "--all",
        action="store_true",
        help="write every capture, not the sample, to odd/research/capture_oracle_full.txt",
    )
    parser.add_argument(
        "--out",
        default=None,
        help="the file to write (default: capture_oracle.txt, or capture_oracle_full.txt with --all)",
    )
    args = parser.parse_args()
    out_path = Path(
        args.out
        if args.out
        else HERE / ("capture_oracle_full.txt" if args.all else "capture_oracle.txt")
    )

    metadata = json.loads(fetch("v1/metadata.json", CACHE / "metadata.json"))
    header = [
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
        "# touched, the memory it left behind, and the flags the chip left. The",
        "# flag mask that goes with an opcode clears the flags the chip leaves",
        "# undefined, so those are not compared.",
        "",
    ]

    # The captures are fetched first: the header names the gaps, so which
    # files are there has to be known before the first case is written.
    captures = []
    no_flat = []
    no_capture = []
    captured = set()
    opcodes = opcode_list(args.opcodes)
    for opcode in opcodes:
        if opcode in GROUP_OPCODES:
            # A group opcode has one file per /digit, and the digit is part
            # of the instruction, so it is part of the oracle's name too.
            names = [f"{opcode}.{d}.json.gz" for d in range(8)]
            no_flat.append(opcode)
        else:
            names = [f"{opcode}.json.gz"]

        for name in names:
            raw = fetch(f"v1/{name}", CACHE / "v1" / name)
            if raw is None:
                if name == f"{opcode}.json.gz":
                    no_flat.append(opcode)
                continue
            captured.add(opcode)
            captures.append((name.removesuffix(".json.gz"), raw))

    no_capture = [opcode for opcode in opcodes if opcode not in captured]
    gap_lines = []
    if no_flat:
        gap_lines.append(
            "# opcodes with no flat capture file "
            "(dotted /digit files may still exist): " + " ".join(no_flat)
        )
    if no_capture:
        gap_lines.append(
            f"# opcodes with no captures in any form: {' '.join(no_capture)}"
        )

    total = 0
    with out_path.open("w", encoding="utf-8") as handle:
        handle.write("\n".join(header[:9]) + "\n")
        if gap_lines:
            handle.write("\n".join(gap_lines) + "\n\n")
        handle.write("\n".join(header[9:]) + "\n")
        for label, raw in captures:
            tests = json.loads(gzip.decompress(raw))
            # The sample keeps a handful of cases per opcode; the full file
            # keeps every capture there is.
            kept = tests if args.all else select(tests, args.keep, label)
            block = [f"op {label}", f"  mask {flag_mask(metadata, label):04X}"]
            block.append(
                "  rows " + " ".join(sorted({row_name(t) for t in kept if row_name(t)}))
            )
            for test in kept:
                memory = memory_state(test)
                emit_case(block, test, memory)
                total += 1
            handle.write("\n".join(block) + "\n\n")

    print(f"wrote {total} cases from {len(captured)} opcodes to {out_path}")
    if no_flat:
        print(f"no flat capture file for: {' '.join(no_flat)}")
    if no_capture:
        print(f"no captures for: {' '.join(no_capture)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
