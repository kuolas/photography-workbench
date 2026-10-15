# ADR014: The parts of the window

- Status: Accepted
- Date: 2026-10-15
- Issue: #4

## Context

[ADR010](adr010-modes-and-the-window.md) named the parts of the window:
an activity bar on the left and a tool rail on the right, their buttons
on trays; a status bar spanning the window's bottom, with the side docks
standing on it; a bottom dock under the centre; and the current mode's
own controls centred in the title bar. Laying Library out showed where
that cost more than it gave:

- On a 1280 × 800 screen height is the scarce side. A status bar across
  the window, with the side docks stopping above it, takes height from
  the docks, whose lists (the folder tree, a photo's information) are
  long.
- The activity bar switched what the left dock shows, at the cost of a
  click and a column of width. What a culler browses from is one thing:
  the folders on the computer and on its memory cards and drives.
- The controls for how the centre shows the photos (grid or viewer, and
  later the zoom and the thumbnail size) were in the title bar, away from
  the photos, in the one zone whose width changes with what is around it.
- A filmstrip under the grid repeats the grid's own thumbnails.
- A tray is one more surface, in one more colour each theme has to get
  right, under buttons that read as well without it.

The layout photo editors use today answers each of these.

## Decision

We will lay the window out as follows. Everything else in ADR010 stands:
the three modes, the title bar of our own and the same look on every
system, no menu bar in the window and the native one on macOS, the
1280 × 800 minimum, and every colour from the theme's palette.

- **The title bar** holds, at its left, the app's logo (which opens
  About; not on macOS) and the mode switcher; at its right, the shortcut
  reference, the settings gear, a toggle for each dock the current mode
  and view have, and the window controls. Nothing is centred in it.
- **The side docks run from the title bar to the window's bottom edge.**
  The left dock is where photos are browsed from: in Library, the tree
  of the home folder and of the mounted memory cards and drives
  ([ADR011](adr011-views-navigation-and-the-keymap.md)). There is no
  activity bar.
- **The tool rail** runs down the window's right edge, outside the right
  dock, and picks what that dock shows. Its buttons stand on the bar
  itself, with no tray; the one that is on has a rounded square behind
  its icon. In Library, Info heads it: when culling, a photo's
  information matters more than any tool.
- **The toolbar** is a fixed bar under the centre, between the side
  docks, for what the centre shows and how: the mode's own controls at
  its start (in Library, the view buttons), and those of the current view
  at its end as they are built (the zoom with the viewer, the thumbnail
  size with the grid).
- **The bottom dock** stands on the toolbar, between the side docks. In
  Library it holds the filmstrip, at a fixed height, and only with the
  viewer: the grid has no bottom dock, and the title bar no toggle for it.
- **There is no status bar.**
- **A dock holds its contents alone**, with no tab and no title over
  them: each shows one thing at a time, and the tool rail already names
  what the right one shows. A dock with nothing to show says so.

The names, in code and docs alike: a **bar** is a fixed strip of chrome
(the title bar, the toolbar, an **action bar**); an action bar is a
column of icon buttons along the window's side that picks what the dock
beside it shows, and the **tool rail** is the one on the right; a
**dock** is an area that collapses and, at the sides, can be resized.
"Tray", "activity bar" and "status bar" no longer name anything;
"sidebar" and "panel" are still not used for any part.

Rejected: ADR010's layout, for the reasons above; a status bar under the
toolbar (another strip of height, and what it would say has room on the
toolbar); the filmstrip under both views (it repeats the grid); an
action bar on the left beside the folder tree (a click and a column of
width, with nothing to switch to); tabs and titles over the docks (room
taken to name what is already known).

## Consequences

- A dock the current mode or view does not have is left out of the
  frame: in Library's grid the bottom dock is not mounted at all. In its
  place the frame skips the one automatic id the dock would take, so the
  widgets after it keep their ids, and their state, between the grid and
  the viewer. A dock that is only hidden is still collapsed, never left
  out, as in ADR010.
- The toolbar takes the controls the title bar's middle and a status bar
  would have held. With both side docks at their widest, at the
  smallest window, it is about 390 points wide, and its controls must fit
  that.
- [ADR013](adr013-a-workspace-of-small-crates.md) lists trays among the
  window parts `desktop-ui` holds; there are none.
