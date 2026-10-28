#!/bin/bash
# Runs the oracle headless with the probe in it, and a command beside it.
#
#   tools/oracle/run.sh [COMMAND...]
#
# Builds the probe, starts an X server of its own (Xvfb, from
# xorg-server-xvfb), launches the game copy of tools/oracle/setup.sh with the
# probe injected, waits until the probe answers and runs COMMAND in the
# current directory (by default: waits for the game to end). Then it ends the
# game and passes COMMAND's exit code on, or 75 if the game crashed. The
# probe's and Wine's logs go to runs/ of the oracle, with the trace of random
# draws (draws.bin); `tools/probe/client.py "shot FILE"` saves the screen.
#
# Without a window manager the game's window stays where the game puts it, so
# its picture is the same from run to run.
set -euo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss-3.1/oracle"}
runs="$oracle/runs"
export A3PROBE_PORT=${A3PROBE_PORT:-47300}
# WINE picks another Wine, as for setup.sh; its wineserver lies beside it.
wine=${WINE:-wine}
wineserver=$(dirname "$(command -v "$wine")")/wineserver
export WINEPREFIX="$oracle/prefix" WINEDLLOVERRIDES="mscoree=;mshtml=" WINEDEBUG=${WINEDEBUG:--all}
export A3PROBE_LOG="Z:${runs//\//\\}\\probe.log"
# Every random draw, from the start (tools/probe/draws.py reads it); set it
# empty for none.
export A3PROBE_TRACE=${A3PROBE_TRACE-"Z:${runs//\//\\}\\draws.bin"}
# The game seeds rand() with the time; the oracle with this, so that runs
# repeat. Set it empty for the time.
export A3PROBE_SRAND=${A3PROBE_SRAND-1}

[ -f "$WINEPREFIX/system.reg" ] || { echo "run tools/oracle/setup.sh first" >&2; exit 1; }
command -v Xvfb > /dev/null || { echo "Xvfb is missing (xorg-server-xvfb)" >&2; exit 1; }
make -s -C "$here/tools/probe"
mkdir -p "$runs"
rm -f "$runs/probe.log"

screen=${A3ORACLE_SCREEN:-93}
Xvfb ":$screen" -screen 0 1024x768x24 -nolisten tcp > "$runs/xvfb.log" 2>&1 &
xvfb=$!
finish() {
    "$wineserver" -k 2> /dev/null || true
    kill "$xvfb" 2> /dev/null || true
}
trap finish EXIT
for _ in $(seq 50); do [ -S "/tmp/.X11-unix/X$screen" ] && break; sleep 0.1; done
export DISPLAY=":$screen"
unset WAYLAND_DISPLAY

caller=$PWD
cd "$WINEPREFIX/drive_c/GOG Games/ANSTOSS 3"
"$wine" "$here/target/probe/launch.exe" anstoss3.exe > "$runs/wine.log" 2>&1 &
game=$!
cd "$caller"
probe() { python3 "$here/tools/probe/client.py" "$@"; }
A3PROBE_WAIT=30 probe ping > /dev/null || { echo "the probe does not answer" >&2; exit 1; }
status=0
if [ $# -gt 0 ]; then
    "$@" &
    command=$!
    # A game that crashes stops in Wine's debugger, probe and all: end it
    # then, so that COMMAND ends too, and exit with 75 (EX_TEMPFAIL). Now and
    # then the game crashes at the start — its mode picker (FUN_004316a2)
    # reads past the list of display modes when none is 640 wide.
    crashed=""
    while kill -0 "$command" 2> /dev/null; do
        if [ -z "$crashed" ] && grep -q '^wine: Unhandled' "$runs/wine.log"; then
            grep -m1 '^wine: Unhandled' "$runs/wine.log" >&2
            crashed=1
            "$wineserver" -k 2> /dev/null || true
        fi
        sleep 1
    done
    wait "$command" || status=$?
    [ -z "$crashed" ] || status=75
    probe quit > /dev/null 2>&1 || true
else
    wait "$game" || status=$?
fi
exit "$status"
