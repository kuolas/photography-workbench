// SPDX-License-Identifier: GPL-3.0-or-later

//! A rectangle of the frame, and where it lands on a smaller plane.

use crate::plane::Size;

/// A rectangle in the frame's pixels: its top-left corner and its size.
///
/// Areas are given in the frame's coordinates, not the plane's, so the
/// same area measures the same place on a small preview as on the full
/// image. The default is the empty area at the frame's top-left corner.
///
/// # Examples
///
/// ```
/// use acutance::{Area, Size};
///
/// let frame = Size { width: 6000, height: 4000 };
/// let square = Size { width: 500, height: 500 };
/// // Near the right edge, the area is moved in to stay in the frame.
/// let focus = Area::centred(5900, 2000, square, frame);
/// assert_eq!(focus, Area { x: 5500, y: 1750, width: 500, height: 500 });
/// assert!(focus.intersects(&Area { x: 5000, y: 1500, width: 600, height: 600 }));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Area {
    /// The left edge.
    pub x: u32,
    /// The top edge.
    pub y: u32,
    /// The width; 0 makes the area empty.
    pub width: u32,
    /// The height; 0 makes the area empty.
    pub height: u32,
}

impl Area {
    /// Whether the two areas share at least one pixel. An empty area shares
    /// none.
    pub fn intersects(&self, other: &Area) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        let (a, b) = (self.span(), other.span());
        a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
    }

    /// An area of `size` centred on (`center_x`, `center_y`), moved and if
    /// need be shrunk so that it lies within `within`, a frame whose
    /// top-left corner is at (0, 0). A side of even length cannot be
    /// centred on a pixel: it has one pixel more before the centre than
    /// after it.
    pub fn centred(center_x: u32, center_y: u32, size: Size, within: Size) -> Area {
        let (x, width) = centred_span(center_x, size.width, within.width);
        let (y, height) = centred_span(center_y, size.height, within.height);
        Area {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether the area has no pixel.
    fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The left, top, right and bottom edges, the right and bottom ones
    /// past the area's last pixel.
    fn span(&self) -> (u64, u64, u64, u64) {
        let (x, y) = (u64::from(self.x), u64::from(self.y));
        (x, y, x + u64::from(self.width), y + u64::from(self.height))
    }

    /// The area on a plane of `plane` pixels showing a frame of `frame`
    /// pixels: clipped to the frame, then scaled outwards, so that every
    /// plane pixel the area touches is in it. `None` when nothing is left.
    pub(crate) fn on(&self, plane: Size, frame: Size) -> Option<Pixels> {
        let (left, top, right, bottom) = self.span();
        let (fw, fh) = (u64::from(frame.width), u64::from(frame.height));
        let (pw, ph) = (u64::from(plane.width), u64::from(plane.height));
        let (right, bottom) = (right.min(fw), bottom.min(fh));
        if left >= right || top >= bottom {
            return None;
        }
        // Each value is at most the plane's side, a u32, so it fits a usize.
        let scale = |v: u64, to: u64, from: u64, up: bool| {
            let scaled = if up {
                (v * to).div_ceil(from)
            } else {
                v * to / from
            };
            scaled as usize
        };
        Some(Pixels {
            left: scale(left, pw, fw, false),
            top: scale(top, ph, fh, false),
            right: scale(right, pw, fw, true),
            bottom: scale(bottom, ph, fh, true),
        })
    }
}

/// A rectangle of a plane's pixels, the right and bottom edges past its
/// last pixel; never empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pixels {
    pub(crate) left: usize,
    pub(crate) top: usize,
    pub(crate) right: usize,
    pub(crate) bottom: usize,
}

/// The start and length of a span of `length` centred on `center`, within
/// `0..within`.
fn centred_span(center: u32, length: u32, within: u32) -> (u32, u32) {
    let length = length.min(within);
    let start = center.saturating_sub(length / 2).min(within - length);
    (start, length)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn area(x: u32, y: u32, width: u32, height: u32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn an_area_goes_to_other_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Area>();
    }

    #[test]
    fn areas_intersect_only_when_they_share_a_pixel() {
        let a = area(10, 10, 10, 10);
        assert!(a.intersects(&area(19, 19, 5, 5)));
        assert!(
            !a.intersects(&area(20, 10, 5, 5)),
            "touching edges share no pixel"
        );
        assert!(
            !a.intersects(&area(12, 12, 0, 5)),
            "an empty area shares none"
        );
        assert!(area(0, 0, u32::MAX, u32::MAX).intersects(&area(u32::MAX - 1, 0, 1, 1)));
    }

    #[test]
    fn a_centred_area_stays_within_its_frame() {
        let size = Size {
            width: 100,
            height: 60,
        };
        let within = Size {
            width: 1000,
            height: 500,
        };
        assert_eq!(
            Area::centred(500, 250, size, within),
            area(450, 220, 100, 60)
        );
        assert_eq!(Area::centred(10, 490, size, within), area(0, 440, 100, 60));
        assert_eq!(Area::centred(2000, 0, size, within), area(900, 0, 100, 60));
        let small = Size {
            width: 40,
            height: 30,
        };
        assert_eq!(Area::centred(5, 5, size, small), area(0, 0, 40, 30));
    }

    #[test]
    fn an_area_lands_on_the_same_pixels_at_any_scale() {
        let frame = Size {
            width: 400,
            height: 300,
        };
        let a = area(101, 57, 83, 41);
        let full = a.on(frame, frame).unwrap();
        assert_eq!(
            full,
            Pixels {
                left: 101,
                top: 57,
                right: 184,
                bottom: 98
            }
        );
        let quarter = a
            .on(
                Size {
                    width: 100,
                    height: 75,
                },
                frame,
            )
            .unwrap();
        // One pixel of the quarter plane is four of the frame.
        for (small, large) in [
            (quarter.left, full.left),
            (quarter.top, full.top),
            (quarter.right, full.right),
            (quarter.bottom, full.bottom),
        ] {
            assert!(
                (small * 4).abs_diff(large) <= 4,
                "{quarter:?} against {full:?}"
            );
        }
        // Scaled outwards: the quarter plane's rectangle covers the area.
        assert!(quarter.left * 4 <= full.left && quarter.right * 4 >= full.right);
        assert!(quarter.top * 4 <= full.top && quarter.bottom * 4 >= full.bottom);
    }

    #[test]
    fn an_area_is_clipped_to_its_frame() {
        let frame = Size {
            width: 400,
            height: 300,
        };
        let plane = Size {
            width: 200,
            height: 150,
        };
        let pixels = area(350, 250, 100, 100).on(plane, frame).unwrap();
        assert_eq!(
            pixels,
            Pixels {
                left: 175,
                top: 125,
                right: 200,
                bottom: 150
            }
        );
        assert_eq!(area(400, 0, 10, 10).on(plane, frame), None);
        assert_eq!(area(10, 10, 0, 10).on(plane, frame), None);
    }
}
