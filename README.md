# Anstoss 3.1

> **Work in progress.** Anstoss 3.1 is not playable from start to finish yet:
> many screens and rules of the original are still being rebuilt.
> `index.html` in this repository lists what works, screen by screen.

An unofficial, fan-made reimplementation of **ANSTOSS 3 – Der Fußballmanager**
(Ascaron, 2000), written in Rust. It runs natively on Linux and Windows and
aims to match the original screen for screen: every finished screen is
compared pixel by pixel with the original game running under Wine.

For now it rebuilds the classic 640×480 game. A remastered 16:9 look will
follow once the rebuild is complete; F9 shows first drafts of it on a few
screens.

## You need the original game

> **Disclaimer.** Anstoss 3.1 is not affiliated with or endorsed by Ascaron or
> the current rights holders of ANSTOSS. "ANSTOSS" is a trademark of its
> owner. This project contains **no files of the game**: no graphics, texts,
> sounds, videos or football data. It reads them at runtime from a copy of
> ANSTOSS 3 that **you own and have installed**. ANSTOSS 3 is still sold, for
> example DRM-free on [GOG](https://www.gog.com/game/anstoss_3) (German only).
> Please don't share or ask for the game's files.

### Which copy works

- **The GOG version** is the one Anstoss 3.1 is developed and tested against.
  GOG ships the final patched executable (patch notes `A3V13.txt` up to 1.30,
  file version 1.1.0.0).
- **The executable must be that final build.** Some of the game's rules are
  tables inside `anstoss3.exe`, which Anstoss 3.1 reads at the addresses of
  that build. An unpatched `anstoss3.exe` from the CD would be read wrongly.
  The program does not check the build yet.
- **An ISO or the CD alone is not enough.** Anstoss 3.1 needs the installed
  game folder, as the installer lays it out, and writes into it (see below).
  A CD install brought up to the final patch may work, but is untested.
- **Season data updates** (new squads in `Data.a3`) load as well; the
  development copy has the community update for 2025/26 installed.

## Getting started

1. Install ANSTOSS 3.
   - Windows: run GOG's installer. It installs to `C:\GOG Games\ANSTOSS 3`.
   - Linux: run GOG's Windows installer with Wine or
     [Bottles](https://usebottles.com/), or unpack it with `innoextract`
     (untested). Only the files are needed; the game itself need not run
     under Wine.
2. Download the archive for your system from the
   [releases](../../releases) and unpack it.
3. Tell Anstoss 3.1 where the game is (next section), and start
   `anstoss-3-1` (Linux) or `anstoss-3-1.exe` (Windows).

## Configuration

Anstoss 3.1 has no settings file of its own. The one thing it needs is the
game folder — the folder that holds `anstoss3.exe`. It takes the first of:

1. **The first command-line argument:**
   `anstoss-3-1 "/path/to/ANSTOSS 3"`.
2. **The environment variable `ANSTOSS3_DIR`.**
   - Linux: `export ANSTOSS3_DIR="$HOME/Games/ANSTOSS 3"`.
   - Windows: `setx ANSTOSS3_DIR "D:\Games\ANSTOSS 3"`, then open a new
     window.
3. **The folder the program sits in.** Copy `anstoss-3-1(.exe)` into the
   game folder and start it from there.
4. **Where GOG installs the game:** `C:\GOG Games\ANSTOSS 3` on Windows,
   `~/.local/share/bottles/bottles/ANSTOSS-3/drive_c/GOG Games/ANSTOSS 3` on
   Linux.

The game folder must be **writable**. Like the original, Anstoss 3.1 keeps
its settings in the game's own files there. You change them in the game under
*Optionen*, and they are written back when you quit:

| file | what it holds |
|---|---|
| `musik.cfg` | music and sound effects (*Musik & Soundeffekte*) |
| `Konfig3D.cfg` | the 3D options (*3D - Optionen*) |
| `zeit.saq` | play time (*Wie oft & Wie lange*) |
| `alltimes.saq` | the all-time records (*Für die Ewigkeit*), read only |
| `saves/*.an3` | savegames, read only for now |

On Linux the program needs the usual desktop libraries: ALSA
(`libasound.so.2`), udev, and X11 or Wayland.

## Building from source

Anstoss 3.1 needs Rust 1.97 or newer; distribution packages are usually
older, so install it with [rustup](https://rustup.rs) everywhere. The build
puts the program at `target/release/anstoss-3-1` (`.exe` on Windows).

### Arch Linux

```sh
sudo pacman -S --needed base-devel git rustup pkgconf alsa-lib systemd-libs \
    libxkbcommon wayland libx11
rustup default stable
git clone <this repository> anstoss-3.1 && cd anstoss-3.1
cargo build --release -p a3-app
```

### Debian and Ubuntu

```sh
sudo apt install build-essential git curl pkg-config libasound2-dev \
    libudev-dev libwayland-dev libxkbcommon-dev libx11-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
git clone <this repository> anstoss-3.1 && cd anstoss-3.1
cargo build --release -p a3-app
```

### Windows

1. Install [Git for Windows](https://git-scm.com/download/win).
2. Run `rustup-init.exe` from [rustup.rs](https://rustup.rs) and let it
   install the Visual Studio Build Tools (the MSVC linker) it asks for.
3. In a new terminal:

   ```powershell
   git clone <this repository> anstoss-3.1
   cd anstoss-3.1
   cargo build --release -p a3-app
   ```

### Tests

On Linux, `cargo test` runs the tests that need no game files; with the game,
`ANSTOSS3_DIR="/path/to/ANSTOSS 3" cargo test -- --include-ignored` runs
them all. The game's tests set up their folders with Unix symlinks, so they
don't build on Windows.

## License

The source code is licensed under the GNU General Public License,
version 3 or later; see `LICENSE`. The license does not cover ANSTOSS 3 or
any of its content, which belong to their rights holders.
