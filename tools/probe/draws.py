#!/usr/bin/env python3
"""Reads the probe's trace of random draws (hooks.c, struct draw).

    python3 tools/probe/draws.py TRACE              # counts and the busiest callers
    python3 tools/probe/draws.py TRACE --list       # every draw
    python3 tools/probe/draws.py TRACE OTHER        # where two runs part

A record is 16 bytes: the caller's return address, the range (0 for rand()),
the result, the source and the thread (in the order threads first drew).
"""

import argparse
import struct
import sys
from collections import Counter
from pathlib import Path

RECORD = struct.Struct("<IIiBBH")
SOURCES = ["main", "second", "third", "rand", "rand-activesim"]


def read(path: Path) -> list[tuple]:
    data = path.read_bytes()
    if len(data) % RECORD.size:
        sys.exit(f"{path}: {len(data)} bytes, not whole records")
    return [RECORD.unpack_from(data, i) for i in range(0, len(data), RECORD.size)]


def show(index: int, draw: tuple) -> str:
    caller, argument, result, source, thread, _ = draw
    name = SOURCES[source] if source < len(SOURCES) else str(source)
    return f"{index:9} {name:14} thread {thread} caller {caller:#09x} range {argument:6} -> {result}"


def main() -> int:
    args = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    args.add_argument("trace", type=Path)
    args.add_argument("other", type=Path, nargs="?")
    args.add_argument("--list", action="store_true")
    args = args.parse_args()
    draws = read(args.trace)
    if args.other:
        other = read(args.other)
        for i, (a, b) in enumerate(zip(draws, other)):
            if a != b:
                print(f"the runs part at draw {i}:\n  {show(i, a)}\n  {show(i, b)}")
                return 1
        if len(draws) != len(other):
            print(f"the same {min(len(draws), len(other))} draws, then one run has more "
                  f"({len(draws)} against {len(other)})")
            return 1
        print(f"the same {len(draws)} draws")
        return 0
    if args.list:
        for i, draw in enumerate(draws):
            print(show(i, draw))
        return 0
    print(f"{len(draws)} draws")
    for source, count in sorted(Counter(d[3] for d in draws).items()):
        print(f"  {SOURCES[source]:14} {count}")
    print("busiest callers:")
    for (source, caller), count in Counter((d[3], d[0]) for d in draws).most_common(15):
        print(f"  {caller:#09x} {SOURCES[source]:14} {count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
