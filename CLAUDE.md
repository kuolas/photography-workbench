# CLAUDE.md

photography-workbench is a desktop app for culling photos (Rust, edition
2024, egui, a Cargo workspace whose crates live in `crates/`,
GPL-3.0-or-later).
It is at an early stage; see [docs/ROADMAP.md](docs/ROADMAP.md) for what is
planned.

[CONTRIBUTING.md](CONTRIBUTING.md) is the rulebook — workflow, commit
messages, checks — and applies to assistants exactly as to humans.
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) explains the structure, and
[docs/architecture-decisions/](docs/architecture-decisions/README.md) why.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Reminders

- Commit messages follow CONTRIBUTING.md §Commit messages; run
  `scripts/install-hooks.sh` once so the `commit-msg` hook checks them.
- Keep history linear: rebase, never merge; every commit builds and passes
  tests (`scripts/check-series.sh`).
- AI-assisted work follows CONTRIBUTING.md §AI-assisted contributions: when a
  significant part of a commit is taken unchanged from AI output, add
  `Assisted-by: Claude` and tick the PR template's AI box. Never name a tool
  in `Co-Authored-By:`.
- New source files start with `// SPDX-License-Identifier: GPL-3.0-or-later`.
