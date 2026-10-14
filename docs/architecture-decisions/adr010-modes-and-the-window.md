# ADR010: Three modes in one window with its own title bar

- Status: Accepted
- Date: 2026-10-14
- Issue: #4

## Context

A photographer's work with a folder of RAW files falls into three jobs that
want three different screens: browsing and culling, bringing files in from
a memory card and renaming them, and developing a RAW file into a picture.
One app can hold all three, and the question is how they share a window,
and what that window is made of, so that the layout stays the same as the
app grows and across macOS, Linux and Windows.

A desktop app usually gets its window frame, title bar and menu bar from the
operating system, and each system draws them differently. An app can
instead draw its own title bar and put the app-wide controls on it, which
is what code editors and photo tools do today; the cost is drawing the
window controls and handling resizing where the system no longer does.

The smallest screen a photographer culls on is a handheld's 1280 × 800
display.

## Decision

We will build the app as **three modes in one window**, switched from a
**title bar of our own**, which looks the same on every system.

- **The modes** are **Library** (browse and cull), **Organizer** (bring
  files in from memory cards, rename them by their metadata) and
  **Develop** (develop a RAW file). Each mode decides what the window's
  parts hold. 0.1.0 ships Library; the other two are shown in the mode
  switcher, disabled, with a tooltip saying they come later, so the
  window's map is stable from the first release.
- **The title bar** holds, left to right: the app's logo (which opens
  About), the mode switcher, the current mode's own controls centred, and
  on the right the shortcut reference, the settings gear, one toggle per
  collapsible dock, and the window controls. It is the window's drag
  region; double-clicking it maximizes the window. **There is no menu bar
  in the window.** On macOS the app installs the native menu bar at the
  top of the screen, since a Mac app without one is wrong there, and About
  moves into it; the menu items drive the same commands as the title bar
  and the keyboard.
- **The same look on every system.** On Windows and Linux the system's
  frame is turned off, and the app draws the close, maximize and minimize
  controls and handles resizing at the window's edges. On macOS the
  system's frame stays but its title bar is hidden and the content extends
  under it, so the system's three window buttons sit on our title bar as
  the window controls (Apple's own convention for such apps).
- **The parts of the window** and their names, used in code and docs
  alike: a **bar** is a fixed strip of chrome, a **dock** is a resizable
  area that collapses, a **tray** is the plate an action bar's buttons sit
  on. The window has a title bar and a **status bar** that span its width;
  between them, from the left: the **activity bar** (icon buttons that pick
  what the left dock shows), the **left dock**, the centre, the **right
  dock**, and the **tool rail** (icon buttons that pick what the right dock
  shows); a **bottom dock** sits under the centre, between the side docks.
  The bars never collapse; the docks do, from their title-bar toggles and
  the keyboard. "Sidebar" and "panel" are not used for any part.
- **The window never shrinks below 1280 × 800**, and every layout is
  designed and tested at that size: nothing wraps, overflows or hides
  there. A first run opens at that size.
- **Colours come from the theme's palette** (one named colour per part),
  never from a literal in a widget, so every theme paints every part.

Rejected: the system's own title bar and menu bar on each platform (three
different windows, and no room for a mode switcher where the user looks
for it); a menu bar drawn in the window on every system (one more strip
on a small screen, and wrong on macOS, where menus belong at the top of
the screen); tabs or separate windows per mode (a photographer moves
between the three jobs on one folder).

## Consequences

- Every mode gets its table: which bars it mounts and what its docks
  hold. Adding a mode means filling that table, not rearranging the
  window.
- The docks are always mounted and only collapsed, never left out of the
  frame, so widget identities stay stable when one is hidden.
- The title bar is laid out so that its three groups fit at 1280 points
  of width with the mode switcher's three names and the window controls
  present; a fourth mode would need a narrower switcher.
- Drawing the window controls and handling edge resizing is our code on
  Windows and Linux; macOS keeps the system's. Headless tests cover the
  parts' presence and behaviour; how they look is checked by eye at the
  minimum size.
- The macOS menu bar is a second surface for the same commands, so every
  command has one implementation that both the menu and the title bar
  call, and a test drives it on every platform.
