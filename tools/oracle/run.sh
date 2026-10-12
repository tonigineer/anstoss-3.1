#!/bin/bash
# Runs the oracle headless with the probe in it, and a command beside it.
#
#   tools/oracle/run.sh [COMMAND...]
#
# Builds the probe, starts a display of its own, launches the game copy of
# tools/oracle/setup.sh with the probe injected, waits until the probe answers
# and runs COMMAND (by default: waits for the game to end). Then it ends the
# game and passes COMMAND's exit code on. The probe's and Wine's logs go to
# runs/ of the oracle; the probe's `capture` saves the game's screen.
#
# The display is Xvfb when it is installed (xorg-server-xvfb): without a
# window manager the game's window stays where the game puts it. Otherwise,
# or with A3ORACLE_DISPLAY=gamescope, a headless gamescope, which moves the
# window about from run to run; `gamescopectl screenshot FILE` works in it.
set -euo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
self="$here/tools/oracle/run.sh"
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss3-menu/oracle"}
export A3PROBE_PORT=${A3PROBE_PORT:-47300}

if [ -z "${A3ORACLE_INSIDE:-}" ]; then
    [ -f "$oracle/prefix/system.reg" ] || { echo "run tools/oracle/setup.sh first" >&2; exit 1; }
    make -s -C "$here/tools/probe"
    rm -f "$oracle/runs/status"
    display=${A3ORACLE_DISPLAY:-$(command -v Xvfb > /dev/null && echo xvfb || echo gamescope)}
    if [ "$display" = xvfb ]; then
        screen=${A3ORACLE_SCREEN:-93}
        Xvfb ":$screen" -screen 0 1024x768x24 -nolisten tcp > "$oracle/runs/xvfb.log" 2>&1 &
        xvfb=$!
        trap 'kill $xvfb 2>/dev/null' EXIT
        for _ in $(seq 50); do [ -S "/tmp/.X11-unix/X$screen" ] && break; sleep 0.1; done
        env -u WAYLAND_DISPLAY DISPLAY=":$screen" A3ORACLE_INSIDE="$PWD" "$self" "$@" || true
    else
        A3ORACLE_INSIDE="$PWD" gamescope --backend headless -W 1280 -H 960 -- "$self" "$@" \
            > "$oracle/runs/gamescope.log" 2>&1 || true
    fi
    cat "$oracle/runs/output.log"
    exit "$(cat "$oracle/runs/status" 2> /dev/null || echo 1)"
fi

exec > "$oracle/runs/output.log" 2>&1
export WINEPREFIX="$oracle/prefix" WINEDLLOVERRIDES="mscoree=;mshtml=" WINEDEBUG=-all
export A3PROBE_LOG="Z:${oracle//\//\\}\\runs\\probe.log"
rm -f "$oracle/runs/probe.log"
cd "$WINEPREFIX/drive_c/GOG Games/ANSTOSS 3"
wine "$here/target/probe/launch.exe" anstoss3.exe > "$oracle/runs/wine.log" 2>&1 &
game=$!
finish() { echo "${1:-1}" > "$oracle/runs/status"; wineserver -k 2>/dev/null || true; }
trap finish EXIT
probe() { python3 "$here/tools/probe/client.py" "$@"; }
A3PROBE_WAIT=30 probe ping > /dev/null || { echo "the probe does not answer" >&2; exit 1; }
status=0
if [ $# -gt 0 ]; then
    (cd "$A3ORACLE_INSIDE" && "$@") || status=$?
    probe quit > /dev/null 2>&1 || true
else
    wait "$game" || status=$?
fi
trap - EXIT
finish "$status"
