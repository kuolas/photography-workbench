# Architecture Decision Records

The substantial decisions that shape photography-workbench live here, one
short record each: the context, what was decided, and what follows from it.
They answer "why is it like this?" for anyone reading the code later.

| ADR | Title | Status |
|---|---|---|
| [ADR001](adr001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [ADR002](adr002-license-gpl-3-or-later.md) | License: GPL-3.0-or-later | Accepted |
| [ADR003](adr003-linear-history.md) | Linear history with rebase-merge | Accepted |
| [ADR004](adr004-gnome-commit-messages.md) | GNOME-style commit messages | Accepted |
| [ADR005](adr005-github-issues-tracker.md) | GitHub Issues as the only tracker | Accepted |
| [ADR006](adr006-ai-assisted-contributions.md) | AI-assisted contributions | Accepted |
| [ADR007](adr007-egui-cross-platform.md) | egui, on macOS, Linux and Windows | Accepted |
| [ADR008](adr008-lints-and-release-profile.md) | No unsafe code, a few extra lints, an optimized release build | Accepted |
| [ADR009](adr009-raw-files-reads-raw-files.md) | raw-files reads RAW files | Accepted; amended by [ADR018](adr018-raw-files-reads-jpeg-files-too.md), which reads plain JPEG files through raw-files too |
| [ADR010](adr010-modes-and-the-window.md) | Three modes in one window with its own title bar | Accepted; its parts of the window superseded by [ADR014](adr014-the-parts-of-the-window.md) |
| [ADR011](adr011-views-navigation-and-the-keymap.md) | Views, navigation and the keymap | Accepted; its choice of keys superseded by [ADR015](adr015-the-keys.md) |
| [ADR012](adr012-version-numbers.md) | Version numbers | Accepted |
| [ADR013](adr013-a-workspace-of-small-crates.md) | A workspace of small crates | Accepted |
| [ADR014](adr014-the-parts-of-the-window.md) | The parts of the window | Accepted; amended by [ADR017](adr017-showing-and-hiding-the-docks.md), which puts the filmstrip's toggle on the toolbar |
| [ADR015](adr015-the-keys.md) | The keys | Accepted |
| [ADR016](adr016-the-viewer-s-zoom.md) | The viewer's zoom | Accepted |
| [ADR017](adr017-showing-and-hiding-the-docks.md) | Showing and hiding the docks | Accepted; amended by [ADR019](adr019-the-photo-s-information-on-the-toolbar.md), which narrows the side docks for the photo's information's toggle |
| [ADR018](adr018-raw-files-reads-jpeg-files-too.md) | raw-files reads JPEG files too | Accepted |
| [ADR019](adr019-the-photo-s-information-on-the-toolbar.md) | The photo's information on the toolbar | Accepted |

## When to write one

Write an ADR when a decision would surprise someone reading only the code, is
hard to reverse, or settles a debate that would otherwise come back. Small,
local choices belong in a code comment or the commit message instead.

## Creating an ADR

1. Copy [`adr000-template.md`](adr000-template.md) to
   `adr000-my-decision.md` (a short, descriptive name; leave the number at 000).
2. Fill it in and open a pull request. Discussion happens on the PR.
3. Before merging, give it the next free number, rename the file to match,
   and add it to the table above.

## Changing a decision

Records are not rewritten or deleted, with two exceptions: corrections,
described next, and the extension at the end of this section.

A correction leaves the decision as it was: a figure measured again, a
term renamed in the code, a slip. It is made in the record itself, with
no new record and no change of status, and the commit that makes it
says why the record is corrected rather than superseded.

A new decision that replaces an old one gets its own ADR; the old one's
status becomes "Superseded by [ADRxxx](…)", linking to the new record.

A new decision that changes part of an old one, and leaves the rest
standing, gets its own ADR too. It says "Amends [ADRxxx](…)" next to its
status and names the point it changes; the old one's status becomes
"Accepted; amended by [ADRyyy](…)", so that a reader of the old record
finds the change.

A new decision that builds on an old one without contradicting it, by
covering what the old one left out, also gets its own ADR. It says
"Builds on [ADRxxx](…)" next to its status, and the old record stays as
it is, Accepted.

The other exception, the extension: a decision that extends an old one
to more values of the same kind, by the same rule, may be added to the
old record in place.
The additions are sections of their own, under the old Context,
Decision and Consequences, each with the date and the issue it was
added in; they say which of the old sentences no longer describe the
whole decision, and the old text stays as it was. The old record's
status names the addition.
