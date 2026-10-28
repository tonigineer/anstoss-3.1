#!/bin/bash
# Plays a script on the oracle with a dump every day into DIR (day-NNN.an3)
# and one at the end (final.an3); runs inside tools/oracle/run.sh:
#
#   tools/oracle/run.sh tools/oracle/play.sh SCRIPT DIR
#
# Once the first turn (the line-up, dialog 147) waits, A3_PLAY_SEED sets the
# first word of the main random generator — loading a save reseeds it from
# the logic thread's own rand(), which no srand() seed reaches — and
# A3_PLAY_SHOT saves the screen to that file.
set -euo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
script=$1 dir=$2
seed=${A3_PLAY_SEED:-} shot=${A3_PLAY_SHOT:-}
probe() { python3 "$here/tools/probe/client.py" "$@"; }
driver() { "$here/target/debug/anstoss-3-1" --original "$@"; }
windows() { local path; path=$(realpath "$1"); echo "Z:${path//\//\\}"; }

mkdir -p "$dir"
rm -f "$dir"/day-*.an3 "$dir"/final.an3
probe "dumps $(windows "$dir")" > /dev/null
if [ -n "$seed$shot" ]; then
    first=$(mktemp)
    { grep -v '^until' "$script"; echo "until dialog 147"; } > "$first"
    driver "$first"
    rm -f "$first"
    if [ -n "$seed" ]; then
        bytes=$(printf '%08x' "$seed" | sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')
        probe "poke 0x89dcc0 $bytes" > /dev/null
    fi
    if [ -n "$shot" ]; then
        probe "shot $shot" > /dev/null
    fi
fi
driver "$script"
probe "dumps off" "dump $(windows "$dir")\\final.an3" > /dev/null
