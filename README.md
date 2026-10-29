# Anstoss 3.1

[![Progress](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/tonigineer/anstoss-3.1/main/.github/badges/progress.json)](#work-in-progress)
[![CI](https://github.com/tonigineer/anstoss-3.1/actions/workflows/ci.yml/badge.svg)](https://github.com/tonigineer/anstoss-3.1/actions/workflows/ci.yml)
[![Tests](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/tonigineer/anstoss-3.1/main/.github/badges/tests.json)](https://github.com/tonigineer/anstoss-3.1/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.97%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Bevy](https://img.shields.io/badge/Bevy-0.20-232326)](https://bevyengine.org)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue)](LICENSE)
[![Platforms](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-blue)](#getting-started)

## Work in progress

Not playable from start to finish yet: many screens and rules of the
original are still being rebuilt.

An unofficial, fan-made reimplementation of **ANSTOSS 3 – Der
Fußballmanager** (Ascaron, 2000), in Rust, for Linux and Windows. It aims to
match the original screen for screen; every finished screen is checked
pixel by pixel against the original running under Wine. The classic 640×480
look comes first; a remastered 16:9 one will follow once the rebuild is
complete.

## You need the original game

> **Disclaimer.** Not affiliated with or endorsed by Ascaron or the current
> rights holders of ANSTOSS; "ANSTOSS" is a trademark of its owner. This
> repository contains **no files of the game** — graphics, texts, sounds and
> football data are read at runtime from a copy of ANSTOSS 3 that **you own
> and have installed**. ANSTOSS 3 is still sold, DRM-free, on
> [GOG](https://www.gog.com/game/anstoss_3) (German only).

### How it is made

[Ghidra](https://ghidra-sre.org) decompiles `anstoss3.exe`, and
[Claude](https://www.anthropic.com/claude) reads that decompile, writes the
code, and compares every finished screen against the original. The
decompiled code itself is not part of this repository.

### Which copy works

- Developed against the **GOG version**: its `anstoss3.exe` is the final
  patched build (file version 1.1.0.0). Some rules are tables inside the
  exe, which this program reads at that build's addresses — an unpatched CD
  exe would be read wrongly. The build is not checked yet.
- Needs the **installed game folder**, and writes into it. An ISO or the CD
  alone is not enough; a CD install brought up to the final patch may work,
  untested.
- Season data updates (new squads in `Data.a3`) load as well.

## Getting started

1. Install ANSTOSS 3. On Linux, get the files out of GOG's Windows
   installer with Wine, [Bottles](https://usebottles.com/) or `innoextract`
   (untested); the game itself need not run under Wine.
2. Unpack the archive for your system from the
   [releases](https://github.com/tonigineer/anstoss-3.1/releases).
3. Point it at the game folder and run `anstoss-3-1` / `anstoss-3-1.exe`.

## Configuration

The one setting is the game folder — the one holding `anstoss3.exe`. The
first of these wins:

1. the first argument: `anstoss-3-1 "/path/to/ANSTOSS 3"`
2. the environment variable `ANSTOSS3_DIR`
3. the program's own folder — drop the binary into the game folder
4. GOG's default: `C:\GOG Games\ANSTOSS 3`, or
   `~/.local/share/bottles/bottles/ANSTOSS-3/drive_c/GOG Games/ANSTOSS 3`

The folder must be **writable**: like the original, the program keeps its
settings in the game's own files there, changed under *Optionen* and written
back on quit:

| file | what it holds |
|---|---|
| `musik.cfg` | music and sound effects |
| `Konfig3D.cfg` | the 3D options |
| `zeit.saq` | play time |
| `alltimes.saq` | the all-time records (read only) |
| `saves/*.an3` | savegames (read only for now) |

On Linux the usual desktop libraries are needed: ALSA (`libasound.so.2`),
udev, and X11 or Wayland.

## Building from source

Rust 1.97 or newer (Bevy 0.20's requirement); distro packages are usually
older, so use [rustup](https://rustup.rs) everywhere. The binary lands at
`target/release/anstoss-3-1`.

### Arch Linux

```sh
sudo pacman -S --needed base-devel git rustup pkgconf alsa-lib systemd-libs \
    libxkbcommon wayland libx11
rustup default stable
git clone https://github.com/tonigineer/anstoss-3.1.git && cd anstoss-3.1
cargo build --release -p a3-app
```

## License

The source code is licensed under the GNU General Public License, version 3
or later; see `LICENSE`. It covers the source only — ANSTOSS 3 and any of
its content belong to their rights holders.
