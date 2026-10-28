#!/bin/bash
# The exit gate of P0 (.plan/P0.md), end to end, one line per item:
#
#   tools/oracle/check.sh
#
# Plays the oracle four times, about 20 minutes: the new game of
# crates/a3-screens/scripts/new-game.txt to day 42, then from its autosave of
# day 35 seven days (tools/oracle/scripts/from-save.txt) three times — twice
# alike, once with another seed. Everything stays in runs/check of the
# oracle; the saves the oracle's copy had are put back at the end.
set -uo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss-3.1/oracle"}
re=${ANSTOSS3_RE:-"$HOME/Programming/anstoss3-re"}
out="$oracle/runs/check"
saves="$oracle/game/saves"
aside="$oracle/saves-aside"
failed=0

line() {
    printf '%-5s %s\n' "$1" "$2"
    [ "$1" = ok ] || failed=$((failed + 1))
}
an3() { "$here/target/debug/examples/an3" "$@"; }
play() { # SCRIPT DIR, with A3PROBE_SRAND and A3_PLAY_* from the caller
    local status
    "$here/tools/oracle/run.sh" "$here/tools/oracle/play.sh" "$1" "$2" > "$2.log" 2>&1
    status=$?
    # Once more if the game crashed (tools/oracle/run.sh).
    if [ $status = 75 ]; then
        echo "(the game crashed; once more)" >> "$2.log"
        "$here/tools/oracle/run.sh" "$here/tools/oracle/play.sh" "$1" "$2" >> "$2.log" 2>&1
        status=$?
    fi
    return $status
}
# Whether every dump in DIR decodes; prints how many.
decodes() {
    local n=0 file
    for file in "$1"/*.an3; do
        an3 "$file" > /dev/null 2>&1 || return 1
        n=$((n + 1))
    done
    echo "$n"
}
# How many blocks of state two saves differ in: not those holding real time
# or what the writer left on the stack.
differ() {
    an3 "$1" "$2" | tail -1 | awk '{print $1}'
}
fresh_saves() {
    rm -f "$saves"/*.an3
    [ -z "${1:-}" ] || cp "$1" "$saves/"
}

(cd "$here" && cargo build -q -p a3-app && cargo build -q -p a3-files --example an3) || exit 1
mkdir -p "$out" "$aside"
mv -f "$saves"/*.an3 "$aside/" 2> /dev/null

# 3. A scripted new game reaches the first day of the season, dumps included.
fresh_saves
A3PROBE_SRAND=1 play "$here/crates/a3-screens/scripts/new-game.txt" "$out/new-game"
if n=$(decodes "$out/new-game") && [ -f "$out/new-game/day-042.an3" ] && [ -f "$out/new-game/final.an3" ]; then
    line ok "3 new game: Germany, two managers, to day 42; $n dumps decode"
else
    line FAIL "3 new game: see $out/new-game.log"
fi
cp "$saves/Autos.an3" "$out/Probe1.an3" 2> /dev/null

# 1. From a fixed save, seven days unattended, a dump after each day.
for run in a b; do
    fresh_saves "$out/Probe1.an3"
    shot=""
    [ $run = a ] && shot="$out/lineup.png"
    A3PROBE_SRAND=1 A3_PLAY_SHOT=$shot play "$here/tools/oracle/scripts/from-save.txt" "$out/save-$run"
done
if n=$(decodes "$out/save-a") && [ -f "$out/save-a/day-042.an3" ] && [ "$n" -ge 9 ]; then
    line ok "1 unattended: from the save of day 35 to day 42 headless; $n dumps decode"
else
    line FAIL "1 unattended: see $out/save-a.log"
fi

# 2. The same seeds give identical dumps, other seeds do not.
fresh_saves "$out/Probe1.an3"
A3PROBE_SRAND=2 A3_PLAY_SEED=12345 play "$here/tools/oracle/scripts/from-save.txt" "$out/save-other"
same=0 days=0
for file in "$out"/save-a/day-*.an3; do
    days=$((days + 1))
    [ "$(differ "$file" "$out/save-b/$(basename "$file")" 2> /dev/null)" = 0 ] && same=$((same + 1))
done
other=$(differ "$out/save-a/day-042.an3" "$out/save-other/day-042.an3" 2> /dev/null || echo 0)
if [ "$days" -gt 0 ] && [ "$same" = "$days" ] && [ "${other:-0}" -gt 0 ]; then
    line ok "2 deterministic: $same of $days days alike in every block of state; another seed: $other blocks differ"
else
    line FAIL "2 deterministic: $same of $days days alike; another seed: ${other:-?} blocks differ"
fi

# 4. The probe captures the line-up of the loaded save as 640×480.
if [ -f "$out/lineup.png" ]; then
    lit=$(magick "$out/lineup.png" -threshold 2% -format '%[fx:mean]' info: 2> /dev/null)
    size=$(magick identify -format '%wx%h' "$out/lineup.png" 2> /dev/null)
    if awk "BEGIN {exit !($lit > 0.5)}"; then
        line ok "4 screens: the line-up as $size"
    else
        line OPEN "4 screens: the line-up as $size, but only $(awk "BEGIN {printf \"%d\", $lit * 100}")% drawn (Wine shows only what the game redraws)"
    fi
else
    line FAIL "4 screens: no shot of the line-up"
fi

# 5. The start year is understood; the replica starts where the oracle does.
if (cd "$here" && ANSTOSS3_DIR="$oracle/game" cargo test -q -p a3-core calendar -- --include-ignored > /dev/null 2>&1); then
    line ok "5 start year: Kleinig.sav, MISC after UEFA (2025); day 0 is 7.7.2025 in both"
else
    line FAIL "5 start year: cargo test -p a3-core calendar"
fi

# 6. The field report covers players, clubs and managers; the coverage table
#    lists all 422 dialogs.
if [ -f "$re/fields/players.md" ] && [ -f "$re/fields/clubs.md" ] && [ -f "$re/fields/managers.md" ] &&
    grep -q "^422 dialogs" "$here/.plan/COVERAGE.md" 2> /dev/null; then
    line ok "6 tools: field report of players, clubs and managers; coverage of 422 dialogs"
else
    line FAIL "6 tools: tools/scripts/fields.py and coverage.py"
fi

mv -f "$aside"/*.an3 "$saves/" 2> /dev/null
echo "$failed of 6 not met"
exit $((failed > 0))
