# Photography Workbench

A fast desktop app for culling photos: open a folder of RAW or JPEG files,
flick through them, and keep the good ones. For macOS, Linux and Windows.

**Status: 0.1.0, the first release.** It browses: choose a folder, look
through its photos and read what the camera recorded. Marking the keepers
comes next; see the [roadmap](docs/ROADMAP.md).

## What it does today

- **Browse folders** — a tree of the home folder and of every memory card
  and drive mounted, which come and go as they are plugged in and out.
- **See a folder's photos at once** — a grid of thumbnails made from the
  previews cameras store in their files, so a folder of RAW files shows
  as quickly as one of JPEGs.
- **Look closely** — a viewer that fits the photo or shows it at 100 %,
  zoomed with the mouse, a trackpad or the keyboard and moved by dragging,
  with a filmstrip of the folder below.
- **Read what the camera recorded** — the camera, the lens, when the
  photo was taken and the exposure, in a panel beside the photo or over
  it.
- **Work from the keyboard** — the shortcuts established photography
  tools use, listed in a reference the app opens.
- **The same window on every system** — with a title bar of its own, a
  menu bar at the top of the screen on macOS, and four themes.

It reads the RAW files of Nikon cameras (NEF, NRW) and Sony cameras (ARW,
SRF, SR2, ARQ), and JPEG files, through
[raw-files](https://crates.io/crates/raw-files).

## What it will do

The app has three modes, one for each of a photographer's jobs. Library is
the one that exists so far; the other two are in the window, disabled
until they are built.

- **Library** — pick the keepers, spot blurred and out-of-focus shots at
  a glance, and see how well each photo is framed.
- **Organizer** — bring photos in from a memory card, and rename them
  from their capture date, camera and other metadata.
- **Develop** — develop a RAW file into a picture.

This app is meant to be local, fast and approachable, no account, no cloud.

## Build and run

Releases are source only for now: there are no installers or packaged
builds yet. Download the source archive of a release from the
[releases page](https://github.com/kuolas/photography-workbench/releases),
or clone the repository. Building it on macOS, Linux or Windows needs Rust
1.95 or later. To build and run the app:

```sh
cargo run --release
```

A release build takes longer than a debug build, as it optimises the
whole program at once.

To build and test it while working on it:

```sh
cargo build
cargo run
cargo test
```

On macOS, an app started with `cargo run` is not packaged as an
application, so the app menu's title reads `photography-workbench` rather
than Photography Workbench.

The app needs a graphics card of the Vulkan, DirectX 12 or Metal class, as
it shows photos in textures of up to 16384 pixels a side, which every such
card supports.

### Changing the app icon

The icon's source is
[crates/photography-workbench/assets/icon.svg](crates/photography-workbench/assets/icon.svg).
The PNG and ICO files next to it are rendered from it and committed, so
building needs nothing extra. After editing the SVG, run
`scripts/render-icons.sh` to render them again. The script needs:

- `rsvg-convert`, from librsvg;
- `python3`.

To install librsvg:

```sh
# macOS
brew install librsvg

# Debian and Ubuntu
sudo apt install librsvg2-bin

# Fedora
sudo dnf install librsvg2-tools
```

## Get involved

- **Found a bug or want a feature?** Open an
  [issue](https://github.com/kuolas/photography-workbench/issues/new/choose).
- **Found a security problem?** Please don't open a public issue; see
  [SECURITY.md](SECURITY.md).
- **Want to contribute code or docs?** Read [CONTRIBUTING.md](CONTRIBUTING.md).

Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) — how to build, test and send a change.
- [docs/ROADMAP.md](docs/ROADMAP.md) — what is planned, in what order.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the code is structured.
- [docs/architecture-decisions/](docs/architecture-decisions/README.md) — why
  things are the way they are.
- [CHANGELOG.md](CHANGELOG.md) — user-visible changes.

## License

Copyright (C) 2026 Alvaro Kuolas and the Photography Workbench contributors.

Photography Workbench is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See [LICENSE](LICENSE) for the full text.

The Inter typeface in `crates/desktop-ui/assets/fonts/` is copyright The
Inter Project Authors and licensed under the SIL Open Font License, version
1.1; see [its licence](crates/desktop-ui/assets/fonts/LICENSE-Inter.txt).
