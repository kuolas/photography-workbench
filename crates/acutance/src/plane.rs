// SPDX-License-Identifier: GPL-3.0-or-later

//! The input: one plane of luminance samples, and how much of its frame's
//! detail it holds.

use std::fmt;

/// A size in pixels. It prints as `6000x4000`.
///
/// # Examples
///
/// ```
/// use acutance::Size;
///
/// let frame = Size { width: 6000, height: 4000 };
/// assert_eq!(frame.to_string(), "6000x4000");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Size {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Size {
    /// Whether either side is zero.
    pub(crate) fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// How much of the frame's detail a plane holds, and so how far a score
/// read from it can be trusted.
///
/// A plane is often smaller than the frame it shows: an embedded preview
/// is a fraction of the sensor's width, and fine detail that is sharp on
/// the sensor is lost in it. Scores read from planes of different
/// fidelity are not comparable. The fidelity is read from the plane's
/// size, so a plane upscaled from a smaller image claims more detail than
/// it holds: make it from the image as it was decoded.
///
/// A fidelity that holds more detail compares greater, and it prints as
/// its [name](Fidelity::name).
///
/// # Examples
///
/// ```
/// use acutance::{Fidelity, Plane, Size};
///
/// # fn main() {
/// #     example().unwrap();
/// # }
/// # fn example() -> Option<()> {
/// // A preview a quarter as wide as the camera's image.
/// let frame = Size { width: 6000, height: 4000 };
/// let preview = Plane::from_samples(1500, 1000, vec![0.5; 1500 * 1000], frame)?;
/// assert_eq!(preview.fidelity(), Fidelity::Coarse);
/// assert!(Fidelity::Coarse < Fidelity::Full);
/// assert_eq!(Fidelity::Coarse.to_string(), "coarse");
/// # Some(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// The plane is less than half as wide as its frame.
    Coarse,
    /// The plane is at least half as wide as its frame.
    Reduced,
    /// The plane is at least as wide as its frame.
    Full,
}

impl Fidelity {
    /// The fidelity of a plane `width` pixels wide showing a frame
    /// `frame_width` pixels wide.
    pub(crate) fn of(width: u32, frame_width: u32) -> Fidelity {
        let width = u64::from(width);
        let frame_width = u64::from(frame_width);
        if width >= frame_width {
            Fidelity::Full
        } else if 2 * width >= frame_width {
            Fidelity::Reduced
        } else {
            Fidelity::Coarse
        }
    }

    /// Its name in lower case: "full", "reduced" or "coarse".
    pub fn name(self) -> &'static str {
        match self {
            Fidelity::Full => "full",
            Fidelity::Reduced => "reduced",
            Fidelity::Coarse => "coarse",
        }
    }
}

impl fmt::Display for Fidelity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Rec. 709's luminance weights of red, green and blue.
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// One luminance sample per pixel, row-major, each in `[0, 1]`, and the
/// size of the frame the plane shows.
///
/// The frame is the picture at the size its areas are given in, such as
/// the camera's upright image. The plane shows the whole frame, upright
/// as the frame is, and may be smaller, as an embedded preview is; every
/// measure maps the frame's areas onto it, scaling each side by the
/// plane's against the frame's. So the plane must have the frame's
/// shape, to within 3 %: one that does not, such as a preview with black
/// bars, or one not turned upright when that changes its shape, would
/// put an area on the wrong pixels, and is refused. A square preview not
/// turned upright keeps its shape and cannot be told apart: turning it is
/// the caller's part.
///
/// Scores compare only between planes made the same way. A plane in
/// linear light, as [`Plane::from_rgb_f32`] makes from a developed
/// image, has weaker edges in its shadows and midtones than a
/// gamma-encoded one, as [`Plane::from_rgb8`] makes from a JPEG, so its
/// scores are lower for the same sharpness, though their fidelity is the
/// same.
///
/// # Examples
///
/// A decoded preview, here a tiny one, black on the left and white on the
/// right, showing a frame four times its size:
///
/// ```
/// use acutance::{Fidelity, Plane, Size};
///
/// # fn main() {
/// #     example().unwrap();
/// # }
/// # fn example() -> Option<()> {
/// let rgb = [0, 0, 0, 255, 255, 255];
/// let frame = Size { width: 8, height: 4 };
/// let plane = Plane::from_rgb8(2, 1, &rgb, frame)?;
/// assert_eq!(plane.samples(), [0.0, 1.0]);
/// assert_eq!(plane.fidelity(), Fidelity::Coarse);
/// # Some(())
/// # }
/// ```
#[derive(Clone, PartialEq)]
pub struct Plane {
    width: u32,
    height: u32,
    frame: Size,
    samples: Vec<f32>,
}

impl Plane {
    /// A plane from 8-bit RGB pixels, interleaved, row-major, as a decoded
    /// JPEG gives them.
    ///
    /// The luminance is weighed with Rec. 709's weights on the 8-bit values
    /// as they are, gamma-encoded, not linearised: edges are what is
    /// measured, and an encoded value tracks what the eye sees as an edge
    /// at least as well as a linear one.
    ///
    /// `None` when `rgb` does not hold `width × height` pixels of three
    /// bytes, when a side of the plane or of the frame is zero, or
    /// when the plane's shape is not its frame's (see [`Plane`]).
    pub fn from_rgb8(width: u32, height: u32, rgb: &[u8], frame: Size) -> Option<Plane> {
        if rgb.len() != pixels(width, height, frame)?.checked_mul(3)? {
            return None;
        }
        let (rgb, _) = rgb.as_chunks::<3>();
        let samples = rgb
            .iter()
            .map(|p| luminance(p.map(|v| f32::from(v) / 255.0)))
            .collect();
        Some(Plane {
            width,
            height,
            frame,
            samples,
        })
    }

    /// A plane from float RGB pixels, interleaved, row-major, as a demosaic
    /// gives them: linear light, unclipped, so a value may be above 1 or
    /// below 0.
    ///
    /// The luminance is weighed with Rec. 709's weights in `f32`, then
    /// clamped to `[0, 1]`; nothing is rounded to fewer levels on the way.
    /// The plane stays in linear light, and its scores compare only with
    /// those of other linear planes (see [`Plane`]).
    ///
    /// `None` when `rgb` does not hold `width × height` pixels of three
    /// values, when a side of the plane or of the frame is zero, or
    /// when the plane's shape is not its frame's (see [`Plane`]).
    pub fn from_rgb_f32(width: u32, height: u32, rgb: &[f32], frame: Size) -> Option<Plane> {
        if rgb.len() != pixels(width, height, frame)?.checked_mul(3)? {
            return None;
        }
        let (rgb, _) = rgb.as_chunks::<3>();
        let samples = rgb.iter().map(|&p| luminance(p)).collect();
        Some(Plane {
            width,
            height,
            frame,
            samples,
        })
    }

    /// A plane from one sample per pixel, row-major: a luminance, or the
    /// green of a sensor's data. Each sample is clamped to `[0, 1]`.
    ///
    /// `None` when `samples` does not hold `width × height` values, when a
    /// side of the plane or of the frame is zero, or when the plane's shape
    /// is not its frame's (see [`Plane`]).
    pub fn from_samples(width: u32, height: u32, samples: Vec<f32>, frame: Size) -> Option<Plane> {
        if samples.len() != pixels(width, height, frame)? {
            return None;
        }
        let samples = samples.into_iter().map(unit).collect();
        Some(Plane {
            width,
            height,
            frame,
            samples,
        })
    }

    /// The plane's width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The plane's height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The size of the frame the plane shows.
    pub fn frame(&self) -> Size {
        self.frame
    }

    /// How much of the frame's detail the plane holds.
    pub fn fidelity(&self) -> Fidelity {
        Fidelity::of(self.width, self.frame.width)
    }

    /// The samples, row-major, each in `[0, 1]`.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Only the sizes: a plane holds millions of samples.
impl fmt::Debug for Plane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plane")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

/// How far a plane's shape may differ from its frame's, as a share of the
/// frame's: a little for a preview rounded to whole pixels or trimmed by
/// a few, never enough to take a rotated or letterboxed one.
const SHAPE_TOLERANCE_PERCENT: u64 = 3;

/// The number of pixels of a `width × height` plane, `None` when it or
/// its frame is empty, when its shape is not its frame's, or when the
/// count overflows.
fn pixels(width: u32, height: u32, frame: Size) -> Option<usize> {
    if width == 0 || height == 0 || frame.is_empty() {
        return None;
    }
    // The plane's aspect ratio against the frame's, as width × frame
    // height against height × frame width, in whole numbers.
    let plane = u128::from(width) * u128::from(frame.height) * 100;
    let shown = u128::from(height) * u128::from(frame.width);
    let tolerance = u128::from(SHAPE_TOLERANCE_PERCENT);
    if plane < shown * (100 - tolerance) || plane > shown * (100 + tolerance) {
        return None;
    }
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)
}

/// The luminance of a red, green and blue, clamped to `[0, 1]`.
fn luminance(rgb: [f32; 3]) -> f32 {
    unit(LUMA[0] * rgb[0] + LUMA[1] * rgb[1] + LUMA[2] * rgb[2])
}

/// A sample clamped to `[0, 1]`; not a number becomes 0.
fn unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Size = Size {
        width: 2,
        height: 1,
    };

    #[test]
    fn from_rgb8_refuses_a_wrong_length() {
        assert!(Plane::from_rgb8(2, 1, &[0; 6], FRAME).is_some());
        assert!(Plane::from_rgb8(2, 1, &[0; 5], FRAME).is_none());
        assert!(Plane::from_rgb8(2, 1, &[0; 7], FRAME).is_none());
        assert!(Plane::from_rgb8(0, 1, &[], FRAME).is_none());
        let empty = Size {
            width: 0,
            height: 1,
        };
        assert!(Plane::from_rgb8(2, 1, &[0; 6], empty).is_none());
    }

    #[test]
    fn from_rgb8_weighs_green_most() {
        let frame = Size {
            width: 3,
            height: 1,
        };
        let plane = Plane::from_rgb8(3, 1, &[255, 0, 0, 0, 255, 0, 0, 0, 255], frame).unwrap();
        let [r, g, b] = [0, 1, 2].map(|i| plane.samples()[i]);
        assert!(
            (r - 0.2126).abs() < 1e-6 && (g - 0.7152).abs() < 1e-6 && (b - 0.0722).abs() < 1e-6
        );
    }

    #[test]
    fn from_samples_clamps_to_unit() {
        let frame = Size {
            width: 4,
            height: 1,
        };
        let plane = Plane::from_samples(4, 1, vec![-0.5, 1.5, 0.3, f32::NAN], frame).unwrap();
        assert_eq!(plane.samples(), &[0.0, 1.0, 0.3, 0.0]);
        assert!(Plane::from_samples(4, 1, vec![0.0; 3], frame).is_none());
    }

    #[test]
    fn from_rgb_f32_takes_a_linear_image_as_it_is() {
        // Above white: the same plane as white.
        let bright = Plane::from_rgb_f32(2, 1, &[3.0, 3.0, 3.0, 0.2, 0.2, 0.2], FRAME).unwrap();
        let white = Plane::from_rgb_f32(2, 1, &[1.0, 1.0, 1.0, 0.2, 0.2, 0.2], FRAME).unwrap();
        assert_eq!(bright, white);
        assert_eq!(bright.samples()[0], 1.0);
        // No rounding to 8 bits: a thousandth apart stays apart.
        let fine = Plane::from_rgb_f32(2, 1, &[0.5, 0.5, 0.5, 0.501, 0.501, 0.501], FRAME).unwrap();
        let [a, b] = [fine.samples()[0], fine.samples()[1]];
        assert!((b - a - 0.001).abs() < 1e-5, "{a} and {b}");
        assert!(Plane::from_rgb_f32(2, 1, &[0.0; 5], FRAME).is_none());
    }

    #[test]
    fn fidelity_follows_the_plane_s_width_against_its_frame() {
        let fidelity = |width, frame_width| {
            // Square planes of square frames.
            let frame = Size {
                width: frame_width,
                height: frame_width,
            };
            Plane::from_samples(width, width, vec![0.0; (width * width) as usize], frame)
                .unwrap()
                .fidelity()
        };
        assert_eq!(fidelity(100, 100), Fidelity::Full);
        assert_eq!(fidelity(120, 100), Fidelity::Full);
        assert_eq!(fidelity(99, 100), Fidelity::Reduced);
        assert_eq!(fidelity(50, 100), Fidelity::Reduced);
        assert_eq!(fidelity(49, 100), Fidelity::Coarse);
        assert_eq!(fidelity(50, 101), Fidelity::Coarse);
    }

    #[test]
    fn a_plane_of_another_shape_than_its_frame_is_refused() {
        let frame = Size {
            width: 6000,
            height: 4000,
        };
        let plane = |width: u32, height: u32| {
            Plane::from_samples(width, height, vec![0.0; (width * height) as usize], frame)
        };
        assert!(plane(1500, 1000).is_some());
        // 3 % taller or shorter: still the frame's shape.
        assert!(plane(1500, 1030).is_some() && plane(1500, 971).is_some());
        // Not turned upright, or with black bars.
        assert!(plane(1000, 1500).is_none());
        assert!(plane(1500, 1125).is_none() && plane(1500, 960).is_none());
        let rgb = vec![0; 1000 * 1500 * 3];
        assert!(Plane::from_rgb8(1000, 1500, &rgb, frame).is_none());
        let rgb = vec![0.0; 1000 * 1500 * 3];
        assert!(Plane::from_rgb_f32(1000, 1500, &rgb, frame).is_none());
    }

    #[test]
    fn the_types_go_to_other_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Size>();
        send_sync::<Fidelity>();
        send_sync::<Plane>();
    }

    #[test]
    fn a_size_prints_as_width_by_height() {
        assert_eq!(
            Size {
                width: 8256,
                height: 5504
            }
            .to_string(),
            "8256x5504"
        );
        assert_eq!(
            Size::default(),
            Size {
                width: 0,
                height: 0
            }
        );
    }

    #[test]
    fn more_detail_compares_greater() {
        assert!(Fidelity::Coarse < Fidelity::Reduced && Fidelity::Reduced < Fidelity::Full);
        assert_eq!(Fidelity::Reduced.to_string(), "reduced");
    }

    #[test]
    fn a_plane_s_debug_shows_its_sizes_only() {
        let plane = Plane::from_samples(2, 1, vec![0.25, 0.75], FRAME).unwrap();
        let debug = format!("{plane:?}");
        assert!(
            debug.contains("width: 2") && !debug.contains("0.25"),
            "{debug}"
        );
    }
}
