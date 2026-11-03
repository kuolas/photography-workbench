// SPDX-License-Identifier: GPL-3.0-or-later

//! The edge map: how strong an edge each pixel sits on, above the plane's
//! noise floor.

use std::fmt;

use crate::plane::{Fidelity, Plane, Size};

/// The largest magnitude the Sobel operator gives on samples in `[0, 1]`:
/// √(4² + 2²), at a corner such as
///
/// ```text
/// 0 0 1
/// 0 · 1
/// 0 1 1
/// ```
///
/// where the horizontal gradient is 4 and the vertical one 2. The two
/// cannot both reach 4, which they each can alone.
const SOBEL_MAX: f32 = 4.472_136; // 2√5

/// The histogram's bins over `[0, 1]`, for the median magnitude: fine
/// enough that the median is off by less than 1/65 535, a tiny fraction of
/// the weakest edge worth measuring.
pub(crate) const BINS: usize = 1 << 16;

/// How strong an edge each pixel of a plane sits on: the magnitude of the
/// plane's gradient, in `[0, 1]`, less the plane's noise floor.
///
/// Made in three steps. The plane is blurred with a 3 × 3 binomial kernel,
/// which takes the edge of one noisy pixel down far more than an edge
/// between two areas. The Sobel operator gives each pixel's gradient,
/// whose magnitude is divided by the largest the operator can give, so it
/// is in `[0, 1]`. Then the plane's median magnitude is subtracted, and
/// what falls below it is set to zero: noise lifts the gradient
/// everywhere, and without this floor a high-ISO frame would score as
/// sharp everywhere. Most of a photo is smooth, so the median is what its
/// noise and grain give, not what its edges give.
///
/// # Examples
///
/// ```
/// use acutance::{EdgeMap, Fidelity, Plane, Size};
///
/// # fn main() {
/// #     example().unwrap();
/// # }
/// # fn example() -> Option<()> {
/// // Dark on the left, light on the right: one edge down the middle.
/// let samples = (0..64 * 48).map(|i| if i % 64 < 32 { 0.2 } else { 0.8 });
/// let frame = Size { width: 64, height: 48 };
/// let plane = Plane::from_samples(64, 48, samples.collect(), frame)?;
/// let edges = EdgeMap::of(&plane);
/// assert_eq!(edges.fidelity(), Fidelity::Full);
/// # Some(())
/// # }
/// ```
#[derive(Clone, PartialEq)]
pub struct EdgeMap {
    pub(crate) size: Size,
    pub(crate) frame: Size,
    /// Row-major, one value per pixel of the plane, each in `[0, 1]`.
    pub(crate) net: Vec<f32>,
}

impl EdgeMap {
    /// The edge map of a plane.
    pub fn of(plane: &Plane) -> EdgeMap {
        let size = Size {
            width: plane.width(),
            height: plane.height(),
        };
        let (width, height) = (size.width as usize, size.height as usize);
        let blurred = blur(plane.samples(), width, height);
        let mut net = sobel(&blurred, width, height);
        let floor = median(&net);
        for value in &mut net {
            *value = (*value - floor).max(0.0);
        }
        EdgeMap {
            size,
            frame: plane.frame(),
            net,
        }
    }

    /// The fidelity of the plane the map was made from.
    pub fn fidelity(&self) -> Fidelity {
        Fidelity::of(self.size.width, self.frame.width)
    }
}

/// Only the sizes: a map holds millions of values.
impl fmt::Debug for EdgeMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdgeMap")
            .field("size", &self.size)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

/// The 3 × 3 binomial blur (1 2 1 across, then down, over 16), the
/// samples past the plane's edge taken as the edge's own.
fn blur(samples: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut across = vec![0.0; samples.len()];
    for (row, out) in samples
        .chunks_exact(width)
        .zip(across.chunks_exact_mut(width))
    {
        for x in 0..width {
            let (left, right) = (row[x.saturating_sub(1)], row[(x + 1).min(width - 1)]);
            out[x] = (left + 2.0 * row[x] + right) * 0.25;
        }
    }
    let mut out = vec![0.0; samples.len()];
    for (y, row) in out.chunks_exact_mut(width).enumerate() {
        let [up, middle, down] = rows(&across, width, height, y);
        for x in 0..width {
            row[x] = (up[x] + 2.0 * middle[x] + down[x]) * 0.25;
        }
    }
    out
}

/// The Sobel operator's magnitude at every pixel, over [`SOBEL_MAX`], the
/// samples past the plane's edge taken as the edge's own.
fn sobel(samples: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut out = vec![0.0; samples.len()];
    for (y, row) in out.chunks_exact_mut(width).enumerate() {
        let [up, middle, down] = rows(samples, width, height, y);
        for x in 0..width {
            let (l, r) = (x.saturating_sub(1), (x + 1).min(width - 1));
            let gx = (up[r] - up[l]) + 2.0 * (middle[r] - middle[l]) + (down[r] - down[l]);
            let gy = (down[l] + 2.0 * down[x] + down[r]) - (up[l] + 2.0 * up[x] + up[r]);
            row[x] = ((gx * gx + gy * gy).sqrt() / SOBEL_MAX).min(1.0);
        }
    }
    out
}

/// The rows above, at and below row `y`, the plane's first and last rows
/// standing in for those past its edge.
fn rows(samples: &[f32], width: usize, height: usize, y: usize) -> [&[f32]; 3] {
    let row = |y: usize| &samples[y * width..(y + 1) * width];
    [
        row(y.saturating_sub(1)),
        row(y),
        row((y + 1).min(height - 1)),
    ]
}

/// The bin of a value in `[0, 1]`.
pub(crate) fn bin(value: f32) -> usize {
    ((value * (BINS - 1) as f32) as usize).min(BINS - 1)
}

/// The median of values in `[0, 1]`, to the lower edge of its bin.
fn median(values: &[f32]) -> f32 {
    let mut histogram = vec![0_usize; BINS];
    for &value in values {
        histogram[bin(value)] += 1;
    }
    let half = values.len().div_ceil(2);
    let mut seen = 0;
    for (index, count) in histogram.into_iter().enumerate() {
        seen += count;
        if seen >= half {
            return index as f32 / (BINS - 1) as f32;
        }
    }
    1.0
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A plane of `width × height` samples the function gives at each
    /// pixel, showing a frame of its own size.
    pub(crate) fn plane(width: u32, height: u32, sample: impl Fn(u32, u32) -> f32) -> Plane {
        let samples = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
        let samples = samples.map(|(x, y)| sample(x, y)).collect();
        Plane::from_samples(width, height, samples, Size { width, height }).unwrap()
    }

    #[test]
    fn an_edge_map_goes_to_other_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<EdgeMap>();
    }

    #[test]
    fn sobel_reaches_one_at_most() {
        let mut largest: f32 = 0.0;
        for pattern in 0_u32..256 {
            // The eight neighbours of the centre, each 0 or 1.
            let mut samples = [0.0; 9];
            for (bit, index) in [0, 1, 2, 3, 5, 6, 7, 8].into_iter().enumerate() {
                samples[index] = ((pattern >> bit) & 1) as f32;
            }
            largest = largest.max(sobel(&samples, 3, 3)[4]);
        }
        assert!((largest - 1.0).abs() < 1e-6, "{largest}");
    }

    #[test]
    fn a_flat_plane_has_no_edges() {
        let edges = EdgeMap::of(&plane(16, 12, |_, _| 0.4));
        assert!(edges.net.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn the_noise_floor_is_the_median_magnitude() {
        // A step: a quarter of the columns sit on its edge, the rest are
        // flat, so the median magnitude is zero and the edge is kept whole.
        let step = EdgeMap::of(&plane(16, 8, |x, _| if x < 8 { 0.0 } else { 1.0 }));
        assert!(step.net.iter().any(|&v| v > 0.5));
        // A grating, two columns dark and two light: every column sits on
        // an edge of the same strength, so the median is that edge, and
        // nothing stands above it but the outermost columns, where the
        // plane's edge breaks the pattern.
        let grating = EdgeMap::of(&plane(16, 8, |x, _| ((x / 2) % 2) as f32));
        let inner = grating
            .net
            .as_chunks::<16>()
            .0
            .iter()
            .flat_map(|row| &row[2..14]);
        assert!(
            inner.into_iter().all(|&v| v < 0.001),
            "{:?}",
            &grating.net[..16]
        );
    }

    #[test]
    #[ignore = "a timing: cargo test -p acutance --release -- --ignored timing --nocapture"]
    fn timing_of_the_edge_map_over_24_megapixels() {
        use std::time::Instant;
        let (width, height) = (6000, 4000);
        let plane = plane(width, height, |x, y| ((x ^ y) & 0xff) as f32 / 255.0);
        let (w, h) = (width as usize, height as usize);
        let start = Instant::now();
        let blurred = blur(plane.samples(), w, h);
        let blurring = start.elapsed();
        let magnitudes = sobel(&blurred, w, h);
        let sobel = start.elapsed() - blurring;
        let floor = median(&magnitudes);
        let median = start.elapsed() - blurring - sobel;
        let start = Instant::now();
        let edges = EdgeMap::of(&plane);
        let whole = start.elapsed();
        eprintln!(
            "6000 × 4000: blur {blurring:?}, Sobel {sobel:?}, median {median:?} \
             ({floor}); EdgeMap::of {whole:?}"
        );
        assert_eq!(edges.net.len(), w * h);
    }

    #[test]
    fn the_median_of_values_is_their_middle_one() {
        assert_eq!(median(&[0.0, 0.0, 1.0]), 0.0);
        let m = median(&[0.1, 0.9, 0.5, 0.2, 0.7]);
        assert!((m - 0.5).abs() < 1e-4, "{m}");
    }
}
