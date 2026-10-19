# ADR015: The keys

- Status: Accepted
- Date: 2026-10-15
- Issue: #4

## Context

[ADR011](adr011-views-navigation-and-the-keymap.md) decided that every
shortcut is a row in one table, and that the keys follow what
photographers' hands already know. It took them from the browsers that
camera makers ship: Alt+0 for the grid and Alt+9 for the viewer, I for
the photo's information, and `?` or F1 for the reference.

Building the keymap showed that those browsers are not where most
photographers' habits come from. Each camera maker's browser has keys
of its own, and they change between brands and between versions. The
photo-management and editing tools photographers spend their working
days in agree with each other on most keys, and a culler moving between
those tools and this app should not have to think. In those tools I
cycles the information shown over the photo, and F1 opens their online
help, so ADR011's own rule, that a familiar key must not do something
else here, rules both out.

## Decision

We will take each key from the established photography tools where they
have one for the same action, from other photo editors where they do not,
and choose our own only where neither has one. Cmd stands for Ctrl and
Option for Alt on macOS, and a key combination is spelled the way each
system spells it: ⌥⌘1 on macOS, Ctrl+Alt+1 elsewhere. The keys today:

| Action | Keys | Where |
|---|---|---|
| Thumbnail grid, image viewer | G, E | Library |
| Back to the thumbnail grid | Esc | the viewer |
| Show or hide the side docks, all the docks | Tab, Shift+Tab | Library |
| Show or hide the filmstrip | F6 | the viewer |
| Show or hide the left dock, the right dock | F7, F8 | Library |
| Library, Organizer, Develop | Ctrl+Alt+1, 2, 3 | everywhere |
| Settings | Ctrl+, | everywhere |
| The reference of keyboard shortcuts | Ctrl+/ | everywhere |
| Quit | Ctrl+Q | everywhere |

- A key for a mode not built yet does nothing, and the reference leaves
  it out until the mode is built.
- The reference also opens from a button beside the settings gear, and
  every button a key also reaches names it in its tooltip.
- The keys stand down while a dialog is open, all but the reference's
  own, which closes it, and while a text field has the keyboard.
- egui has keys of its own, outside the table. Its Ctrl+Q is turned
  off, so quitting follows the table's rules everywhere but on macOS,
  where the system's app menu takes Cmd+Q before the window sees it,
  until the app's own menu bar there
  ([ADR010](adr010-modes-and-the-window.md)) replaces it. Its Tab and
  Shift+Tab move the keyboard focus between widgets; here they show and
  hide the docks and move no focus. Its Ctrl+=, Ctrl+- and Ctrl+0 zoom
  the whole window, and stay on until the viewer's zoom is built.
- The keys still to come (moving between photos, the viewer's zoom,
  ratings) are chosen by the same rule when the features they drive are
  built.

The rest of ADR011 stands: the one table, the handlers that ask it, the
reference built from it, and the tests that keep them in line. Its
choice of keys is superseded by this record.

Rejected: the camera makers' keys (they differ between brands, and fewer
photographers have them in their hands); `?` or F1 for the reference
(F1 is help elsewhere, and `?` is Shift and a key that differs between
keyboard layouts); letting egui's own Ctrl+Q stand beside the table (it
quits even while a dialog is open).

## Consequences

- The viewer's zoom keys in those tools, Ctrl+= and Ctrl+-, are egui's
  keys for zooming the whole window; egui's are turned off when the
  viewer's zoom is built.
- Ctrl+Alt is AltGr on some Windows keyboards, where Ctrl+Alt+1 types a
  character instead; the mode switcher's buttons are the way to a mode
  there.
- The function keys need Fn on many laptop keyboards, as they do in the
  tools these keys come from.
