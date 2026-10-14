# Architecture

How the code is structured, and why. Update this file in the same change as
any structural change.

## Today

photography-workbench is a Cargo workspace (Rust, edition 2024) of crates
under `crates/`
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)).
The root `Cargo.toml` holds what they share: the version, the package
fields, the dependency versions, the lints and the release profile.

So far there is one crate, `crates/photography-workbench`, the app: a
library and a thin binary, laid out like
[eframe_template](https://github.com/emilk/eframe_template) but desktop-only
([ADR007](architecture-decisions/adr007-egui-cross-platform.md)). Paths
below are relative to it:

| File | Holds | May depend on |
|---|---|---|
| `src/main.rs` | the binary: sets up logging and the window, then hands control to eframe | the library, `eframe`, `env_logger` |
| `src/lib.rs` | the library root; re-exports `PhotographyWorkbenchApp` | its own modules |
| `src/app.rs` | `PhotographyWorkbenchApp`, the `eframe::App`: the window's panels, menus and icon, and the theme choice it saves and applies | `eframe`, `egui`, `theme` |
| `src/theme.rs` | the colour themes, private to the crate: the concrete `Theme`, the saved choice (an `Option<Theme>`, `None` for System) that `resolve` turns into one, and each theme's `Palette` (none for Dark, which is egui's stock dark visuals) and how it maps onto egui's visuals | `egui` |
| `build.rs` | embeds the icon and version information in the Windows executable | `winresource` (build only) |
| `assets/` | the app icon: `icon.svg` is the source, and `scripts/render-icons.sh` renders the PNG and ICO files from it, which are committed | — |
| `tests/package_metadata.rs` | checks that the app's name and copyright agree across Cargo.toml's packaging sections and `PhotographyWorkbenchApp::NAME`, and that every crate's version is the workspace's | the library, `toml` |

Keeping everything but start-up in the library lets tests drive the UI
without a display: [egui_kittest](https://github.com/emilk/egui/tree/main/crates/egui_kittest)
runs the app headless and queries its widgets through AccessKit.

**Data flow.** eframe owns the event loop and calls
`PhotographyWorkbenchApp::ui` every frame; egui is immediate-mode, so the UI
is rebuilt from the app's state each time rather than kept as a widget tree.
The only state so far is the theme choice; the window shows a menu bar and
an empty view. Each frame starts by resolving the choice against the OS
light/dark preference and, when the resulting theme changed, reinstalling
egui's visuals and restyling the frame's root `Ui`, which took its style
before the app ran. The theme menu only records the choice; the next frame
installs it, so no frame is painted in two themes. The same step keeps
egui's theme preference in line with the choice. egui passes that
preference on to the window as its light or dark appearance (its title
bar, for one), so a fixed theme sets it to the theme's dark or light, and
System sets it to System. A window given a fixed appearance stops
hearing about OS appearance changes on macOS and Windows, so pinning it
under System would freeze the theme at the one the OS had at launch.

**Persisted state.** eframe's `persistence` feature saves the window geometry,
egui's own settings and the app's theme choice in a
per-user directory named after the app id, which `main.rs` sets to the
binary's name (for example
`~/Library/Application Support/photography-workbench` on macOS). The theme
choice is stored as text under the key `theme`: the theme's display name, or
`System`; a missing or unrecognised value means System, so renaming a theme
resets users who chose it. egui's own
theme preference is not where the theme lives: the app sets it from the
choice each frame, as above. Renaming the binary moves that directory,
which resets those settings.

Choices that shape the code still to come, such as rating storage, are open
— see [ROADMAP.md](ROADMAP.md#open-questions).

**Decided, not yet built.** The first release's work adds the other
crates of the workspace, laid out engine-first, with the photos on disk
and the decoding below a GUI-free line and the window above it
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)),
and replaces the menu bar with a title bar of the app's own that holds
the mode switcher
([ADR010](architecture-decisions/adr010-modes-and-the-window.md)). This
file is rewritten with each of those changes, in the same pull request.

## As the code grows

Each new module adds a row to the table above, and a change that moves data
between modules updates the data flow. A constraint that forces the shape
gets [an ADR](architecture-decisions/README.md).
