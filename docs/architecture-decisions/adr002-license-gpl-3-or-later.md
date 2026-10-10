# ADR002: License: GPL-3.0-or-later

- Status: Accepted
- Date: 2026-10-10

## Context

photography-workbench is an open-source application. The license decides
whether derivatives must stay free software, which dependencies can be used,
and how contributions come in. The options considered were a permissive
license (MIT or Apache-2.0), GPLv2-only (as in the Linux kernel) and GPLv3.

## Decision

We will license photography-workbench under the GNU General Public License,
version 3 or (at your option) any later version. Contributions come in under
the same license (inbound = outbound), with no CLA and no `Signed-off-by:`
requirement.

- **Over MIT or Apache-2.0:** a copyleft license keeps the program and its
  derivatives free software; a permissive one would allow closed forks.
- **Over GPLv2-only:** GPLv3 adds an explicit patent license from every
  contributor, forbids "tivoization" (shipping the program on a device that
  refuses modified versions), and is compatible with Apache-2.0 — which
  matters because much of the Rust ecosystem is `MIT OR Apache-2.0`.
  GPLv2-only code cannot be combined with Apache-2.0 code.
- **"Or later":** the GNOME convention for applications; it lets the project
  follow future GPL revisions without contacting every contributor.

## Consequences

Every dependency must be GPLv3-compatible: MIT, BSD, Apache-2.0, MPL-2.0, LGPL
and similar are; GPLv2-only is not. Each source file carries
`// SPDX-License-Identifier: GPL-3.0-or-later`, and `Cargo.toml` declares the
same identifier.
