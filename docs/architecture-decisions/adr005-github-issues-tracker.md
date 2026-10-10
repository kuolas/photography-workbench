# ADR005: GitHub Issues as the only tracker

- Status: Accepted
- Date: 2026-10-10

## Context

Known bugs, deferred work and feature requests need one home. The options
considered were GitHub Issues alone, or Issues plus in-repo files (a register
of known shortcuts, a status page, a work log).

## Decision

We will track all of it as GitHub issues, labelled from GitHub's default set
(`bug`, `enhancement`, `documentation`, `good first issue`, `help wanted`).
There is no in-repo register, status page or work log.

## Consequences

An issue is something an outside contributor can find, discuss, pick up and
close from a commit (`Closes: #N`); a second list in the tree would only drift
from it. Long-lived context goes in these ADRs,
[ROADMAP.md](../ROADMAP.md) and [ARCHITECTURE.md](../ARCHITECTURE.md). Code
comments that mark a known shortcut cite their issue (`// See #123`).
