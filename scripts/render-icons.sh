#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Renders the icon files in the app crate's assets/ from assets/icon.svg.
# Run it after editing the SVG and commit the results: builds use the
# rendered files, so nobody needs these tools just to build.
#
#   icon-256.png   the window icon on Linux and Windows (src/app.rs)
#   icon-1024.png  the window icon on macOS, and the bundle icon (cargo-bundle)
#   icon.ico       the Windows executable icon, embedded by build.rs
#
# Needs rsvg-convert (librsvg) and python3.

set -eu

cd "$(dirname "$0")/../crates/photography-workbench"
svg=assets/icon.svg

for tool in rsvg-convert python3; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "render-icons: $tool not found" >&2
        exit 1
    fi
done

# Each argument to the check below is size:file. --keep-aspect-ratio makes a
# non-square SVG come out non-square, which the check rejects, rather than
# stretched.
render() {
    rsvg-convert --keep-aspect-ratio --width "$1" --height "$1" "$svg" --output "$2"
}

set --
for size in 256 1024; do
    render "$size" "assets/icon-$size.png"
    set -- "$@" "$size:assets/icon-$size.png"
done

# One image per size Windows asks for, from the small taskbar icon up to
# Explorer's largest view; each is rendered from the SVG rather than scaled.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
set -- "$@" --ico
for size in 16 24 32 48 64 256; do
    render "$size" "$tmp/$size.png"
    set -- "$@" "$size:$tmp/$size.png"
done

# Check every image, then pack those after --ico into an .ico, each kept as
# PNG (Windows Vista and later read these); ImageMagick would store each one
# as an uncompressed bitmap, six times the size.
python3 -I - assets/icon.ico "$@" <<'EOF'
import struct
import sys

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def read_png(arg):
    size, path = arg.split(":", 1)
    size = int(size)
    with open(path, "rb") as f:
        blob = f.read()
    # The IHDR chunk comes first: its length, its type, then width and height.
    if blob[:8] != PNG_SIGNATURE or blob[12:16] != b"IHDR":
        sys.exit(f"render-icons: {path} is not a PNG")
    width, height = struct.unpack(">II", blob[16:24])
    if (width, height) != (size, size):
        sys.exit(
            f"render-icons: {path} is {width}x{height}, not {size}x{size}"
            " (is icon.svg square?)"
        )
    return size, blob


out, args = sys.argv[1], sys.argv[2:]
split = args.index("--ico")
for arg in args[:split]:
    read_png(arg)
images = [read_png(arg) for arg in args[split + 1 :]]

directory = struct.pack("<HHH", 0, 1, len(images))
offset = len(directory) + 16 * len(images)
for size, blob in images:
    # A directory entry holds sizes up to 255, and 0 for 256; nothing larger.
    if size > 256:
        sys.exit(f"render-icons: an .ico image is at most 256x256, not {size}x{size}")
    directory += struct.pack(
        "<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(blob), offset
    )
    offset += len(blob)
with open(out, "wb") as f:
    f.write(directory + b"".join(blob for _, blob in images))
EOF

echo "render-icons: wrote assets/icon-256.png, assets/icon-1024.png, assets/icon.ico"
