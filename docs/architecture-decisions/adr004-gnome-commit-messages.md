# ADR004: GNOME-style commit messages

- Status: Accepted
- Date: 2026-10-10

## Context

With rebase-merge ([ADR003](adr003-linear-history.md)) every commit lands on
`master` as written, so every commit message is permanent public history. The
two formats considered were the GNOME commit message guidelines
(`component: Summary`) and Angular / Conventional Commits
(`type(scope): summary`).

## Decision

We will follow the GNOME commit message guidelines: a `component: Summary`
subject, a body explaining why, and `Closes:` / `Fixes:` / `Bug:` issue
trailers, as CONTRIBUTING.md describes.

- **Rejected Angular / Conventional Commits:** its value is changelog and
  version automation, which this project does not use; `CHANGELOG.md` is
  written by hand.

## Consequences

History reads as plain sentences and the bar for new contributors is low.
Nothing can be generated from commit messages, so the changelog and version
numbers stay manual. Switching later would need a new ADR and a change to
CONTRIBUTING.md.
