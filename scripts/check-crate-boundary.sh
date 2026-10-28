#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# check-crate-boundary.sh — check that the workspace's crates depend on one
# another in one direction only (ADR013): the engine crates, catalog and
# previews, have no GUI (nor the window and GPU crates under it, winit and
# wgpu), and desktop-ui knows nothing of photos. CI runs it; run it yourself
# after adding a dependency.
#
#   scripts/check-crate-boundary.sh
#
# Every target platform and every feature are checked at once (--target all,
# --all-features), so a dependency added for one system only, or behind an
# optional feature, is caught on any machine. Normal, build and dev
# dependencies all count: an engine test that needed the GUI would need a
# display.
set -euo pipefail

cd "$(dirname "$0")/.."

fail=0

# forbid <crate> <dependency>... — fail if <crate> depends on any of the
# dependencies, directly or through another crate, and show how.
forbid() {
	local crate=$1 tree dep found
	shift
	tree=$(cargo tree --quiet -p "$crate" --all-features --target all -e normal,build,dev \
		--prefix none --format '{p}')
	for dep in "$@"; do
		# The first line naming it: "egui v0.37.0", or for one of our crates
		# "catalog v0.1.0 (/path/to/crates/catalog)".
		found=$(grep -m 1 "^$dep v" <<<"$tree") || continue
		fail=1
		printf '\ncheck-crate-boundary: %s must not depend on %s (ADR013). It does, through:\n\n' \
			"$crate" "$dep" >&2
		# With the version, so the spec is not ambiguous when the lock file
		# holds the crate at two versions. Only a diagnostic: it never stops
		# the remaining checks.
		found=${found#"$dep v"}
		cargo tree --quiet -p "$crate" --all-features --target all -e normal,build,dev \
			-i "$dep@${found%% *}" >&2 || true
	done
}

forbid catalog egui eframe desktop-ui winit wgpu
forbid previews egui eframe desktop-ui winit wgpu
forbid desktop-ui raw-files image catalog previews

if [ "$fail" = 0 ]; then
	printf 'check-crate-boundary: ok\n'
fi
exit $fail
