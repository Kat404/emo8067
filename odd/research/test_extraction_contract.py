"""The contract the extraction holds to, pinned as a test.

Two rules of the recovery are easy to break and hard to see: a byte on the
bus sits in the lane its address selects, and a byte the chip only wrote
carries the value the chip wrote and not the value that was there before.
The captures show both over all 646 000 cases; this test pins them where a
change to the script has to break it.

Run:  python3 odd/research/test_extraction_contract.py
"""

import sys
import unittest
from pathlib import Path

# The script imports nothing but the standard library, and this test has to
# find it next to itself: the repository root is the working directory, and
# the script's own directory is not on the path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True

from extract_capture_oracle import final_memory, memory_state  # noqa: E402


class ExtractionContract(unittest.TestCase):
    def test_odd_read_and_write_only_byte(self):
        # A read at an odd address: the byte is in the upper half of the
        # bus, so the recovery has to take it from there and not from the
        # low half. The write at an odd address carries the byte the chip
        # wrote, and a byte that was only written is not part of the
        # initial memory: the emulator has to work the write out itself.
        test = {
            "initial": {"ram": []},
            "final": {"ram": [[1, 0xAB], [2, 0xCD]]},
            "cycles": [
                [1, 1, "--", "---", "---", 1, 0, "MEMR", "T1", "-", 0],
                [0, 0, "--", "---", "---", 1, 0xAB00, "PASV", "T3", "-", 0],
                [1, 2, "--", "---", "---", 1, 0, "MEMW", "T1", "-", 0],
                [0, 0, "--", "---", "---", 1, 0xCD00, "PASV", "T3", "-", 0],
            ],
        }
        memory = memory_state(test)
        self.assertEqual(memory, {1: 0xAB})
        self.assertEqual(final_memory(test, memory), {1: 0xAB, 2: 0xCD})


if __name__ == "__main__":
    unittest.main()
