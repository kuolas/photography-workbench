// SPDX-License-Identifier: GPL-3.0-or-later

//! A photo's file turned into a small upright image: [`decode`], which
//! gives a [`Decoded`] holding the [`Image`], the [`Size`] it was scaled
//! down from and that of the largest image the file holds, or the
//! [`Failure`] that stopped it.
//!
//! What a file holds is told by its content, never its name, as raw-files
//! tells its formats: a file that starts as every JPEG does is decoded as
//! one, and any other is opened with raw-files, which refuses what it does
//! not read. Of a RAW file only an embedded preview is decoded, never the
//! sensor data.

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Take};
use std::path::Path;

use image::codecs::jpeg::JpegDecoder;
use image::metadata::Orientation;
use image::{ColorType, DynamicImage, ImageDecoder, Limits};
use raw_files::{Preview, PreviewFormat, RawFile};

use crate::failure::Failure;

/// The bytes every JPEG starts with: the start-of-image marker and the
/// first byte of the marker after it.
const JPEG_START: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// The most bytes a JPEG file may take, all of which the decoder holds
/// while it decodes: the most raw-files reads of one preview, by default.
/// A camera's largest JPEG takes a fraction of it.
const MAX_JPEG_BYTES: u64 = 128 * 1024 * 1024;

/// The most pixels a JPEG may have on a side. The largest preview the
/// cameras raw-files reads embed is 9504 pixels wide, and a camera's own
/// panorama is longer still; a side longer than this is taken for a
/// damaged or hostile file, refused before anything is decoded.
const MAX_SIDE: u32 = 32_768;

/// The most bytes the pixels of one decode may take at once, in all the
/// buffers it holds together ([`peak_bytes`]), not counting the JPEG
/// decoder's own working memory: the `image` crate's default. A photo
/// shown whole and turned a quarter holds two RGB copies at the turn, six
/// bytes a pixel, so it may have about 89 million pixels; the largest
/// previews have 60 million.
const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

/// A width and a height in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// An image of 8-bit RGB pixels.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    /// The width in pixels, at least 1.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels, at least 1.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels: three bytes each, red, green and blue, from the left of
    /// the top row to the right of the bottom one, rows one after another
    /// with nothing between them. So `width × height × 3` bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// The size, never the pixels, which would fill a screen.
impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// A photo decoded: the image to show, the size of what it was scaled
/// down from, and the size of the largest image the file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    image: Image,
    full: Size,
    largest: Size,
}

impl Decoded {
    /// The photo, upright, no larger than it was asked for.
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The size of the image decoded, upright, before it was scaled down:
    /// a JPEG's own size, or the size of the RAW file's preview that was
    /// chosen, which may be smaller than the largest it holds.
    pub fn full(&self) -> Size {
        self.full
    }

    /// The size of the largest image the file holds, upright, whichever
    /// was decoded: a JPEG's own size, or the size of the RAW file's
    /// largest preview. Showing the photo at 100 % shows this many pixels.
    ///
    /// When the largest preview is the one decoded, this is
    /// [`full`](Self::full), the size the decoder found; otherwise it is
    /// the size the preview's own frame header gives, read when the file
    /// was opened, which is where the decoder finds it too.
    pub fn largest(&self) -> Size {
        self.largest
    }
}

/// The photo in the file at `path`, upright, its longer side at most
/// `longest` pixels: scaled down to that when it is longer, never scaled
/// up. A `longest` of 0 is taken as 1.
///
/// A JPEG is turned upright by the orientation its own EXIF block gives.
/// Of a RAW file's embedded previews, the smallest whose longer side is
/// at least `longest` is decoded, or the largest when none is that long,
/// and turned upright by the orientation the RAW file gives, which is the
/// camera's at the moment of capture. Whichever is decoded, the size of
/// the largest is told too, at no cost: raw-files measures every preview
/// when it opens the file.
///
/// This reads the file and decodes it, which takes a while, so it belongs
/// off the UI thread.
///
/// # Errors
///
/// The [`Failure`] that stopped it, whatever the file holds: a file that
/// cannot be read, is not a photo the app shows, holds no preview, is
/// damaged, or is too large.
pub fn decode(path: &Path, longest: u32) -> Result<Decoded, Failure> {
    let mut file = File::open(path).map_err(|error| Failure::from_io(&error))?;
    if starts_as_jpeg(&mut file)? {
        return decode_jpeg(jpeg_file(file)?, None, longest, MAX_PIXEL_BYTES);
    }
    let mut raw = RawFile::from_reader(file).map_err(|error| Failure::from_raw(&error))?;
    let preview = choose(raw.previews(), longest).ok_or(Failure::NoPreview)?;
    // There is one whenever there is a preview to choose.
    let largest = largest(raw.previews()).unwrap_or(preview);
    let bytes = raw
        .read_preview(&preview)
        .map_err(|error| Failure::from_raw(&error))?;
    let orientation = upright_by(preview.orientation());
    // The decoder copies the bytes into a buffer of its own, so a
    // preview is held twice while it is decoded: it takes a reader, and
    // raw-files hands out a preview's bytes, not a reader over them.
    let mut decoded = decode_jpeg(
        Cursor::new(bytes),
        Some(orientation),
        longest,
        MAX_PIXEL_BYTES,
    )?;
    if preview != largest {
        // As its frame header gives it, which raw-files read.
        decoded.largest = upright_size(
            largest.width(),
            largest.height(),
            upright_by(largest.orientation()),
        );
    }
    Ok(decoded)
}

/// Whether the file starts as every JPEG does. A file too short to hold
/// the start does not.
fn starts_as_jpeg(file: &mut File) -> Result<bool, Failure> {
    let mut start = Vec::with_capacity(JPEG_START.len());
    file.take(3)
        .read_to_end(&mut start)
        .map_err(|error| Failure::from_io(&error))?;
    Ok(start == JPEG_START)
}

/// A JPEG file, for the decoder to read from its start, unless it is
/// longer than [`MAX_JPEG_BYTES`].
///
/// The decoder reads it whole into a buffer of its own, so it is handed
/// the file rather than a copy of it, and the bytes are held once. It
/// reads no further than the cap, so a file that grew past the cap since
/// its length was checked is decoded cut short, and is damaged.
fn jpeg_file(mut file: File) -> Result<BufReader<Take<File>>, Failure> {
    let read = |error| Failure::from_io(&error);
    if file.metadata().map_err(read)?.len() > MAX_JPEG_BYTES {
        return Err(Failure::TooLarge);
    }
    file.seek(SeekFrom::Start(0)).map_err(read)?;
    Ok(BufReader::new(file.take(MAX_JPEG_BYTES)))
}

/// The preview to decode of `previews`, which raw-files lists smallest
/// first: the first JPEG whose longer side is at least `longest`, or else
/// the last JPEG, the largest. `None` when there is no JPEG.
///
/// The longer side is the same whichever way the preview is turned, so
/// the orientation does not enter into it.
fn choose(previews: &[Preview], longest: u32) -> Option<Preview> {
    jpegs(previews)
        .find(|preview| preview.width().max(preview.height()) >= longest)
        .copied()
        .or_else(|| largest(previews))
}

/// The largest JPEG of `previews`, which raw-files lists smallest first:
/// the last. `None` when there is no JPEG.
fn largest(previews: &[Preview]) -> Option<Preview> {
    jpegs(previews).next_back().copied()
}

/// The JPEGs of `previews`, in their order.
fn jpegs(previews: &[Preview]) -> impl DoubleEndedIterator<Item = &Preview> {
    previews
        .iter()
        .filter(|preview| preview.format() == PreviewFormat::Jpeg)
}

/// The size of a `width` by `height` image turned upright by
/// `orientation`: its width and height change places when it turns a
/// quarter.
fn upright_size(width: u32, height: u32, orientation: Orientation) -> Size {
    if turns(orientation) {
        Size {
            width: height,
            height: width,
        }
    } else {
        Size { width, height }
    }
}

/// The JPEG that `jpeg` reads decoded, scaled down so that its longer
/// side is at most `longest`, then turned upright by `orientation`, or by
/// the orientation of its own EXIF block when that is `None`. Scaling
/// first leaves fewer pixels to turn. Its pixels may take at most
/// `budget` bytes at once, which is checked before it is decoded. It is
/// taken for the largest image there is, as a JPEG file is.
fn decode_jpeg<R: BufRead + Seek>(
    jpeg: R,
    orientation: Option<Orientation>,
    longest: u32,
    budget: u64,
) -> Result<Decoded, Failure> {
    let failed = |error| Failure::from_image(&error);
    let mut decoder = JpegDecoder::new(jpeg).map_err(failed)?;
    let orientation = match orientation {
        Some(orientation) => orientation,
        None => decoder.orientation().map_err(failed)?,
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(budget);
    decoder.set_limits(limits.clone()).map_err(failed)?;
    let (width, height) = decoder.dimensions();
    let small = fit(width, height, longest);
    // The JPEG decoder checks the sides but not the memory: this does,
    // before anything is allocated.
    let peak = peak_bytes(
        (width, height),
        decoder.color_type(),
        small,
        turns(orientation),
    );
    limits.reserve(peak).map_err(failed)?;

    let decoded = DynamicImage::from_decoder(decoder).map_err(failed)?;
    let small = if small == (width, height) {
        decoded
    } else {
        // Area averaging: each pixel kept is the mean of those it stands
        // for, as fast as scaling gets and without the jagged edges of
        // keeping one pixel in so many. The decoded image is let go
        // here, as `peak_bytes` counts it, not at the end.
        let (small_width, small_height) = small;
        let scaled = decoded.thumbnail_exact(small_width, small_height);
        drop(decoded);
        scaled
    };
    let mut upright = DynamicImage::ImageRgb8(small.into_rgb8());
    upright.apply_orientation(orientation);
    let full = upright_size(width, height, orientation);
    let upright = upright.into_rgb8();
    Ok(Decoded {
        image: Image {
            width: upright.width(),
            height: upright.height(),
            pixels: upright.into_raw(),
        },
        full,
        largest: full,
    })
}

/// The most bytes the pixel buffers of a decode hold at once: the image
/// decoded, `size` pixels of `colour`; its copy scaled to `small`, while
/// the decoded one is still held, when it is scaled; the RGB copy of
/// that, while it is held, when `colour` is not RGB already; and the
/// copy turned a quarter, while the one before it is held, when `turn`.
/// A half turn or a flip is done in place, and turning an RGB image into
/// RGB moves it, so neither holds a copy.
fn peak_bytes(size: (u32, u32), colour: ColorType, small: (u32, u32), turn: bool) -> u64 {
    let pixels = |(width, height): (u32, u32)| u64::from(width) * u64::from(height);
    let each = u64::from(colour.bytes_per_pixel());
    let decoded = pixels(size) * each;
    let (mut peak, held) = if small == size {
        (decoded, decoded)
    } else {
        let scaled = pixels(small) * each;
        (decoded + scaled, scaled)
    };
    let rgb = pixels(small) * 3;
    if colour != ColorType::Rgb8 {
        peak = peak.max(held + rgb);
    }
    if turn {
        peak = peak.max(rgb + rgb);
    }
    peak
}

/// The size of a `width` by `height` image scaled down so that its longer
/// side is `longest`, its shorter side rounded to the nearest pixel; the
/// size itself when its longer side is no more than `longest`. No side is
/// ever 0, so a `longest` of 0 gives the sides of 1.
fn fit(width: u32, height: u32, longest: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= longest {
        return (width, height);
    }
    let scale = |side: u32| {
        let scaled = (u64::from(side) * u64::from(longest) + u64::from(long) / 2) / u64::from(long);
        // At most `longest`, since `side` is at most `long`.
        u32::try_from(scaled).unwrap_or(longest).max(1)
    };
    (scale(width), scale(height))
}

/// The orientation of the `image` crate that does what raw-files'
/// `orientation` says: the two name the eight EXIF values alike.
fn upright_by(orientation: raw_files::Orientation) -> Orientation {
    use raw_files::Orientation as Raw;
    match orientation {
        Raw::NoTransforms => Orientation::NoTransforms,
        Raw::FlipHorizontal => Orientation::FlipHorizontal,
        Raw::Rotate180 => Orientation::Rotate180,
        Raw::FlipVertical => Orientation::FlipVertical,
        Raw::Rotate90FlipH => Orientation::Rotate90FlipH,
        Raw::Rotate90 => Orientation::Rotate90,
        Raw::Rotate270FlipH => Orientation::Rotate270FlipH,
        Raw::Rotate270 => Orientation::Rotate270,
    }
}

/// Whether `orientation` turns the image a quarter, so that its width
/// and height change places.
fn turns(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{self, BLUE, GREEN, RED, Tiff, YELLOW, near};
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    /// `bytes` written to a file named `name` in a new folder.
    fn file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    /// The photo `bytes` hold, decoded with its longer side at most
    /// `longest`.
    fn decoded(bytes: &[u8], longest: u32) -> Result<Decoded, Failure> {
        let (_dir, path) = file("photo", bytes);
        decode(&path, longest)
    }

    /// The pixel at `x`, `y` of `image`.
    fn pixel(image: &Image, x: u32, y: u32) -> &[u8] {
        let at = usize::try_from((y * image.width() + x) * 3).unwrap();
        &image.pixels()[at..at + 3]
    }

    /// The size of `image`.
    fn size(image: &Image) -> (u32, u32) {
        (image.width(), image.height())
    }

    /// The colours a quarter of the way into `image` and three quarters,
    /// across when `across`, else down, through its middle.
    fn ends(image: &Image, across: bool) -> (&[u8], &[u8]) {
        let (w, h) = size(image);
        if across {
            (pixel(image, w / 4, h / 2), pixel(image, w * 3 / 4, h / 2))
        } else {
            (pixel(image, w / 2, h / 4), pixel(image, w / 2, h * 3 / 4))
        }
    }

    #[test]
    fn a_jpeg_no_longer_than_asked_keeps_its_size() {
        let photo = decoded(&fixtures::jpeg(64, 48, RED, GREEN, None), 100).unwrap();
        assert_eq!(size(photo.image()), (64, 48));
        assert_eq!(
            photo.full(),
            Size {
                width: 64,
                height: 48
            }
        );
        assert_eq!(photo.image().pixels().len(), 64 * 48 * 3);
        let (left, right) = ends(photo.image(), true);
        assert!(near(left, RED) && near(right, GREEN), "{left:?} {right:?}");
    }

    #[test]
    fn a_longer_jpeg_is_scaled_down_to_the_longest_side_keeping_its_shape() {
        let wide = decoded(&fixtures::jpeg(400, 300, RED, GREEN, None), 100).unwrap();
        assert_eq!(size(wide.image()), (100, 75));
        assert_eq!(
            wide.full(),
            Size {
                width: 400,
                height: 300
            }
        );
        assert_eq!(wide.image().pixels().len(), 100 * 75 * 3);
        let tall = decoded(&fixtures::jpeg(300, 400, RED, GREEN, None), 100).unwrap();
        assert_eq!(size(tall.image()), (75, 100));
        assert_eq!(
            tall.full(),
            Size {
                width: 300,
                height: 400
            }
        );
    }

    #[test]
    fn a_longest_side_of_0_is_taken_as_1() {
        let photo = decoded(&fixtures::jpeg(40, 20, RED, GREEN, None), 0).unwrap();
        assert_eq!(size(photo.image()), (1, 1));
    }

    /// Each probe is a pixel's colour, so a turn that only swapped the
    /// width and height, or turned the wrong way, would show.
    #[test]
    fn a_jpeg_is_turned_upright_by_its_own_exif_orientation() {
        let turned =
            |orientation| decoded(&fixtures::jpeg(80, 40, RED, GREEN, orientation), 1000).unwrap();
        // A quarter turn clockwise brings the left half to the top.
        let photo = turned(Some(6));
        assert_eq!(size(photo.image()), (40, 80));
        assert_eq!(
            photo.full(),
            Size {
                width: 40,
                height: 80
            }
        );
        let (top, bottom) = ends(photo.image(), false);
        assert!(near(top, RED) && near(bottom, GREEN), "{top:?} {bottom:?}");
        // A quarter turn the other way brings it to the bottom.
        let photo = turned(Some(8));
        assert_eq!(size(photo.image()), (40, 80));
        let (top, bottom) = ends(photo.image(), false);
        assert!(near(top, GREEN) && near(bottom, RED), "{top:?} {bottom:?}");
        // A half turn brings it to the right.
        let photo = turned(Some(3));
        assert_eq!(size(photo.image()), (80, 40));
        let (left, right) = ends(photo.image(), true);
        assert!(near(left, GREEN) && near(right, RED), "{left:?} {right:?}");
        // No orientation, or 1, leaves it where it is.
        for orientation in [None, Some(1)] {
            let photo = turned(orientation);
            let (left, right) = ends(photo.image(), true);
            assert!(near(left, RED) && near(right, GREEN), "{orientation:?}");
        }
    }

    #[test]
    fn a_jpeg_s_largest_size_is_its_own_upright_however_small_it_is_decoded() {
        let largest = |orientation| {
            decoded(&fixtures::jpeg(80, 40, RED, GREEN, orientation), 10)
                .unwrap()
                .largest()
        };
        assert_eq!(
            largest(None),
            Size {
                width: 80,
                height: 40
            }
        );
        // A quarter turn.
        assert_eq!(
            largest(Some(6)),
            Size {
                width: 40,
                height: 80
            }
        );
    }

    /// A NEF holding a 160 × 120 preview, left half red and right half
    /// green, and a 640 × 480 one, left half blue and right half yellow,
    /// shown upright by a quarter turn clockwise. The small one's own EXIF
    /// asks for a half turn, which the RAW file's orientation overrides.
    fn nef() -> Vec<u8> {
        fixtures::nef(
            &[
                fixtures::jpeg(160, 120, RED, GREEN, Some(3)),
                fixtures::jpeg(640, 480, BLUE, YELLOW, None),
            ],
            6,
        )
    }

    #[test]
    fn a_raw_file_s_smallest_preview_long_enough_is_decoded_upright() {
        for longest in [100, 160] {
            let photo = decoded(&nef(), longest).unwrap();
            assert_eq!(
                photo.full(),
                Size {
                    width: 120,
                    height: 160
                },
                "{longest}"
            );
            assert_eq!(size(photo.image()).1, longest);
            let (top, bottom) = ends(photo.image(), false);
            assert!(near(top, RED) && near(bottom, GREEN), "{top:?} {bottom:?}");
        }
        assert_eq!(size(decoded(&nef(), 100).unwrap().image()), (75, 100));
    }

    #[test]
    fn a_raw_file_s_larger_preview_is_decoded_when_the_smaller_is_too_short() {
        let photo = decoded(&nef(), 161).unwrap();
        assert_eq!(
            photo.full(),
            Size {
                width: 480,
                height: 640
            }
        );
        assert_eq!(size(photo.image()), (121, 161));
        let (top, bottom) = ends(photo.image(), false);
        assert!(
            near(top, BLUE) && near(bottom, YELLOW),
            "{top:?} {bottom:?}"
        );
    }

    #[test]
    fn a_raw_file_s_largest_preview_is_decoded_and_not_scaled_up_when_none_is_long_enough() {
        let photo = decoded(&nef(), 4000).unwrap();
        assert_eq!(
            photo.full(),
            Size {
                width: 480,
                height: 640
            }
        );
        assert_eq!(size(photo.image()), (480, 640));
    }

    /// The largest preview, turned upright by the RAW file's quarter
    /// turn, whichever is decoded.
    #[test]
    fn a_raw_file_tells_its_largest_preview_s_upright_size_whichever_is_decoded() {
        let largest = Size {
            width: 480,
            height: 640,
        };
        for longest in [100, 161, 4000] {
            assert_eq!(
                decoded(&nef(), longest).unwrap().largest(),
                largest,
                "{longest}"
            );
        }
    }

    /// The largest is told by size, not by where the file keeps it.
    #[test]
    fn a_raw_file_s_largest_preview_is_the_largest_wherever_it_is() {
        let file = fixtures::nef(
            &[
                fixtures::jpeg(640, 480, BLUE, YELLOW, None),
                fixtures::jpeg(120, 160, RED, GREEN, None),
            ],
            1,
        );
        assert_eq!(
            decoded(&file, 150).unwrap().largest(),
            Size {
                width: 640,
                height: 480
            }
        );
    }

    /// The previews are told apart by size, not by where the file keeps
    /// them, and by their longer side, whichever way they are stored.
    #[test]
    fn a_preview_is_chosen_by_its_longer_side_wherever_it_is() {
        let file = fixtures::nef(
            &[
                fixtures::jpeg(640, 480, BLUE, YELLOW, None),
                fixtures::jpeg(120, 160, RED, GREEN, None),
            ],
            1,
        );
        let photo = decoded(&file, 150).unwrap();
        assert_eq!(
            photo.full(),
            Size {
                width: 120,
                height: 160
            }
        );
        let (left, right) = ends(photo.image(), true);
        assert!(near(left, RED) && near(right, GREEN), "{left:?} {right:?}");
    }

    #[test]
    fn a_file_that_is_not_there_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            decode(&dir.path().join("DSC_0001.NEF"), 512),
            Err(Failure::Unreadable(io::ErrorKind::NotFound))
        );
    }

    /// Text, an empty file and a TIFF file of no camera raw-files reads,
    /// whatever their names say.
    #[test]
    fn a_file_that_is_not_a_photo_is_unsupported() {
        let mut tiff = Tiff::default();
        let ifd0 = tiff.ifd();
        tiff.ascii(ifd0, fixtures::MAKE, "Scanner Co.");
        for bytes in [&b"Not a photo at all\n"[..], b"", &tiff.build()] {
            let (_dir, path) = file("DSC_0001.NEF", bytes);
            assert_eq!(decode(&path, 512), Err(Failure::Unsupported));
        }
    }

    #[test]
    fn a_raw_file_with_no_preview_says_so() {
        assert_eq!(
            decoded(&fixtures::nef(&[], 1), 512),
            Err(Failure::NoPreview)
        );
    }

    #[test]
    fn a_damaged_file_is_damaged() {
        let jpeg = fixtures::jpeg(64, 48, RED, GREEN, None);
        // A JPEG that ends before its frame header.
        assert_eq!(decoded(&jpeg[..20], 512), Err(Failure::Damaged));
        // A TIFF file whose first directory would lie inside its header.
        assert_eq!(decoded(b"II*\0\x04\0\0\0", 512), Err(Failure::Damaged));
        // A NEF whose preview has a frame header, so raw-files lists it,
        // but ends before its image.
        let scan = jpeg
            .windows(2)
            .position(|marker| marker == [0xFF, 0xDA])
            .unwrap();
        let file = fixtures::nef(&[jpeg[..scan].to_vec()], 1);
        assert_eq!(decoded(&file, 512), Err(Failure::Damaged));
    }

    /// A JPEG whose frame header says it is `width` by `height`.
    fn claiming(width: u16, height: u16) -> Vec<u8> {
        let mut jpeg = fixtures::jpeg(8, 8, RED, GREEN, None);
        let sof = jpeg
            .windows(2)
            .position(|marker| marker == [0xFF, 0xC0])
            .unwrap();
        // The marker, the length and the precision, then the height and
        // the width.
        jpeg[sof + 5..sof + 7].copy_from_slice(&height.to_be_bytes());
        jpeg[sof + 7..sof + 9].copy_from_slice(&width.to_be_bytes());
        jpeg
    }

    #[test]
    fn a_photo_too_large_to_decode_is_refused() {
        // A side past the most allowed.
        assert_eq!(decoded(&claiming(40_000, 100), 512), Err(Failure::TooLarge));
        // Both sides allowed, but more pixels than the memory allowed.
        assert_eq!(
            decoded(&claiming(30_000, 30_000), 512),
            Err(Failure::TooLarge)
        );
        // A TIFF file whose first directory says it holds 1001 entries,
        // more than raw-files reads in one.
        let mut tiff = b"II*\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&1001_u16.to_le_bytes());
        tiff.resize(tiff.len() + 1001 * 12 + 4, 0);
        assert_eq!(decoded(&tiff, 512), Err(Failure::TooLarge));
        // A file longer than a JPEG may be, which is never read.
        let (_dir, path) = file("huge.jpg", &fixtures::jpeg(8, 8, RED, GREEN, None));
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_JPEG_BYTES + 1)
            .unwrap();
        assert_eq!(decode(&path, 512), Err(Failure::TooLarge));
    }

    /// A grayscale JPEG shown whole holds, at once, its decoded pixels
    /// and their RGB copy, and turned a quarter, two RGB copies: one byte
    /// short of either is too much, and the exact peak is enough. Counted
    /// by the decoded pixels alone, as JPEG holds them, either would fit
    /// in a sixth of it.
    #[test]
    fn every_pixel_buffer_a_decode_holds_at_once_counts() {
        let gray = |orientation, budget| {
            let jpeg = Cursor::new(fixtures::gray_jpeg(1000, 1000, orientation));
            decode_jpeg(jpeg, None, 1000, budget).map(|photo| photo.full())
        };
        let whole = Size {
            width: 1000,
            height: 1000,
        };
        // The decoded pixels, one byte each, and the RGB copy.
        assert_eq!(gray(None, 4_000_000), Ok(whole));
        assert_eq!(gray(None, 3_999_999), Err(Failure::TooLarge));
        // The RGB copy, and the turned copy of it.
        assert_eq!(gray(Some(6), 6_000_000), Ok(whole));
        assert_eq!(gray(Some(6), 5_999_999), Err(Failure::TooLarge));
    }

    #[test]
    fn the_peak_of_a_decode_is_its_largest_set_of_buffers_held_together() {
        let peak = |colour, small, turn| peak_bytes((400, 300), colour, small, turn);
        let whole = (400, 300);
        let pixels = 400 * 300;
        // Nothing but the decoded image: no copy is made.
        assert_eq!(peak(ColorType::Rgb8, whole, false), 3 * pixels);
        assert_eq!(peak(ColorType::Rgb8, whole, true), 6 * pixels);
        assert_eq!(peak(ColorType::L8, whole, false), 4 * pixels);
        assert_eq!(peak(ColorType::L8, whole, true), 6 * pixels);
        assert_eq!(peak(ColorType::Rgba8, whole, false), 7 * pixels);
        // Scaled to 40 × 30: the decoded and the scaled image together.
        let small = (40, 30);
        let few = 40 * 30;
        assert_eq!(peak(ColorType::Rgb8, small, true), 3 * pixels + 3 * few);
        assert_eq!(peak(ColorType::L8, small, true), pixels + few);
        // Scaled so little that the copies after it weigh the most.
        assert_eq!(peak(ColorType::L8, (399, 300), true), 6 * 399 * 300);
    }

    #[test]
    fn every_raw_files_orientation_has_the_same_exif_value_in_image() {
        for value in 1..=8 {
            let raw = raw_files::Orientation::from_exif(value).unwrap();
            assert_eq!(u16::from(upright_by(raw).to_exif()), value);
            assert_eq!(turns(upright_by(raw)), raw.swaps_dimensions());
        }
    }

    #[test]
    fn fitting_rounds_the_shorter_side_and_never_makes_it_0() {
        assert_eq!(fit(400, 300, 100), (100, 75));
        assert_eq!(fit(300, 400, 100), (75, 100));
        assert_eq!(fit(640, 480, 161), (161, 121));
        assert_eq!(fit(1000, 1, 10), (10, 1));
        assert_eq!(fit(100, 100, 100), (100, 100));
        assert_eq!(fit(50, 20, 100), (50, 20));
        assert_eq!(fit(u32::MAX, u32::MAX - 1, 512), (512, 512));
    }
}
