# ADR003: Linear history with rebase-merge

- Status: Accepted
- Date: 2026-10-10

## Context

How changes reach `master` decides whether the history can be read and
bisected. The options considered were merge commits, squash-merge and
rebase-merge. The GNOME programming guidelines on version control recommend
"rebase commits rather than merging, to have a linear history".

## Decision

We will land every change through a pull request, rebased onto `master` and
merged with GitHub's *Rebase and merge*: no merge commits, no squash. Every
commit on `master` must build and pass the tests.

- **Rejected merge commits:** they add nodes that `git log` and `git bisect`
  have to step around, for no benefit in a project this size.
- **Rejected squash-merge:** it folds well-separated commits into one, which
  defeats small atomic commits.

## Consequences

`git bisect` works on every commit, and history reads as a straight line of
small changes. Contributors must keep each commit green and rebase before
merging; CONTRIBUTING.md describes how to check the series.

GitHub's rebase-merge always creates new commit hashes, so commits are cited
by their hash on `master`, not on the branch.
