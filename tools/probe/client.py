#!/usr/bin/env python3
"""The test side of the probe: sends commands to the probe inside the
original and prints the replies.

    python3 tools/probe/client.py ping
    python3 tools/probe/client.py "peek 0x89dcc0 8"

As a module: `Probe().ask("peek 0x89dcc0 8")` returns the reply after "ok",
`Probe().peek(address, length)` the bytes; an "error" reply raises.
"""

import os
import socket
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

    def close(self) -> None:
        self.socket.close()


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    probe = Probe(wait=float(os.environ.get("A3PROBE_WAIT", "0")))
    try:
        for command in sys.argv[1:]:
            print(probe.ask(command), flush=True)
    except ProbeError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
