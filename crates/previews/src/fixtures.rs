// SPDX-License-Identifier: GPL-3.0-or-later

//! What the tests decode, made in code so that no photo is committed:
//! JPEGs encoded from flat colours ([`jpeg`], [`exif_jpeg`], [`gray_jpeg`]),
//! and the TIFF structures ([`Tiff`]) that a RAW file and a JPEG's EXIF
//! block are built on.

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgb, RgbImage};

/// The TIFF tags the tests write.
pub const COMPRESSION: u16 = 0x0103;
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
pub const CYAN: [u8; 3] = [30, 210, 220];
pub const MAGENTA: [u8; 3] = [220, 30, 220];

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
    exif_jpeg(width, height, left, right, orientation.map(oriented))
}

/// A JPEG of `width` by `height` pixels, its left half `left` and its
/// right half `right`, with `exif` as its EXIF block when there is one.
pub fn exif_jpeg(
    width: u32,
    height: u32,
    left: [u8; 3],
    right: [u8; 3],
    exif: Option<Tiff>,
) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, _| {
        Rgb(if x < width / 2 { left } else { right })
    });
    encode(image.as_raw(), width, height, ExtendedColorType::Rgb8, exif)
}

/// A grayscale JPEG of `width` by `height` pixels, one grey all over,
/// with an EXIF block giving `orientation` when there is one.
pub fn gray_jpeg(width: u32, height: u32, orientation: Option<u16>) -> Vec<u8> {
    let pixels = vec![128; usize::try_from(width * height).unwrap()];
    encode(
        &pixels,
        width,
        height,
        ExtendedColorType::L8,
        orientation.map(oriented),
    )
}

/// An EXIF block whose IFD0 gives `orientation` and nothing else.
pub fn oriented(orientation: u16) -> Tiff {
    let mut exif = Tiff::default();
    let ifd0 = exif.ifd();
    exif.short(ifd0, ORIENTATION, orientation);
    exif
}

/// An EXIF block whose IFD0 gives `orientation`, followed by an IFD1
/// holding `thumbnail`, a JPEG, as a camera stores its thumbnail.
pub fn thumbnailed(orientation: u16, thumbnail: Vec<u8>) -> Tiff {
    let mut exif = oriented(orientation);
    let ifd1 = exif.ifd();
    exif.chain(0, ifd1);
    let blob = exif.blob(thumbnail);
    exif.thumbnail(ifd1, blob);
    exif
}

/// The tags of the Multi-Picture Format's index that the tests write.
pub const MPF_VERSION: u16 = 0xB000;
pub const NUMBER_OF_IMAGES: u16 = 0xB001;
pub const MP_ENTRY: u16 = 0xB002;

/// `main`, a JPEG file, holding `images`, JPEGs too, as a camera stores
/// the smaller copies of its picture: each after `main`'s end, in their
/// order, and listed in the index of the Multi-Picture Format, an APP2
/// segment put after `main`'s application segments.
///
/// The index lists `main` first, at offset 0, then each image at its
/// offset from the index's own header, each over its bytes from its SOI
/// to the end of its EOI.
pub fn with_images(main: &[u8], images: &[Vec<u8>]) -> Vec<u8> {
    let at = after_application_segments(main);
    // The index's header comes after the segment's marker, its length and
    // its identifier.
    let header = at + 8;
    // The segment's length does not depend on the numbers in its entries,
    // so a build with none measures it.
    let count = images.len() + 1;
    let added = mpf_segment(&vec![0; 16 * count], count).len();
    let mut entries = mp_entry(0x2003_0000, main.len() + added, 0).to_vec();
    let mut start = main.len() + added;
    for image in images {
        entries.extend(mp_entry(0x0001_0001, image.len(), start - header));
        start += image.len();
    }
    let segment = mpf_segment(&entries, count);
    [&main[..at], &segment, &main[at..], &images.concat()].concat()
}

/// Where `jpeg`'s first segment after its SOI and its application
/// segments starts.
fn after_application_segments(jpeg: &[u8]) -> usize {
    let mut at = 2;
    while jpeg[at] == 0xFF && (0xE0..=0xEF).contains(&jpeg[at + 1]) {
        at += 2 + usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
    }
    at
}

/// The APP2 segment of the Multi-Picture Format's index of `count`
/// images, whose MP Entries are `entries`: its identifier, then the MP
/// header, in the shape of a TIFF header, and the MP Index IFD.
fn mpf_segment(entries: &[u8], count: usize) -> Vec<u8> {
    let mut index = Tiff::default();
    let ifd = index.ifd();
    index.undefined(ifd, MPF_VERSION, b"0100".to_vec());
    index.long(ifd, NUMBER_OF_IMAGES, u32::try_from(count).unwrap());
    index.undefined(ifd, MP_ENTRY, entries.to_vec());
    segment(0xE2, &[&b"MPF\0"[..], &index.build()].concat())
}

/// An MP Entry: the image's attribute, its size and its offset, then the
/// entry numbers of the images that depend on it, none.
fn mp_entry(attribute: u32, size: usize, offset: usize) -> [u8; 16] {
    let mut entry = [0; 16];
    entry[..4].copy_from_slice(&attribute.to_le_bytes());
    entry[4..8].copy_from_slice(&u32::try_from(size).unwrap().to_le_bytes());
    entry[8..12].copy_from_slice(&u32::try_from(offset).unwrap().to_le_bytes());
    entry
}

/// A JPEG segment of `marker` holding `payload`.
pub fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let length = u16::try_from(payload.len() + 2).unwrap();
    [&[0xFF, marker][..], &length.to_be_bytes(), payload].concat()
}

/// `jpeg` cut short before its scan: its frame header is whole, so it
/// can be measured, but its image is missing.
pub fn cut_before_scan(jpeg: &[u8]) -> Vec<u8> {
    let scan = jpeg
        .windows(2)
        .rposition(|marker| marker == [0xFF, 0xDA])
        .unwrap();
    jpeg[..scan].to_vec()
}

/// `pixels` of `colour`, `width` by `height`, encoded as a JPEG with
/// `exif` as its EXIF block when there is one.
fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    colour: ExtendedColorType,
    exif: Option<Tiff>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 95);
    if let Some(exif) = exif {
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
    Long(u32),
    Ascii(String),
    /// Bytes of no type of their own: an UNDEFINED.
    Undefined(Vec<u8>),
    /// The offset of a blob in the file: a LONG.
    BlobOffset(usize),
    /// The length of a blob: a LONG.
    BlobLength(usize),
    /// The offset of an IFD in the file: a LONG.
    IfdOffset(usize),
}

impl Value {
    /// The type and the bytes of a value that is a run of bytes, an
    /// ASCII's with the zero that ends it; `None` for a number.
    fn run(&self) -> Option<(u16, Vec<u8>)> {
        match self {
            Value::Ascii(text) => Some((2, [text.as_bytes(), &[0]].concat())),
            Value::Undefined(bytes) => Some((7, bytes.clone())),
            _ => None,
        }
    }
}

/// A little-endian TIFF structure: IFDs, each a list of entries, and
/// blobs of bytes that the entries point at. The first IFD made is IFD0;
/// the others are reached only through entries that point at them, such
/// as SubIFDs, or by following the IFD before them, as IFD1 follows IFD0.
///
/// The layout is the header, then the IFDs one after another, then the
/// values too long to sit in their entry, then the blobs, each starting
/// on an even byte.
#[derive(Default)]
pub struct Tiff {
    ifds: Vec<Vec<(u16, Value)>>,
    /// The IFD that follows each, by its number, if one does.
    next: Vec<Option<usize>>,
    blobs: Vec<Vec<u8>>,
}

impl Tiff {
    /// A new, empty IFD, by its number.
    pub fn ifd(&mut self) -> usize {
        self.ifds.push(Vec::new());
        self.next.push(None);
        self.ifds.len() - 1
    }

    /// `next` made the IFD that follows `ifd`, as IFD1 follows IFD0.
    pub fn chain(&mut self, ifd: usize, next: usize) {
        self.next[ifd] = Some(next);
    }

    /// A blob holding `bytes`, by its number.
    pub fn blob(&mut self, bytes: Vec<u8>) -> usize {
        self.blobs.push(bytes);
        self.blobs.len() - 1
    }

    pub fn short(&mut self, ifd: usize, tag: u16, value: u16) {
        self.ifds[ifd].push((tag, Value::Short(value)));
    }

    pub fn long(&mut self, ifd: usize, tag: u16, value: u32) {
        self.ifds[ifd].push((tag, Value::Long(value)));
    }

    pub fn ascii(&mut self, ifd: usize, tag: u16, value: &str) {
        self.ifds[ifd].push((tag, Value::Ascii(value.to_owned())));
    }

    pub fn undefined(&mut self, ifd: usize, tag: u16, value: Vec<u8>) {
        self.ifds[ifd].push((tag, Value::Undefined(value)));
    }

    /// The JPEG in `blob` declared in `ifd` as its JPEGInterchangeFormat
    /// and JPEGInterchangeFormatLength.
    pub fn jpeg(&mut self, ifd: usize, blob: usize) {
        self.ifds[ifd].push((JPEG_OFFSET, Value::BlobOffset(blob)));
        self.ifds[ifd].push((JPEG_LENGTH, Value::BlobLength(blob)));
    }

    /// The JPEG in `blob` declared in `ifd` as a thumbnail is: compressed
    /// as JPEG, at its JPEGInterchangeFormat, over its
    /// JPEGInterchangeFormatLength.
    pub fn thumbnail(&mut self, ifd: usize, blob: usize) {
        self.short(ifd, COMPRESSION, 6);
        self.jpeg(ifd, blob);
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
            if let Some((_, run)) = value.run()
                && run.len() > 4
            {
                long_at.push((*tag, at));
                at = even(at + run.len());
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
        for ((entries, next), &start) in self.ifds.iter().zip(&self.next).zip(&ifd_at) {
            let count = u16::try_from(entries.len()).unwrap();
            bytes[start..start + 2].copy_from_slice(&count.to_le_bytes());
            for (i, (tag, value)) in entries.iter().enumerate() {
                let entry = start + 2 + 12 * i;
                let (kind, count, field): (u16, usize, [u8; 4]) = match value {
                    Value::Short(n) => {
                        let [a, b] = n.to_le_bytes();
                        (3, 1, [a, b, 0, 0])
                    }
                    Value::Long(n) => (4, 1, n.to_le_bytes()),
                    Value::Ascii(_) | Value::Undefined(_) => {
                        let (kind, run) = value.run().unwrap();
                        if run.len() > 4 {
                            let &(_, place) = longs.next().unwrap();
                            bytes[place..place + run.len()].copy_from_slice(&run);
                            (kind, run.len(), u32_of(place))
                        } else {
                            let mut field = [0; 4];
                            field[..run.len()].copy_from_slice(&run);
                            (kind, run.len(), field)
                        }
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
            // The next-IFD pointer stays 0 when no IFD follows.
            if let Some(next) = next {
                let pointer = start + 2 + 12 * entries.len();
                bytes[pointer..pointer + 4].copy_from_slice(&u32_of(ifd_at[*next]));
            }
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
