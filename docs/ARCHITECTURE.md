# Architecture

How the code is structured, and why. Update this file in the same change as
any structural change.

## Today

photography-workbench is a Cargo workspace (Rust, edition 2024) of crates
under `crates/`
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)).
The root `Cargo.toml` holds what they share: the version, the package
fields, the dependency versions, the lints and the release profile.

`scripts/check-crate-boundary.sh`, which CI runs, fails when a crate
depends, directly or not, on one that its "May depend on" cell says
never:

| Crate | Holds | May depend on |
|---|---|---|
| `crates/catalog` | the photos on disk: the folder tree, a folder's photo list, the current photo. Empty so far | never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/previews` | decoding the photos' embedded previews on worker threads. Empty so far | never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/desktop-ui` | the custom widgets and the shared parts of the window; so far the colour themes, the style, the typeface, the icons, the parts of the title bar, the action bars, the docks, the toolbar, the dialogs' frame, the About dialog and the keyboard shortcuts' chords. Knows nothing of photos | `egui`, `iconflow`; never `catalog`, `previews`, `raw-files` or `image` |
| `crates/photography-workbench` | the app: the window and what it shows. The only crate with a binary | any other crate here, and any external one |

### photography-workbench

The app is a library and a thin binary, laid out like
[eframe_template](https://github.com/emilk/eframe_template) but desktop-only
([ADR007](architecture-decisions/adr007-egui-cross-platform.md)). Paths are
relative to `crates/photography-workbench`:

| File | Holds | May depend on |
|---|---|---|
| `src/main.rs` | the binary: sets up logging and the window (its size, and its frame on each system), then hands control to eframe | the library, `eframe`, `env_logger` |
| `src/lib.rs` | the library root; re-exports `PhotographyWorkbenchApp` | its own modules |
| `src/dialogs.rs` | the app's dialogs, in the frame `desktop_ui::dialog` gives every dialog: Settings, which holds the theme choice, and About, which `desktop_ui::about` draws from the app's name, tagline, version, copyright, licence and home page | `egui`, `desktop_ui` |
| `src/mode.rs` | `Mode`, the app's modes (Library, Organizer, Develop): each one's name and icon, whether it is built yet, and its `Layout`, the parts of the window it has around its centre | `desktop_ui` (tests only) |
| `src/view.rs` | `View`, the ways Library shows a folder's photos (a grid of thumbnails, or one photo in the viewer): each one's name and icon | `desktop_ui` (tests only) |
| `src/layout.rs` | the parts of the window around a mode's centre: the tool rail, with the one button Library has so far (Info), which shows the right dock, the docks with their sizes and what they hold (so far, the right dock's message when no photo is selected, and the filmstrip's backdrop), the title bar's toggles for them, and `Docks`, which of them are shown, saved and restored | `eframe`, `egui`, `desktop_ui` |
| `src/app.rs` | `PhotographyWorkbenchApp`, the `eframe::App`: the window's title bar with its icon (off macOS), mode switcher and settings gear, the toolbar under the centre with the mode's own controls (Library's view buttons), the order the window's panels are laid out in, the window's icon, the dialogs it opens, the theme choice it saves and applies, and the mode, docks and view it saves | `eframe`, `egui`, `desktop_ui`; on macOS `raw-window-handle`, to measure the system's window buttons |
| `build.rs` | embeds the icon and version information in the Windows executable | `winresource` (build only) |
| `assets/` | the app icon: `icon.svg` is the source, and `scripts/render-icons.sh` renders the PNG and ICO files from it, which are committed | — |
| `tests/package_metadata.rs` | checks that the app's name, tagline and copyright agree across Cargo.toml's packaging sections and `PhotographyWorkbenchApp`'s constants, and that every crate's version is the workspace's | the library, `toml` |

Keeping everything but start-up in the library lets tests drive the UI
without a display: [egui_kittest](https://github.com/emilk/egui/tree/main/crates/egui_kittest)
runs the app headless and queries its widgets through AccessKit.

### desktop-ui

Paths are relative to `crates/desktop-ui`:

| File | Holds | May depend on |
|---|---|---|
| `src/lib.rs` | the library root | its own modules |
| `src/about.rs` | the About dialog: the app's icon over its name, tagline, version, copyright, licence and a link to its home page, all of which the app passes in | `egui`, its own `dialog` |
| `src/action_bar.rs` | the action bars, the fixed columns of icon buttons along the window's sides: the bar itself, filled like the docks and marked off from them by a line, with buttons at its top and its foot, and the buttons, which show a rounded square behind the icon of the one that is on | `egui`, its own `icons` and `theme` |
| `src/chrome.rs` | the parts of the title bar the app draws itself: its frame and height, the region that drags the window and maximizes it on a double-click, its buttons (one showing an icon, one showing an icon and a word that can be shown selected, and one that shows or hides a dock, its icon a small window with the dock's side filled while the dock is shown), the close, maximize and minimize buttons, the strips along the window's edges that resize it, and the gap that keeps the bar clear of macOS's window buttons. The window controls and the strips add nothing on macOS, where the system's frame provides them | `egui`, its own `dock`, `icons` and `theme` |
| `src/dialog.rs` | dialogs: the frame every dialog shares (the theme's popup frame with 16 points of room inside), and `show`, which shows one. A dialog closes on Esc or a click outside it, never acting as it closes | `egui` |
| `src/dock.rs` | the docks, the panels along the window's sides and bottom, and the edge each is on: filled to their edges, with a message for when there is nothing to show; shown and hidden by sliding, which keeps the ids of the widgets after a hidden dock as they were; and, where one can be resized, an edge that shows a resize cursor under the pointer but lights up only while it is dragged | `egui`, its own `fonts` |
| `src/fonts.rs` | the typeface, Inter, regular for text and semibold for headings, installed with the icon font and backed up by egui's own fonts | `egui`, its own `icons` |
| `src/icons.rs` | the icons: Lucide's icon font, given to egui once by `install`, or added to a set of fonts by `add_to`, and an icon by its Lucide name as text (`lucide`), before a word as one run of text for a button (`labeled`), or as a character and a font to paint (`glyph`), with a fallback for a name that does not resolve | `egui`, `iconflow` |
| `src/keys.rs` | the keyboard shortcuts' chords: a key with the modifiers held with it, spelled as macOS spells it (⌥⌘1) or as Windows and Linux do (Ctrl+Alt+1), and whether it was pressed in a frame, with exactly its modifiers (Shift aside for a punctuation character the keyboard types with Shift on another key, as `/` is Shift+7 on a German one) and not by a key repeating. What a chord does is the app's | `egui` |
| `src/style.rs` | the style every part of the window shares apart from its colours: the spacing (the room inside buttons, the gaps between widgets, the height of a control), the sizes of small print and headings, and the pace of its animations, installed once in both of egui's styles so a change of theme keeps it | `egui` |
| `src/theme.rs` | the colour themes: the concrete `Theme`, the saved choice (an `Option<Theme>`, `None` for System) that `resolve` turns into one and `picker` offers in a drop-down list, and each theme's `Palette`, the colours of every part of the window, and how it maps onto egui's visuals (Dark keeps the colours of egui's stock dark visuals as they are, and its palette repeats them), and the rounded corners every theme gives buttons and fields, and the shadows it gives popups and dialogs | `egui` |
| `src/toolbar.rs` | the toolbar, the fixed row of controls under the window's centre: filled like the docks and marked off from the centre by a line, with controls running from both its ends, and its buttons, which look like an action bar's, smaller | `egui`, its own `action_bar` and `theme` |
| `assets/fonts/` | Inter's regular and semibold weights, from the font's own release, and its licence, the SIL Open Font License | — |

### catalog and previews

Both are empty so far: each `src/lib.rs` holds only the crate's
description. They have no GUI dependency, so their tests will run on a
machine with no display.

### How it fits together

**Data flow.** eframe owns the event loop and calls
`PhotographyWorkbenchApp::ui` every frame; egui is immediate-mode, so the UI
is rebuilt from the app's state each time rather than kept as a widget tree.
The only state so far is the theme choice, the mode, Library's view,
which docks are shown and which dialogs are open; the window shows its own title bar,
which holds the app's icon (which opens About; not on macOS), the mode
switcher, the settings gear, a toggle for each of the mode's docks and
the window controls. Only Library is
built: the switcher shows Organizer and Develop disabled. Below the
title bar the panels
([ADR014](architecture-decisions/adr014-the-parts-of-the-window.md))
are laid out in an order that gives each its place:
first the tool rail, so it takes the window's right edge below the title
bar; then the side docks, the right one inside the tool rail, so they
run down to the window's bottom; then the toolbar along the bottom
between them, holding the mode's own controls (in Library, a button per
view); then, in Library's viewer, the bottom dock above the toolbar,
for the filmstrip, which the grid has no need of; and last the centre,
empty so far, which takes what is left. Each frame starts by resolving the theme
choice against the OS light/dark preference and, when the resulting theme
changed, reinstalling egui's visuals and restyling the frame's root `Ui`,
which took its style before the app ran. The Settings dialog only records
the choice; the next frame installs it, so no frame is painted in two
themes. The same step keeps
egui's theme preference in line with the choice. egui passes that
preference on to the window as its light or dark appearance (the
system's window buttons on macOS, for one), so a fixed theme sets it to
the theme's dark or light, and System sets it to System. A window given
a fixed appearance stops hearing about OS appearance changes on macOS and
Windows, so pinning it under System would freeze the theme at the one the
OS had at launch.

**Persisted state.** eframe's `persistence` feature saves the window geometry,
egui's own settings and the app's theme choice, mode, docks and view in a
per-user directory named after the app id, which `main.rs` sets to the
binary's name (for example
`~/Library/Application Support/photography-workbench` on macOS). The theme
choice is stored as text under the key `theme`: the theme's display name, or
`System`; a missing or unrecognised value means System, so renaming a theme
resets users who chose it. The mode is stored under the key `mode`, as
its name; a missing or unrecognised value, or a mode not built yet, means
Library. Whether each dock is shown is stored under `show_left_dock`,
`show_right_dock` and `show_bottom_dock`, as `true` or `false`; anything
else, or nothing, means shown. The docks' sizes are egui's own settings.
Library's view is stored under `view`, as its name; a missing or
unrecognised value means the grid. egui's own
theme preference is not where the theme lives: the app sets it from the
choice each frame, as above. Renaming the binary moves that directory,
which resets those settings.

Choices that shape the code still to come, such as rating storage, are open
— see [ROADMAP.md](ROADMAP.md#open-questions).

**Decided, not yet built.** The first release's work fills `catalog`
and `previews`, below the GUI-free line, and builds the window on them
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)),
and adds the native menu bar on macOS
([ADR010](architecture-decisions/adr010-modes-and-the-window.md)). This
file is rewritten with each of those changes, in the same pull request.

## As the code grows

Each new crate adds a row to the crate table and a section of its own, each
new module a row to its crate's table, and a change that moves data
between modules updates the data flow. A constraint that forces the shape
gets [an ADR](architecture-decisions/README.md).
