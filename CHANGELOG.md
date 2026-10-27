# Changelog

All notable changes to photography-workbench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Releases are numbered `MAJOR.MINOR.PATCH`: while the first part is 0 the
scope is still settling and any release may change what the app does or how
it stores things; the second part goes up when a release adds something a
user can see or do, the third when it only fixes things (the record of that
decision is
[ADR012](docs/architecture-decisions/adr012-version-numbers.md)).

## [Unreleased]

### Changed

- The grid glides to the current photo when it brings it into view, as
  when the arrow keys move to another row or the grid comes back on
  screen, rather than jumping there.

## [0.1.0] - 2026-10-26

The first release: Library, the first of the app's three modes, which
browses folders of the RAW files of Nikon and Sony cameras and of JPEG
files, showing the previews the cameras stored in them and what the
cameras recorded. Releases are source only for now: the app is built
from the release's source archive with Cargo, as the README says. Every
pull request is built and tested on macOS, Linux and Windows, but the
app has been tried by hand on macOS only, so reports from Linux and
Windows are welcome.

### Added

- The app opens a window with a title bar of its own, the same on every
  system. The window opens at 1280 × 800 points the first time and cannot
  be made smaller, so every part of it fits on a handheld's screen; its
  size is remembered between runs.
- Dragging the title bar moves the window, and double-clicking it maximizes
  or restores the window. On Windows and Linux the title bar has the close,
  maximize and minimize buttons, and the window resizes from any edge or
  corner; on macOS the system's own window buttons sit on the title bar.
- The title bar has a mode switcher, for the app's three modes: Library,
  where photos are browsed and culled, and Organizer and Develop, which are
  shown disabled until a later release. The mode is remembered between runs.
- In Library, docks surround the centre of the window: one on the left
  for folders and one on the right for a photo's information, each
  resized by dragging its edge, and, with the viewer, one along the
  bottom for the filmstrip. The side docks are shown or hidden from the
  title bar, and the filmstrip from a button on the toolbar beside the
  zoom; which are shown is remembered between runs. A bar of buttons
  along the right edge picks what the right dock shows: so far Info, the
  current photo's information; a click on Info while the dock shows it
  hides the dock.
- In Library, two buttons on a toolbar under the centre choose how the
  centre shows a folder's photos: as a grid of thumbnails, or one at a
  time in a viewer. The choice is remembered between runs.
- In Library, the left dock shows a tree of the home folder and the
  mounted memory cards and drives. A folder expands from the arrow
  beside it, and a click selects it. Cards and drives appear and
  disappear as they are mounted and unmounted, and a folder changed
  meanwhile is read again when the window comes back to the front. What
  is expanded and selected is remembered between runs.
- In Library, selecting a folder shows its photos, the RAW files of
  Nikon and Sony cameras and JPEG files, in the grid: a tile for each,
  showing its name and its thumbnail, the current one outlined. A
  camera's JPEG files show their thumbnails from the smaller copies the
  camera stores in them, so a folder of them shows about as quickly as
  one of RAW files. A photo that cannot be shown says why when the
  pointer rests on it. A click makes a photo the current one, and a
  double-click, or Enter, opens it in the viewer, which shows it alone,
  as large as the centre allows and sharp at the screen's resolution, or
  why it cannot be shown when the pointer rests on it. While it shows a
  photo, the next and the previous are made ready, so stepping to either
  shows it sharp at once, and "Loading…" shows under the photo when it
  takes a while. The toolbar shows which photo of how many is the
  current one. The photos are read again when the window comes back to
  the front, so photos copied in meanwhile show, photos changed
  meanwhile show as they are now, and the current photo stays the
  current one.
- In Library's viewer, a click on the photo switches between fitted
  and 100 %, where each pixel of the photo's largest image is a pixel
  of the screen, the point clicked brought to the middle. The mouse
  wheel, with or without Ctrl (Cmd on macOS), or a trackpad's pinch,
  zooms about the pointer, from fitted up to 800 %, and a drag, or a
  trackpad's two fingers, moves a photo zoomed larger than the viewer.
  The zoom stays as it is from one photo to the next, and a photo
  zoomed past the screen's size is shown sharp from its largest image.
- In Library's viewer, the keys established photography tools use zoom
  the photo too: Z or Space to switch between fitted and 100 %, Ctrl+=
  (or Ctrl++) to zoom in and Ctrl+- to zoom out, through 25, 50, 100,
  200, 400 and 800 %, Ctrl+0 to fit the photo and Ctrl+Alt+0 to show it
  at 100 %, with Cmd for Ctrl and Option for Alt on macOS. They keep the
  middle of what is shown where it is, and the photo glides there. The
  toolbar shows the zoom as a percentage, fitted too, beside buttons
  that do the same, and the reference of keyboard shortcuts lists the keys
  under Zoom. Elsewhere those keys do nothing; none of them changes the
  size of the whole window.
- In Library, with the viewer, the filmstrip under it shows the
  folder's photos in a row, each with its name and its format, the
  file's extension in capitals (NEF, NRW, ARW) and JPEG for a JPEG file,
  above its thumbnail, the current one outlined; a long name
  is cut short, and the format stays in sight. A photo that cannot be
  shown says why when the pointer rests on it. A click makes a photo
  the current one. A mouse wheel scrolls it
  sideways, and it brings the current photo into view when the current
  photo changes, never while it is being scrolled.
- In Library, Info in the right dock shows the current photo's
  information in three boxes. Shooting Information gives the camera's
  make and model, the lens, the focal length, the ISO, the shutter
  speed, the aperture and the exposure bias. Image Information gives when
  the photo was taken, its size in pixels, whether it is portrait,
  landscape or square, and, for a RAW file, the size of the largest
  preview the file holds, which is what the viewer shows at 100 %, and
  how many previews it holds. File Information gives the file's name,
  its size on disk and its format, the file's extension in capitals
  (NEF, NRW, ARW) and JPEG for a JPEG file. Makers'
  names in the camera and the lens are written as the makers market
  them, "Nikon" rather than the "NIKON" a file records, but "NIKKOR" as
  Nikon writes it, here and on the panel over the viewer's photo. A
  JPEG file's information is read from its EXIF block, as a RAW file's
  is. Anything the file does not record shows as "—", and a value too
  long to fit is cut short and shows whole when the pointer rests on
  it. Until the photo has been read Info says so, and for a photo that
  cannot be shown it says why.
- In Library's viewer, I shows or hides the current photo's information
  on a dark translucent panel at the viewer's top left, in labelled
  rows: when it was taken, the camera, the lens, the image's size in
  pixels, its format and its size on disk, the focal length, and the
  exposure: the ISO, the shutter speed, the aperture and the exposure
  bias, each only where the file records it, written as chosen in
  Settings. A button on the toolbar beside the filmstrip's does the
  same, lit while the panel is turned on, and on macOS so does an item
  of the View menu, checked while it is. The reference of
  keyboard shortcuts lists the key under Views, and whether the panel is
  shown is remembered between runs.
- Keyboard shortcuts, the ones established photography tools use for
  the same things: G and E for the thumbnail grid and the image viewer,
  Enter to open the current photo in the viewer from the grid, Esc from
  the viewer back to the grid, Tab and Shift+Tab to show or hide the
  side docks or all of them, F6, F7 and F8 for the filmstrip
  and the left and right docks, Ctrl+Alt+1 for Library, Ctrl+, for
  Settings and Ctrl+Q to quit, with Cmd for Ctrl and Option for Alt on
  macOS. Ctrl+/, or a button beside the settings gear, opens a
  reference that lists them all, and a button's tooltip names the
  shortcut that does what it does. The shortcuts stand down while a
  dialog is open.
- In Library, Left and Right move to the previous and next photo, in
  the grid and the viewer; in the grid, Up and Down move a row, and
  Home and End go to the first and last photo. They stop at the first
  and last photo rather than wrap around. Holding one of these keys
  goes on moving, once the system's key-repeat delay has passed, at the
  system's pace. The grid scrolls just far enough to show the photo
  moved to, the filmstrip brings it into view, and the reference of
  keyboard shortcuts lists the keys under Photos.
- On macOS, a menu bar at the top of the screen: About, Settings and
  Quit in the app menu, the views, the viewer's zoom and the docks in
  View, the modes in Window, and the reference of keyboard shortcuts in
  Help. The view, the mode and the docks shown are checked, and the
  zoom's items can be chosen when the toolbar's zoom controls can. Its
  items stand down while a dialog is open, as the shortcuts do.
- A settings gear on the title bar opens Settings, which offers Paper,
  Graphite, Nexus and Dark themes, plus System, which follows the OS
  (Paper when it is light, Nexus otherwise). The app starts in Nexus,
  and the theme chosen is remembered between runs.
- Settings offers a choice of how Info writes when a photo was taken,
  "2026-08-15 10:20:30 UTC+02:00" or "15 August 2026 at 10:20:30
  (UTC+02:00)", and of how it counts a file's size, in decimal units,
  "52.3 MB", or in binary ones, "49.9 MiB", where 1 KiB is 1,024
  bytes. Info, and the panel over the viewer's photo, show a new choice
  at once, and both are remembered between runs.
- An About dialog shows the app's version, copyright and licence, and
  links to its home page. On Windows and Linux the app's icon at the left
  of the title bar opens it; on macOS, the app menu.
- The app has an icon, shown on its window, in the taskbar or Dock, and on
  the Windows executable. Under Wayland the desktop shows it only for an
  installed package, which provides the icon through its desktop entry.
- The project is licensed under the GNU General Public License, version 3 or
  (at your option) any later version.

[Unreleased]: https://github.com/kuolas/photography-workbench/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kuolas/photography-workbench/releases/tag/v0.1.0
