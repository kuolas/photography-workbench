# ADR007: egui, on macOS, Linux and Windows

- Status: Accepted
- Date: 2026-10-10

## Context

photography-workbench is a desktop app for culling photos, and photographers
use all three desktop systems. The GUI toolkit decides which platforms are
practical, how fast large thumbnail grids can be drawn, and whether the app
can have a look of its own. The options considered were GTK 4 with
libadwaita, Slint, Iced and egui.

## Decision

We will support **macOS, Linux and Windows** as equal targets, and build the
interface with **[egui](https://github.com/emilk/egui)** (through `eframe`).

- **egui:** pure Rust, one code base on all three systems, GPU-rendered and
  fast at drawing large thumbnail grids, and free of any platform's design
  language — so the app looks the same everywhere rather than like GNOME,
  Windows or macOS. Dual-licensed MIT / Apache-2.0, compatible with
  GPL-3.0-or-later.
- **Rejected GTK 4 / libadwaita:** native on GNOME, but weak on Windows and
  macOS, and its design guidelines tie the app's look to GNOME.
- **Rejected Slint and Iced:** both workable; egui has the larger ecosystem
  of image-viewer-style apps and the simplest model for a fast, custom UI.

## Consequences

- CI builds and tests on all three systems on every pull request.
- egui is an immediate-mode toolkit: it does not use native widgets, so the
  app looks the same everywhere and native accessibility depends on egui's
  AccessKit integration.
- The project keeps the GNOME *development* workflow ([ADR003](adr003-linear-history.md),
  [ADR004](adr004-gnome-commit-messages.md)) without being a GNOME app.
- Packaging differs per system: `.app`/`.dmg` on macOS, `.msi` on Windows,
  Flatpak and/or `.deb` on Linux.
