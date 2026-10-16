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
| `crates/catalog` | the photos on disk: the folder tree of the home folder and the mounted volumes, and a folder's photo list and its current photo | `raw-files`; never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/previews` | decoding the photos' embedded previews on worker threads. Empty so far | never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/desktop-ui` | the custom widgets and the shared parts of the window; so far the colour themes, the style, the typeface, the icons, the parts of the title bar, the action bars, the docks, the toolbar, the dialogs' frame, the About dialog, and the keyboard shortcuts' chords and reference. Knows nothing of photos | `egui`, `iconflow`; never `catalog`, `previews`, `raw-files` or `image` |
| `crates/photography-workbench` | the app: the window and what it shows. The only crate with a binary | any other crate here, and any external one |

### photography-workbench

The app is a library and a thin binary, laid out like
[eframe_template](https://github.com/emilk/eframe_template) but desktop-only
([ADR007](architecture-decisions/adr007-egui-cross-platform.md)). Paths are
relative to `crates/photography-workbench`:

| File | Holds | May depend on |
|---|---|---|
| `src/main.rs` | the binary: sets up logging and the window (its size, and its frame on each system), turns off the menu bar winit makes on macOS by default, whose Quit would end the app without closing the window, then hands control to eframe | the library, `eframe`, `env_logger`; on macOS `winit` |
| `src/lib.rs` | the library root; re-exports `PhotographyWorkbenchApp` | its own modules |
| `src/dialogs.rs` | the app's dialogs, in the frame `desktop_ui::dialog` gives every dialog: Settings, which holds the theme choice, About, which `desktop_ui::about` draws from the app's name, tagline, version, copyright, licence and home page, and the reference of keyboard shortcuts, which `desktop_ui::keys` draws from the keymap's sections | `egui`, `desktop_ui` |
| `src/mode.rs` | `Mode`, the app's modes (Library, Organizer, Develop): each one's name and icon, whether it is built yet, and its `Layout`, the parts of the window it has around its centre | `desktop_ui` |
| `src/view.rs` | `View`, the ways Library shows a folder's photos (a grid of thumbnails, or one photo in the viewer): each one's name and icon | `desktop_ui` (tests only) |
| `src/keymap.rs` | the keyboard shortcuts: one table of what each chord does (switch mode, switch view, move to another photo, show or hide docks, open Settings or the reference, quit), on which surfaces (the grid, the viewer), and the group and description the reference lists it under (Views, Photos, Docks, General): Left and Right move to the previous and next photo in both views, Up and Down to the photo above and below and Home and End to the first and last in the grid only, and those six do it again while their key is held, as the system repeats it, where every other shortcut is done once however long its key is held; the actions they do, which buttons and menu items do too; `pressed` and `pressed_actions`, which the handlers ask, `chord_for`, the chord a control names in its tooltip, `chord_everywhere`, the one a menu item carries, and the reference's `sections`, built from the same table. A shortcut to a mode not built yet does nothing and is not listed ([ADR015](architecture-decisions/adr015-the-keys.md)) | `egui`, `desktop_ui` |
| `src/menu.rs` | the menu bar at the top of the screen on macOS: the app menu (About, Settings, Quit), View (the views and the docks), Window (the modes) and Help (the reference of keyboard shortcuts), whose items ask for the app's actions; `State`, what the items show of the app, which can be chosen and which are checked, worked out on every system; and `MacMenu`, which hands the menus to the system and passes their clicks on, a stand-in never made elsewhere. Only the chords that hold Cmd are the items' key equivalents, so the keys without Cmd stay the window's, and not those of a toggle, which the system would repeat while the keys are held | `eframe`, `desktop_ui`; on macOS `log`, `muda`, `raw-window-handle` |
| `src/disk.rs` | `Disk`, which reads the disk off the window's thread: a folder's subfolders and a folder's photos, as `catalog` reads them, each read on a short-lived thread of its own, so a listing that hangs on a network share that has gone holds up no other, and the roots of the folder tree, watched on a thread of their own that reads them every 2 seconds and wakes the window only when they change, so an idle window draws nothing. The window takes the answers at the start of a frame; a thread asks for a frame when it has one. A read of photos carries the number its caller gave it back with its answer. Its caller keeps at most one listing of a folder, and one read of its photos, under way at a time, so a share that hangs holds no more threads however often its folders are opened or chosen; the one exception is a read of photos left hanging on a card taken out, as the card put back is read afresh beside it. With no thread to be had, a read is done on the window's thread rather than never, and the roots are read there once, which the log warns of | `catalog`, `egui`, `log` |
| `src/browse.rs` | `Browser`, which keeps the folder tree fed from disk, once per frame before anything is drawn: it hands the tree what the disk has read, has the disk watch the roots from the first frame on, as memory cards and drives come and go and no system tells a portable app at once, refreshes the tree when the window comes back to the front, so a folder changed meanwhile in the system's file manager is read again, and lists the folders the tree asks for, one read per folder at a time; what is expanded and selected in the tree, saved and restored; and `Photos`, the selected folder's photos and the current one, as far as they have been read, the current one moved to an index, by a step or to either end. The photos are read each time a folder is selected, again when the window comes back to the front, and once the selected folder's root is among the tree's roots, as at start-up, but never by two reads at once: wanted while a read of them is under way, they are that read's answer; they are dropped when the root goes, and a read of them under way then is forgotten, so they are read afresh when the root comes back, beside that read if it hangs, and its answer is dropped. Each read is numbered and only the answer of the one wanted last is taken, so a slow read of a folder left behind never replaces the one chosen since; a folder just chosen shows nothing until its photos come, and the folder shown read again keeps its photos on screen, then keeps the current photo by its path if it is still there | `catalog`, `eframe`, `egui` |
| `src/folder_tree.rs` | the folder tree in Library's left dock: a row per folder shown, indented by its depth, with the arrow that expands or collapses it, an icon for the home folder, a volume or any other folder, and its name, cut short with "…" where it does not fit and dimmed where the folder cannot be read; a click elsewhere on the row selects it, which reads its photos. Only the rows in view are laid out | `catalog`, `egui`, `desktop_ui` |
| `src/grid.rs` | the thumbnail grid in Library's centre: a 220-point square tile per photo, 1 point apart, as many to a row as fit, showing the photo's name, cut short with "…" where it does not fit, until the app decodes previews, the current one outlined; a click makes a photo the current one, a double-click opens it too. It draws a list of photos and the current one's index, whatever they were read from, and brings the current photo's row into view, scrolled just far enough that its tile is whole, when the current photo or the list changes or the grid comes back on screen, and only if the tile is not wholly in view already, never because the grid was scrolled, nor after a click, as the photo clicked on is under the pointer (`src/follow.rs` decides when). It tells the app how many tiles it laid out in a row, and which photo is above or below another, for the keys that move a row. Only the rows in view are laid out | `catalog`, `egui`, `desktop_ui` |
| `src/filmstrip.rs` | the filmstrip in the viewer's bottom dock: a 120-point cell per photo, side by side in one row running the dock's height, each holding, over a tile where its thumbnail goes once the app decodes previews, the photo's name without its extension, cut short with "…" where it would run into the photo's format (NEF, ARW or JPEG) at the right, the current one's tile outlined; a click makes a photo the current one. A mouse's vertical wheel scrolls it sideways. It draws a list of photos and the current one's index, whatever they were read from, and brings the current photo into view, in its middle, when the current photo or the list changes or the strip comes back on screen, and only if it is not in view already, never because the strip was scrolled (`src/follow.rs` decides when); a click on a cell leaves the strip where it is, even when the cell is only partly in view. Only the cells in view and four more at each end are laid out | `catalog`, `egui`, `desktop_ui` |
| `src/follow.rs` | when a view of a folder's photos brings the current photo into view: when the current photo or the list changes, or the view was not drawn the frame before, as when it comes back on screen; never because time passed or the view was scrolled ([ADR011](architecture-decisions/adr011-views-navigation-and-the-keymap.md)). What each view last followed, and when it was last drawn, is kept in egui's memory between frames; a photo made current by a click on it counts as followed already | `egui` |
| `src/layout.rs` | the parts of the window around a mode's centre: the tool rail, with the one button Library has so far (Info), which shows the right dock, the docks with their sizes and what they hold (so far, the folder tree the app gives the left dock, the right dock's message, which says no photo is selected, as the photo's information is not shown yet, and the viewer's backdrop under the filmstrip the app gives the bottom dock), the title bar's toggles for them, and `Docks`, which of them are shown, shown or hidden together by a key, saved and restored | `eframe`, `egui`, `desktop_ui` |
| `src/app.rs` | `PhotographyWorkbenchApp`, the `eframe::App`: the window's title bar with its icon (off macOS), mode switcher, settings gear and button for the reference of keyboard shortcuts, the toolbar under the centre with the mode's own controls (Library's view buttons) at its start and, at its end, which photo of how many is the current one, the centre, which shows the selected folder's photos in the grid or the current photo's name in the viewer, or says no folder is selected, it cannot be read or it holds no photos, the filmstrip it gives the viewer's bottom dock, which shows the same photos and the same current one, the order the window's panels are laid out in, the keyboard shortcuts it handles and the menu bar it gives macOS, `perform`, which does each action whatever asked for it, among them the moves to another photo, a row up or down going by the number of tiles the grid laid out in a row the last time it was drawn, and doing nothing from a change of mode, view or docks shown until it is drawn again, the folder tree it keeps fed from disk and shows in Library's left dock, the window's icon, the dialogs it opens, the theme choice it saves and applies, and the mode, docks, view and folder tree it saves | `eframe`, `egui`, `desktop_ui`; on macOS `raw-window-handle`, to measure the system's window buttons |
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
| `src/chrome.rs` | the parts of the title bar the app draws itself: its frame and height, the region that drags the window and maximizes it on a double-click, its buttons (one showing an icon, one showing an icon and a word that can be shown selected, and one that shows or hides a dock, its icon a small window with the dock's side filled while the dock is shown; each names in its tooltip the chord that does what it does, if one does), the close, maximize and minimize buttons, the strips along the window's edges that resize it, and the gap that keeps the bar clear of macOS's window buttons. The window controls and the strips add nothing on macOS, where the system's frame provides them | `egui`, its own `dock`, `icons` and `theme` |
| `src/dialog.rs` | dialogs: the frame every dialog shares (the theme's popup frame with 16 points of room inside), and `show`, which shows one. A dialog closes on Esc or a click outside it, never acting as it closes | `egui` |
| `src/dock.rs` | the docks, the panels along the window's sides and bottom, and the edge each is on: filled to their edges, with a message for when there is nothing to show; shown and hidden by sliding, which keeps the ids of the widgets after a hidden dock as they were; and, where one can be resized, an edge that shows a resize cursor under the pointer but lights up only while it is dragged | `egui`, its own `fonts` |
| `src/fonts.rs` | the typeface, Inter, regular for text and semibold for headings, installed with the icon font and backed up by egui's own fonts | `egui`, its own `icons` |
| `src/icons.rs` | the icons: Lucide's icon font, given to egui once by `install`, or added to a set of fonts by `add_to`, and an icon by its Lucide name as text (`lucide`), before a word as one run of text for a button (`labeled`), or as a character and a font to paint (`glyph`), with a fallback for a name that does not resolve | `egui`, `iconflow` |
| `src/keys.rs` | the keyboard shortcuts' chords: a key with the modifiers held with it, spelled as macOS spells it (⌥⌘1) or as Windows and Linux do (Ctrl+Alt+1), and whether it was pressed in a frame, with exactly its modifiers (Shift aside for a punctuation character the keyboard types with Shift on another key, as `/` is Shift+7 on a German one) and not by a key repeating, or, for a chord that moves through something, also by its key held down repeating, after the system's delay and at its pace; the tooltip of a control a chord does the work of, which names the chord; and the reference of an app's shortcuts, sections of chords beside what they do, the descriptions lined up across sections and wrapped where they are long. What a chord does is the app's | `egui`, its own `fonts` |
| `src/style.rs` | the style every part of the window shares apart from its colours: the spacing (the room inside buttons, the gaps between widgets, the height of a control), the sizes of small print and headings, and the pace of its animations, installed once in both of egui's styles so a change of theme keeps it | `egui` |
| `src/theme.rs` | the colour themes: the concrete `Theme`, the saved choice (an `Option<Theme>`, `None` for System) that `resolve` turns into one and `picker` offers in a drop-down list, and each theme's `Palette`, the colours of every part of the window, and how it maps onto egui's visuals (Dark keeps the colours of egui's stock dark visuals as they are, and its palette repeats them), and the rounded corners every theme gives buttons and fields, and the shadows it gives popups and dialogs | `egui` |
| `src/toolbar.rs` | the toolbar, the fixed row of controls under the window's centre: filled like the docks and marked off from the centre by a line, with controls running from both its ends, and its buttons, which look like an action bar's, smaller, and name in their tooltips the chord that does what they do | `egui`, its own `action_bar` and `theme` |
| `assets/fonts/` | Inter's regular and semibold weights, from the font's own release, and its licence, the SIL Open Font License | — |

### catalog

The catalog reads the disk but starts no thread of its own: whoever
calls it decides where the waiting happens, which in the app is on
threads of its own (`src/disk.rs`). It has no GUI dependency, so
its tests run on a machine with no display, over temporary folders.
Paths are relative to `crates/catalog`:

| File | Holds | May depend on |
|---|---|---|
| `src/lib.rs` | the library root | its own modules |
| `src/folders.rs` | the folder tree: its roots, the home folder first and then the mounted volumes in natural order of their names (on macOS the folders in `/Volumes` other than symbolic links, the startup disk among them, and hidden ones; on Linux the mount points in `/media/<user>`, `/run/media/<user>` and `/mnt`, folders on another device than the one they are in, the user named by `USER` or else by the home folder, and a place reached by a link to another read once; on Windows the drives `C:` to `Z:`); a folder's subfolders in natural order of their names, links followed and hidden ones left out; and `FolderTree`, the roots, which folders are expanded and which one is selected, and the listings read so far. The tree reads nothing itself: it tells its caller which expanded folders it can reach have not been listed, a restored tree's whole depth at once, takes their listings back with the requests, and draws its rows from the folders listed so far, each saying whether its folder has subfolders, has none, could not be read or has not been listed. A refresh makes every listing stale, those under way among them, so they are asked for again while the old rows stay until the new ones arrive. New roots drop the listings of the roots that are gone, and refresh the tree when one goes, so a card put back, or another mounted at the same path, is listed again and a late listing of the one taken out is not taken for its own. Collapsing a folder keeps its listing and what is expanded below it; what is saved leaves out a folder below a collapsed one, under no root, or missing from its parent's fresh listing | its own `entry` and `natural` |
| `src/photos.rs` | `PhotoList`, a folder's photos in natural order of their names, read from disk, and the current one, which there is exactly when the list is not empty and which moves by a step, to an index or to either end, stopping at the ends; `Photo`, a photo's file and its `Kind`, a raw file of one of the formats raw-files reads or a JPEG, told by the extension alone | `raw-files`, its own `entry` and `natural` |
| `src/entry.rs` | which entries of a folder the catalog shows, and whether each is a file or a folder once a symbolic link is followed: not those whose name begins with a dot, nor on macOS and Windows those the system flags hidden, nor a broken link, nor anything that is neither a file nor a folder. It also tells a link from what it points at, for a listing that leaves links out | — |
| `src/natural.rs` | natural order for file and folder names: numbers by their value (`DSC_2` before `DSC_10`), letters whatever their case, and a fixed order for names alike by those rules, so a folder lists the same whatever order the system gave its entries in | — |

### previews

Empty so far: its `src/lib.rs` holds only the crate's description. It
has no GUI dependency, so its tests will run on a machine with no
display.

### How it fits together

**Data flow.** eframe owns the event loop and calls
`PhotographyWorkbenchApp::ui` every frame; egui is immediate-mode, so the UI
is rebuilt from the app's state each time rather than kept as a widget tree.
The only state so far is the theme choice, the mode, Library's view,
which docks are shown, the folder tree, the selected folder's photos
and which dialogs are open; the window shows its own title bar, which
holds the app's icon (which opens About; not on macOS), the mode
switcher, the button for the reference of keyboard shortcuts, the
settings gear, a toggle for each of the mode's docks and the window
controls. Only Library is
built: the switcher shows Organizer and Develop disabled. Below the
title bar the panels
([ADR014](architecture-decisions/adr014-the-parts-of-the-window.md))
are laid out in an order that gives each its place:
first the tool rail, so it takes the window's right edge below the title
bar; then the side docks, the left one holding the folder tree, the
right one inside the tool rail, so they run down to the window's
bottom; then the toolbar along the bottom
between them, holding the mode's own controls (in Library, a button per
view) and, in Library, which photo of how many is the current one;
then, in Library's viewer, the bottom dock above the toolbar, holding
the filmstrip, which the grid has no need of; and last the centre, which
takes what is left: the selected folder's photos as the grid's tiles,
or the current photo's name in the viewer, or a message saying why
there are none. Before each frame, in eframe's
`logic` step, the app does what the items clicked on the menu bar ask,
on macOS; eframe runs that step while the window is minimized or
hidden too, when it draws no frame, so the menu bar still acts then.
Each frame starts by resolving the theme
choice against the OS light/dark preference and, when the resulting theme
changed, reinstalling egui's visuals and restyling the frame's root `Ui`,
which took its style before the app ran. Next come the keyboard
shortcuts pressed in the frame, before anything is drawn, so a click
or a key that switches the view or hides a dock shows in that same
frame. The keys stand down while a dialog is open, all but the
reference's own key, which closes it, and while a text field has the
keyboard. egui's own Ctrl+Q is turned off, so quitting follows the
keys' rules. On macOS the menu bar takes the chords that hold Cmd
before the window sees them, even from a text field, as on any Mac,
since they type nothing. Its items stand down while a dialog is open,
all but the reference's own, which closes it; a click is checked again
when it is done, and the frame's last step shows the items the app as
the frame left it, the mode, the view and the docks shown checked.
After the keys, still before anything is drawn, the folder tree takes
what the disk has read since the last frame, and starts reading what it
needs next, each read on a thread of its own that asks for a frame when
it is done: the whole tree again when the window comes back to the
front, and the folders the tree asks for, one read per folder at a
time, so the tree drawn in the frame is as up to date as the disk has
answered. The roots are watched on a thread of their own, started on
the first frame, which reads them every 2 seconds and wakes the window
only when they change, so an idle window draws nothing. The selected
folder's photos are read like the folders: when a folder is selected,
by a click on its row, again
when the window comes back to the front, and once a folder restored at
start-up has its root among the tree's, which is read on a later frame.
A folder's photos are never read by two reads at once, but for a read
left hanging on a card taken out, and only the answer of the read
wanted last is taken, so the photos drawn are always
those of the folder selected last. The grid draws whatever list of
photos it is handed, and a click on a tile makes its photo the current
one; a double-click opens the viewer the way its key and its button
do, through `perform`. The filmstrip draws the same list with the same
current photo, and a click on a cell makes its photo the current one
too. It brings the current photo into view when the photo changes,
when the list does, or when the strip was not drawn the frame before,
as when the viewer opens or the dock is shown again; it remembers in
egui's memory what it followed and when it was last drawn, so time
passing, or the strip being scrolled, never moves it. The grid
follows the current photo the same way, scrolled just far enough that
the photo's tile is whole, but not after a click on it, as the photo
clicked on is under the pointer. The keys that move between photos go
through `perform` like every other: Left and Right to the previous and
next photo in the grid and the viewer, Up and Down a row and Home and
End to the first and last photo in the grid, a row being as many tiles
as the grid laid out in one the last time it was drawn, so Up and Down
do nothing from a change of mode, view or docks shown until the grid is
drawn again, its rows then perhaps of another length; each stops at the
first and the last photo.
The Settings dialog only records
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
egui's own settings and the app's theme choice, mode, docks, view and
folder tree in a per-user directory named after the app id, which
`main.rs` sets to the binary's name (for example
`~/Library/Application Support/photography-workbench` on macOS). The theme
choice is stored as text under the key `theme`: the theme's display name, or
`System`; a missing or unrecognised value means System, so renaming a theme
resets users who chose it. The mode is stored under the key `mode`, as
its name; a missing or unrecognised value, or a mode not built yet, means
Library. Whether each dock is shown is stored under `show_left_dock`,
`show_right_dock` and `show_bottom_dock`, as `true` or `false`; anything
else, or nothing, means shown. The docks' sizes are egui's own settings.
Library's view is stored under `view`, as its name; a missing or
unrecognised value means the grid. The folder tree's selected folder is
stored under `folder`, as its path, and its expanded folders under
`expanded_folders`, their paths one per line; a missing or empty value
means none. A path that is not valid Unicode, or holds a line break,
cannot be stored as a line of text, and is not kept: that folder is
not selected or expanded on the next run. egui's own
theme preference is not where the theme lives: the app sets it from the
choice each frame, as above. Renaming the binary moves that directory,
which resets those settings.

Choices that shape the code still to come, such as rating storage, are open
— see [ROADMAP.md](ROADMAP.md#open-questions).

**Decided, not yet built.** `catalog` holds what the first release needs
of it. The rest of that release's work fills `previews`, below the
GUI-free line, and builds the window on both
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)).
This file is rewritten with each of those changes, in the same pull request.

## As the code grows

Each new crate adds a row to the crate table and a section of its own, each
new module a row to its crate's table, and a change that moves data
between modules updates the data flow. A constraint that forces the shape
gets [an ADR](architecture-decisions/README.md).
