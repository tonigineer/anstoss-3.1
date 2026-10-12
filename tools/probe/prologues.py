#!/usr/bin/env python3
"""Checks the inline hooks of hooks.c against the game's code: each routine
starts with the bytes the hook moves, those are whole instructions without
relative jumps or calls, and no jump in the routine leads into them — unless
the hook says "Its loop jumps back in" and its detour copes.

    python3 tools/probe/prologues.py [--exe anstoss3.exe] [--re ~/Programming/anstoss3-re]

Routine sizes come from the RE repo's functions.tsv; the disassembly from
mingw-w64's objdump.
"""

import argparse
import csv
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ORACLE = Path(os.environ.get("ANSTOSS3_ORACLE") or Path.home() / ".cache/anstoss3-menu/oracle")
HOOK = re.compile(r'(/\* Its loop jumps back in[^*]*\*/\s*)?HOOK\("([^"]+)",\s*(0x[0-9a-f]+),\s*"([^"]*)"')
OBJDUMP = "i686-w64-mingw32-objdump"


def main() -> int:
    args = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    args.add_argument("--exe", type=Path, default=ORACLE / "game/anstoss3.exe")
    args.add_argument("--re", type=Path, default=Path.home() / "Programming/anstoss3-re")
    args = args.parse_args()
    sizes = {int(row["address"], 16): int(row["bytes"])
             for row in csv.DictReader(open(args.re / "export/anstoss3.exe/functions.tsv"), delimiter="\t")}
    failed = 0
    for loops_back, name, address, prologue in HOOK.findall((HERE / "hooks.c").read_text()):
        start = int(address, 16)
        moved = bytes(int(b, 16) for b in re.findall(r"\\x([0-9a-f]{2})", prologue))
        end = start + sizes.get(start, 4096)
        lines = subprocess.run([OBJDUMP, "-d", f"--start-address={start}", f"--stop-address={end}",
                                str(args.exe)], capture_output=True, text=True, check=True).stdout
        code = [(int(a, 16), bytes.fromhex(b.replace(" ", "")), ins.strip())
                for a, b, ins in re.findall(r"^\s*([0-9a-f]+):\t([0-9a-f ]+?)\s*\t(.*)$", lines, re.M)]
        problems = []
        head = b"".join(c for a, c, _ in code if a < start + len(moved))
        if not head.startswith(moved):
            problems.append(f"starts {head[:len(moved)].hex(' ')}, expected {moved.hex(' ')}")
        if start + len(moved) not in {a for a, _, _ in code}:
            problems.append(f"{len(moved)} bytes end inside an instruction")
        for a, _, ins in code:
            if a < start + len(moved) and re.match(r"(j\w+|call|loop\w*)\s", ins):
                problems.append(f"moves a relative {ins.split()[0]} at {a:#x}")
            target = re.match(r"(j\w+|loop\w*)\s+0x([0-9a-f]+)", ins)
            if target and start < int(target[2], 16) < start + len(moved) and not loops_back:
                problems.append(f"{a:#x} jumps into the moved bytes: {ins}")
        print(f"{name:16} {start:#x} {'ok' if not problems else '; '.join(problems)}")
        failed += bool(problems)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
