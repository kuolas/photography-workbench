# Photography Workbench

A fast desktop app for culling photos: open a folder of RAW or JPEG files,
flick through them, and keep the good ones. For macOS, Linux and Windows.

**Status: early.** Nothing below works yet — the app opens an empty window
with a menu bar. See the [roadmap](docs/ROADMAP.md) for the plan.

## What it will do

- **Rename by EXIF** — name files from their capture date, camera and other
  metadata.
- **Rate sharpness** — spot blurred and out-of-focus shots at a glance.
- **Pick keepers** — mark the photos worth keeping.
- **Rate composition** — see how well each photo is framed.
- **Cull RAW files fast** — show the JPEG preview embedded in every RAW file,
  so browsing RAWs is as quick as browsing JPEGs.

This app is meant to be local, fast and approachable, no account, no cloud.

## Build and run

There are no releases yet. To build from source on macOS, Linux or Windows
you need Rust 1.95 or later:

```sh
cargo build
cargo run
cargo test
```

## Get involved

- **Found a bug or want a feature?** Open an
  [issue](https://github.com/kuolas/photography-workbench/issues/new/choose).
- **Found a security problem?** Please don't open a public issue; see
  [SECURITY.md](SECURITY.md).
- **Want to contribute code or docs?** Read [CONTRIBUTING.md](CONTRIBUTING.md).

Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) — how to build, test and send a change.
- [docs/ROADMAP.md](docs/ROADMAP.md) — what is planned, in what order.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the code is structured.
- [docs/architecture-decisions/](docs/architecture-decisions/README.md) — why
  things are the way they are.
- [CHANGELOG.md](CHANGELOG.md) — user-visible changes.

## License

Copyright (C) 2026 Alvaro Kuolas and the Photography Workbench contributors.

Photography Workbench is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See [LICENSE](LICENSE) for the full text.
