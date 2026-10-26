# ADR017: Showing and hiding the docks

- Status: Accepted; amended by
  [ADR019](adr019-the-photo-s-information-on-the-toolbar.md), which
  narrows the side docks for the photo's information's toggle
- Amends: [ADR014](adr014-the-parts-of-the-window.md), in one point:
  the filmstrip's toggle is on the toolbar, not in the title bar
- Date: 2026-10-25
- Issue: #4

## Context

[ADR014](adr014-the-parts-of-the-window.md) put a toggle for each dock
the current mode and view have in the title bar, and made the tool
rail's buttons pick what the right dock shows. Using Library showed
two things against that:

- The filmstrip's toggle sat at the window's top right, far from the
  filmstrip along the bottom, and came and went with the view, moving
  the title bar's other buttons by its width. The toolbar is where what
  the centre shows, and how, is chosen, and the filmstrip is part of
  how the viewer shows the photos.
- A click on the tool rail's button that is on did nothing, where the
  side buttons of established photography tools hide their panel, and
  photographers reach for them to put the panel away.

## Decision

We will put the filmstrip's toggle on the toolbar, in the viewer only,
just before the zoom controls and set apart from them. The title bar
keeps the side docks' toggles.

A click on the tool rail's button that is on hides the right dock; a
click while the dock is hidden shows it, with what the button picks. A
button is on only while the dock shows what it picks, so a dock hidden
by a key leaves every button off.

The keys ([ADR015](adr015-the-keys.md)) and the macOS View menu's items
stay as they are.

So that the toolbar holds all this at the smallest window with both
side docks at their widest, the right dock can be dragged to 410
points at most.

Rejected: the filmstrip's toggle in the title bar, for the reasons
above; the side docks' toggles on the toolbar too (the toolbar runs
between the side docks and serves the centre, and they would take room
the zoom and the thumbnail size need); a rail button that only shows
its dock; a click that brings back every dock Tab or Shift+Tab hid (it
would have to remember what they hid, and the button and F8 would then
differ).

## Consequences

- The title bar's buttons no longer move when the view changes.
- At its narrowest the toolbar is about 440 points wide, and holds the
  view buttons, the filmstrip's toggle, the zoom controls and a count
  of four digits.
- The right dock is 50 points narrower at its widest, so more of Info's
  long values are cut short.
- ADR014 is amended in that one point; the rest of it stands.
