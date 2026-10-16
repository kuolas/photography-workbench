# Changelog

All notable changes to photography-workbench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Releases are numbered `MAJOR.MINOR.PATCH`: while the first part is 0 the
scope is still settling and any release may change what the app does or how
it stores things; the second part goes up when a release adds something a
user can see or do, the third when it only fixes things (the record of that
decision is
[ADR012](docs/architecture-decisions/adr012-version-numbers.md)).
Nothing has been released yet.

## [Unreleased]

### Added

- The app opens a window with a title bar of its own, the same on every
  system, holding a settings gear. The gear opens Settings, which offers
  Paper, Graphite, Nexus and Dark themes, plus System, which follows the OS
  (Paper when it is light, Graphite otherwise); the window size and theme
  are remembered between runs.
- An About dialog shows the app's version, copyright and licence, and
  links to its home page. On Windows and Linux the app's icon at the left
  of the title bar opens it; on macOS, the app menu.
- The title bar has a mode switcher, for the app's three modes: Library,
  where photos are browsed and culled, and Organizer and Develop, which are
  shown disabled until a later release. The mode is remembered between runs.
- In Library, docks surround the centre of the window: one on the left
  for folders and one on the right for a photo's information, each
  resized by dragging its edge, and, with the viewer, one along the
  bottom for the filmstrip. Each is shown or hidden from the title bar;
  which are shown is remembered between runs. A bar of buttons along the
  right edge picks what the right dock shows. The right dock and the
  filmstrip are empty so far: the right dock says no photo is selected.
- In Library, the left dock shows a tree of the home folder and the
  mounted memory cards and drives. A folder expands from the arrow
  beside it, and a click selects it. Cards and drives appear and
  disappear as they are mounted and unmounted, and a folder changed
  meanwhile is read again when the window comes back to the front. What
  is expanded and selected is remembered between runs.
- In Library, selecting a folder shows its photos, the RAW files of
  Nikon and Sony cameras and JPEG files, in the grid: a tile for each,
  showing its name until thumbnails come, the current one outlined. A
  click makes a photo the current one, and a double-click opens it in
  the viewer, which shows its name for now. The toolbar shows which
  photo of how many is the current one. The photos are read again when
  the window comes back to the front, so photos copied in meanwhile
  show, and the current photo stays the current one.
- In Library, two buttons on a toolbar under the centre choose how the
  centre shows a folder's photos: as a grid of thumbnails, or one at a
  time in a viewer. The choice is remembered between runs.
- Keyboard shortcuts, the ones established photography tools use for
  the same things: G and E for the thumbnail grid and the image viewer,
  Esc from the viewer back to the grid, Tab and Shift+Tab to show or
  hide the side docks or all of them, F6, F7 and F8 for the filmstrip
  and the left and right docks, Ctrl+Alt+1 for Library, Ctrl+, for
  Settings and Ctrl+Q to quit, with Cmd for Ctrl and Option for Alt on
  macOS. Ctrl+/, or a button beside the settings gear, opens a
  reference that lists them all, and a button's tooltip names the
  shortcut that does what it does. The shortcuts stand down while a
  dialog is open.
- On macOS, a menu bar at the top of the screen: About, Settings and
  Quit in the app menu, the views and the docks in View, the modes in
  Window, and the reference of keyboard shortcuts in Help. The view, the
  mode and the docks shown are checked. Its items stand down while a
  dialog is open, as the shortcuts do.
- The window opens at 1280 × 800 points the first time and cannot be made
  smaller, so every part of it fits on a handheld's screen.
- Dragging the title bar moves the window, and double-clicking it maximizes
  or restores the window. On Windows and Linux the title bar has the close,
  maximize and minimize buttons, and the window resizes from any edge or
  corner; on macOS the system's own window buttons sit on the title bar.
- The app has an icon, shown on its window, in the taskbar or Dock, and on
  the Windows executable. Under Wayland the desktop shows it only for an
  installed package, which provides the icon through its desktop entry.
- The project is licensed under the GNU General Public License, version 3 or
  (at your option) any later version.