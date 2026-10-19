# ADR016: The viewer's zoom

- Status: Accepted
- Date: 2026-10-19
- Issue: #4

## Context

A photographer culling a shoot looks at each photo whole, then up close
to check its focus, and moves on, often through a burst of near-identical
frames. The viewer has to show a photo fitted, at 100 %, and anywhere
between, and to move about a photo larger than the screen.

What the app can show of a photo is what it decodes: a JPEG file's own
image, or one of the previews a RAW file embeds, never the sensor data
([ADR009](adr009-raw-files-reads-raw-files.md)). A RAW file holds several
previews, from a small one to one nearly as large as the sensor: a full
frame camera's can be 8256 × 5504 pixels. Decoding the largest takes a
noticeable fraction of a second and a texture of a few hundred megabytes,
and drawing it much smaller than its pixels shows jagged detail, as the
renderer keeps no smaller copies of a texture.

egui asks every graphics card for textures of 8192 pixels a side, whatever
the card supports, and has keys of its own, Ctrl+=, Ctrl+- and Ctrl+0,
that zoom the whole window ([ADR015](adr015-the-keys.md)).

## Decision

We will define **100 %** as one pixel of the photo's largest image, a
JPEG's own or a RAW file's largest preview, to one physical pixel of the
screen. Every decode tells the size of that image, whichever preview it
decoded, so 100 % is known from the first thumbnail and does not change
as larger decodes arrive. At 100 % the photo's corner is put on a whole
pixel.

We will ask the graphics card for textures as large as it reports it
supports, never more, so the largest previews fit in one texture. The app
needs a card of the Vulkan, DirectX 12 or Metal class, every one of which
supports 16384 pixels a side. On OpenGL egui's limits are kept.

We will decode the photo the viewer shows at two sizes: a **picture** the
size of the viewer in the screen's pixels, rounded up to a step of 256 so
resizing the window does not decode it again at every pixel, and, only
once the photo is zoomed past that picture, its **largest** image. A
thumbnail stands in until the picture comes, and "Loading…" shows when
that takes half a second. Once the current photo's picture is settled,
the next and the previous photos' pictures are decoded too. The
pictures share the thumbnails' store and decoder, so one order of
priorities holds across both, but keep a budget of memory of their own,
and are never handed to the grid or the filmstrip.

We will zoom as the established photography tools do:

- **Fit** fills the viewer less a 24-point margin, enlarging a photo
  smaller than that. The zoom goes from the fit up to 800 %, and the keys
  and buttons step through 25, 50, 100, 200, 400 and 800 %.
- **A click** switches between fitted and 100 %, bringing the point
  clicked to the middle; the photo glides there over 0.25 s.
- **A mouse wheel's notches**, the wheel with Ctrl or Cmd, and a
  trackpad's pinch zoom about the pointer; **a drag** or a trackpad's two
  fingers pan, never so far that the photo's edge comes inside the
  viewer's.
- **The keys**, in the viewer: Z or Space between fitted and 100 %,
  Ctrl+= (or Ctrl++) and Ctrl+- in and out by a step, Ctrl+0 to fit and
  Ctrl+Alt+0 to 100 %, keeping the middle of what is shown where it is.
  egui's own zoom keys are turned off, as ADR015 foresaw.
- **The toolbar** shows the zoom as a whole percent, fitted included,
  with buttons doing the same as the keys, and the macOS menu bar's View
  menu holds the same four commands with the same keys.
- **The zoom and the pan stay** as they are from one photo to the next,
  so a burst can be checked at the same spot one key at a time.

Rejected: 100 % as one pixel of whatever was decoded (it would change
with the screen and the texture limit), or as one pixel to a point (on a
high-density screen each pixel would cover four, too soft to judge
focus); decoding only the largest image (too slow and too heavy to step
through a folder, and jagged when fitted); a store of the viewer's own
with its own decoder (two pools of threads with no common order);
fitting a small photo no larger than 100 % (small photos would look
lost, and fit and 100 % would be the same); the wheel panning instead of
zooming; going back to fit on every photo; a zoom to 100 % only while a
key is held.

## Consequences

- The zoom percent is a fact about the photo, the same on every screen
  and graphics card, and the Info dock can say what 100 % is of.
- A card that reports less than the largest preview's size shows that
  preview slightly scaled at 100 %; such cards are below the stated
  requirement.
- Zoomed in, stepping to another photo decodes its largest image as it
  is shown, so each zoomed step starts soft for a moment; the neighbours
  decoded ahead are picture-sized only.
- Several large previews decoded at once hold a few hundred megabytes of
  memory until they are scaled down.
- On macOS the menu takes its keys before the window, even from an item
  that cannot be chosen, so the items are enabled by the same rule as
  the toolbar's controls, and a held Cmd+= keeps zooming in as the
  system repeats it, where the keys elsewhere zoom once per press.
- egui's keys for zooming the whole window are gone everywhere; the
  window is never zoomed by the keyboard.
