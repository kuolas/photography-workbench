// SPDX-License-Identifier: GPL-3.0-or-later

//! How large the viewer shows its photo, and which part of it: the maths
//! of the zoom, with nothing of egui's input or drawing in it, so it is
//! tested on its own.
//!
//! A photo is shown fitted in the viewer's room ([`Zoom::Fit`]), or at a
//! scale relative to 100 % ([`Zoom::Scale`]). At 100 % each pixel of the
//! photo's largest image, a JPEG's own or a RAW file's largest preview,
//! takes one pixel of the screen, so the photo is as many points long as
//! that image has pixels, divided by the screen's pixels a point. Zooming
//! goes no further out than the fit and no further in than
//! [`MAX_SCALE`]; only a click goes to 100 % when that is smaller than the
//! fit, as a small photo is enlarged to fit.
//!
//! Which part of a photo larger than the room is shown is its pan: the
//! point of the photo at the room's middle, in fractions of the photo's
//! width and height. It is bounded on each side apart ([`Stage::bound`]):
//! where the photo is larger than the room, its edge never comes inside
//! the room's edge, and where it is not, it is in the middle. The zoom and
//! the pan together are a [`Framing`], which says nothing of the photo
//! itself, so it is kept from one photo to the next, and bounded for each.
//!
//! A [`Stage`], a photo's size in a room on a screen, works out where a
//! framing puts the photo ([`Stage::place`]) and how the pointer changes
//! it: zooming about a point of the room, the point of the photo under it
//! staying there ([`Stage::zoom_about`]); a click, which goes between
//! fitted and 100 % ([`Stage::click`]); and a pan by a drag or a scroll
//! ([`Stage::pan_by`]). A photo drawn at 100 % has its corner put on a
//! whole pixel of the screen ([`Stage::snapped`]), so each of its pixels
//! falls on one of the screen's and none is blurred across two. The move
//! a click makes is eased over [`GLIDE`] seconds ([`glided`]).

use std::cmp::Ordering;

use egui::emath::easing;
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use previews::Size;

/// The largest scale a photo is zoomed to: 800 %.
pub const MAX_SCALE: f32 = 8.0;

/// How much larger than the room, in points, a photo must be on a side to
/// count as larger: a photo fitted, or zoomed in by a hair, is not.
const LARGER_BY: f32 = 1.0;

/// How long the move a click makes takes, in seconds.
pub const GLIDE: f64 = 0.25;

/// How large a photo is shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    /// As large as fits in the room, its aspect kept.
    Fit,
    /// At this scale relative to 100 %: 1 is 100 %, 2 is 200 %.
    Scale(f32),
}

/// How large a photo is shown and which part of it, whatever the photo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// How large.
    pub zoom: Zoom,
    /// The point of the photo at the room's middle, in fractions of its
    /// width and height, from its top left corner. A photo fitted is in
    /// the middle, whatever this says.
    pub pan: Vec2,
}

impl Default for Framing {
    /// Fitted, in the middle.
    fn default() -> Self {
        Self {
            zoom: Zoom::Fit,
            pan: vec2(0.5, 0.5),
        }
    }
}

/// A photo in a room on a screen: what a [`Framing`] is worked out from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    /// The width and height of the photo's largest image, in pixels.
    photo: Vec2,
    /// Where the photo is shown, in points.
    room: Rect,
    /// The screen's pixels a point.
    pixels_per_point: f32,
}

impl Stage {
    /// A photo whose largest image is `largest`, shown in `room` on a
    /// screen of `pixels_per_point`.
    pub fn new(largest: Size, room: Rect, pixels_per_point: f32) -> Self {
        Self {
            photo: vec2(largest.width as f32, largest.height as f32),
            room,
            pixels_per_point,
        }
    }

    /// The scale, relative to 100 %, at which the photo fits in the room:
    /// above 1 for a photo smaller than the room at 100 %.
    pub fn fit(&self) -> f32 {
        let room = self.room.size();
        self.pixels_per_point * (room.x / self.photo.x).min(room.y / self.photo.y)
    }

    /// The scale `zoom` shows the photo at.
    fn scale(&self, zoom: Zoom) -> f32 {
        match zoom {
            Zoom::Fit => self.fit(),
            Zoom::Scale(scale) => scale,
        }
    }

    /// The photo's width and height at `scale`, in points.
    fn size(&self, scale: f32) -> Vec2 {
        self.photo * scale / self.pixels_per_point
    }

    /// `pan` bounded for the photo at `scale`, on each side apart: where
    /// the photo is larger than the room, so far that its edge comes no
    /// further in than the room's edge; where it is not, the middle.
    pub fn bound(&self, scale: f32, pan: Vec2) -> Vec2 {
        let size = self.size(scale);
        let room = self.room.size();
        let side = |pan: f32, size: f32, room: f32| {
            if size > room {
                // Half the part of the photo the room shows.
                let half = room / size / 2.0;
                pan.clamp(half, 1.0 - half)
            } else {
                0.5
            }
        };
        vec2(side(pan.x, size.x, room.x), side(pan.y, size.y, room.y))
    }

    /// Where `framing` puts the photo, in points: fitted in the middle of
    /// the room, or at its scale with its pan bounded at the room's
    /// middle. It may reach past the room, which shows only what is in it.
    pub fn place(&self, framing: Framing) -> Rect {
        let scale = self.scale(framing.zoom);
        let size = self.size(scale);
        match framing.zoom {
            Zoom::Fit => Rect::from_center_size(self.room.center(), size),
            Zoom::Scale(_) => {
                let pan = self.bound(scale, framing.pan);
                Rect::from_min_size(self.room.center() - pan * size, size)
            }
        }
    }

    /// Whether `framing` shows the photo larger than the room, by more
    /// than [`LARGER_BY`] on either side, so a drag pans it.
    pub fn larger(&self, framing: Framing) -> bool {
        let over = self.size(self.scale(framing.zoom)) - self.room.size();
        over.x > LARGER_BY || over.y > LARGER_BY
    }

    /// How the photo drawn `width` pixels wide on the screen stands to
    /// 100 %: at it, `Equal`, while its width is within half a pixel of
    /// its largest image's, smaller or larger past that.
    fn against_full(&self, width: f32) -> Ordering {
        if width < self.photo.x - 0.5 {
            Ordering::Less
        } else if width > self.photo.x + 0.5 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }

    /// Whether a click on the photo framed by `framing` fits it, rather
    /// than showing it at 100 %: it does when the photo is zoomed larger
    /// than the room, or at 100 % or past it, as a small photo is when
    /// zoomed past 100 % but no larger than the room, which the click
    /// enlarges to fit; never when it is fitted.
    pub fn click_fits(&self, framing: Framing) -> bool {
        match framing.zoom {
            Zoom::Fit => false,
            Zoom::Scale(scale) => {
                self.larger(framing) || self.against_full(self.photo.x * scale) != Ordering::Less
            }
        }
    }

    /// Whether a click on the photo framed by `framing` shows it larger
    /// than it is now, fitting it or showing it at 100 %
    /// ([`click_fits`](Self::click_fits)), rather than smaller or as large.
    pub fn click_enlarges(&self, framing: Framing) -> bool {
        let to = if self.click_fits(framing) {
            Zoom::Fit
        } else {
            Zoom::Scale(1.0)
        };
        self.scale(to) > self.scale(framing.zoom)
    }

    /// The framing a click at `at` makes of the photo framed by `framing`
    /// and drawn at `drawn`: fitted ([`click_fits`](Self::click_fits)), or
    /// else at 100 % with the point clicked at the room's middle, as far
    /// as the bounds allow. A click past the photo's edge goes by the
    /// nearest point of the photo.
    pub fn click(&self, framing: Framing, drawn: Rect, at: Pos2) -> Framing {
        if self.click_fits(framing) {
            return Framing {
                zoom: Zoom::Fit,
                ..framing
            };
        }
        let point = ((at - drawn.min) / drawn.size()).clamp(Vec2::ZERO, Vec2::splat(1.0));
        Framing {
            zoom: Zoom::Scale(1.0),
            pan: self.bound(1.0, point),
        }
    }

    /// The framing that zooming the photo framed by `framing` by `factor`
    /// about `anchor`, a point of the room, makes: the point of the photo
    /// under `anchor` stays there, as far as the bounds allow. It zooms in
    /// no further than [`MAX_SCALE`], and zooming out to the fit or below
    /// fits the photo; a photo shown no larger than fitted, as a small one
    /// at 100 %, is not zoomed out at all.
    pub fn zoom_about(&self, framing: Framing, factor: f32, anchor: Pos2) -> Framing {
        let scale = self.scale(framing.zoom);
        let fit = self.fit();
        let zoom = if factor < 1.0 {
            if scale <= fit {
                return framing;
            }
            let scale = scale * factor;
            if scale <= fit {
                Zoom::Fit
            } else {
                Zoom::Scale(scale)
            }
        } else if factor > 1.0 && scale < MAX_SCALE {
            Zoom::Scale((scale * factor).min(MAX_SCALE))
        } else {
            return framing;
        };
        let drawn = self.place(framing);
        let point = (anchor - drawn.min) / drawn.size();
        let scale = self.scale(zoom);
        let pan = point + (self.room.center() - anchor) / self.size(scale);
        Framing {
            zoom,
            pan: self.bound(scale, pan),
        }
    }

    /// The framing that moving the photo framed by `framing` by `delta`
    /// points makes, the photo following it as far as the bounds allow.
    /// A photo fitted is not moved.
    pub fn pan_by(&self, framing: Framing, delta: Vec2) -> Framing {
        let Zoom::Scale(scale) = framing.zoom else {
            return framing;
        };
        let pan = self.bound(scale, framing.pan) - delta / self.size(scale);
        Framing {
            pan: self.bound(scale, pan),
            ..framing
        }
    }

    /// `drawn`, where the photo is drawn, with its top left corner on a
    /// whole pixel of the screen when it is drawn at 100 %, so each of its
    /// pixels falls on one of the screen's; as it is otherwise.
    pub fn snapped(&self, drawn: Rect) -> Rect {
        let ppp = self.pixels_per_point;
        if self.against_full(drawn.width() * ppp) != Ordering::Equal {
            return drawn;
        }
        let corner = pos2(
            (drawn.min.x * ppp).round() / ppp,
            (drawn.min.y * ppp).round() / ppp,
        );
        Rect::from_min_size(corner, drawn.size())
    }
}

/// Where a photo moving from `from` to `to` over [`GLIDE`] seconds since
/// egui's time `since` is drawn at egui's time `now`: eased in and out,
/// at `to` from then on.
pub fn glided(from: Rect, to: Rect, since: f64, now: f64) -> Rect {
    let t = ((now - since) / GLIDE).clamp(0.0, 1.0) as f32;
    from.lerp_towards(&to, easing::cubic_in_out(t))
}

/// Whether a move begun at egui's time `since` is still under way at
/// `now`.
pub fn gliding(since: f64, now: f64) -> bool {
    now - since < GLIDE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 600 by 400 point room, its top left corner at (100, 50).
    fn room() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(600.0, 400.0))
    }

    /// A photo whose largest image is `width` by `height` pixels, in
    /// [`room`] on a screen of `pixels_per_point`.
    fn photo(width: u32, height: u32, pixels_per_point: f32) -> Stage {
        Stage::new(Size { width, height }, room(), pixels_per_point)
    }

    /// Whether `a` and `b` are the same within a hundredth.
    fn near(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 0.01
    }

    /// Whether `a` and `b` are the same rectangle within a hundredth of a
    /// point.
    fn same(a: Rect, b: Rect) -> bool {
        near(a.min.to_vec2(), b.min.to_vec2()) && near(a.max.to_vec2(), b.max.to_vec2())
    }

    /// The photo fits at the scale that makes it reach two sides of the
    /// room: below 1 for a photo larger than the room at 100 %, above for
    /// a smaller one, and twice as much on a screen of two pixels a point.
    #[test]
    fn the_fit_is_the_scale_at_which_the_photo_reaches_two_sides_of_the_room() {
        assert_eq!(photo(3000, 2000, 1.0).fit(), 0.2);
        assert_eq!(photo(3000, 2000, 2.0).fit(), 0.4);
        assert_eq!(photo(2000, 3000, 1.0).fit(), 400.0 / 3000.0);
        assert_eq!(photo(300, 100, 1.0).fit(), 2.0);
        let fitted = photo(3000, 2000, 1.0).place(Framing::default());
        assert_eq!(
            fitted,
            Rect::from_center_size(room().center(), vec2(600.0, 400.0))
        );
        let tall = photo(2000, 3000, 1.0).place(Framing::default());
        assert!(same(
            tall,
            Rect::from_center_size(room().center(), vec2(800.0 / 3.0, 400.0))
        ));
    }

    /// At 100 % the photo is as many points long as its largest image has
    /// pixels on a screen of one pixel a point, and half as many on a
    /// screen of two.
    #[test]
    fn at_100_percent_each_pixel_of_the_photo_is_a_pixel_of_the_screen() {
        let full = Framing {
            zoom: Zoom::Scale(1.0),
            pan: vec2(0.5, 0.5),
        };
        assert_eq!(
            photo(3000, 2000, 1.0).place(full).size(),
            vec2(3000.0, 2000.0)
        );
        assert_eq!(
            photo(3000, 2000, 2.0).place(full).size(),
            vec2(1500.0, 1000.0)
        );
        assert_eq!(photo(3000, 2000, 2.0).place(full).center(), room().center());
    }

    /// The pan of a photo larger than the room keeps the photo's edges
    /// out of the room, and the photo where it is smaller than the room
    /// on a side in the middle on that side.
    #[test]
    fn the_pan_keeps_the_photo_s_edges_out_of_the_room_or_it_in_the_middle() {
        let stage = photo(3000, 2000, 1.0);
        // 3000 by 2000 points: the room shows a fifth of it across.
        assert_eq!(stage.bound(1.0, vec2(0.0, 1.0)), vec2(0.1, 0.9));
        assert_eq!(stage.bound(1.0, vec2(0.3, 0.6)), vec2(0.3, 0.6));
        // 450 by 300 points: smaller than the room both ways.
        assert_eq!(stage.bound(0.15, vec2(0.0, 1.0)), vec2(0.5, 0.5));
        // 750 by 500 points, then 900 by 600, then 600 by 400.
        for scale in [0.25, 0.3, 0.2] {
            for pan in [vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.4, 0.7)] {
                let framing = Framing {
                    zoom: Zoom::Scale(scale),
                    pan,
                };
                let drawn = stage.place(framing);
                assert!(drawn.contains_rect(room()), "{scale} {pan:?}: {drawn:?}");
            }
        }
        // 300 by 450 points, larger only down; 1200 by 400, only across.
        let tall = photo(1200, 1800, 1.0);
        assert!(near(
            tall.bound(0.25, Vec2::ZERO),
            vec2(0.5, 400.0 / 450.0 / 2.0)
        ));
        let wide = photo(2400, 800, 1.0);
        assert_eq!(wide.bound(0.5, Vec2::ZERO), vec2(0.25, 0.5));
    }

    /// Zooming about a point of the room keeps the point of the photo
    /// under it there, as far as the bounds allow.
    #[test]
    fn zooming_about_a_point_keeps_the_point_of_the_photo_under_it() {
        let stage = photo(3000, 2000, 1.0);
        let under = |drawn: Rect, at: Pos2| (at - drawn.min) / drawn.size();
        let framing = Framing {
            zoom: Zoom::Scale(0.5),
            pan: vec2(0.4, 0.45),
        };
        let at = pos2(500.0, 200.0);
        let before = stage.place(framing);
        for factor in [1.5, 0.8, 1.2214] {
            let zoomed = stage.zoom_about(framing, factor, at);
            assert_eq!(zoomed.zoom, Zoom::Scale(0.5 * factor), "{factor}");
            let after = stage.place(zoomed);
            assert!(near(under(after, at), under(before, at)), "{factor}");
        }
        // From fitted: the photo fills the room, so in by a little the
        // point stays.
        let zoomed = stage.zoom_about(Framing::default(), 2.0, at);
        assert_eq!(zoomed.zoom, Zoom::Scale(0.4));
        let fitted = stage.place(Framing::default());
        assert!(near(under(stage.place(zoomed), at), under(fitted, at)));
        // Near the room's corner the bounds win: the edges stay out.
        let corner = pos2(101.0, 51.0);
        let zoomed = stage.place(stage.zoom_about(framing, 0.8, corner));
        assert!(zoomed.contains_rect(room()), "{zoomed:?}");
    }

    /// Zooming out goes no further than fitted, and fits a photo zoomed
    /// out to the fit or below; a photo no larger than fitted is not
    /// zoomed out. Zooming in goes no further than 800 %.
    #[test]
    fn zooming_out_stops_at_the_fit_and_in_at_800_percent() {
        let stage = photo(3000, 2000, 1.0);
        let at = room().center();
        let scaled = |scale| Framing {
            zoom: Zoom::Scale(scale),
            pan: vec2(0.5, 0.5),
        };
        assert_eq!(stage.zoom_about(scaled(0.25), 0.5, at).zoom, Zoom::Fit);
        assert_eq!(stage.zoom_about(scaled(0.25), 0.8, at).zoom, Zoom::Fit);
        assert_eq!(
            stage.zoom_about(scaled(0.3), 0.9, at).zoom,
            Zoom::Scale(0.3 * 0.9)
        );
        let fitted = Framing::default();
        assert_eq!(stage.zoom_about(fitted, 0.5, at), fitted);
        assert_eq!(
            stage.zoom_about(scaled(6.0), 2.0, at).zoom,
            Zoom::Scale(MAX_SCALE)
        );
        assert_eq!(
            stage.zoom_about(scaled(MAX_SCALE), 1.5, at).zoom,
            Zoom::Scale(MAX_SCALE)
        );
        // A small photo at 100 %, smaller than fitted.
        let small = photo(300, 200, 1.0);
        assert_eq!(small.zoom_about(scaled(1.0), 0.5, at), scaled(1.0));
        assert_eq!(
            small.zoom_about(scaled(1.0), 1.5, at).zoom,
            Zoom::Scale(1.5)
        );
    }

    /// A click on a fitted photo shows it at 100 %, the point clicked at
    /// the room's middle as far as the bounds allow; a click on a photo
    /// zoomed larger than the room, or at 100 % already, fits it. A small
    /// photo, enlarged to fit, is shown at 100 % too, in the middle.
    #[test]
    fn a_click_goes_between_fitted_and_100_percent() {
        let stage = photo(3000, 2000, 1.0);
        let fitted = stage.place(Framing::default());
        // A tenth of the way across the photo and a quarter down.
        let at = fitted.min + vec2(60.0, 100.0);
        let full = stage.click(Framing::default(), fitted, at);
        assert_eq!(full.zoom, Zoom::Scale(1.0));
        assert!(near(full.pan, vec2(0.1, 0.25)), "{full:?}");
        // Nearer the corner than the bounds allow.
        let full = stage.click(Framing::default(), fitted, fitted.min + vec2(6.0, 4.0));
        assert!(near(full.pan, vec2(0.1, 0.1)), "{full:?}");
        let drawn = stage.place(full);
        assert!(stage.click_fits(full));
        assert_eq!(stage.click(full, drawn, at).zoom, Zoom::Fit);
        // Zoomed in by a hair from the fit: larger, but not by a point.
        let hair = Framing {
            zoom: Zoom::Scale(0.2 * 1.001),
            pan: vec2(0.5, 0.5),
        };
        assert!(!stage.click_fits(hair));
        assert_eq!(
            stage.click(hair, stage.place(hair), at).zoom,
            Zoom::Scale(1.0)
        );

        let small = photo(300, 200, 1.0);
        let fitted = small.place(Framing::default());
        let full = small.click(Framing::default(), fitted, fitted.min + vec2(10.0, 10.0));
        assert_eq!(full.zoom, Zoom::Scale(1.0));
        let drawn = small.place(full);
        assert_eq!(
            drawn,
            Rect::from_center_size(room().center(), vec2(300.0, 200.0))
        );
        assert!(small.click_fits(full));
        assert_eq!(small.click(full, drawn, drawn.center()).zoom, Zoom::Fit);
    }

    /// A click fits a photo zoomed past 100 % even while it is no larger
    /// than the room, as a small photo is, enlarging it, rather than
    /// taking it down to 100 %; one zoomed below 100 % and no larger than
    /// the room goes to 100 %. A click enlarges the photo exactly when it
    /// shows it larger than it is.
    #[test]
    fn a_click_fits_a_photo_zoomed_past_100_percent_however_small() {
        // Fitted at 600 %.
        let tiny = photo(100, 50, 1.0);
        let twice = Framing {
            zoom: Zoom::Scale(2.0),
            pan: vec2(0.5, 0.5),
        };
        assert!(!tiny.larger(twice));
        assert!(tiny.click_fits(twice));
        let drawn = tiny.place(twice);
        assert_eq!(tiny.click(twice, drawn, drawn.center()).zoom, Zoom::Fit);
        assert!(tiny.click_enlarges(twice));
        let half = Framing {
            zoom: Zoom::Scale(0.5),
            ..twice
        };
        assert!(!tiny.click_fits(half));
        assert!(tiny.click_enlarges(half), "to 100 %");
        assert!(!tiny.click_enlarges(Framing::default()), "to 100 %");

        let large = photo(3000, 2000, 1.0);
        assert!(large.click_enlarges(Framing::default()), "to 100 %");
        let zoomed = Framing {
            zoom: Zoom::Scale(0.5),
            ..twice
        };
        assert!(large.larger(zoomed) && large.click_fits(zoomed));
        assert!(!large.click_enlarges(zoomed), "to fitted");
    }

    /// A pan moves the photo with the pointer, as far as the bounds
    /// allow, and leaves a fitted photo where it is.
    #[test]
    fn a_pan_moves_the_photo_with_the_pointer_within_the_bounds() {
        let stage = photo(3000, 2000, 1.0);
        let framing = Framing {
            zoom: Zoom::Scale(1.0),
            pan: vec2(0.5, 0.5),
        };
        let before = stage.place(framing);
        let moved = stage.place(stage.pan_by(framing, vec2(30.0, -20.0)));
        assert!(near(
            moved.min.to_vec2(),
            (before.min + vec2(30.0, -20.0)).to_vec2()
        ));
        let far = stage.place(stage.pan_by(framing, vec2(-9000.0, 9000.0)));
        assert_eq!((far.right(), far.top()), (room().right(), room().top()));
        assert_eq!(
            stage.pan_by(Framing::default(), vec2(30.0, 0.0)),
            Framing::default()
        );
    }

    /// A photo drawn at 100 % on a screen of two pixels a point, in a room
    /// whose middle falls between two pixels, has its corner on a whole
    /// pixel; at any other scale it is drawn where it is placed.
    #[test]
    fn at_100_percent_the_photo_s_corner_is_on_a_whole_pixel() {
        let odd = Rect::from_min_size(pos2(100.25, 50.0), vec2(600.5, 400.75));
        let stage = Stage::new(
            Size {
                width: 3001,
                height: 2001,
            },
            odd,
            2.0,
        );
        let full = Framing {
            zoom: Zoom::Scale(1.0),
            pan: vec2(0.5, 0.5),
        };
        let placed = stage.place(full);
        let snapped = stage.snapped(placed);
        let pixels = snapped.min.to_vec2() * 2.0;
        assert_ne!(placed.min.to_vec2() * 2.0, pixels.round(), "already whole");
        assert_eq!(pixels, pixels.round(), "{snapped:?}");
        assert_eq!(snapped.size(), placed.size());
        assert!((snapped.min - placed.min).length() <= 0.5);
        let half = Framing {
            zoom: Zoom::Scale(0.5),
            ..full
        };
        assert_eq!(stage.snapped(stage.place(half)), stage.place(half));
    }

    /// The move a click makes starts where the photo was, is half way at
    /// half its time, eased in and out, and ends where the photo goes.
    #[test]
    fn a_click_s_move_is_eased_over_its_time() {
        let from = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let to = Rect::from_min_size(pos2(-100.0, -50.0), vec2(300.0, 150.0));
        assert_eq!(glided(from, to, 10.0, 10.0), from);
        assert!(same(
            glided(from, to, 10.0, 10.0 + GLIDE / 2.0),
            from.lerp_towards(&to, 0.5)
        ));
        let early = glided(from, to, 10.0, 10.0 + GLIDE / 4.0);
        assert!(
            early.width() < from.lerp_towards(&to, 0.25).width(),
            "not eased"
        );
        assert_eq!(glided(from, to, 10.0, 10.0 + GLIDE), to);
        assert_eq!(glided(from, to, 10.0, 11.0), to);
        assert!(gliding(10.0, 10.0 + GLIDE - 0.01));
        assert!(!gliding(10.0, 10.0 + GLIDE));
    }

    /// A framing kept from one photo to another of another size and
    /// orientation keeps its zoom and pan, bounded for the new photo.
    #[test]
    fn a_framing_carried_to_another_photo_is_bounded_for_it() {
        let wide = photo(3000, 2000, 1.0);
        let framing = Framing {
            zoom: Zoom::Scale(1.0),
            pan: vec2(0.12, 0.85),
        };
        assert_eq!(wide.bound(1.0, framing.pan), framing.pan);
        let tall = photo(2000, 3000, 1.0);
        let drawn = tall.place(framing);
        assert_eq!(drawn.size(), vec2(2000.0, 3000.0));
        assert!(drawn.contains_rect(room()), "{drawn:?}");
        // Across, a fifth of 2000 points is shown: 0.15 at the least.
        let shown = (room().center() - drawn.min) / drawn.size();
        assert!(near(shown, vec2(0.15, 0.85)), "{shown:?}");
        // Smaller than the room: in the middle, at its own 100 %.
        let small = photo(400, 300, 1.0);
        let drawn = small.place(framing);
        assert_eq!(
            drawn,
            Rect::from_center_size(room().center(), vec2(400.0, 300.0))
        );
    }
}
