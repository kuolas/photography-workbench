# ADR018: raw-files reads JPEG files too

- Status: Accepted
- Amends: [ADR009](adr009-raw-files-reads-raw-files.md), in one point:
  plain JPEG files are read through raw-files, not opened directly
- Date: 2026-10-26
- Issue: #4

## Context

[ADR009](adr009-raw-files-reads-raw-files.md) decided that plain JPEG
files are opened directly, since the JPEG decoder is there for the RAW
files' previews anyway. So the app decoded a JPEG file whole, whatever
size it was shown at, and read nothing of its EXIF block but its
orientation: the Info dock and the panel over the viewer's photo showed
no capture time, camera, lens or exposure for it.

Two things changed that:

- raw-files reads JPEG files as a format of its own since its version
  0.4.0: what the camera recorded in the EXIF block, read as in a RAW
  file, and the images the file holds, listed as a RAW file's previews
  are: the file's own picture, and the smaller copies of it a camera
  stores, a thumbnail in the EXIF block, larger ones listed in a
  Multi-Picture Format (MPF) index, and on some Nikon bodies one in the
  MakerNote.
- Decoding a camera's JPEG whole for a thumbnail costs far more than
  decoding one of those copies: on a folder of a full frame camera's
  JPEGs, of 8256 × 5504 pixels, which store a 640 × 424 copy beside the
  picture, from twenty-five to fifty times as long, by the thumbnail's
  size. A folder of JPEGs filled the grid many times more slowly than a
  folder of RAW files.

The options considered:

- **Keep opening JPEG files directly**, and read their EXIF block and
  MPF index in the app. That is a parser of the app's own, which
  ADR009 rejected for RAW files: the work is the library's.
- **A faster decoder, or one that scales while it decodes.** Either
  still reads every block of the whole picture, where a copy a tenth of
  its size holds a hundredth of its pixels.
- **raw-files for JPEG files as for RAW files.**

## Decision

We will read JPEG files through raw-files, as RAW files are read: what
the camera recorded comes from it, and so do the images to decode.

- Of a JPEG's images, the smallest whose longer side is as long as the
  size asked for is decoded, as of a RAW file's previews, or else the
  file's own picture. Only copies smaller than the picture count, so an
  image the file lists that is larger than its picture is never
  decoded.
- Only copies of the picture's shape count, to within 3 %, as cameras
  round the sides of their copies, so a copy of another shape is never
  shown, such as a 4:3 thumbnail of a 3:2 picture or a copy that an
  export turned a quarter left as it was.
- 100 % stays one pixel of the JPEG's own picture to a pixel of the
  screen, as [ADR016](adr016-the-viewer-s-zoom.md) decided, whichever
  image was decoded.
- Every image is turned upright by the orientation of the file's EXIF
  block, as raw-files reads it: cameras store their copies turned as
  they store the picture.
- Only the copy chosen is tried. One that cannot be decoded, damaged,
  too large or coded in a way the decoder does not read, gives way to
  the file's own picture, so a JPEG whose copies were cut short still
  shows; only a file that cannot be read stops it. No other copy is
  tried, so a file listing thousands of damaged copies costs one.
- The file's own picture is decoded from the file, not from a copy of
  its bytes that raw-files reads, so its bytes are held once.
- A file that starts as a JPEG but that raw-files refuses is decoded
  whole, as before, and tells nothing of what the camera recorded, so
  every JPEG the app showed it still shows.

ADR009's other points stand: the formats the app opens are those
raw-files reads, and what the app needs of a file is asked of raw-files.

**Rejected:** reading the EXIF block and the MPF index in the app, for
the reason ADR009 gives; a faster or scaling decoder, as the speed-up
is in decoding fewer pixels, not in decoding them faster.

## Consequences

- A camera's JPEG shows its capture time, camera, lens and exposure as
  a RAW file does, and its thumbnails come about as quickly.
- A JPEG that carries no copies of its picture, such as one an editor
  exported, is decoded whole for every size, as before.
- What raw-files lists as a JPEG's images is what the app may decode:
  every image of the MPF index counts, so a phone's JPEG whose index
  lists a gain map, a grey image beside the picture, may show that map
  as its thumbnail until raw-files leaves such images out
  (kuolas/raw-files#53).
- A stale copy of the picture's own shape cannot be told apart from a
  true one: an export turned a half turn, flipped or cropped to the same
  shape that keeps the camera's copies as they were, as `jpegtran
  -rotate 180 -copy all` keeps a D70's MakerNote preview, may show a
  thumbnail of the picture as the camera took it, not as exported.
- The viewer's picture of a JPEG, sized to the screen, comes from a copy
  where one is long enough, so zooming past it decodes the file whole
  only once; on a screen larger than the copies, the picture is decoded
  from the file whole too, and zooming in decodes it again (#17).
