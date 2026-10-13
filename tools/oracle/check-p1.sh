#!/bin/bash
# The exit gate of P1 (.plan/P1.md), one line per item:
#
#   tools/oracle/check-p1.sh
#
# Needs the runs of tools/oracle/check.sh (its save of day 35, Probe1.an3,
# and the week played from it, save-a). Plays the oracle twice, about five
# minutes: the week from that save written uncompressed by the replica, and
# the save up to its line-up. Everything stays in runs/check of the oracle;
# the saves the oracle's copy had are put back at the end.
set -uo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss3-menu/oracle"}
out="$oracle/runs/check"
saves="$oracle/game/saves"
aside="$oracle/saves-aside"
failed=0

line() {
    printf '%-5s %s\n' "$1" "$2"
    [ "$1" = ok ] || failed=$((failed + 1))
}
an3() { "$here/target/debug/examples/an3" "$@"; }
tests() { (cd "$here" && ANSTOSS3_DIR="$oracle/game" ANSTOSS3_ORACLE="$oracle" cargo test -q "$@" -- --include-ignored > /dev/null 2>&1); }
play() { # SCRIPT DIR; once more if the game crashed (tools/oracle/run.sh)
    local status
    A3PROBE_SRAND=1 "$here/tools/oracle/run.sh" "$here/tools/oracle/play.sh" "$1" "$2" > "$2.log" 2>&1
    status=$?
    if [ $status = 75 ]; then
        echo "(the game crashed; once more)" >> "$2.log"
        A3PROBE_SRAND=1 "$here/tools/oracle/run.sh" "$here/tools/oracle/play.sh" "$1" "$2" >> "$2.log" 2>&1
        status=$?
    fi
    return $status
}
only() { # SAVE: the oracle's saves aside, this one alone in their place
    rm -f "$saves"/*.an3
    cp "$1" "$saves/"
}

[ -f "$out/Probe1.an3" ] && [ -f "$out/save-a/day-042.an3" ] || {
    echo "run tools/oracle/check.sh first" >&2
    exit 1
}
(cd "$here" && cargo build -q -p a3-app && cargo build -q -p a3-files --example an3) || exit 1

# 1. Every save and dump writes back byte for byte, and through the records.
if tests -p a3-files an3 && tests -p a3-core records; then
    line ok "1 round trip: the installation's saves and the oracle's dumps, byte for byte"
else
    line FAIL "1 round trip: cargo test -p a3-files an3, -p a3-core records"
fi

mkdir -p "$aside"
mv -f "$saves"/*.an3 "$aside/" 2> /dev/null

# 2. The original loads a save the replica wrote uncompressed and plays the
#    same week as from the save itself.
an3 --stored "$out/Probe1.an3" "$out/Stored.an3"
only "$out/Stored.an3"
play "$here/tools/oracle/scripts/from-save.txt" "$out/save-stored"
same=0 days=0
for file in "$out"/save-a/day-*.an3; do
    days=$((days + 1))
    other="$out/save-stored/$(basename "$file")"
    [ -f "$other" ] && [ "$(an3 "$file" "$other" | tail -1 | awk '{print $1}')" = 0 ] && same=$((same + 1))
done
if [ "$days" -gt 0 ] && [ "$same" = "$days" ]; then
    line ok "2 the original reads ours: from the save written uncompressed, $same of $days days alike"
else
    line FAIL "2 the original reads ours: $same of $days days alike; see $out/save-stored.log"
fi

# 3. The replica loads every save on the line-up, as the records have it.
if tests -p a3-screens every_save_opens_on_its_line_up; then
    line ok "3 load in the replica: every save opens on its line-up as its records say"
else
    line FAIL "3 load in the replica: cargo test -p a3-screens every_save_opens_on_its_line_up"
fi

# 4. The line-up of the save as the original shows it, and as the replica
#    does; they differ where their fonts do (smoothing, the caption's place).
only "$out/Probe1.an3"
A3PROBE_SRAND=1 "$here/tools/oracle/run.sh" bash -c "'$here/target/debug/anstoss3-menu' --original '$here/tools/oracle/scripts/lineup.txt' && python3 '$here/tools/probe/client.py' 'shot $out/lineup-original.png'" > "$out/lineup-original.log" 2>&1
ANSTOSS3_DIR="$oracle/game" "$here/target/debug/anstoss3-menu" --script "$here/crates/a3-screens/scripts/load.txt" --shot "$out/lineup-replica.png" > "$out/lineup-replica.log" 2>&1
if [ -f "$out/lineup-original.png" ] && [ -f "$out/lineup-replica.png" ]; then
    share=$(magick compare -metric AE "$out/lineup-original.png" "$out/lineup-replica.png" null: 2>&1 | sed 's/.*(\(.*\)).*/\1/')
    if awk "BEGIN {exit !($share < 0.05)}"; then
        line ok "4 like the original: the line-up differs in $(awk "BEGIN {printf \"%.1f\", $share * 100}")% of its pixels (fonts)"
    else
        line FAIL "4 like the original: $(awk "BEGIN {printf \"%.1f\", $share * 100}")% of the pixels differ; compare $out/lineup-*.png"
    fi
else
    line FAIL "4 like the original: no line-up; see $out/lineup-*.log"
fi

rm -f "$saves"/*.an3
mv -f "$aside"/*.an3 "$saves/" 2> /dev/null
echo "$failed of 4 not met"
exit $((failed > 0))
