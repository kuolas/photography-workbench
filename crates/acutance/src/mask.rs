// SPDX-License-Identifier: GPL-3.0-or-later

//! Focus peaking: the pixels on the plane's strongest edges.

use std::fmt;

use crate::edges::{BINS, EdgeMap, bin};

/// How many pixels focus peaking marks: the strongest tenth, twentieth or
/// fiftieth of the edge map.
///
/// A level that marks fewer pixels compares greater, and it prints as its
/// [name](Level::name).
///
/// # Examples
///
/// ```
/// use acutance::Level;
///
/// let saved = Level::High.name();
/// assert_eq!(Level::from_saved(saved), Some(Level::High));
/// assert!(Level::Low < Level::High);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// The strongest 10 %.
    Low,
    /// The strongest 5 %.
    Mid,
    /// The strongest 2 %.
    High,
}

impl Level {
    /// Every level, from the one that marks the most.
    pub const ALL: [Level; 3] = [Level::Low, Level::Mid, Level::High];

    /// Its name in lower case, as it is saved: "low", "mid" or "high".
    pub fn name(self) -> &'static str {
        match self {
            Level::Low => "low",
            Level::Mid => "mid",
            Level::High => "high",
        }
    }

    /// The level saved under a name [`Level::name`] gave; `None` for any
    /// other.
    pub fn from_saved(name: &str) -> Option<Level> {
        Level::ALL.into_iter().find(|level| level.name() == name)
    }

    /// The share of the plane's pixels it marks.
    fn share(self) -> f64 {
        match self {
            Level::Low => 0.10,
            Level::Mid => 0.05,
            Level::High => 0.02,
        }
    }
}

/// The pixels focus peaking marks, one byte each over the plane the edge
/// map was made from: 255 on a peak, 0 elsewhere.
///
/// # Examples
///
/// ```
/// use acutance::{EdgeMap, Level, Plane, Size};
///
/// # fn main() {
/// #     example().unwrap();
/// # }
/// # fn example() -> Option<()> {
/// // Dark on the left, light on the right: one edge down the middle.
/// let samples = (0..64 * 48).map(|i| if i % 64 < 32 { 0.2 } else { 0.8 });
/// let frame = Size { width: 64, height: 48 };
/// let plane = Plane::from_samples(64, 48, samples.collect(), frame)?;
/// let mask = EdgeMap::of(&plane).peaks(Level::High);
/// assert_eq!((mask.width(), mask.height()), (64, 48));
/// // Only the columns at the edge are marked.
/// let row = &mask.alpha()[..64];
/// assert!(row[..28].iter().chain(&row[36..]).all(|&a| a == 0));
/// assert!(row[28..36].contains(&255));
/// # Some(())
/// # }
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Mask {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

impl Mask {
    /// The width in pixels, the plane's.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels, the plane's.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// One byte per pixel, row-major: 255 on a peak, 0 elsewhere.
    pub fn alpha(&self) -> &[u8] {
        &self.alpha
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Only the sizes: a mask holds millions of bytes.
impl fmt::Debug for Mask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mask")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl EdgeMap {
    /// The pixels on the plane's strongest edges, at a level.
    ///
    /// A level marks its share of the plane's pixels, the strongest; more
    /// when pixels at the threshold are as strong as each other, since all
    /// of them are marked, and fewer when fewer stand above the noise
    /// floor at all. A plane with no edges has no peaks.
    pub fn peaks(&self, level: Level) -> Mask {
        let mut histogram = vec![0_usize; BINS];
        for &value in &self.net {
            histogram[bin(value)] += 1;
        }
        let wanted = (self.net.len() as f64 * level.share()).ceil() as usize;
        // From the strongest bin down, the first at which the wanted count
        // is reached; bin 0, at the noise floor, is never a peak.
        let mut seen = 0;
        let mut threshold = 1;
        for index in (1..BINS).rev() {
            seen += histogram[index];
            if seen >= wanted {
                threshold = index;
                break;
            }
        }
        let alpha = self
            .net
            .iter()
            .map(|&value| if bin(value) >= threshold { 255 } else { 0 })
            .collect();
        Mask {
            width: self.size.width,
            height: self.size.height,
            alpha,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edges::tests::plane;

    fn marked(mask: &Mask) -> usize {
        mask.alpha().iter().filter(|&&a| a == 255).count()
    }

    #[test]
    fn the_mask_of_a_vertical_edge_is_a_vertical_band() {
        let edges = EdgeMap::of(&plane(64, 48, |x, _| if x < 32 { 0.0 } else { 1.0 }));
        let mask = edges.peaks(Level::High);
        assert_eq!((mask.width(), mask.height()), (64, 48));
        assert!(mask.alpha().iter().all(|&a| a == 0 || a == 255));
        let rows: Vec<&[u8]> = mask.alpha().chunks(64).collect();
        let first = rows[0];
        assert!(
            rows.iter().all(|row| *row == first),
            "every row marks the same columns"
        );
        let columns: Vec<usize> = (0..64).filter(|&x| first[x] == 255).collect();
        assert!(!columns.is_empty());
        assert!(
            columns.iter().all(|&x| (29..=34).contains(&x)),
            "{columns:?}"
        );
    }

    #[test]
    fn a_flat_plane_has_no_peaks() {
        let edges = EdgeMap::of(&plane(32, 24, |_, _| 0.3));
        for level in Level::ALL {
            assert_eq!(marked(&edges.peaks(level)), 0, "{level:?}");
        }
    }

    #[test]
    fn each_level_marks_its_share() {
        // Smooth ripples of every strength, few of them equal.
        let edges = EdgeMap::of(&plane(200, 150, |x, y| {
            let (x, y) = (x as f32, y as f32);
            0.5 + 0.2 * (x * 0.37 + (y * 0.11).sin() * 3.0).sin() * (y * 0.05).cos()
        }));
        let pixels = 200.0 * 150.0;
        for (level, wanted) in [(Level::Low, 0.10), (Level::Mid, 0.05), (Level::High, 0.02)] {
            let share = marked(&edges.peaks(level)) as f64 / pixels;
            assert!(
                share >= wanted && share < wanted + 0.01,
                "{level:?} marks {share}"
            );
        }
    }

    #[test]
    fn the_types_go_to_other_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Level>();
        send_sync::<Mask>();
    }

    #[test]
    fn a_level_marking_fewer_compares_greater() {
        assert!(Level::Low < Level::Mid && Level::Mid < Level::High);
        assert_eq!(Level::ALL.map(|l| l.to_string()), ["low", "mid", "high"]);
    }

    #[test]
    fn level_round_trips_through_its_saved_name() {
        for level in Level::ALL {
            assert_eq!(Level::from_saved(level.name()), Some(level));
        }
        assert_eq!(Level::from_saved("High"), None);
        assert_eq!(Level::from_saved(""), None);
    }
}
