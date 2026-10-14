# ADR013: A workspace of small crates

- Status: Accepted
- Date: 2026-10-14
- Issue: #4

## Context

The app began as one crate: a library with the window in it and a thin
binary. The work planned for the first release mixes three kinds of code
that have nothing to say to each other: reading the file system and
listing a folder's photos, decoding previews on worker threads, and
drawing the window with egui. In one crate nothing below the window can
be tested without the toolkit, a change to a widget recompiles the
decoder, and the next mode or a second app could reuse none of it
without copying.

Cargo workspaces let one repository hold several crates with one lock
file, one set of lints and one version, and `cargo` runs the checks over
all of them at once.

## Decision

We will make the repository a **Cargo workspace** of small crates under
`crates/`, laid out engine-first:

- **`catalog`** — the photos on disk: the folder tree, a folder's photo
  list, the current photo. No GUI dependency.
- **`previews`** — decoding: the requests, the worker threads, the
  decoded images. No GUI dependency.
- **`desktop-ui`** — the custom widgets and shared window parts: the
  themes, the icons, the title bar's parts, the trays and docks, the
  key-binding primitives. Depends on egui; knows nothing of photos.
- **`photography-workbench`** — the app: the window, the modes, the
  surfaces. The only crate that knows both the photos and the toolkit,
  and the only one with a binary.

The rules that keep the shape:

- The dependency direction is enforced by a script that CI runs:
  `catalog` and `previews` must not depend on egui, eframe or
  `desktop-ui`; `desktop-ui` must not depend on raw-files, `image`,
  `catalog` or `previews`.
- A new engine capability (metadata, focus points, raw development) is a
  GUI-free crate, or a module of one, before any widget shows it.
- One version for the workspace ([ADR012](adr012-version-numbers.md)),
  one lint set ([ADR008](adr008-lints-and-release-profile.md)) and one
  list of dependency versions, all in the root `Cargo.toml`; the crates
  inherit them. No crate is published.
- `cargo run`, `cargo test` and the other commands in CONTRIBUTING keep
  working from the repository root; a single crate is tested with
  `cargo test -p <crate>`.

Rejected: one crate with modules (the boundary is a convention nobody
checks, and everything is one compilation unit); a crate per widget or
per surface (one app, one window: too fine to be worth the manifests).

## Consequences

- The engine crates' tests run on a machine with no display, so CI can
  run them anywhere and a contributor can work on decoding without
  starting the window.
- A move into the workspace is a pure relocation with no behaviour
  change, checked by the same tests before and after.
- The boundary script is one more gate to keep green, and a dependency
  added in the wrong crate fails CI rather than review.
- Commit subjects name the crate they touch (`catalog:`, `previews:`,
  `desktop-ui:`, `app:`), so the history reads by layer.
