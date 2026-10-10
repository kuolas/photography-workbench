# Architecture

How the code is structured, and why. Update this file in the same change as
any structural change.

## Today

photography-workbench is one Rust crate (edition 2024) with a library and a
thin binary, laid out like
[eframe_template](https://github.com/emilk/eframe_template) but desktop-only
([ADR007](architecture-decisions/adr007-egui-cross-platform.md)):

| File | Holds | May depend on |
|---|---|---|
| `src/main.rs` | the binary: sets up logging and the window, then hands control to eframe | the library, `eframe`, `env_logger` |
| `src/lib.rs` | the library root; re-exports `PhotographyWorkbenchApp` | its own modules |
| `src/app.rs` | `PhotographyWorkbenchApp`, the `eframe::App`: the window's panels and menus | `eframe`, `egui` |

Keeping everything but start-up in the library lets tests drive the UI
without a display: [egui_kittest](https://github.com/emilk/egui/tree/main/crates/egui_kittest)
runs the app headless and queries its widgets through AccessKit.

**Data flow.** eframe owns the event loop and calls
`PhotographyWorkbenchApp::ui` every frame; egui is immediate-mode, so the UI
is rebuilt from the app's state each time rather than kept as a widget tree.
There is no state yet: the window shows a menu bar and an empty view.

**Persisted state.** eframe's `persistence` feature saves the window geometry
and egui's own settings, such as the theme, in a per-user directory named
after `PhotographyWorkbenchApp::NAME` (for example
`~/Library/Application Support/Photography-Workbench` on macOS). Renaming the
app moves that directory, which resets those settings.

Choices that shape the code still to come, such as rating storage, are open
— see [ROADMAP.md](ROADMAP.md#open-questions).

## As the code grows

Each new module adds a row to the table above, and a change that moves data
between modules updates the data flow. A constraint that forces the shape
gets [an ADR](architecture-decisions/README.md).
