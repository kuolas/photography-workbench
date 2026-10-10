# Roadmap

The big picture: what is planned, and in what order. Bugs and smaller
requests live in the
[issue tracker](https://github.com/kuolas/photography-workbench/issues).

## Where we are

The repository, its license and its contribution process exist; the scope
is still being decided. The app opens an empty egui window with a menu
bar; it does nothing else yet.

## Milestones

1. **Browse fast.** Open a folder, show a thumbnail grid and a full-size
   view, using the JPEG previews embedded in RAW files.
2. **Pick keepers.** Mark photos as keepers or rejects, and filter by it.
3. **EXIF renamer.** Rename files from their metadata, with a preview of every
   new name before anything changes on disk.
4. **Rate sharpness.** Score each photo's sharpness and sort or filter by it.
5. **Rate composition.** Score each photo's framing.

The order is a plan, not a promise; a change of direction gets an ADR.

## Open questions

- **Where ratings live.** XMP sidecar files (readable by other photo tools),
  a local catalog database, or both.
- **Composition scoring.** Rule-based heuristics or a machine-learning
  model; a model must run locally and be GPLv3-compatible.
- **RAW formats.** Which cameras' RAW files to support first.

Ideas and use cases are welcome as
[feature requests](https://github.com/kuolas/photography-workbench/issues/new/choose).
