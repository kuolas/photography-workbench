// SPDX-License-Identifier: GPL-3.0-or-later

//! What the tests decode, made in code so that no photo is committed:
//! JPEGs encoded from flat colours ([`jpeg`], [`gray_jpeg`]), and the TIFF structures
//! ([`Tiff`]) that a RAW file and a JPEG's EXIF block are built on.

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgb, RgbImage};

/// The TIFF tags the tests write.
pub const MAKE: u16 = 0x010F;
pub const MODEL: u16 = 0x0110;
pub const ORIENTATION: u16 = 0x0112;
pub const SUB_IFDS: u16 = 0x014A;
pub const JPEG_OFFSET: u16 = 0x0201;
pub const JPEG_LENGTH: u16 = 0x0202;
pub const EXIF_IFD: u16 = 0x8769;
pub const DATE_TIME_ORIGINAL: u16 = 0x9003;

/// The Make a Nikon camera writes, which is what makes a TIFF file a NEF.
pub const NIKON: &str = "NIKON CORPORATION";

/// Flat colours far apart, so each is told from the others after JPEG's
/// losses.
pub const RED: [u8; 3] = [220, 30, 30];
pub const GREEN: [u8; 3] = [30, 200, 30];
pub const BLUE: [u8; 3] = [30, 30, 220];
pub const YELLOW: [u8; 3] = [230, 220, 30];

/// A JPEG of `width` by `height` pixels, its left half `left` and its
/// right half `right`, with an EXIF block giving `orientation` when there
/// is one.
pub fn jpeg(
    width: u32,
    height: u32,
    left: [u8; 3],
    right: [u8; 3],
    orientation: Option<u16>,
) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, _| {
        Rgb(if x < width / 2 { left } else { right })
    });
    encode(
        image.as_raw(),
        width,
        height,
        ExtendedColorType::Rgb8,
        orientation,
    )
}

/// A grayscale JPEG of `width` by `height` pixels, one grey all over,
/// with an EXIF block giving `orientation` when there is one.
pub fn gray_jpeg(width: u32, height: u32, orientation: Option<u16>) -> Vec<u8> {
    let pixels = vec![128; usize::try_from(width * height).unwrap()];
    encode(&pixels, width, height, ExtendedColorType::L8, orientation)
}

/// `pixels` of `colour`, `width` by `height`, encoded as a JPEG with an
/// EXIF block giving `orientation` when there is one.
fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    colour: ExtendedColorType,
    orientation: Option<u16>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 95);
    if let Some(orientation) = orientation {
        let mut exif = Tiff::default();
        let ifd0 = exif.ifd();
        exif.short(ifd0, ORIENTATION, orientation);
        encoder.set_exif_metadata(exif.build()).unwrap();
    }
    encoder.write_image(pixels, width, height, colour).unwrap();
    bytes
}

/// Whether `pixel` is `colour`, give or take what JPEG loses.
pub fn near(pixel: &[u8], colour: [u8; 3]) -> bool {
    pixel.len() == 3
        && pixel
            .iter()
            .zip(colour)
            .all(|(&have, want)| have.abs_diff(want) <= 40)
}

/// The value of a TIFF entry, as the tests write it.
enum Value {
    Short(u16),
    Ascii(String),
    /// The offset of a blob in the file: a LONG.
    BlobOffset(usize),
    /// The length of a blob: a LONG.
    BlobLength(usize),
    /// The offset of an IFD in the file: a LONG.
    IfdOffset(usize),
}

/// A little-endian TIFF structure: IFDs, each a list of entries, and
/// blobs of bytes that the entries point at. The first IFD made is IFD0;
/// the others are reached only through entries that point at them, such
/// as SubIFDs.
///
/// The layout is the header, then the IFDs one after another, then the
/// values too long to sit in their entry, then the blobs, each starting
/// on an even byte.
#[derive(Default)]
pub struct Tiff {
    ifds: Vec<Vec<(u16, Value)>>,
    blobs: Vec<Vec<u8>>,
}

impl Tiff {
    /// A new, empty IFD, by its number.
    pub fn ifd(&mut self) -> usize {
        self.ifds.push(Vec::new());
        self.ifds.len() - 1
    }

    /// A blob holding `bytes`, by its number.
    pub fn blob(&mut self, bytes: Vec<u8>) -> usize {
        self.blobs.push(bytes);
        self.blobs.len() - 1
    }

    pub fn short(&mut self, ifd: usize, tag: u16, value: u16) {
        self.ifds[ifd].push((tag, Value::Short(value)));
    }

    pub fn ascii(&mut self, ifd: usize, tag: u16, value: &str) {
        self.ifds[ifd].push((tag, Value::Ascii(value.to_owned())));
    }

    /// The JPEG in `blob` declared in `ifd` as its JPEGInterchangeFormat
    /// and JPEGInterchangeFormatLength.
    pub fn jpeg(&mut self, ifd: usize, blob: usize) {
        self.ifds[ifd].push((JPEG_OFFSET, Value::BlobOffset(blob)));
        self.ifds[ifd].push((JPEG_LENGTH, Value::BlobLength(blob)));
    }

    /// `child` made a SubIFD of `parent`.
    pub fn sub_ifd(&mut self, parent: usize, child: usize) {
        self.ifds[parent].push((SUB_IFDS, Value::IfdOffset(child)));
    }

    /// `exif` made the Exif IFD of `ifd0`, where a camera records how and
    /// when the photo was taken.
    pub fn exif_ifd(&mut self, ifd0: usize, exif: usize) {
        self.ifds[ifd0].push((EXIF_IFD, Value::IfdOffset(exif)));
    }

    /// The bytes of the structure.
    pub fn build(mut self) -> Vec<u8> {
        for ifd in &mut self.ifds {
            ifd.sort_by_key(|&(tag, _)| tag);
        }
        let even = |n: usize| n + n % 2;
        // Where each IFD, each long value and each blob goes.
        let mut at = 8;
        let ifd_at: Vec<usize> = self
            .ifds
            .iter()
            .map(|entries| {
                let here = at;
                at += 2 + 12 * entries.len() + 4;
                here
            })
            .collect();
        let mut long_at = Vec::new();
        for (tag, value) in self.ifds.iter().flatten() {
            if let Value::Ascii(text) = value
                && text.len() + 1 > 4
            {
                long_at.push((*tag, at));
                at = even(at + text.len() + 1);
            }
        }
        let blob_at: Vec<usize> = self
            .blobs
            .iter()
            .map(|blob| {
                let here = at;
                at = even(at + blob.len());
                here
            })
            .collect();

        let u32_of = |n: usize| u32::try_from(n).unwrap().to_le_bytes();
        let mut bytes = vec![0; at];
        bytes[..4].copy_from_slice(b"II*\0");
        bytes[4..8].copy_from_slice(&u32_of(ifd_at[0]));
        let mut longs = long_at.iter();
        for (entries, &start) in self.ifds.iter().zip(&ifd_at) {
            let count = u16::try_from(entries.len()).unwrap();
            bytes[start..start + 2].copy_from_slice(&count.to_le_bytes());
            for (i, (tag, value)) in entries.iter().enumerate() {
                let entry = start + 2 + 12 * i;
                let (kind, count, field): (u16, usize, [u8; 4]) = match value {
                    Value::Short(n) => {
                        let [a, b] = n.to_le_bytes();
                        (3, 1, [a, b, 0, 0])
                    }
                    Value::Ascii(text) if text.len() + 1 > 4 => {
                        let &(_, place) = longs.next().unwrap();
                        bytes[place..place + text.len()].copy_from_slice(text.as_bytes());
                        (2, text.len() + 1, u32_of(place))
                    }
                    Value::Ascii(text) => {
                        let mut field = [0; 4];
                        field[..text.len()].copy_from_slice(text.as_bytes());
                        (2, text.len() + 1, field)
                    }
                    Value::BlobOffset(blob) => (4, 1, u32_of(blob_at[*blob])),
                    Value::BlobLength(blob) => (4, 1, u32_of(self.blobs[*blob].len())),
                    Value::IfdOffset(ifd) => (4, 1, u32_of(ifd_at[*ifd])),
                };
                bytes[entry..entry + 2].copy_from_slice(&tag.to_le_bytes());
                bytes[entry + 2..entry + 4].copy_from_slice(&kind.to_le_bytes());
                bytes[entry + 4..entry + 8].copy_from_slice(&u32_of(count));
                bytes[entry + 8..entry + 12].copy_from_slice(&field);
            }
            // The next-IFD pointer stays 0: no IFD is chained.
        }
        for (blob, &start) in self.blobs.iter().zip(&blob_at) {
            bytes[start..start + blob.len()].copy_from_slice(blob);
        }
        bytes
    }
}

/// A NEF holding `previews`, at most two JPEGs' bytes: the first in IFD0,
/// where recent Nikon bodies put their smallest, the second in a SubIFD of
/// IFD0, both shown upright by `orientation`.
pub fn nef(previews: &[Vec<u8>], orientation: u16) -> Vec<u8> {
    assert!(
        previews.len() <= 2,
        "one SubIFDs entry holds one SubIFD here"
    );
    let mut tiff = Tiff::default();
    let ifd0 = tiff.ifd();
    tiff.ascii(ifd0, MAKE, NIKON);
    tiff.short(ifd0, ORIENTATION, orientation);
    for (i, preview) in previews.iter().enumerate() {
        let blob = tiff.blob(preview.clone());
        if i == 0 {
            tiff.jpeg(ifd0, blob);
        } else {
            let sub = tiff.ifd();
            tiff.sub_ifd(ifd0, sub);
            tiff.jpeg(sub, blob);
        }
    }
    tiff.build()
}
