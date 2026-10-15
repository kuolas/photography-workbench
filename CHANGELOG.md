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
  of the title bar opens it.
- The title bar has a mode switcher, for the app's three modes: Library,
  where photos are browsed and culled, and Organizer and Develop, which are
  shown disabled until a later release. The mode is remembered between runs.
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