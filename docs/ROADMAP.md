# Roadmap

The big picture: what the app aims for, what it does so far, and what
comes next. Bugs and smaller requests live in the
[issue tracker](https://github.com/kuolas/photography-workbench/issues).

## Where we are

The first release, 0.1.0, ships Library's first capability, Browse fast:

- **A folder tree** — the home folder and every memory card and drive
  mounted, to choose a folder from.
- **A grid of thumbnails** — the folder's photos, shown from the previews
  cameras store in their files.
- **A viewer with a filmstrip** — one photo at a time, fitted or zoomed,
  with the folder's photos in a row below.
- **The photo's information** — what the camera recorded, beside the
  photo or over it.
- **The keyboard** — the views, the photos, the zoom and the docks,
  driven from it.
- **A window of its own** — the same on macOS, Linux and Windows.

It was built one pull request at a time under
[issue #4](https://github.com/kuolas/photography-workbench/issues/4), and
the decisions that shape it are recorded in the
[architecture decisions](architecture-decisions/README.md). Next in
Library is rating sharpness, where the photographer focused; picking the
keepers, which needs a decision on where ratings live
([open questions](#open-questions)), comes after it.

## Three modes

The app is one window with three modes, each for one of a photographer's
jobs ([ADR010](architecture-decisions/adr010-modes-and-the-window.md)):

- **Library** — browse a folder of RAW files and cull it.
- **Organizer** — bring photos in from a memory card and rename them by
  their metadata.
- **Develop** — develop a RAW file into a picture.

0.1.0 ships Library. The other two are in the mode switcher from the
start, disabled until they exist.

## Capabilities

What the app gives is grouped by mode into seven capabilities. A
capability is a lasting area of work, not a step towards a finish: each
grows with every release, as Browse fast did after 0.1.0, so none is
ever done.

### Library

- **Browse fast** — *release 0.1.0.* A folder tree of the home folder and
  every mounted volume; a thumbnail grid or a single-photo viewer in the
  centre; a filmstrip below; the photo's information on the right; all
  driven from the keyboard. RAW files show the JPEG preview the camera
  embedded, read without decoding the sensor data.
- **Pick keepers.** Mark photos as keepers or rejects, and filter by it.
- **Rate sharpness.** Score each photo's sharpness where the photographer
  focused, and sort or filter by it.
- **Rate composition.** Score each photo's framing.

### Organizer

- **Bring photos in.** Copy a memory card's photos into a folder scheme,
  verify the copy, and show what arrived.
- **Rename by EXIF.** Name files from their capture date, camera and
  other metadata, with a preview of every new name before anything
  changes on disk.

### Develop

- **Develop a RAW file.** From the sensor data to a picture: white
  balance, exposure, tone, colour and the rest, without touching the
  original.

Which capability comes next is a plan, not a promise; a change of
direction gets an ADR.

## Following raw-files

RAW files are read by [raw-files](https://crates.io/crates/raw-files)
([ADR009](architecture-decisions/adr009-raw-files-reads-raw-files.md)),
whose capabilities unlock the ones above:

| raw-files capability | What it unlocks here |
|---|---|
| Previews: the JPEG previews a NEF or an ARW file embeds (released as 0.1.0) | Browse fast |
| Metadata: capture time, camera, lens, exposure, orientation, sizes (released as 0.2.0), and the camera's settings, such as the white balance (released as 0.5.0); open, every value a file records | The Info dock's camera data; later, the camera's settings beside it, Rename by EXIF and sorting by capture time |
| Focus points: where the camera focused, in image coordinates (released as 0.3.0), and a DSLR's point placed from a table of the body's points, with the points the camera used (released as 0.6.0) | Rate sharpness |
| JPEG as a format of its own: a plain JPEG file's metadata and the smaller copies of its picture (released as 0.4.0) | A camera JPEG's camera data in the Info dock, and its thumbnails as quick as a RAW file's |
| Raw sensor data, and the developed image: a linear image with its colour model (not started) | Develop a RAW file, and sharpness scores at full resolution for files whose largest preview is small |

The cameras the app can open are the ones raw-files reads: Nikon NEF and
Sony ARW first, then DNG, Canon and Fujifilm in raw-files' order. Plain
JPEG files are read by raw-files too
([ADR018](architecture-decisions/adr018-raw-files-reads-jpeg-files-too.md)).

## Smaller things after 0.1.0

- A grid whose tiles stretch to fill its width, and a control for the
  size of the thumbnails.
- A full-screen view, and comparing two or four photos side by side.
- Sorting by capture time.
- A thumbnail cache on disk, so a folder opens as fast the second time.
- Packaged builds: an AppImage and a Flatpak on Linux, a disk image on
  macOS, an installer on Windows. Until then releases are source only.

## Open questions

- **Where ratings live.** XMP sidecar files (readable by other photo tools),
  a local catalog database, or both.
- **Application storage.** 0.1.0 keeps only its preferences, in the GUI
  toolkit's own key-value store. A settings file, the catalog store (a
  SQLite database, or plain JSON documents) and recovery after a crash
  get an ADR when keepers need to persist.
- **Composition scoring.** Rule-based heuristics or a machine-learning
  model; a model must run locally and be GPLv3-compatible.
- **The Windows installer.** An MSI, or an Inno Setup installer.

Ideas and use cases are welcome as
[feature requests](https://github.com/kuolas/photography-workbench/issues/new/choose).
