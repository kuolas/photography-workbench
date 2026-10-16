// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer in Library's centre: the current photo alone, as large as
//! the centre allows, sharp at the screen's resolution.
//!
//! The photo is fitted in the centre less a margin on every side
//! ([`MARGIN`]), its aspect kept, in the middle, and enlarged when it is
//! smaller than that room, so it always reaches two of the room's sides.
//! It sits straight on the viewer's backdrop with nothing behind it.
//!
//! What is drawn is the photo's picture ([`crate::textures`]): the photo
//! decoded at the long side of that room in the screen's pixels, rounded
//! up to a step of [`STEP`] pixels, so resizing the window does not
//! decode it again at every pixel, and no longer than the largest texture
//! the graphics card takes ([`picture_side`]). A picture takes a while to
//! decode, as it is a RAW file's largest preview, or a JPEG whole, so the
//! photo's thumbnail at the largest size thumbnails are decoded at is
//! wanted before it, as a quick stand-in; while the picture is on its
//! way, the best texture of the photo held is shown in its place, so a
//! photo stepped to shows at once when the filmstrip holds its thumbnail.
//! Once the picture is settled, held or known not to decode, or the photo
//! shown as one that cannot be shown, the pictures of the next photo and
//! of the one before are wanted too, at the same size and below everything
//! on screen, so stepping to either shows it sharp at once; not before, so
//! they never hold up the current one, and a damaged current photo does
//! not keep them from being made ready ([`want_order`]).
//!
//! When the picture has taken half a second ([`LOADING_AFTER`]) since the
//! photo or the size wanted last changed, "Loading…" shows in the margin
//! under the photo until it comes; not when it cannot be decoded and
//! something else of the photo is shown, nor while the photo is shown as
//! one that cannot be shown. A photo that cannot be shown at all shows an
//! icon in the middle, and why in its tooltip, once its thumbnails and its
//! picture at some size are known not to decode, while a size wanted that
//! is not tried yet is still decoded; nothing is drawn until some texture
//! of the photo is decoded or that is known.
//!
//! The viewer draws the photos it is handed, whatever they were read
//! from, and the textures the app holds, and names the photo to screen
//! readers by its file name. What it keeps between frames is how long it
//! has waited ([`Waiting`]), which the app holds for it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::{Margin, Rect, Sense, WidgetInfo, WidgetType};

use crate::textures::{self, CURRENT, CURRENT_PICTURE, NEIGHBOURS, Textures, Thumbnail};

/// The room kept between the centre's edges and the photo, on every
/// side, so the photo stands clear of the parts of the window around it.
const MARGIN: i8 = 24;

/// The size of the icon shown when the photo cannot be shown.
const FAILED_ICON_SIZE: f32 = 48.0;

/// The step a picture's longest side is rounded up to, in pixels: a
/// window resized by a little keeps the picture it has.
const STEP: u32 = 256;

/// How long the viewer waits for its picture before it says the picture
/// is loading, in seconds: one that comes sooner needs no word, and a
/// word shown only for a moment would flicker.
const LOADING_AFTER: f64 = 0.5;

/// What the viewer says while its picture takes a while.
const LOADING: &str = "Loading…";

/// How long the viewer has wanted the picture it shows, kept by the app
/// from one frame to the next and never saved.
#[derive(Debug, Default)]
pub struct Waiting {
    /// What the viewer last wanted, and since when. `None` until the
    /// viewer is first drawn.
    since: Option<Since>,
}

/// The picture the viewer wanted when it was last drawn, and since when.
#[derive(Debug)]
struct Since {
    /// The photo's file.
    path: PathBuf,
    /// The longest side of the picture, in pixels.
    longest: u32,
    /// egui's time when the viewer began wanting it, in seconds.
    at: f64,
    /// The number of the pass of egui's the viewer was last drawn in.
    pass: u64,
}

impl Waiting {
    /// How long, in seconds, the viewer drawn at egui's time `now` in its
    /// pass numbered `pass` has wanted the picture of the photo at `path`
    /// at `longest` pixels: since the photo or the size last changed, or
    /// since the viewer came back on screen, not drawn in the pass before.
    /// Remembers what it was asked, for the next pass.
    fn waited(&mut self, path: &Path, longest: u32, now: f64, pass: u64) -> f64 {
        match &mut self.since {
            Some(since)
                if since.path == path && since.longest == longest && pass <= since.pass + 1 =>
            {
                since.pass = pass;
                now - since.at
            }
            _ => {
                self.since = Some(Since {
                    path: path.to_path_buf(),
                    longest,
                    at: now,
                    pass,
                });
                0.0
            }
        }
    }
}

/// Whether the viewer says its picture is loading.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Loading {
    /// Not, as the picture is not on its way: it is shown, or it cannot
    /// be decoded; or the photo is shown as one that cannot be shown.
    No,
    /// Not yet, but in this many seconds if the picture has not come.
    In(f64),
    /// Now.
    Yes,
}

/// Whether the viewer says its picture is loading, when the picture is
/// `coming`, on its way, and has been wanted for `waited` seconds: once
/// it has been wanted for [`LOADING_AFTER`], and never once it is shown
/// or cannot be decoded.
fn loading(coming: bool, waited: f64) -> Loading {
    if !coming {
        Loading::No
    } else if waited >= LOADING_AFTER {
        Loading::Yes
    } else {
        Loading::In(LOADING_AFTER - waited)
    }
}

/// What the viewer wants decoded of a photo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wanted {
    /// Its thumbnail at the largest size thumbnails are decoded at.
    Thumbnail,
    /// Its picture.
    Picture,
}

/// The viewer of the photo at `current` of `photos`, filling `ui`, with
/// the textures in `textures`; it says in `palette`'s colours when the
/// picture takes a while, as `waiting` times it. Nothing is drawn when
/// there is no photo at `current`.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    textures: &mut Textures,
    waiting: &mut Waiting,
    photos: &[Photo],
    current: usize,
) {
    let Some(photo) = photos.get(current) else {
        return;
    };
    let centre = ui.max_rect();
    let room = room(centre);
    let points = room.size().max_elem();
    let pixels_per_point = ui.pixels_per_point();
    // Asked every frame: what egui holds is the card's own limit once the
    // integration reports it, and its default until then.
    let max_side = ui.input(|i| i.max_texture_side);
    let longest = picture_side(points, pixels_per_point, max_side);
    let picture = textures.picture(photo.path(), longest);
    // A photo shown as one that cannot be shown, its picture failed at
    // another size and its thumbnails too, is settled as one whose picture
    // failed at this size is, though this size is still tried: the viewer
    // says nothing of loading meanwhile, nor holds the neighbours up.
    let cannot = matches!(picture.shown, Thumbnail::Failed(_));
    let thumbnail = textures::longest(points, pixels_per_point);
    let failed = picture.failed || cannot;
    let order = want_order(current, photos.len(), picture.decoded, failed);
    for (index, wanted, priority) in order {
        let side = match wanted {
            Wanted::Thumbnail => thumbnail,
            Wanted::Picture => longest,
        };
        textures.want(photos[index].path(), side, priority);
    }

    let name = photo.name().into_owned();
    let id = ui.make_persistent_id("viewer");
    let response = ui.interact(textures::placed(picture.shown, room), id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, &name));
    // The viewer shows no name, so its tooltip only says why a photo
    // cannot be shown: one naming the photo would cover it whenever the
    // pointer rests on it.
    if let Some(failure) = textures::paint(ui, picture.shown, room, FAILED_ICON_SIZE) {
        response.on_hover_text(failure.to_string());
    }

    let now = ui.input(|i| i.time);
    let waited = waiting.waited(photo.path(), longest, now, ui.ctx().cumulative_pass_nr());
    match loading(picture.coming && !cannot, waited) {
        Loading::No => {}
        // A frame then, for an idle window to say it on time.
        Loading::In(seconds) => ui
            .ctx()
            .request_repaint_after(Duration::from_secs_f64(seconds)),
        Loading::Yes => {
            // In the margin under the room, which the photo never
            // reaches, so it covers none of the photo.
            let below = Rect::from_x_y_ranges(room.x_range(), room.bottom()..=centre.bottom());
            let text = egui::RichText::new(LOADING).small().color(palette.text);
            ui.put(below, egui::Label::new(text).selectable(false));
        }
    }
}

/// Where the photo is fitted in a centre at `centre`: all of it but
/// [`MARGIN`] on every side.
fn room(centre: Rect) -> Rect {
    centre - Margin::same(MARGIN)
}

/// The longest side, in pixels, to decode the picture of a photo at that
/// is fitted in a room whose long side is `points` long, on a screen of
/// `pixels_per_point`, where the graphics card takes textures of at most
/// `max_side` pixels a side: the room's long side in pixels, rounded up to
/// a step of [`STEP`], no longer than `max_side`, and never as short as a
/// thumbnail ([`textures::LARGEST_THUMBNAIL`]), so it is always a picture,
/// at least a step longer than the largest thumbnail. Every card takes
/// textures of 2048 pixels a side, far more than that.
pub fn picture_side(points: f32, pixels_per_point: f32, max_side: usize) -> u32 {
    // A negative or not-a-number length is cast to none.
    let pixels = (points * pixels_per_point).ceil() as u32;
    let stepped = pixels.div_ceil(STEP).saturating_mul(STEP);
    let max_side = u32::try_from(max_side).unwrap_or(u32::MAX);
    stepped
        .min(max_side)
        .max(textures::LARGEST_THUMBNAIL + STEP)
}

/// What the viewer of the photo at `current`, of `len` photos, wants
/// decoded, the best first, each with a priority of its own from the
/// highest down ([`textures::ranked`]), when the photo's picture is
/// `decoded` or not, and is known to have `failed` or not, as it is too
/// while the photo is shown as one that cannot be shown.
///
/// Until the picture is decoded, the photo's thumbnail ([`CURRENT`]), the
/// quick stand-in, then its picture ([`CURRENT_PICTURE`]); once it is, the
/// picture alone, kept by being wanted, as the thumbnail is not shown.
/// Once the picture is settled, decoded or known to have failed, the
/// pictures of the next photo and of the one before ([`NEIGHBOURS`]) come
/// after, as far as there are photos, so stepping to either shows it
/// sharp at once; they are wanted only then, so they never hold up the
/// current one, and a current photo that cannot be decoded does not stop
/// them.
fn want_order(
    current: usize,
    len: usize,
    decoded: bool,
    failed: bool,
) -> Vec<(usize, Wanted, u32)> {
    let mut order = Vec::new();
    if !decoded {
        order.push((current, Wanted::Thumbnail, CURRENT));
    }
    order.push((current, Wanted::Picture, CURRENT_PICTURE));
    if decoded || failed {
        let next = (current + 1 < len).then_some(current + 1);
        for neighbour in [next, current.checked_sub(1)].into_iter().flatten() {
            order.push((neighbour, Wanted::Picture, NEIGHBOURS));
        }
    }
    let tiers = order
        .iter()
        .enumerate()
        .map(|(at, &(_, _, tier))| (at, tier));
    textures::ranked(tiers)
        .into_iter()
        .map(|(at, priority)| (order[at].0, order[at].1, priority))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use egui::{Rect, TextureId, pos2, vec2};

    use super::{LOADING_AFTER, Loading, Waiting, Wanted, loading, picture_side, room, want_order};
    use crate::textures::{self, CURRENT, CURRENT_PICTURE, NEIGHBOURS, Thumbnail, placed};

    /// Where the viewer draws a photo of `size` pixels in a centre at
    /// `centre`.
    fn drawn(size: [usize; 2], centre: Rect) -> Rect {
        let thumbnail = Thumbnail::Ready {
            texture: TextureId::Managed(1),
            size,
        };
        placed(thumbnail, room(centre))
    }

    /// A photo is fitted in the centre less 24 points on every side, as
    /// large as fits there, enlarged when it is smaller, its aspect kept,
    /// in the middle: a wide one reaches the room's left and right, a tall
    /// one its top and bottom.
    #[test]
    fn a_photo_fills_the_centre_less_the_margin_its_aspect_kept() {
        let centre = Rect::from_min_size(pos2(268.0, 40.0), vec2(648.0, 448.0));
        let room = Rect::from_min_size(pos2(292.0, 64.0), vec2(600.0, 400.0));
        let in_middle = |size| Rect::from_center_size(room.center(), size);
        // Smaller than the room, wide and tall: enlarged.
        assert_eq!(drawn([300, 100], centre), in_middle(vec2(600.0, 200.0)));
        assert_eq!(drawn([100, 200], centre), in_middle(vec2(200.0, 400.0)));
        // Larger than the room.
        assert_eq!(drawn([1200, 200], centre), in_middle(vec2(600.0, 100.0)));
        // Of the room's own aspect: the whole room.
        assert_eq!(drawn([300, 200], centre), room);
    }

    /// A picture is as long as the room's long side in pixels, rounded up
    /// to a step of 256, on a screen of one pixel a point or two; no
    /// longer than the card's largest texture; and always a step longer
    /// than the largest thumbnail, however small the room.
    #[test]
    fn a_picture_is_the_room_in_pixels_rounded_up_to_a_step() {
        assert_eq!(picture_side(1000.0, 1.0, 16384), 1024);
        assert_eq!(picture_side(1024.0, 1.0, 16384), 1024);
        assert_eq!(picture_side(1024.5, 1.0, 16384), 1280);
        assert_eq!(picture_side(1000.0, 2.0, 16384), 2048);
        assert_eq!(picture_side(1100.0, 2.0, 16384), 2304);
        assert_eq!(picture_side(1100.0, 2.0, 2048), 2048);
        assert_eq!(picture_side(9000.0, 2.0, 16384), 16384);
        assert_eq!(picture_side(300.0, 1.0, 16384), 768);
        assert_eq!(picture_side(0.0, 2.0, 16384), 768);
        assert!(picture_side(512.0, 1.0, 16384) > textures::LARGEST_THUMBNAIL);
    }

    /// Until the current photo's picture is decoded, the viewer wants its
    /// thumbnail, then the picture; once it is, the picture alone. Once
    /// the picture is settled, decoded or known to have failed, the next
    /// photo's picture and the one before's come after, as far as there
    /// are photos, and not before. Each is at a priority of its own,
    /// falling.
    #[test]
    fn the_neighbours_are_wanted_once_the_picture_is_settled() {
        use Wanted::{Picture, Thumbnail};
        let wanted = |current, len, decoded, failed| -> Vec<(usize, Wanted)> {
            let order = want_order(current, len, decoded, failed);
            assert!(
                order.windows(2).all(|pair| pair[0].2 > pair[1].2),
                "{order:?}"
            );
            order
                .into_iter()
                .map(|(index, of, _)| (index, of))
                .collect()
        };
        assert_eq!(wanted(4, 10, false, false), [(4, Thumbnail), (4, Picture)]);
        assert_eq!(
            wanted(4, 10, true, false),
            [(4, Picture), (5, Picture), (3, Picture)]
        );
        assert_eq!(
            wanted(4, 10, false, true),
            [(4, Thumbnail), (4, Picture), (5, Picture), (3, Picture)]
        );
        assert_eq!(wanted(0, 10, true, false), [(0, Picture), (1, Picture)]);
        assert_eq!(wanted(9, 10, true, false), [(9, Picture), (8, Picture)]);
        assert_eq!(wanted(0, 1, true, false), [(0, Picture)]);
        let priorities = |tiers: &[(usize, u32)]| -> Vec<u32> {
            let ranked = textures::ranked(tiers.iter().copied());
            ranked.into_iter().map(|(_, p)| p).collect()
        };
        let of = |decoded, failed| -> Vec<u32> {
            let order = want_order(4, 10, decoded, failed);
            order.into_iter().map(|(_, _, p)| p).collect()
        };
        assert_eq!(
            of(false, false),
            priorities(&[(0, CURRENT), (1, CURRENT_PICTURE)])
        );
        let settled = [(0, CURRENT_PICTURE), (1, NEIGHBOURS), (2, NEIGHBOURS)];
        assert_eq!(of(true, false), priorities(&settled));
    }

    /// The wait for a picture runs from when its photo or its size last
    /// changed, or the viewer last came back on screen, not drawn in the
    /// pass before.
    #[test]
    fn the_wait_runs_from_a_change_of_photo_or_size_or_the_viewer_coming_back() {
        let (a, b) = (Path::new("a.jpg"), Path::new("b.jpg"));
        let mut waiting = Waiting::default();
        assert_eq!(waiting.waited(a, 1024, 10.0, 1), 0.0);
        assert_eq!(waiting.waited(a, 1024, 10.25, 2), 0.25);
        assert_eq!(waiting.waited(a, 1024, 10.5, 3), 0.5);
        assert_eq!(waiting.waited(b, 1024, 11.0, 4), 0.0, "another photo");
        assert_eq!(waiting.waited(b, 1024, 11.25, 5), 0.25);
        assert_eq!(waiting.waited(b, 1280, 11.5, 6), 0.0, "another size");
        assert_eq!(waiting.waited(b, 1280, 12.0, 7), 0.5);
        assert_eq!(waiting.waited(b, 1280, 13.0, 9), 0.0, "back on screen");
    }

    /// The viewer says its picture is loading once the picture has been on
    /// its way for half a second, asking meanwhile for a frame when that
    /// will be; never when it is not on its way, shown or failed.
    #[test]
    fn loading_shows_after_half_a_second_while_the_picture_is_on_its_way() {
        assert_eq!(loading(true, 0.0), Loading::In(LOADING_AFTER));
        assert_eq!(loading(true, 0.375), Loading::In(0.125));
        assert_eq!(loading(true, 0.5), Loading::Yes);
        assert_eq!(loading(true, 3.0), Loading::Yes);
        assert_eq!(loading(false, 0.0), Loading::No);
        assert_eq!(loading(false, 3.0), Loading::No);
    }
}
