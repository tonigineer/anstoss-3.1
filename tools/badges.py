#!/usr/bin/env python3
"""Writes the README's changing badges as shields.io endpoint files in
`.github/badges/`: how many of the original's screens the replica runs, and
how many tests pass.

Most tests need an installed game, which CI does not have, so these badges
are made here and committed with the work:

    ANSTOSS3_DIR="/path/to/ANSTOSS 3" python3 -I tools/badges.py
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".github/badges"
# The dialogs of the game's dialog file, deb/MeineDialoge.rch.
ORIGINAL_DIALOGS = 422


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True)


def colour(percent: float) -> str:
    for limit, name in ((25, "red"), (50, "orange"), (75, "yellow"), (90, "green")):
        if percent < limit:
            return name
    return "brightgreen"


def badge(name: str, label: str, message: str, color: str) -> None:
    body = {"schemaVersion": 1, "label": label, "message": message, "color": color}
    (OUT / f"{name}.json").write_text(json.dumps(body) + "\n")
    print(f"{label}: {message}")


def main() -> int:
    if not os.environ.get("ANSTOSS3_DIR"):
        print("ANSTOSS3_DIR must name the installed game: most tests need it", file=sys.stderr)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    # One test decodes the dumps tools/oracle/check.sh leaves; the oracle
    # scripts' own default for it is this folder.
    os.environ.setdefault("ANSTOSS3_ORACLE", str(Path.home() / ".cache/anstoss-3.1/oracle"))

    screens = run("cargo", "run", "-q", "--bin", "anstoss-3-1", "--", "--screens")
    if screens.returncode:
        print(screens.stderr, file=sys.stderr)
        return 1
    count = len(screens.stdout.split())
    badge("progress", "progress", f"{count}/{ORIGINAL_DIALOGS} screens",
          colour(100 * count / ORIGINAL_DIALOGS))

    tests = run("cargo", "test", "--workspace", "--", "--include-ignored")
    output = tests.stdout + tests.stderr
    # A test that finds no save skips without failing; then the count lies.
    if "among the saves" in output:
        print("some tests found no save and skipped; is an oracle run active?",
              file=sys.stderr)
        return 1
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", output)
    if not results:
        print(output, file=sys.stderr)
        return 1
    passed = sum(int(p) for p, _ in results)
    failed = sum(int(f) for _, f in results)
    if failed or tests.returncode:
        badge("tests", "tests", f"{passed} passed, {failed} failed", "red")
        return 1
    badge("tests", "tests", f"{passed} passed", "brightgreen")
    return 0


if __name__ == "__main__":
    sys.exit(main())
