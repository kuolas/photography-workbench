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
| [ADR009](adr009-raw-files-reads-raw-files.md) | raw-files reads RAW files | Accepted |
| [ADR010](adr010-modes-and-the-window.md) | Three modes in one window with its own title bar | Accepted; its parts of the window superseded by [ADR014](adr014-the-parts-of-the-window.md) |
| [ADR011](adr011-views-navigation-and-the-keymap.md) | Views, navigation and the keymap | Accepted; its choice of keys superseded by [ADR015](adr015-the-keys.md) |
| [ADR012](adr012-version-numbers.md) | Version numbers | Accepted |
| [ADR013](adr013-a-workspace-of-small-crates.md) | A workspace of small crates | Accepted |
| [ADR014](adr014-the-parts-of-the-window.md) | The parts of the window | Accepted |
| [ADR015](adr015-the-keys.md) | The keys | Accepted |
| [ADR016](adr016-the-viewer-s-zoom.md) | The viewer's zoom | Accepted |

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

Records are not rewritten or deleted. A new decision that replaces an old one
gets its own ADR; the old one's status becomes "Superseded by
[ADRxxx](…)", linking to the new record.
