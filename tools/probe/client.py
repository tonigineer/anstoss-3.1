#!/usr/bin/env python3
"""The test side of the probe: sends commands to the probe inside the
original and prints the replies.

    python3 tools/probe/client.py ping
    python3 tools/probe/client.py "peek 0x89dcc0 8"
    python3 tools/probe/client.py "shot runs/start.png"

"shot PATH" is the client's own: it saves the game's 640×480 screen from the
X display (ImageMagick's import), at the client area of the game's window.

As a module: `Probe().ask("peek 0x89dcc0 8")` returns the reply after "ok",
`Probe().peek(address, length)` the bytes; an "error" reply raises.
"""

import os
import re
import socket
import subprocess
import sys
import time

PORT = int(os.environ.get("A3PROBE_PORT", "47300"))


class ProbeError(Exception):
    pass


class Probe:
    def __init__(self, port: int = PORT, wait: float = 0.0):
        """Connects to the probe, retrying for `wait` seconds while the game starts."""
        deadline = time.monotonic() + wait
        while True:
            try:
                self.socket = socket.create_connection(("127.0.0.1", port), timeout=30)
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.5)
        self.lines = self.socket.makefile("r", encoding="ascii", newline="\n")

    def ask(self, command: str) -> str:
        self.socket.sendall(command.encode("ascii") + b"\n")
        reply = self.lines.readline().rstrip("\n")
        if not reply:
            raise ProbeError(f"{command}: no reply")
        if reply.startswith("error"):
            raise ProbeError(f"{command}: {reply[6:]}")
        return reply[3:] if reply.startswith("ok ") else ""

    def peek(self, address: int, length: int) -> bytes:
        return bytes.fromhex(self.ask(f"peek {address:#x} {length}"))

    def poke(self, address: int, data: bytes) -> None:
        self.ask(f"poke {address:#x} {data.hex()}")

    def windows(self) -> list[dict]:
        """The game's visible top-level windows, topmost first."""
        found = []
        for entry in filter(None, self.ask("windows").split(";")):
            m = re.match(r'\s*(\S+) (\S+) (-?\d+) (-?\d+) (\d+) (\d+) client (-?\d+) (-?\d+) '
                         r'(\d+) (\d+) style (\S+) "(.*)"$', entry)
            if not m:
                raise ProbeError(f"windows: cannot read {entry!r}")
            n = [int(v) for v in m.groups()[2:10]]
            found.append({"handle": int(m[1], 16), "class": m[2], "frame": n[:4],
                          "client": n[4:], "style": int(m[11], 16), "title": m[12]})
        return found

    def shot(self, path: str) -> None:
        """Saves the game's screen: 640×480 at the client area of its main window
        (the bottommost one), from the X display the game runs on."""
        main = self.windows()[-1]
        x, y = main["client"][:2]
        subprocess.run(["magick", "import", "-display", os.environ["DISPLAY"], "-window", "root",
                        "-crop", f"640x480+{x}+{y}", "+repage", path], check=True)

    def close(self) -> None:
        self.socket.close()


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    probe = Probe(wait=float(os.environ.get("A3PROBE_WAIT", "0")))
    try:
        for command in sys.argv[1:]:
            if command.startswith("shot "):
                probe.shot(command[5:])
                print(command[5:], flush=True)
            else:
                print(probe.ask(command), flush=True)
    except ProbeError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
