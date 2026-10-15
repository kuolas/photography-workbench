# ADR011: Views, navigation and the keymap

- Status: Accepted; its choice of keys superseded by [ADR015](adr015-the-keys.md)
- Date: 2026-10-14
- Issue: #4

## Context

Culling is done hundreds of times a day on folders of a thousand files, so
how a photographer reaches a folder, moves between its photos and switches
between an overview and one picture decides whether the app feels fast.
Photographers already know this from the browsers that come with their
cameras and from Lightroom, and their hands have the keys.

Two choices had to be made: how a folder is opened, and how the views,
the current photo and the keyboard relate to each other.

## Decision

- **A folder is reached in a folder tree, not an Open dialog.** The left
  dock holds a tree of the home folder and of every mounted volume (memory
  cards and external drives are where culling starts). Folders expand on
  demand; selecting one shows its photos. The tree remembers what was
  expanded and selected. There is no File ▸ Open item, and dropping a
  folder on the window is not a way in either: the tree is the one way,
  and it keeps the place between sessions.
- **Two views of the centre, one filmstrip, one current photo.** Library
  mode shows the folder either as a thumbnail grid or as a single-photo
  viewer, and the filmstrip in the bottom dock shows the same photos as a
  row. The grid, the viewer and the filmstrip share one current photo:
  moving in any of them moves the others, and the filmstrip scrolls to the
  current photo only when it changes, never while the user scrolls it.
  The grid clamps at its ends and never wraps around.
- **One table of key bindings per app.** Every shortcut is a row in one
  table: the action, the key with its modifiers, the surfaces it applies
  to (the grid, the viewer, the folder tree, everywhere), and its
  description. Handlers ask the table whether their action was pressed and
  never match keys themselves; a control that has a shortcut prints it from
  the table, in its tooltip; the shortcut reference the user opens with `?`
  or F1 is rendered from the same table. Tests check that no two rows
  collide on a surface, that every row has a description, and that every
  bound key reaches the reference. Shortcuts stand down while a text field
  has focus or a dialog is open.
- **The keys follow what photographers' hands know.** Where the browsers
  that camera makers ship agree on a key, that key is used: Alt+0 for the
  grid and Alt+9 for the viewer, arrows for the previous and next photo,
  a click on the viewer for 100 % and Shift held for a temporary 100 %,
  I for the photo's information. For zooming, the convention of the
  design tools (Ctrl+0 fit, Ctrl+1 actual size, Ctrl+= and Ctrl+- in and
  out) is used, with Cmd for Ctrl on macOS. For hiding the side docks, the
  photo editors' Tab and Shift+Tab. A key that one of those tools uses for
  something else is not taken for a different meaning here: a familiar key
  that lies is worse than an unfamiliar one.

Rejected: an Open-folder dialog (it forgets the place, and a dialog is the
slow way to a memory card); a floating thumbnail palette instead of a
filmstrip dock (two scrolling surfaces fighting for the same row);
per-widget key handling (no single reference, and collisions found by
users); inventing keys where a convention exists.

## Consequences

- Reading a folder's subfolders and files is engine code with no GUI
  dependency, testable on a temporary directory.
- Rating and labelling keys will arrive with the keepers feature; the
  camera makers' browsers use Ctrl plus a digit for ratings, which
  collides with Ctrl+0 for fit, so that assignment is decided then, in a
  new record.
- Adding a shortcut is one table row and one `pressed` call; a rebinding
  moves every tooltip and the reference with it.
- The folder tree shows no photo counts: counting would scan every folder
  on show, which is what the tree exists to avoid.
