# Architecture

How the code is structured, and why. Update this file in the same change as
any structural change.

## Today

photography-workbench is a Cargo workspace (Rust, edition 2024) of crates
under `crates/`
([ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)).
The root `Cargo.toml` holds what they share: the version, the package
fields, the dependency versions, the lints and the build profiles.

`scripts/check-crate-boundary.sh`, which CI runs, fails when a crate
depends, directly or not, on one that its "May depend on" cell says
never:

| Crate | Holds | May depend on |
|---|---|---|
| `crates/catalog` | the photos on disk: the folder tree of the home folder and the mounted volumes, and a folder's photo list and its current photo | `raw-files`; never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/previews` | decoding the photos into images to show: a JPEG, or the embedded preview of a RAW file, made small and upright, on worker threads, the most wanted first, each with what its file tells of itself: its size on disk, the sizes of its previews and, of a RAW file, what the camera recorded | `raw-files`, `image`; never `egui`, `eframe`, `desktop-ui`, `winit` or `wgpu` |
| `crates/desktop-ui` | the custom widgets and the shared parts of the window; so far the colour themes, the style, the typeface, the icons, the parts of the title bar, the action bars, the docks, the toolbar, the dialogs' frame, the About dialog, and the keyboard shortcuts' key combinations and reference. Knows nothing of photos | `egui`, `iconflow`; never `catalog`, `previews`, `raw-files` or `image` |
| `crates/photography-workbench` | the app: the window and what it shows. The only crate with a binary | any other crate here, and any external one |

### photography-workbench

The app is a library and a thin binary, laid out like
[eframe_template](https://github.com/emilk/eframe_template) but desktop-only
([ADR007](architecture-decisions/adr007-egui-cross-platform.md)). Paths are
relative to `crates/photography-workbench`:

| File | Holds | May depend on |
|---|---|---|
| `src/main.rs` | the binary: sets up logging and the window (its size, and its frame on each system), turns off the menu bar winit makes on macOS by default, whose Quit would end the app without closing the window, has eframe open the graphics card as `src/graphics.rs` says, then hands control to eframe | the library, `eframe`, `env_logger`; on macOS `winit` |
| `src/lib.rs` | the library root; re-exports `PhotographyWorkbenchApp` and `wgpu_setup` | its own modules |
| `src/graphics.rs` | `wgpu_setup`, how eframe opens the graphics card: as egui does, but asking for textures as large as the card reports it supports, rather than egui's 8192 pixels a side, so a photo's largest preview fits in one; on OpenGL, egui's limits as they are | `eframe` |
| `src/dialogs.rs` | the app's dialogs, in the frame `desktop_ui::dialog` gives every dialog: Settings, which holds the theme choice and how the Info dock writes capture times and counts file sizes, each in a drop-down list `desktop_ui::drop_down` draws, under its label, About, which `desktop_ui::about` draws from the app's name, tagline, version, copyright, licence and home page, and the reference of keyboard shortcuts, which `desktop_ui::keys` draws from the keymap's sections | `egui`, `desktop_ui` |
| `src/mode.rs` | `Mode`, the app's modes (Library, Organizer, Develop): each one's name and icon, whether it is built yet, and its `Layout`, the parts of the window it has around its centre | `desktop_ui` |
| `src/view.rs` | `View`, the ways Library shows a folder's photos (a grid of thumbnails, or one photo in the viewer): each one's name and icon | `desktop_ui` (tests only) |
| `src/keymap.rs` | the keyboard shortcuts: one table of what each key combination does (switch mode, switch view, move to another photo, zoom the viewer's photo, show or hide docks, open Settings or the reference, quit), on which surfaces (the grid, the viewer), and the group and description the reference lists it under (Views, Photos, Zoom, Docks, General): Enter opens the viewer from the grid only, and Esc goes back to the grid from the viewer only; Z and Space switch the viewer's photo between fitted and 100 %, Ctrl+= and Ctrl++ zoom it in, Ctrl+- out, Ctrl+0 fits it and Ctrl+Alt+0 shows it at 100 %, in the viewer only; Left and Right move to the previous and next photo in both views, Up and Down to the photo above and below and Home and End to the first and last in the grid only, and those six do it again while their key is held, as the system repeats it, where every other shortcut is done once however long its key is held; the actions they do, which buttons and menu items do too; `take_pressed` and `take_pressed_actions`, which the handlers ask, and which take the combinations that apply out of the frame's input, pressed or held down, so no control drawn after also acts on their keys, `key_combination_for`, the combination a control names in its tooltip and a zoom item of the menu carries, `key_combination_everywhere`, the one any other menu item carries, and the reference's `sections`, built from the same table. A shortcut to a mode not built yet does nothing and is not listed ([ADR015](architecture-decisions/adr015-the-keys.md)) | `egui`, `desktop_ui` |
| `src/menu.rs` | the menu bar at the top of the screen on macOS: the app menu (About, Settings, Quit), View (the views, the viewer's zoom and the docks), Window (the modes) and Help (the reference of keyboard shortcuts), whose items ask for the app's actions; `State`, what the items show of the app, which can be chosen and which are checked, worked out on every system, the zoom's items enabled by the same rule as the toolbar's zoom controls (`viewer::can_zoom`); and `MacMenu`, which hands the menus to the system and passes their clicks on, a stand-in never made elsewhere. Only the key combinations that hold Cmd are the items' key equivalents, so the keys without Cmd stay the window's, and not those of a toggle, which the system would repeat while the keys are held; the zoom's items carry the viewer's combinations, as they can be chosen only there. The system takes an item's key equivalent before the window sees it, even while the item cannot be chosen, so its combination is never done twice, and does nothing where the window's keys would do nothing either | `eframe`, `desktop_ui`; on macOS `log`, `muda`, `raw-window-handle` |
| `src/disk.rs` | `Disk`, which reads the disk off the window's thread: a folder's subfolders and a folder's photos, as `catalog` reads them, each read on a short-lived thread of its own, so a listing that hangs on a network share that has gone holds up no other, and the roots of the folder tree, watched on a thread of their own that reads them every 2 seconds and wakes the window only when they change, so an idle window draws nothing. The window takes the answers at the start of a frame; a thread asks for a frame when it has one. A read of photos carries the number its caller gave it back with its answer. Its caller keeps at most one listing of a folder, and one read of its photos, under way at a time, so a share that hangs holds no more threads however often its folders are opened or chosen; the one exception is a read of photos left hanging on a card taken out, as the card put back is read afresh beside it. With no thread to be had, a read is done on the window's thread rather than never, and the roots are read there once, which the log warns of | `catalog`, `egui`, `log` |
| `src/browse.rs` | `Browser`, which keeps the folder tree fed from disk, once per frame before anything is drawn: it hands the tree what the disk has read, has the disk watch the roots from the first frame on, as memory cards and drives come and go and no system tells a portable app at once, refreshes the tree when the window comes back to the front, so a folder changed meanwhile in the system's file manager is read again, and lists the folders the tree asks for, one read per folder at a time; what is expanded and selected in the tree, saved and restored; and `Photos`, the selected folder's photos and the current one, as far as they have been read, the current one moved to an index, by a step or to either end. The photos are read each time a folder is selected, again when the window comes back to the front, and once the selected folder's root is among the tree's roots, as at start-up, but never by two reads at once: wanted while a read of them is under way, they are that read's answer; they are dropped when the root goes, and a read of them under way then is forgotten, so they are read afresh when the root comes back, beside that read if it hangs, and its answer is dropped. Each read is numbered and only the answer of the one wanted last is taken, so a slow read of a folder left behind never replaces the one chosen since, and the number of the read the photos shown came from tells photos read afresh from those before; a folder just chosen shows nothing until its photos come, and the folder shown read again keeps its photos on screen, then keeps the current photo by its path if it is still there | `catalog`, `eframe`, `egui` |
| `src/folder_tree.rs` | the folder tree in Library's left dock: a row per folder shown, indented by its depth, with the arrow that expands or collapses it, an icon for the home folder, a volume or any other folder, and its name, cut short with "…" where it does not fit and dimmed where the folder cannot be read; a click elsewhere on the row selects it, which reads its photos. Only the rows in view are laid out | `catalog`, `egui`, `desktop_ui` |
| `src/grid.rs` | the thumbnail grid in Library's centre: a 220-point square tile per photo, 1 point apart, as many to a row as fit, showing the photo's name, cut short with "…" where it does not fit, over its thumbnail, fitted below the name with its aspect kept and drawn straight on the grid's backdrop with no fill behind it, or, for a photo that cannot be shown, an icon there, with why in the tile's tooltip, the current one's thumbnail outlined 2 points clear of it, or, while there is none to draw, the room it goes in; a click makes a photo the current one, a double-click opens it too. It draws a list of photos and the current one's index, whatever they were read from, and brings the current photo's row into view, scrolled just far enough that its tile is whole, when the current photo or the list changes or the grid comes back on screen, and only if the tile is not wholly in view already, never because the grid was scrolled, nor after a click, as the photo clicked on is under the pointer (`src/follow.rs` decides when). It tells the app how many tiles it laid out in a row, and which photo is above or below another, for the keys that move a row. Only the rows in view are laid out; it wants the thumbnails of the photos in them decoded the current photo's first, then in reading order, and after those the thumbnails of a screen of rows above them and one below, the nearest first, so they are ready when scrolled to | `catalog`, `egui`, `desktop_ui` |
| `src/filmstrip.rs` | the filmstrip in the viewer's bottom dock: a 120-point cell per photo, side by side in one row running the dock's height, each holding, over its thumbnail, fitted with its aspect kept and drawn straight on the strip's backdrop with no fill behind it, or, for a photo that cannot be shown, an icon there, with why in the cell's tooltip, the photo's name without its extension, cut short with "…" where it would run into the photo's format, its file's extension in capitals ("NEF", "NRW") or "JPEG" for a JPEG file, at the right, the current one's thumbnail outlined 2 points clear of it, or, while there is none to draw, the room it goes in; a click makes a photo the current one. A mouse's vertical wheel scrolls it sideways. It draws a list of photos and the current one's index, whatever they were read from, and brings the current photo into view, in its middle, when the current photo or the list changes or the strip comes back on screen, and only if it is not in view already, never because the strip was scrolled (`src/follow.rs` decides when); a click on a cell leaves the strip where it is, even when the cell is only partly in view. Only the cells in view and four more at each end are laid out; it wants their thumbnails decoded the current photo's first, then those in view, then those of the four at each end, each the nearest the current photo first and, of two as near, the one after it, as the viewer is stepped through from the current photo | `catalog`, `egui`, `desktop_ui` |
| `src/info.rs` | the Info dock, what Library's right dock shows: the current photo's information, as a decode of its file told it, or, with no current photo, a message saying none is selected. The information is in three group boxes, each with a 1-point border in the separator colour and its heading, in the semibold weight, set on the border near the box's top-left corner, the line broken behind the words by a strip of the dock's fill: File Information, the file's name, its size on disk and its format, named as in the filmstrip; Image Information, when the photo was taken, as the camera's clock recorded it, with the fraction of a second and the offset from UTC only where recorded, the image's size (of a RAW file, the visible image the camera recorded, turned upright, or its largest preview's where it records none), for a RAW file the size of its largest preview, which 100 % shows, and how many previews it holds, and whether it is portrait, landscape or square, by its largest image; and Shooting Information, the camera's make and model, the lens, its model or else its range ("24-70 mm f/2.8-4"), the focal length, the exposure in one line ("1/2000 s at f/8", or the part known), the ISO and the exposure bias. A row reads "Label:" at the left and its value at the box's right edge, cut short with "…" where it would come within 12 points of the label and whole in its tooltip then, and is told to screen readers as "Label: value"; a value the file does not record is "—". A JPEG file's EXIF block is not read, so its Shooting Information holds only a note that camera data is read from RAW files only. Until a decode of the photo has told what its file holds, and once the photo cannot be shown, as the views show it, whatever a decode before told, File Information shows the name and format and "—" for the size on disk, a line under it says "Reading…", or, once the photo cannot be shown, why, and the other boxes are left out. Every frame it is shown, once the views of the photos have wanted what they draw, `want` wants the smallest texture of the current photo held, so the details are never let go while it shows them, or, with none held, its smallest thumbnail, at the current photo's tier, but only when no view wanted any texture of the photo in the frame (`Textures::is_wanted`): so the details come even when no view draws the photo, and cost no decode of their own when a view wants it, but that a texture held from before a new read of photos is decoded again when the dock wants it, as when a view does. Each value is written by a function of its own with no egui in it. How a capture time is written, `TimeForm`, and how a size on disk is counted, `SizeUnits`, are chosen in Settings, each a parameter of its one function that the app passes in every frame, so a change shows at once: the numeric form, the date first and year first, as ISO 8601 orders them, then the time of day and the offset from UTC, "2026-08-15 10:20:30.85 UTC+02:00", or the long one, "15 August 2026 at 10:20:30.85 (UTC+02:00)", with the month's English name and the offset in parentheses; decimal units, a kilobyte being 1,000 bytes, "52.3 MB", or binary ones, a kibibyte being 1,024 bytes, "49.9 MiB", rounded alike. Each choice has a name, which Settings lists and the app saves | `catalog`, `previews`, `raw-files`, `egui`, `desktop_ui` |
| `src/viewer.rs` | the viewer in Library's centre: the current photo alone, fitted in the centre less 24 points on every side with its aspect kept, in the middle, enlarged where it is smaller than that, drawn straight on the viewer's backdrop, or, for a photo that cannot be shown, an icon in the middle, with why in its tooltip, and nothing until some texture of it is decoded. Once a decode of the photo has told the size of its largest image, the pointer zooms it, as `src/zoom.rs` works out, and until then does nothing: a click on the photo shows it at 100 %, the point clicked at the room's middle, and a click on it zoomed larger than the room, or at 100 % or past it, fits it again, the photo gliding there, a double-click counting as one click; the mouse wheel, the wheel with Ctrl or Cmd held, and a trackpad's pinch zoom it about the pointer, from the fit up to 800 %, as egui smooths them, the wheel's last event telling a mouse's notches, which zoom, from a trackpad's scrolling, which moves the photo; a drag and a trackpad's two fingers move a photo larger than the room; the keys and the toolbar's zoom controls zoom it too, as `src/zoom.rs` works out, between fitted and 100 %, to either, and in and out by steps, about the room's middle, gliding there as after a click, before the viewer is drawn in the frame, in the room it was drawn in during the frame before, and nothing when it was not drawn then, until it is drawn again, the viewer having egui run the pass again when it is drawn again, so the toolbar reads its new room in the same frame, and, when its room changes while it is shown, as the window is resized or a dock shown or hidden, asking for the next frame, so the toolbar shows an old room's zoom for a frame at most; only the room shows the photo, its margin left clear, and the pointer shows what a click or a drag does there. What it draws is the photo's picture, decoded at the long side of that room in the screen's pixels, rounded up to a step of 256 pixels so resizing the window does not decode it again at every pixel, and no longer than the largest texture the graphics card takes, which egui holds from the first frame on and the viewer asks for every frame; the photo's thumbnail at the largest size thumbnails are decoded at is wanted before the picture, as the quick stand-in, and while the picture is on its way the best texture of the photo held stands in for it. Once the picture is settled, held or known not to decode, or the photo shown as one that cannot be shown, the pictures of the next photo and of the one before are wanted too, at the same size, below what is on screen, so stepping to either shows it sharp at once, and a damaged current photo does not keep them from being made ready. Once the picture is held, or known not to decode, and the photo is drawn longer than it in the screen's pixels, the photo at its largest is wanted too, right after the picture, in its tier: its largest image whole, no longer than the largest texture the card takes; only the current photo's, the picture standing in for it meanwhile and when it cannot be decoded. When the sharpest texture wanted, the picture or the photo at its largest, has been on its way for half a second since the photo or the size wanted last changed, or since the viewer came back on screen, "Loading…" shows in the margin under the photo, a frame being asked for then so an idle window shows it on time; never once the picture is shown, nor when it cannot be decoded and something else of the photo is shown, nor while the photo is shown as one that cannot be shown, its icon staying while a picture size not tried yet is decoded. `State`, how large it shows the photo and which part of it, kept from one photo to the next and read by the toolbar too, so the two never say different zooms, the glide a click, a key or the toolbar began, what the wheel last was, how long it has waited and the room it last drew the photo in, is all it keeps between frames, and the app holds it; `zoom_label`, the zoom as the toolbar says it, a whole percent, "200 %", fitted too, or nothing before the photo's 100 % is known; and `can_zoom`, which zoom changes can be asked for, by which the toolbar's zoom controls and the menu bar's zoom items are both enabled: none while nothing can be zoomed, and zooming in and out only where they would change something. It draws the photo it is handed, whatever it was read from, and names it, by its file name, as an image to screen readers | `catalog`, `previews`, `egui`, `desktop_ui` |
| `src/zoom.rs` | the maths of the viewer's zoom, with nothing of egui's input or drawing in it: `Zoom`, fitted or a scale relative to 100 %, at which each pixel of the photo's largest image takes a pixel of the screen; `Framing`, the zoom and the pan, the point of the photo at the room's middle in fractions of the photo, which says nothing of the photo itself, so it is kept from one photo to the next; and `Stage`, a photo's largest size in a room on a screen, which works out where a framing puts the photo, the pan bounded on each side so a photo larger than the room never has its edge inside the room's and one no larger is in the middle, and how the pointer changes it: zooming about a point of the room, the point of the photo under it staying there, no further in than 800 % and, zoomed out to the fit or below, fitted; a click, which goes between fitted and 100 %; a pan by a drag or a scroll; and `Zooming`, how the keys and the toolbar change it, keeping the point of the photo at the room's middle: between fitted and 100 %, as a click does, to either, or in or out to the next of the steps 25, 50, 100, 200, 400 and 800 % past the scale shown, in whole percent, as the toolbar reads it (`Stage::percent`, a fitted photo's being the fit's), zooming out to fitted rather than to a step no larger than the fit. A photo at 100 % has its corner put on a whole pixel of the screen, so none of its pixels is blurred across two, and the move a click, a key or the toolbar makes is eased in and out over 0.25 seconds ([ADR016](architecture-decisions/adr016-the-viewer-s-zoom.md)) | `egui`, `previews` |
| `src/follow.rs` | when a view of a folder's photos brings the current photo into view: when the current photo or the list changes, or the view was not drawn the frame before, as when it comes back on screen; never because time passed or the view was scrolled ([ADR011](architecture-decisions/adr011-views-navigation-and-the-keymap.md)). What each view last followed, and when it was last drawn, is kept in egui's memory between frames; a photo made current by a click on it counts as followed already | `egui` |
| `src/textures.rs` | `Textures`, the photos, which `previews` decodes on its worker threads, kept as textures, one a photo and size, of two kinds: thumbnails, for the grid and the filmstrip, each decoded at 256, 384 or 512 pixels on its longest side, the smallest that covers what it is drawn at, and pictures, for the viewer, decoded longer than any thumbnail, at the size the viewer shows the photo at on the screen; so each view draws a texture made for it, as the renderer keeps no smaller copies of a texture and one drawn much smaller than its pixels shows jagged, flickering detail. While the size wanted is on its way, another texture of the photo held stands in for it: in the grid and the filmstrip only a thumbnail, never a picture, the nearest larger or else the largest smaller; in the viewer one of either kind, the largest smaller or else the smallest larger, the viewer being told too whether it was shown the picture it wants and whether that is still on its way. Each frame a view wants the photos it draws, and those it soon may, the best first, each at a priority of its own, a tier and a rank within it, then draws them. The tiers, from the highest: the current photo's thumbnail, the current photo's picture and after it, while the viewer is zoomed past it, the photo at its largest, the thumbnails on screen, the pictures of the photos next to the current one, and the thumbnails wanted ahead. Both kinds go to one decoder, which serves the highest priority queued first, and a worker woken by a want takes what is queued at that moment, so whatever it takes is the best wanted so far. Each frame, before anything is drawn, the photos decoded since the last frame become textures, and after everything is drawn the requests of the photos not wanted in the frame are dropped, so scrolling fast leaves no queue behind, and the textures past the budget of their kind, 256 MiB of thumbnails and 512 MiB of pictures, are let go, those wanted least recently first, but never one wanted or drawn in the frame. The decoder wakes the window when a photo is decoded, so an idle window draws nothing. Every decode of a photo also tells what its file holds, whatever size was decoded, which the store keeps with the texture (`Details`): the size of the photo's largest image, a JPEG's own or a RAW file's largest preview, for the viewer's 100 %, and, for the Info dock, the file's size on disk, the sizes of its previews and what the camera recorded; known once any texture of the photo is held, the newest decode's, from before a new read of photos until the photo is decoded again, and gone with the photo's last texture. Why a photo cannot be shown is remembered for each size apart, as its textures are, so it is not decoded again every frame, and a size that cannot be decoded leaves the others to be tried: a picture that cannot be decoded, as from a RAW file whose largest preview is damaged, neither marks the photo as one that cannot be shown in the grid and the filmstrip nor stops its thumbnails being decoded, nor a picture of another size, as once the window is resized, and a thumbnail that cannot be decoded does not stop the picture being tried. A new read of photos, of the folder shown again or of another, starts afresh, as files may have changed: it drops what was wanted before it and every decode under way, whose answer is never handed out, decoding again the few photos under way then; and has each photo whose texture is held, or which could not be shown, decoded again when it is next wanted, what was known of it shown until the new answer comes, so going back to a folder shows its thumbnails at once. How every view draws a texture, fitted in its room with its aspect kept, or, for a photo that cannot be shown, an icon, and the tooltip that says why | `previews`, `raw-files`, `egui`, `desktop_ui` |
| `src/layout.rs` | the parts of the window around a mode's centre: the tool rail, with the one button Library has so far (Info), which shows the right dock, the docks with their sizes and what they hold (so far, the folder tree the app gives the left dock, the photo's information, Info, the app gives the right dock, and the viewer's backdrop under the filmstrip the app gives the bottom dock), the title bar's toggles for them, and `Docks`, which of them are shown, shown or hidden together by a key, saved and restored | `eframe`, `egui`, `desktop_ui` |
| `src/app.rs` | `PhotographyWorkbenchApp`, the `eframe::App`: the window's title bar with its icon (off macOS), mode switcher, settings gear and button for the reference of keyboard shortcuts, the toolbar under the centre with the mode's own controls (Library's view buttons) at its start and, at its end, which photo of how many is the current one and, in the viewer, just before it, the zoom controls: zoom out, the zoom shown as a whole percent, fitted too, the fit's for the room, in a fixed width so the controls never shift, and empty until the photo's 100 % is known, zoom in, fit and 100 %, disabled until the photo's 100 % is known, and zooming in and out where they would change nothing, the centre, which shows the selected folder's photos in the grid or the current photo in the viewer, or says no folder is selected, it cannot be read or it holds no photos, the photos' thumbnails and the viewer's pictures it keeps for the grid, the filmstrip and the viewer, taken from the decoder before anything is drawn and their requests trimmed once the filmstrip and the centre are, and what the viewer keeps between frames, how large it shows the photo and which part of it and how long it has waited for its picture, which is not saved, the filmstrip it gives the viewer's bottom dock, which shows the same photos and the same current one, the order the window's panels are laid out in, the keyboard shortcuts it handles, egui's own keys for quitting and for zooming the whole window turned off, and the menu bar it gives macOS, `perform`, which does each action whatever asked for it, among them the moves to another photo, a row up or down going by the number of tiles the grid laid out in a row the last time it was drawn, and doing nothing from a change of mode, view or docks shown until it is drawn again, the folder tree it keeps fed from disk and shows in Library's left dock, the current photo's information it shows in the right dock, for the photo current as the frame began, as the folder tree holds the photos while it is drawn beside it, the window's icon, the dialogs it opens, the theme choice it saves and applies, how the Info dock writes capture times and counts file sizes, which it saves and passes to the dock every frame, and the mode, docks, view and folder tree it saves | `eframe`, `egui`, `desktop_ui`; on macOS `raw-window-handle`, to measure the system's window buttons |
| `build.rs` | embeds the icon and version information in the Windows executable | `winresource` (build only) |
| `assets/` | the app icon: `icon.svg` is the source, and `scripts/render-icons.sh` renders the PNG and ICO files from it, which are committed | — |
| `tests/package_metadata.rs` | checks that the app's name, tagline and copyright agree across Cargo.toml's packaging sections and `PhotographyWorkbenchApp`'s constants, and that every crate's version is the workspace's | the library, `toml` |

Keeping everything but start-up in the library lets tests drive the UI
without a display: [egui_kittest](https://github.com/emilk/egui/tree/main/crates/egui_kittest)
runs the app headless and queries its widgets through AccessKit. One
test, `screenshots` in `src/app.rs`, runs only when asked for, with
`PW_SCREENSHOTS` naming a folder: it has the graphics card draw the
window at its smallest size, one pixel a point, and writes a PNG file
of each scene (the grid, the viewer, each dialog, a folder with no
photos) in each theme, for people to review the look of every surface,
showing the photos of the folder `PW_SCREENSHOT_PHOTOS` names, or a few
JPEGs made for the run.

### desktop-ui

Paths are relative to `crates/desktop-ui`:

| File | Holds | May depend on |
|---|---|---|
| `src/lib.rs` | the library root | its own modules |
| `src/about.rs` | the About dialog: the app's icon over its name, tagline, version, copyright, licence and a link to its home page, all of which the app passes in | `egui`, its own `dialog` |
| `src/action_bar.rs` | the action bars, the fixed columns of icon buttons along the window's sides: the bar itself, filled like the docks and marked off from them by a line, with buttons at its top and its foot, and the buttons, which show a rounded square behind the icon of the one that is on | `egui`, its own `icons` and `theme` |
| `src/chrome.rs` | the parts of the title bar the app draws itself: its frame and height, the region that drags the window and maximizes it on a double-click, its buttons (one showing an icon, one showing an icon and a word that can be shown selected, and one that shows or hides a dock, its icon a small window with the dock's side filled while the dock is shown; each names in its tooltip the key combination that does what it does, if one does), the close, maximize and minimize buttons, the strips along the window's edges that resize it, and the gap that keeps the bar clear of macOS's window buttons. The window controls and the strips add nothing on macOS, where the system's frame provides them | `egui`, its own `dock`, `icons` and `theme` |
| `src/dialog.rs` | dialogs: the frame every dialog shares (the theme's popup frame with 16 points of room inside), and `show`, which shows one. A dialog closes on Esc or a click outside it, never acting as it closes | `egui` |
| `src/dock.rs` | the docks, the panels along the window's sides and bottom, and the edge each is on: filled to their edges, with a message for when there is nothing to show; shown and hidden by sliding, which keeps the ids of the widgets after a hidden dock as they were; and, where one can be resized, an edge that shows a resize cursor under the pointer but lights up only while it is dragged | `egui`, its own `fonts` |
| `src/drop_down.rs` | drop-down lists: a choice among a few values, each shown by the name the caller gives it, all of one width, so a column of them lines up, as in Settings, the closed list's name cut short with "…" where it is too long for it | `egui` |
| `src/fonts.rs` | the typeface, Inter, regular for text and semibold for headings, installed with the icon font and backed up by egui's own fonts | `egui`, its own `icons` |
| `src/icons.rs` | the icons: Lucide's icon font, given to egui once by `install`, or added to a set of fonts by `add_to`, and an icon by its Lucide name as text (`lucide`), before a word as one run of text for a button (`labeled`), or as a character and a font to paint (`glyph`), with a fallback for a name that does not resolve | `egui`, `iconflow` |
| `src/keys.rs` | the keyboard shortcuts' key combinations: a key with the modifiers held with it, spelled as macOS spells it (⌥⌘1) or as Windows and Linux do (Ctrl+Alt+1), and whether it was pressed in a frame, with exactly its modifiers (Shift aside for a punctuation character the keyboard types with Shift on another key, as `/` is Shift+7 on a German one) and not by a key repeating, or, for a combination that moves through something, also by its key held down repeating, after the system's delay and at its pace; taking a combination out of the frame's input, its key going down and repeating but not coming up, with the same matching, so nothing drawn after sees the key; the tooltip of a control a combination does the work of, which names the combination; and the reference of an app's shortcuts, sections of combinations beside what they do, the descriptions lined up across sections and wrapped where they are long. What a combination does is the app's | `egui`, its own `fonts` |
| `src/style.rs` | the style every part of the window shares apart from its colours: the spacing (the room inside buttons, the gaps between widgets, the height of a control), the sizes of small print and headings, and the pace of its animations, installed once in both of egui's styles so a change of theme keeps it | `egui` |
| `src/theme.rs` | the colour themes: the concrete `Theme`, the saved choice (an `Option<Theme>`, `None` for System) that `resolve` turns into one and `picker` offers in a drop-down list, System first, and each theme's `Palette`, the colours of every part of the window and of the panel drawn over a photo, which are the same in every theme, and how it maps onto egui's visuals (Dark keeps the colours of egui's stock dark visuals as they are, and its palette repeats them), and the rounded corners every theme gives buttons and fields, and the shadows it gives popups and dialogs | `egui`, its own `drop_down` |
| `src/toolbar.rs` | the toolbar, the fixed row of controls under the window's centre: filled like the docks and marked off from the centre by a line, with controls running from both its ends, and its buttons, which look like an action bar's, smaller, and name in their tooltips the key combination that does what they do | `egui`, its own `action_bar` and `theme` |
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
| `src/photos.rs` | `PhotoList`, a folder's photos in natural order of their names, read from disk, and the current one, which there is exactly when the list is not empty and which moves by a step, to an index or to either end, stopping at the ends; `Photo`, a photo's file and its `Kind`, a raw file of one of the formats raw-files reads or a JPEG, told by the extension alone, and its format, for showing: a RAW file's own extension in capitals ("NEF", "NRW"), or "JPEG" for a JPEG file whatever its extension | `raw-files`, its own `entry` and `natural` |
| `src/entry.rs` | which entries of a folder the catalog shows, and whether each is a file or a folder once a symbolic link is followed: not those whose name begins with a dot, nor on macOS and Windows those the system flags hidden, nor a broken link, nor anything that is neither a file nor a folder. It also tells a link from what it points at, for a listing that leaves links out | — |
| `src/natural.rs` | natural order for file and folder names: numbers by their value (`DSC_2` before `DSC_10`), letters whatever their case, and a fixed order for names alike by those rules, so a folder lists the same whatever order the system gave its entries in | — |

### previews

The previews crate decodes a photo on its caller's thread, or on
worker threads of its own for a window, which must never wait. It turns
a photo into an image of its own, 8-bit RGB pixels with no GUI's type,
so it has no GUI dependency and its tests run on a machine with no
display, over files they build themselves: JPEGs encoded in the test
and NEF files written around them. Every file is taken for untrusted: a
damaged or hostile one is a `Failure`, never a panic, and the memory one
file may take is capped. Paths are relative to `crates/previews`:

| File | Holds | May depend on |
|---|---|---|
| `src/lib.rs` | the library root | its own modules |
| `src/decode.rs` | `decode`, a photo's file turned into a `Decoded`: the `Image`, upright, its longer side at most as long as asked, scaled down by area averaging and never up; the `Size`, upright, of what it was scaled down from, and that of the largest image the file holds, a JPEG's own or a RAW file's largest preview, which is what 100 % shows; and what the file tells of itself, read with it at no further cost: its size on disk, from the file already open, and, of a RAW file, the upright sizes of all its JPEG previews, smallest first, and raw-files' `Metadata`, what the camera recorded, such as the camera, the capture time and the exposure. A JPEG file's EXIF block is read for its orientation only, so it tells no previews and no metadata. What the file holds is told by its content, never its name: a file that starts as every JPEG does (`FF D8 FF`) is handed to the JPEG decoder, which reads it whole, up to 128 MiB, and turned upright by its own EXIF orientation; any other is opened with raw-files, which refuses what it does not read, and of its embedded previews the smallest whose longer side is as long as asked is decoded, or the largest when none is, turned upright by the RAW file's orientation. A JPEG may have at most 32,768 pixels on a side, and the pixels of a decode may take at most 512 MiB in all the buffers it holds at once (the decoded image, and the copy scaled, made RGB or turned a quarter, each while the one before it is held), both checked before it is decoded | `image`, `raw-files`, its own `failure` |
| `src/decoder.rs` | `Decoder`, which decodes on worker threads what its caller wants, each request for a slot the caller names, such as a grid's cell, and hands back each `Answer`, with the request it answers, when it is ready, calling a wake function the caller gave it then, outside its lock. A slot has at most one request queued: wanting another replaces it, and wanting what is queued, under way, or answered and not yet taken costs nothing, so a window can want every cell it shows on every frame. The highest priority goes first, and among equals the one first wanted last; wanting a request again sets its priority but keeps its place, so cells wanted every frame are decoded in the order they were first wanted. `retain` drops the queued requests of the slots not kept, `clear` drops every queued request and every answer still to be handed out, those of the decodes under way included. As many workers as cores less one, at least one and at most four, start with the first request. A worker held by a decode whose answer `clear` dropped, as on a network share that has gone, is replaced on the next request, up to twice the workers in all, and leaves once it is free; dropping the decoder stops them after the decode each is in, without waiting for it, so a decode hung on a share does not hold up closing the window, and drops the answers not taken. A decode that panics answers that the file is damaged, and the worker goes on; so does a worker whose wake panics. Each decode may take what `decode` allows one photo, at most 768 MiB (a RAW file's preview, 128 MiB, held twice, and 512 MiB of pixels in all its buffers at once), not counting the JPEG decoder's working memory, and up to twice the workers decode at once, so eight photos with four workers: about 6 GiB at the very worst, which only damaged or hostile files come near. An inline decoder decodes on the caller's thread when it takes the answers, for tests | its own `decode` and `failure` |
| `src/failure.rs` | `Failure`, why a photo could not be shown, in a sentence for the photographer: the file could not be read (with the system's kind of error), is not a photo the app shows, is a RAW file with no preview, is damaged, or is too large; and how each error of the file system, raw-files and the JPEG decoder becomes one | `image`, `raw-files` |
| `src/fixtures.rs` | for the tests only: JPEGs encoded from flat colours, and a small writer of TIFF structures, for a JPEG's EXIF block and for NEF files holding up to two previews and the tags a camera records, in IFD0 and the Exif IFD | `image` |
| `tests/local_corpus.rs` | a test run only when asked for, with `PW_LOCAL_CORPUS` naming a folder of one's own photos: it decodes as a thumbnail every file under it that the app's photo list would show, one of a format raw-files reads or a JPEG, and lists those that fail | the crate |

### How it fits together

**Data flow.** eframe owns the event loop and calls
`PhotographyWorkbenchApp::ui` every frame; egui is immediate-mode, so the UI
is rebuilt from the app's state each time rather than kept as a widget tree.
The only state so far is the theme choice, how capture times are
written and file sizes counted, the mode, Library's view,
which docks are shown, the folder tree, the selected folder's photos,
their thumbnails, the viewer's pictures of them, what each photo's file
holds as their decodes told it, how large it shows
the current one and how long it has waited for its picture, and which
dialogs are open; the window shows its own
title bar, which
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
right one inside the tool rail, holding the current photo's
information, so they run down to the window's
bottom; then the toolbar along the bottom
between them, holding the mode's own controls (in Library, a button per
view) and, in Library, which photo of how many is the current one and,
with the viewer, its zoom;
then, in Library's viewer, the bottom dock above the toolbar, holding
the filmstrip, which the grid has no need of; and last the centre, which
takes what is left: the selected folder's photos as the grid's tiles,
or the current photo in the viewer, or a message saying why
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
frame. The keys of the shortcuts that apply are then taken out of the
frame's input, as egui takes Enter and Space for a click on the control
that has the keyboard focus, which assistive technology can give any
control, and one press would otherwise do two things. The keys stand
down while a dialog is open, all but the reference's own key, which
closes it, and while a text field has the keyboard; the keys standing
down are left in the input, for the dialog or the field. egui's own
Ctrl+Q is turned off, so quitting follows the keys' rules, and so are its
Ctrl+=, Ctrl+- and Ctrl+0, which would zoom the whole window wherever the
viewer's keys do not apply. On macOS the menu bar takes the key
combinations that hold Cmd before the window sees them, even from a text
field, as on any Mac, since they type nothing. Its items stand down while
a dialog is open, all but the reference's own, which closes it; a click is
checked again when it is done, and the frame's last step shows the items
the app as the frame left it, the mode, the view and the docks shown
checked, and the zoom's items enabled as the toolbar's zoom controls are.
A Cmd key combination an item carries is the item's alone, even while the
item cannot be chosen, so of the viewer's zoom keys the menu bar does
Cmd+=, Cmd+-, Cmd+0 and Cmd+Option+0, and the window's keys the others.
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
those of the folder selected last. Next, still before anything is
drawn, the thumbnails and pictures decoded on worker threads since the
last frame become textures, each worker asking for a frame when it has decoded a
photo; a new read of photos, of the folder shown again or of another,
drops what was wanted before it and every decode under way, and has
each photo whose texture is held, or which could not be shown, decoded
again, what was known of it shown until the new answer comes. The
right dock, drawn before the centre, shows what the newest decode of
the current photo held told of its file, and wants nothing decoded for
it until the centre is drawn. The grid draws whatever list of
photos it is handed, each tile with the thumbnail held for its photo,
or nothing until one is, wanting the thumbnails of the rows in view and
of a screen of rows on either side of them; a click on a tile makes its
photo the current one; a double-click opens the viewer the way its keys
and its button do, through `perform`. The filmstrip draws the same
list, thumbnails and
current photo, and a click on a cell makes its photo the current one
too. It brings the current photo into view when the photo changes,
when the list does, or when the strip was not drawn the frame before,
as when the viewer opens or the dock is shown again; it remembers in
egui's memory what it followed and when it was last drawn, so time
passing, or the strip being scrolled, never moves it. The grid
follows the current photo the same way, scrolled just far enough that
the photo's tile is whole, but not after a click on it, as the photo
clicked on is under the pointer. The viewer draws the current photo
alone, fitted in the centre less a margin and enlarged where it is
smaller, from its picture, decoded as long as that room is in the
screen's pixels, rounded up to a step; it wants the photo's thumbnail
first, at the highest priority, as every view wants the current photo,
then the picture, above the thumbnails on screen, and until the picture
comes the best texture of the photo held, as the filmstrip's thumbnail,
stands in. Once the picture is settled, held or known not to decode,
it wants the pictures of the next photo and of the one before too,
below the thumbnails on screen and above those wanted ahead, so
stepping to either shows it sharp at once.
When the picture takes half a second, it says it is loading under the
photo, having asked for a frame for then. Once a decode of the photo
has told the size of its largest image, the pointer zooms it: a click
goes between fitted and 100 %, at which each pixel of that image is a
pixel of the screen, the wheel and a pinch zoom about the pointer up
to 800 %, and a drag or a trackpad's two fingers move a photo larger
than the room; the zoom and the pan are kept from one photo to the
next. The zoom keys and the toolbar's zoom controls go through
`perform` like every other action, before the centre is drawn, and
zoom the photo about the room's middle in the room the viewer was drawn
in during the frame before, and nothing when it was not drawn then; the
toolbar reads the same framing the viewer draws with. Zoomed past the picture, the viewer wants the photo at its
largest too, right after the picture, and draws it once it comes. The keys that move between photos go
through `perform` like every other: Left and Right to the previous and
next photo in the grid and the viewer, Up and Down a row and Home and
End to the first and last photo in the grid, a row being as many tiles
as the grid laid out in one the last time it was drawn, so Up and Down
do nothing from a change of mode, view or docks shown until the grid is
drawn again, its rows then perhaps of another length; each stops at the
first and the last photo. Once the filmstrip and the centre are
drawn, the Info dock, while shown, wants the smallest texture of the
current photo held, so it is never let go while the dock shows what it
told; or, with none held, the photo's smallest thumbnail, at the
current photo's tier, but only when no view wanted any texture of the
photo in the frame, as a view's decode brings the details too. So they
come even when no view draws the photo, and cost no decode of their
own when a view wants it, but that a texture held from before a new
read of photos is decoded again when the dock wants it, as when a view
does. Then the thumbnails not wanted in the frame are no longer decoded,
so scrolling fast leaves no queue behind, and the textures past the
budget of their kind, 256 MiB of thumbnails and 512 MiB of pictures,
are let go, those wanted least recently first, but never one wanted or
drawn in the frame.
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
egui's own settings and the app's theme choice, how capture times are
written and file sizes counted, mode, docks, view and folder tree in a
per-user directory named after the app id, which
`main.rs` sets to the binary's name (for example
`~/Library/Application Support/photography-workbench` on macOS). The theme
choice is stored as text under the key `theme`: the theme's display name, or
`System`; a missing or unrecognised value means System, so renaming a theme
resets users who chose it. How the Info dock writes capture times is
stored under `capture_time`, as `Numeric` or `Long`, and how it counts
file sizes under `file_sizes`, as `Decimal` or `Binary`; a missing or
unrecognised value means numeric or decimal. The mode is stored under
the key `mode`, as its name; a missing or unrecognised value, or a mode
not built yet, means Library. Whether each dock is shown is stored
under `show_left_dock`, `show_right_dock` and `show_bottom_dock`, as
`true` or `false`; anything else, or nothing, means shown. The docks'
sizes are egui's own settings.
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

The structure above is the first release's, laid out as
[ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)
decided: `catalog` and `previews` below the GUI-free line, and the
window drawing what they list and decode. This file is rewritten with
each change to the structure, in the same pull request.

## As the code grows

Each new crate adds a row to the crate table and a section of its own, each
new module a row to its crate's table, and a change that moves data
between modules updates the data flow. A constraint that forces the shape
gets [an ADR](architecture-decisions/README.md).
