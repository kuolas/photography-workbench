# ADR009: raw-files reads RAW files

- Status: Accepted
- Date: 2026-10-14
- Issue: #4

## Context

The app's first job is to browse a folder of RAW files quickly, by showing
the JPEG preview that every camera embeds in its RAW file, without decoding
the sensor data. Later jobs need more from the same files: the capture time,
camera and lens for renaming, the autofocus points for judging sharpness,
and eventually the sensor data itself for a Develop mode. Every file comes
from an outside source and must be treated as untrusted input, and many sit
on network shares, where reading a 50 MB file to show a 1 MB preview is the
difference between a browser that feels instant and one that does not.

The options considered:

- **LibRaw**, the C++ library behind most RAW converters. Dual-licensed
  LGPL-2.1 or CDDL-1.0. It decodes everything on open, and a C++ dependency
  has to be built, linked and shipped on macOS, Linux and Windows.
- **rawler**, a Rust RAW decoder (LGPL-2.1). Its purpose is decoding the
  sensor data for conversion; embedded previews and the metadata a browser
  needs are secondary, and it reads whole files.
- **A whole-file decoder of our own.** Simple to call, but every file is
  read in full, and the work belongs in a library that other programs can
  share.
- **raw-files**, a pure-Rust library (`MIT OR Apache-2.0`, no dependencies,
  no `unsafe` code) by this app's maintainer, published as version 0.1.0.
  It opens a file through a handle over any `Read + Seek` source, reads only
  the file's structure, lists the embedded previews with their pixel sizes
  and orientation, and fetches the chosen one's bytes on request. Every
  allocation is capped by limits that default to values safe for untrusted
  input; a malformed file is an error, never a panic. Its roadmap continues
  with what this app needs next, in order: basic metadata, focus points, the
  sensor data, and development to a linear image.

raw-files returns the preview's JPEG bytes and does not decode them.

## Decision

We will read RAW files with **raw-files**, and decode the JPEG previews it
returns with the **`image`** crate (`MIT OR Apache-2.0`), built with only its
pure-Rust JPEG decoder.

- The app's features follow raw-files' milestones: renaming by EXIF needs
  its basic metadata, sharpness at the point of focus needs its focus
  points, and a Develop mode needs its sensor data and development.
  Until a milestone ships there, the corresponding feature waits here.
- The formats the app opens are the formats raw-files recognises: Nikon NEF
  (with the NRW files of Coolpix cameras) and Sony ARW (with SRF, SR2 and
  ARQ) in 0.1.0; DNG, Canon and Fujifilm files as raw-files adds them.
- Plain JPEG files are opened directly, since the decoder is there anyway.
- raw-files stays a separate, published library: what this app needs from
  a RAW file is a request on that project, not a parser in this one.

**Rejected:** LibRaw, for the C++ dependency on three systems and because it
decodes what a browser never shows; rawler, because previews and metadata
are not what it is built for and it reads whole files; a decoder of our
own, because the work is a library's.

## Consequences

- No native library to build or ship; the app stays pure Rust on every
  system (see [ADR007](adr007-egui-cross-platform.md)) and keeps
  `unsafe_code = "forbid"` ([ADR008](adr008-lints-and-release-profile.md)).
- Both dependencies are `MIT OR Apache-2.0`, compatible with the app's
  GPL-3.0-or-later ([ADR002](adr002-license-gpl-3-or-later.md)).
- A camera whose files raw-files cannot open cannot be browsed here, and the
  fix lands there first. Format support is a raw-files issue, not an app
  issue.
- A preview is the camera's JPEG, not the sensor data: its pixel size is
  what the camera chose (often smaller than the sensor), and "100 %" in the
  viewer means 100 % of that preview.
