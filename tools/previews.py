#!/usr/bin/env python3
"""Draws the overview page's preview GIFs: one per area of index.html, plus a
tour of the menu bar.

Every frame is a run of the replica without a window (`--script … --shot …`):
a savegame or a new game, the clicks to one screen, and the screen it ends
on, drawn in the classic look. A frame that does not end on the screen it is
meant for is left out and named, so a GIF never shows a wrong screen.

The GIFs are made of the game's own graphics, so they go to `previews/`,
which git ignores — like the game, they are not part of the repository.

    cargo build --release -p a3-app
    python3 -I tools/previews.py            # all GIFs
    python3 -I tools/previews.py squad      # only these

It needs ImageMagick (`magick`) and gifsicle, and the oracle's game folder
with its kept saves (see tools/oracle/setup.sh).
"""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPLICA = ROOT / "target/release/anstoss3-menu"
ORACLE = Path.home() / ".cache/anstoss3-menu/oracle"
OUT = ROOT / "previews"
LOAD = (ROOT / "crates/a3-screens/scripts/load.txt").read_text()
# The new-game script without its last line, so a frame can stop on any
# dialog on the way: its `on` lines answer everything before.
NEW_GAME = "".join(
    line for line in (ROOT / "crates/a3-screens/scripts/new-game.txt").read_text().splitlines(True)
    if not line.startswith("until")
)
DELAY = 140  # hundredths of a second per frame


def bar(symbol, dialog, *then):
    """A frame on A.an3: menu-bar symbol (1-19, as the page counts), then actions."""
    return ("A.an3", [f"now click bar {symbol - 1}", f"until dialog {dialog}", *then])


def on_a(*lines):
    return ("A.an3", list(lines))


def on_menu(*lines):
    """A frame on Menu.an3: Toni's save with the text menu chosen, which
    the load script then leaves it on instead of the line-up."""
    return ("Menu.an3", list(lines))


def on_office(*lines):
    """A frame on Office.an3: Toni's save with the office chosen."""
    return ("Office.an3", list(lines))


def new_game(dialog):
    return (None, [f"until dialog {dialog}"])


def menu(symbol, dialog, combo, item, target):
    return bar(symbol, dialog, f"now choose {combo} {item}", f"until dialog {target}")


# Area → frames as (save, script lines after loading it, the dialog it must end on).
AREAS = {
    "tour": [
        (*on_a("until dialog 147"), 147),
        *[(*bar(s, d), d) for s, d in [
            (2, 78), (3, 271), (4, 345), (5, 346), (6, 347), (7, 348), (8, 413), (9, 349),
            (10, 417), (11, 218), (12, 464), (13, 404), (14, 439), (15, 18), (17, 175),
            (18, 302)]],
    ],
    "start": [(*new_game(d), d) for d in (9, 140, 209, 212, 374, 99, 103)] + [
        # The fifth entry's high scores, captioned like every window.
        (None, ["now choose 4771 5; ok", "until dialog 95"], 95)],
    "week": [(*new_game(2), 2), (*on_office(), 483), (*on_menu(), 501),
             (*on_menu("now choose 6302 0", "until dialog 147"), 147),
             (*bar(8, 413), 413)],
    "lineup": [(*on_a("until dialog 147"), 147),
               *[(*on_a(f"now choose 5266 {i}", f"until dialog {d}"), d)
                 for i, d in [(0, 429), (1, 244), (3, 418), (6, 151), (7, 83)]],
               (*on_a("now choose 5266 6", "until dialog 151", "now press 4929",
                      "until dialog 394"), 394)],
    "players": [(*on_a(f"now rclick 2510; choose 9950 {i}", f"until dialog {d}"), d)
                for i, d in [(0, 399), (1, 412), (3, 195), (4, 85)]],
    "comparison": [(*bar(2, 78), 78), (*bar(2, 78, "now choose 5618 1"), 78),
                   (*bar(2, 78, "now choose 5618 1", "now click at 93 248"), 78),
                   # "Gegner" shows the opponent's own squad (476).
                   (*bar(2, 78, "now press 5664", "until dialog 476"), 476),
                   # A right click on his keeper shows his info (399).
                   (*bar(2, 78, "now rclick 5631", "until dialog 399"), 399)],
    "tactics": [(*bar(3, 271), 271)],
    "long": [(*bar(4, 345), 345), (*bar(4, 345, "now choose 5035 2"), 345),
             (*bar(4, 345, "now choose 5035 2", "now click at 134 130"), 345),
             # The double click of a week asks which player to watch (281).
             (*bar(4, 345, "now dbl row 4407 15", "until dialog 281"), 281),
             # The strip's last item asks whether the plan may go (384).
             (*bar(4, 345, "now choose 5652 5", "until dialog 384"), 384),
             # Saving the weeks asks first (241), then names the file (506).
             (*bar(4, 345, "now choose 5652 0", "until dialog 241"), 241),
             (*bar(4, 345, "now choose 5652 0", "until dialog 241", "now ok",
                   "until dialog 506"), 506),
             # The co-trainer's corner button opens his settings (391).
             (*bar(4, 345, "now press 4417", "until dialog 391"), 391)],
    "short": [(*bar(5, 346), 346), (*bar(5, 346, "now choose 5035 4"), 346),
              (*bar(5, 346, "now choose 5035 4", "now click 4427", "now row 4427 3"), 346),
              (*bar(5, 346, "now rclick 5035 item 0", "until dialog 350", "now click 35003"), 350)],
    "individual": [(*bar(6, 347), 347), (*bar(6, 347, "now cell 3298 1 8"), 347)],
    "calendar": [(*bar(7, 348), 348), (*bar(7, 348, "now drag at 544 355 to 244 180"), 348)],
    "planner": [(*bar(8, 413), 413), (*bar(8, 413, "now press 6215", "until dialog 358"), 358),
                (*bar(8, 413, "now press 5036", "until dialog 397"), 397),
                (*bar(8, 413, "now press 5036", "until dialog 397",
                      "now type 5414 Abstieg knapp; ok", "until dialog 413",
                      "now press 5036", "until dialog 397"), 397)],
    "balance": [(*bar(9, 349), 349), (*bar(9, 349, "now press 4461", "until dialog 16"), 16),
                (*bar(9, 349, "now press 4462", "until dialog 335"), 335)],
    "finances": [(*bar(10, 417), 417)],
    "squad": [(*bar(11, 218), 218),
              *[(*menu(11, 218, 5042, i, d), d) for i, d in
                [(0, 216), (1, 219), (2, 428), (3, 427), (4, 92), (5, 300), (6, 217)]],
              (*bar(11, 218, "now choose 5042 2", "until dialog 428", "now choose 5271 0",
                    "until dialog 430"), 430),
              *[(*menu(11, 218, 6371, i, d), d) for i, d in
                [(0, 287), (1, 25), (2, 27), (3, 107), (4, 306), (5, 462), (6, 154), (7, 156)]]],
    "stadium": [(*bar(12, 464), 464), (*bar(12, 464, "now click at 311 145", "until dialog 463"), 463),
                (*bar(12, 464, "now press 5861", "until dialog 466"), 466),
                # The page lists contacts, the board and the honours in the stadium's card.
                (*bar(13, 404), 404), (*menu(13, 404, 4975, 0, 35), 35),
                (*menu(13, 404, 4975, 1, 35), 35),
                (*bar(13, 404, "now choose 4975 0", "until dialog 35",
                       "now click 1361", "until dialog 36"), 36),
                (*bar(13, 404, "now choose 4975 0", "until dialog 35", "now row 1359 0",
                       "until dialog 32"), 32),
                (*menu(13, 404, 4975, 7, 493), 493),
                (*bar(14, 439), 439), (*menu(14, 439, 5387, 4, 384), 384),
                (*bar(17, 175), 175),
                *[(*menu(17, 175, 5643, i, d), d)
                  for i, d in [(0, 405), (1, 443), (3, 384), (4, 84), (5, 26), (6, 211),
                               (7, 174), (8, 95)]]],
    # The archive's tables: the current one, at home, the 2. Bundesliga's
    # forecast one step along the strip, a cup's round, the Ligapokal's,
    # the Europa League's.
    "statistics": [(*bar(15, 18), 18), (*bar(15, 18, "now row 1284 1"), 18),
                   (*bar(15, 18, "now click at 545 436", "now row 1284 6"), 18),
                   (*bar(15, 18, "now row 1284 7", "now click at 290 64"), 18),
                   (*bar(15, 18, "now row 1284 8"), 18),
                   (*bar(15, 18, "now row 1284 13"), 18),
                   (*bar(15, 18, "now row 1284 15"), 18),
                   (*bar(15, 18, "now click at 545 436", "now row 1284 15",
                        "now click at 545 436"), 18),
                   (*bar(15, 18, "now row 1284 16"), 18)],
    "options": [(*bar(18, 302), 302), (*menu(18, 302, 4035, 3, 384), 384),
                (*menu(18, 302, 4035, 4, 384), 384), (*menu(18, 302, 4035, 7, 511), 511),
                (*menu(18, 302, 4035, 1, 259), 259), (*menu(18, 302, 4035, 6, 503), 503),
                (*menu(18, 302, 4035, 8, 158), 158)],
}


def game_dir(tmp, save):
    """The oracle's game folder with only `save` among its saves."""
    folder = tmp / (save or "new")
    if not folder.exists():
        (folder / "saves").mkdir(parents=True)
        for entry in (ORACLE / "game").iterdir():
            if entry.name != "saves":
                (folder / entry.name).symlink_to(entry)
        if save:
            shutil.copy(ORACLE / "saves-kept" / save, folder / "saves" / save)
    return folder


def shoot(tmp, save, lines, want, out):
    script = tmp / "frame.txt"
    load = LOAD if save else NEW_GAME
    # saves that chose another start view open there, not on the line-up
    opening = {"Menu.an3": 501, "Office.an3": 483}.get(save)
    if opening:
        load = load.replace("until dialog 147", f"until dialog {opening}")
    script.write_text(load + "\n".join(lines) + "\n")
    run = subprocess.run([REPLICA, game_dir(tmp, save), "--script", script, "--shot", out],
                         capture_output=True, text=True)
    screens = re.findall(r"^screen (\d+)", run.stdout, re.M)
    ended = screens[-1] if screens else "none"
    if run.returncode != 0 or ended != str(want) or not out.exists():
        reason = run.stderr.strip().splitlines()[-1:] or [f"ended on {ended}"]
        print(f"  left out: {' / '.join(lines)} (wanted {want}; {reason[0]})")
        return False
    return True


def main():
    if not REPLICA.exists():
        sys.exit(f"{REPLICA} is missing: cargo build --release -p a3-app")
    wanted = sys.argv[1:] or list(AREAS)
    OUT.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for name in wanted:
            frames = []
            for i, (save, lines, want) in enumerate(AREAS[name]):
                png = tmp / f"{name}-{i:02}.png"
                if shoot(tmp, save, lines, want, png):
                    frames.append(png)
            if not frames:
                print(f"{name}: no frames")
                continue
            gif = OUT / f"{name}.gif"
            subprocess.run(["magick", "-delay", str(DELAY), "-loop", "0", *frames, gif], check=True)
            subprocess.run(["gifsicle", "-O3", "--colors", "256", "--lossy=40", "-b", gif], check=True)
            print(f"{name}: {len(frames)} frames, {gif.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
