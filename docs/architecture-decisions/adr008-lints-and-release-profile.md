# ADR008: No unsafe code, a few extra lints, an optimized release build

- Status: Accepted
- Date: 2026-10-10

## Context

The app parses untrusted files — RAW, JPEG and EXIF data from any camera —
which is where memory-safety bugs turn into security bugs. Contributors also
need the same lint rules locally and in CI, and users want a small, fast
binary.

## Decision

In `Cargo.toml`:

- **`unsafe_code = "forbid"`.** Our own code contains no `unsafe`;
  dependencies may. A real need for `unsafe` (for example a platform API)
  needs a new ADR, and goes in its own small module.
- **Clippy `dbg_macro`, `todo`, `unimplemented` warn** — with CI's
  `-D warnings` they keep debugging leftovers and stubs off `master`.
  Clippy's default lint set otherwise applies; the stricter `pedantic` group
  is not enabled, because its casting lints are noisy in image-processing
  code.
- **Release profile:** `lto = true`, `codegen-units = 1`, `strip = true`.

## Consequences

The lint rules live in one place and apply identically in editors, local
runs and CI. Release builds are smaller and faster but compile more slowly;
debug builds are unaffected.
