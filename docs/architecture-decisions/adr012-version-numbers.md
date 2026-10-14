# ADR012: Version numbers

- Status: Accepted
- Date: 2026-10-14
- Issue: #4

## Context

The first release needs a version number, and the changelog has deferred
the question of what the numbers promise until now. photography-workbench
is an application: nobody links against it, so its version carries no API
contract. What the number must do is tell a user which build they have,
let the changelog name each release, and order releases over time.
Libraries in the Rust ecosystem use semantic versioning, which Cargo
understands; some applications use calendar versions instead.

## Decision

We will number releases **`MAJOR.MINOR.PATCH`**, starting at **0.1.0**,
and read the parts as follows:

- while `MAJOR` is 0 the scope is still settling, and any release may
  change what the app does or how it stores things;
- `MINOR` goes up for a release that adds something a user can see or do;
- `PATCH` goes up for a release that only fixes things;
- `MAJOR` goes to 1 when the three modes exist and the app's files on disk
  (its settings, and later its catalog) get a compatibility promise, which
  will be its own record.

The number lives in one place, the root `Cargo.toml`, and every crate
the repository holds carries the same one; the crates are not published,
so a version names the application, never a library. A release is a git
tag `vMAJOR.MINOR.PATCH` on `master` and a GitHub release whose notes are
the changelog's section for it. Each release gets a `## [MAJOR.MINOR.PATCH]
- YYYY-MM-DD` section in the changelog.

Rejected: calendar versions (`2026.10`), which do not say whether a release
changed anything a user cares about; a version per crate, which would
suggest an API promise that does not exist.

## Consequences

- The changelog's "no versioning promise yet" sentence is replaced by a
  pointer to this record.
- Until 1.0, a user upgrading may find settings reset or a feature
  changed; the changelog says so when it happens.
- A dependency's version is that dependency's business: raw-files moving
  from 0.1 to 0.2 does not move this app's number.
