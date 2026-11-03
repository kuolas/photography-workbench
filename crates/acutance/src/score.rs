// SPDX-License-Identifier: GPL-3.0-or-later

//! How sharp an area of the frame is.

use crate::area::Area;
use crate::edges::EdgeMap;
use crate::plane::Fidelity;

/// The share of an area's pixels, the strongest, that its score is the
/// mean of.
const STRONGEST: f64 = 0.05;

/// How sharp an area is: a value in `[0, 1]`, higher for sharper, with how
/// many pixels it was read from and the fidelity of the plane it was read
/// on.
///
/// Scores of different fidelity are not comparable, so a score has no
/// order of its own: compare the [values](Score::value) of scores of one
/// fidelity.
///
/// # Examples
///
/// ```
/// use acutance::{Area, EdgeMap, Plane, Size};
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
/// let left = edges.score(&Area { x: 0, y: 0, width: 16, height: 48 });
/// let middle = edges.score(&Area { x: 24, y: 0, width: 16, height: 48 });
/// assert_eq!(left.value(), 0.0);
/// assert!(middle.value() > 0.1);
/// assert_eq!(middle.pixels(), 16 * 48);
/// # Some(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    value: f32,
    pixels: u32,
    fidelity: Fidelity,
}

impl Score {
    /// A score made by hand, as a test of a consumer makes one. The value
    /// is stored as it is given.
    pub fn new(value: f32, pixels: u32, fidelity: Fidelity) -> Score {
        Score {
            value,
            pixels,
            fidelity,
        }
    }

    /// How sharp: in `[0, 1]` for a score the crate reads, 0 for an area
    /// with no edge above the noise floor; as given for one made with
    /// [`Score::new`].
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The number of the plane's pixels the score was read from; 0 when
    /// the area lies outside the frame.
    pub fn pixels(&self) -> u32 {
        self.pixels
    }

    /// The fidelity of the plane the score was read on.
    pub fn fidelity(&self) -> Fidelity {
        self.fidelity
    }
}

impl EdgeMap {
    /// How sharp an area of the frame is: the mean of the strongest 5 % of
    /// its edge map's values.
    ///
    /// The strongest, not all: a subject in focus rarely fills the area
    /// it is measured in, and the mean of every pixel would mark down a
    /// sharp eye against a smooth cheek. The area is clipped to the frame;
    /// an empty one scores 0 from 0 pixels.
    pub fn score(&self, area: &Area) -> Score {
        let mut values = Vec::new();
        self.score_with(area, &mut values)
    }

    /// [`EdgeMap::score`], with a buffer to reuse for the area's values.
    pub(crate) fn score_with(&self, area: &Area, values: &mut Vec<f32>) -> Score {
        values.clear();
        if let Some(pixels) = area.on(self.size, self.frame) {
            let width = self.size.width as usize;
            for y in pixels.top..pixels.bottom {
                values.extend_from_slice(
                    &self.net[y * width + pixels.left..y * width + pixels.right],
                );
            }
        }
        let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
        Score::new(strongest_mean(values), count, self.fidelity())
    }
}

/// The mean of the strongest [`STRONGEST`] of the values, at least one of
/// them; 0 when there are none. Reorders the values.
fn strongest_mean(values: &mut [f32]) -> f32 {
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    let k = ((n as f64 * STRONGEST).ceil() as usize).clamp(1, n);
    values.select_nth_unstable_by(n - k, f32::total_cmp);
    let sum: f64 = values[n - k..].iter().map(|&v| f64::from(v)).sum();
    (sum / k as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edges::tests::plane;
    use crate::plane::Plane;

    /// The whole frame of a plane.
    fn whole(plane: &Plane) -> Area {
        let frame = plane.frame();
        Area {
            x: 0,
            y: 0,
            width: frame.width,
            height: frame.height,
        }
    }

    /// A pseudo-random value in `[0, 1)` for each step of a fixed linear
    /// congruential generator (Knuth's MMIX constants).
    fn noise(seed: u64) -> impl FnMut() -> f32 {
        let mut state = seed;
        move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 40) as f32 / (1_u64 << 24) as f32
        }
    }

    #[test]
    fn a_blurred_edge_scores_below_the_sharp_one() {
        let step = |x: u32| if x < 32 { 0.2 } else { 0.8 };
        let sharp = plane(64, 48, |x, _| step(x));
        // A 5 × 5 box blur of the same step is a 5 × 1 one: it runs
        // down the edge.
        let blurred = plane(64, 48, |x, _| {
            (x.saturating_sub(2)..=(x + 2))
                .map(|x| step(x.min(63)))
                .sum::<f32>()
                / 5.0
        });
        let sharp = EdgeMap::of(&sharp).score(&whole(&sharp));
        let blurred = EdgeMap::of(&blurred).score(&whole(&blurred));
        assert!(
            blurred.value() < sharp.value(),
            "{blurred:?} against {sharp:?}"
        );
        assert!(blurred.value() > 0.0);
    }

    #[test]
    fn a_flat_area_scores_zero() {
        let flat = plane(32, 24, |_, _| 0.6);
        let score = EdgeMap::of(&flat).score(&whole(&flat));
        assert_eq!(score.value(), 0.0);
        assert_eq!(score.pixels(), 32 * 24);
        assert_eq!(score.fidelity(), Fidelity::Full);
    }

    #[test]
    fn an_area_outside_the_frame_scores_nothing() {
        let edges = EdgeMap::of(&plane(32, 24, |x, _| if x < 16 { 0.0 } else { 1.0 }));
        let outside = Area {
            x: 32,
            y: 0,
            width: 10,
            height: 10,
        };
        assert_eq!(edges.score(&outside), Score::new(0.0, 0, Fidelity::Full));
        let empty = Area {
            x: 10,
            y: 10,
            width: 0,
            height: 4,
        };
        assert_eq!(edges.score(&empty).pixels(), 0);
    }

    #[test]
    fn an_area_scores_its_own_edges_only() {
        let edges = EdgeMap::of(&plane(64, 48, |x, _| if x < 48 { 0.0 } else { 1.0 }));
        let left = edges.score(&Area {
            x: 0,
            y: 0,
            width: 32,
            height: 48,
        });
        let right = edges.score(&Area {
            x: 32,
            y: 0,
            width: 32,
            height: 48,
        });
        assert_eq!(left.value(), 0.0);
        assert!(right.value() > 0.1, "{right:?}");
    }

    #[test]
    fn the_noise_floor_removes_uniform_noise() {
        // Grain of ±2 % about mid grey, as a high-ISO frame has.
        let mut grain = noise(7);
        let grain: Vec<f32> = (0..128 * 96).map(|_| (grain() - 0.5) * 0.04).collect();
        let noisy = plane(128, 96, |x, y| 0.5 + grain[(y * 128 + x) as usize]);
        let with_edge = plane(128, 96, |x, y| {
            let step = if x < 64 { 0.25 } else { 0.75 };
            step + grain[(y * 128 + x) as usize]
        });
        let noisy = EdgeMap::of(&noisy).score(&whole(&noisy));
        let with_edge = EdgeMap::of(&with_edge).score(&whole(&with_edge));
        assert!(noisy.value() < 0.02, "{noisy:?}");
        assert!(with_edge.value() > 0.1, "{with_edge:?}");
        assert!(
            with_edge.value() > 5.0 * noisy.value(),
            "{with_edge:?} against {noisy:?}"
        );
    }

    #[test]
    fn the_types_go_to_other_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Score>();
    }

    #[test]
    fn the_strongest_mean_takes_the_top_twentieth() {
        let mut values: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        // The top five: 0.95 to 0.99.
        assert!((strongest_mean(&mut values) - 0.97).abs() < 1e-6);
        assert_eq!(strongest_mean(&mut [0.3]), 0.3);
        assert_eq!(strongest_mean(&mut []), 0.0);
    }
}
