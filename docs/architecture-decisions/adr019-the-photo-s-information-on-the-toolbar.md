# ADR019: The photo's information on the toolbar

- Status: Accepted
- Amends: [ADR017](adr017-showing-and-hiding-the-docks.md), in one point:
  how wide the side docks can be dragged
- Date: 2026-10-26

## Context

The viewer can show the photo's information over the photo: when it
was taken, the camera, the lens, the image and the exposure. The key I
shows or hides it, and so does an item of the View menu on macOS, but
nothing on the screen did: on Windows and Linux, which have no menu
bar, a user who did not know the key could not find it. Established
photography tools put this toggle on the toolbar under the photo, lit
while the information is shown.

[ADR017](adr017-showing-and-hiding-the-docks.md) sized the side docks so
that the toolbar, at the smallest window with both side docks at their
widest, holds the view buttons, the filmstrip's toggle, the zoom
controls and a count of four digits, in about 440 points. One more
button does not fit in that.

## Decision

We will put a toggle for the photo's information on the toolbar, in the
viewer only, just before the filmstrip's toggle, shown selected while
the information is shown. The two sit together, as both choose what the
viewer shows with the photo: the filmstrip under it, the information
over it. It does what I does, and names I in its tooltip.

So that the toolbar holds it, the side docks give up 20 points each at
their widest: the left dock can be dragged to 360 points at most, and
the right dock to 390.

Rejected: the toggle just after the view buttons (they choose how the
photos are shown, not what the viewer shows with the photo); taking the
40 points from one dock alone (either dock's long names or values would
be cut short much sooner, where 20 points from each are hardly seen).

## Consequences

- The photo's information can be found and turned on with the pointer
  on every system.
- At its narrowest the toolbar is about 480 points wide, and holds the
  view buttons, the information's toggle and the filmstrip's, the zoom
  controls and a count of four digits.
- Both side docks are 20 points narrower at their widest, so long folder
  names and long values in Info are cut short a little sooner.
- ADR017 is amended in that one point; the rest of it stands.
