# Roadmap

The big picture: what is planned, and in what order. Bugs and smaller
requests live in the
[issue tracker](https://github.com/kuolas/photography-workbench/issues).

## Where we are

The repository, its license and its contribution process exist, and the
decisions that shape the app are recorded
([ADR009](architecture-decisions/adr009-raw-files-reads-raw-files.md) to
[ADR013](architecture-decisions/adr013-a-workspace-of-small-crates.md)).
The app opens a window with a title bar of its own and a choice of
themes; it does nothing else yet. The first release, 0.1.0, is being
built, one pull request at a time, under
[issue #4](https://github.com/kuolas/photography-workbench/issues/4).

## Three modes

The app is one window with three modes, each for one of a photographer's
jobs ([ADR010](architecture-decisions/adr010-modes-and-the-window.md)):

- **Library** — browse a folder of RAW files and cull it.
- **Organizer** — bring photos in from a memory card and rename them by
  their metadata.
- **Develop** — develop a RAW file into a picture.

0.1.0 ships Library. The other two are in the mode switcher from the
start, disabled until they exist.

## Milestones

### Library

1. **Browse fast** — *release 0.1.0.* A folder tree of the home folder and
   every mounted volume; a thumbnail grid or a single-photo viewer in the
   centre; a filmstrip below; the photo's information on the right; all
   driven from the keyboard. RAW files show the JPEG preview the camera
   embedded, read without decoding the sensor data.
2. **Pick keepers.** Mark photos as keepers or rejects, and filter by it.
3. **Rate sharpness.** Score each photo's sharpness where the photographer
   focused, and sort or filter by it.
4. **Rate composition.** Score each photo's framing.

### Organizer

5. **Bring photos in.** Copy a memory card's photos into a folder scheme,
   verify the copy, and show what arrived.
6. **Rename by EXIF.** Name files from their capture date, camera and
   other metadata, with a preview of every new name before anything
   changes on disk.

### Develop

7. **Develop a RAW file.** From the sensor data to a picture: white
   balance, exposure, tone, colour and the rest, without touching the
   original.

The order within a mode is a plan, not a promise; a change of direction
gets an ADR.

## Following raw-files

RAW files are read by [raw-files](https://crates.io/crates/raw-files)
([ADR009](architecture-decisions/adr009-raw-files-reads-raw-files.md)),
whose milestones unlock the ones above:

| raw-files milestone | What it unlocks here |
|---|---|
| 1 and 2, embedded previews of NEF and ARW files (released as 0.1.0) | Browse fast |
| 3, basic metadata: capture time, camera, lens, exposure, orientation, sizes (released as 0.2.0) | The Info dock's camera data; later, Rename by EXIF and sorting by capture time |
| 4, focus points in image coordinates (released as 0.3.0) | Rate sharpness |
| 5, the sensor data; 6, development to a linear image with its colour model | Develop a RAW file |

The cameras the app can open are the ones raw-files reads: Nikon NEF and
Sony ARW first, then DNG, Canon and Fujifilm in raw-files' order. Plain
JPEG files open directly.

## Smaller things after 0.1.0

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
