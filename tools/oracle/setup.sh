#!/bin/bash
# Sets up the oracle: a copy of the installed ANSTOSS 3 with a Wine prefix of
# its own in the cache, so the user's installation and saves stay untouched.
# The registry gets the entries the installer writes, without which the game
# says it is "nicht korrekt installiert"; sound is off, the game runs
# windowed, with a 640×480 client area. tools/oracle/run.sh runs it.
#
#   tools/oracle/setup.sh            # $ANSTOSS3_DIR or the Bottles prefix
#
# WINE picks another Wine, for example a Bottles runner's files/bin/wine.
set -euo pipefail

game=${ANSTOSS3_DIR:-"$HOME/.local/share/bottles/bottles/ANSTOSS-3/drive_c/GOG Games/ANSTOSS 3"}
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss3-menu/oracle"}
wine=${WINE:-wine}
export WINEPREFIX="$oracle/prefix" WINEDLLOVERRIDES="mscoree=;mshtml=" WINEDEBUG=-all

mkdir -p "$oracle/runs"
if [ ! -d "$oracle/game" ]; then
    echo "copying $game"
    cp -a "$game" "$oracle/game"
fi
# No window on the user's screen while the prefix is made.
headless() { env -u DISPLAY -u WAYLAND_DISPLAY "$@"; }
if [ ! -f "$WINEPREFIX/system.reg" ]; then
    echo "making the prefix with $($wine --version)"
    headless "${wine}boot" -i 2>/dev/null || headless "$wine" wineboot -i
fi
# Windowed, also for the graphics engine (its D3D section): full screen,
# DirectDraw stretches the window to the display, before or after the game
# sizes it, and the engine zooms its picture to whichever size won.
sed -i 's/^FullScreen = TRUE/FullScreen = FALSE/; s/^forcefullscreen = TRUE/forcefullscreen = FALSE/' \
    "$oracle/game/gl.cfg"
# The game where the installer put it, as the registry says.
mkdir -p "$WINEPREFIX/drive_c/GOG Games"
ln -sfn "$oracle/game" "$WINEPREFIX/drive_c/GOG Games/ANSTOSS 3"
reg() { headless "$wine" reg add "$1" /v "$2" /t REG_SZ /d "$3" /f > /dev/null; }
reg 'HKLM\Software\Wow6432Node\Ascaron\Anstoss 3\1.00' InstalledTo 'C:\GOG Games\ANSTOSS 3'
reg 'HKCU\Software\Ascaron Entertainment\ANSTOSS 3' InstallDir 'C:\GOG Games\ANSTOSS 3'
reg 'HKCU\Software\Ascaron Entertainment\ANSTOSS 3' Language DEU
reg 'HKCU\Software\Ascaron Entertainment\ANSTOSS 3' Version 1_4_0
# The caption of Windows 9x: the game sizes its window for a 640×480 client
# area under a 19-pixel caption (18 and its border line, in twips).
reg 'HKCU\Control Panel\Desktop\WindowMetrics' CaptionHeight -270
# No sound: the oracle needs none.
reg 'HKCU\Software\Wine\Drivers' Audio ''
headless "${wine}server" -w 2>/dev/null || true
echo "oracle ready in $oracle"
