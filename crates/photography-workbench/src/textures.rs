// SPDX-License-Identifier: GPL-3.0-or-later

//! The photos' thumbnails, decoded off the window's thread and kept as
//! textures: [`Textures`], which the views of a folder's photos draw
//! from.
//!
//! Each frame a view wants the photos it shows, and those it soon may
//! ([`Textures::want`]), the best first, each at a priority of its own
//! ([`ranked`]), so they are decoded in that order however the decoder's
//! workers wake; then it draws them ([`Textures::thumbnail`]), getting
//! the texture held for each, if any, or why the photo cannot be shown.
//! Wanting a photo decodes it unless a texture of that size is held, so
//! a view wants every photo it shows on every frame and the store does
//! the rest: each frame, before anything is drawn,
//! [`Textures::begin_frame`] turns the photos decoded since the last
//! frame into textures, and after everything is drawn
//! [`Textures::end_frame`] drops the requests of the photos no longer
//! wanted, so scrolling fast through thousands of photos leaves no queue
//! behind, and lets go of the textures least recently wanted past a
//! budget of memory. The decoder wakes the window when a photo is
//! decoded, so an idle window draws nothing.
//!
//! Thumbnails are decoded at a few sizes ([`longest`]), and a photo has
//! a texture of each size a view wants, so each view draws one made for
//! it: a texture drawn much smaller than its pixels shows jagged,
//! flickering detail, as the renderer keeps no smaller copies of it.
//! While the size wanted is on its way, another size of the photo held
//! is shown in its place. Going back to a folder shows its thumbnails at
//! once, if they are still held.
//!
//! A read of a folder's photos, of the folder shown again or of another,
//! starts afresh, as files may have changed since the last: what was
//! wanted before it is dropped, and so is every decode under way, whose
//! answer is never handed out, so what is decoded after it is read after
//! it; and each photo whose texture is held, or which could not be shown,
//! as a file still being copied off a card is damaged until it is whole,
//! is decoded again when it is next wanted, what was known of it shown
//! until the new answer comes.
//!
//! Every view draws a thumbnail the same way ([`paint`]): fitted in the
//! room it has, its aspect kept, or, for a photo that cannot be shown,
//! an icon there, with why in the view's tooltip ([`tooltip`]).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use desktop_ui::icons;
use egui::load::SizedTexture;
use egui::{Align2, ColorImage, Rect, TextureHandle, TextureId, TextureOptions, Vec2, vec2};
use previews::{Answer, Decoder, Failure};

/// The sizes thumbnails are decoded at, as the longest side in pixels,
/// smallest first. Few sizes mean few textures of a photo, each serving
/// a view at any zoom near its size.
const SIZES: [u32; 3] = [256, 384, 512];

/// The most memory the textures may take, in bytes, unless the photos
/// wanted in one frame need more.
const BUDGET: usize = 256 * 1024 * 1024;

/// The tier of the current photo, the one a photographer looks at: above
/// every other ([`ranked`]).
pub const CURRENT: u32 = 2;

/// The tier of a photo whose thumbnail is drawn: it is on screen.
pub const ON_SCREEN: u32 = 1;

/// The tier of a photo wanted ahead of being drawn, near what is on
/// screen, so it is ready when scrolled to.
pub const AHEAD: u32 = 0;

/// How many of a priority's bits rank a photo within its tier: the tier
/// is above them, so it outranks any rank.
const RANK_BITS: u32 = 20;

/// The most photos of a tier ranked apart; those past it share the last
/// rank, far beyond any screen of thumbnails.
const MAX_RANK: usize = (1 << RANK_BITS) - 1;

/// The Lucide icon shown in place of a photo that cannot be shown, and
/// the text shown in its place if its name does not resolve.
const FAILED_ICON: (&str, char) = ("image-off", '⊘');

/// What a request to the decoder is for: a photo's path, and the longest
/// side asked for. Two sizes of a photo are two requests, so neither
/// replaces the other.
type Slot = (PathBuf, u32);

/// A photo's thumbnail, as a view draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thumbnail {
    /// Not decoded yet.
    Pending,
    /// A texture of the photo: the one of the size asked for, or another
    /// size of it while that one is on its way.
    Ready {
        /// The texture, to draw.
        texture: TextureId,
        /// Its width and height, in pixels.
        size: [usize; 2],
    },
    /// Why the photo cannot be shown.
    Failed(Failure),
}

/// A texture of a photo, and what it was decoded at.
struct Held {
    /// The texture, freed when this is dropped.
    texture: TextureHandle,
    /// The longest side asked of the decode it came from, in pixels:
    /// the size it serves, even when the photo itself is smaller.
    longest: u32,
    /// The memory it takes, in bytes.
    bytes: usize,
    /// The number of the frame it was last wanted or drawn in.
    used: u64,
    /// How many reads of photos the store had seen when it was decoded:
    /// fewer than it has seen now, and the file may have changed since.
    reads: u64,
}

/// The photos' thumbnails, decoded on worker threads and kept as
/// textures, one a photo and size, within a budget of memory.
pub struct Textures {
    /// What decodes the photos, one request a photo and size. Made on the
    /// first frame, as it wakes the window.
    decoder: Option<Decoder<Slot>>,
    /// Whether the decoder decodes on the window's thread, when it hands
    /// out the answers, as in the tests. It has no thread to wake the
    /// window with, so the store asks for the next frame itself while
    /// answers are still to come.
    #[cfg(test)]
    inline: bool,
    /// The textures, by the path of the photo they are of, one a size.
    held: HashMap<PathBuf, Vec<Held>>,
    /// Why each photo that could not be shown could not be, by its path,
    /// with how many reads of photos the store had seen then, as for a
    /// texture ([`Held::reads`]).
    failed: HashMap<PathBuf, (Failure, u64)>,
    /// The photos and sizes wanted in this frame, drawn or wanted ahead.
    wanted: HashSet<Slot>,
    /// The number of this frame, counted from 0.
    frame: u64,
    /// The most memory the textures may take, in bytes.
    budget: usize,
    /// The number of the read of photos the photos shown came from.
    read: Option<u64>,
    /// How many reads of photos the store has seen.
    reads: u64,
}

impl Default for Textures {
    /// No thumbnails yet, and no decoder until the first frame.
    fn default() -> Self {
        Self {
            decoder: None,
            #[cfg(test)]
            inline: false,
            held: HashMap::new(),
            failed: HashMap::new(),
            wanted: HashSet::new(),
            frame: 0,
            budget: BUDGET,
            read: None,
            reads: 0,
        }
    }
}

impl Textures {
    /// A store whose decoder decodes on the window's thread, when the
    /// frame begins, so a test sees each thumbnail on the frame after it
    /// was first wanted.
    #[cfg(test)]
    pub fn inline() -> Self {
        Self {
            decoder: Some(Decoder::inline()),
            inline: true,
            ..Self::default()
        }
    }

    /// The texture held for the photo at `path` at `longest` pixels, if
    /// any.
    #[cfg(test)]
    pub fn texture(&self, path: &Path, longest: u32) -> Option<TextureId> {
        let sizes = self.held.get(path)?;
        let held = sizes.iter().find(|held| held.longest == longest)?;
        Some(held.texture.id())
    }

    /// Start a frame, before anything is drawn: turn the photos decoded
    /// since the last frame into textures, each replacing the texture of
    /// its photo and size held before, and remember why the others could
    /// not be, each answer replacing what was known of its photo before.
    /// A decode for a photo no longer wanted is kept, for when it is
    /// wanted again. Makes the decoder on the first frame, waking the
    /// window when a photo is decoded.
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        let decoder = self.decoder.get_or_insert_with(|| {
            let ctx = ctx.clone();
            Decoder::new(move || ctx.request_repaint())
        });
        for answer in decoder.answers() {
            self.take(ctx, answer);
        }
    }

    /// Take `answer`, a photo decoded or why it could not be.
    fn take(&mut self, ctx: &egui::Context, answer: Answer<Slot>) {
        let Answer {
            path,
            longest,
            result,
            ..
        } = answer;
        let decoded = match result {
            Ok(decoded) => decoded,
            Err(failure) => {
                self.failed.insert(path, (failure, self.reads));
                return;
            }
        };
        let image = decoded.image();
        let size = [image.width() as usize, image.height() as usize];
        let texture = ctx.load_texture(
            path.display().to_string(),
            ColorImage::from_rgb(size, image.pixels()),
            TextureOptions::LINEAR,
        );
        let held = Held {
            texture,
            longest,
            // Four bytes a pixel on the graphics card.
            bytes: 4 * size[0] * size[1],
            used: self.frame,
            reads: self.reads,
        };
        self.failed.remove(&path);
        let sizes = self.held.entry(path).or_default();
        sizes.retain(|held| held.longest != longest);
        sizes.push(held);
    }

    /// Tell the store that the photos shown came from the read of a
    /// folder's photos numbered `read`; once a frame, after
    /// [`begin_frame`](Self::begin_frame) and before anything is drawn.
    ///
    /// Another read, of the folder shown again or of another, drops what
    /// was wanted before it and every answer not yet handed out, those of
    /// the decodes under way included, and has each photo whose texture
    /// is held, or which could not be shown, decoded again when it is
    /// next wanted, what was known of it shown until the new answer
    /// comes. The photos being decoded then, as many as the decoder has
    /// workers at most, are decoded again: the price of never showing
    /// what was read before the photos were.
    pub fn shown(&mut self, read: u64) {
        if self.read == Some(read) {
            return;
        }
        self.read = Some(read);
        if let Some(decoder) = &mut self.decoder {
            decoder.clear();
        }
        self.reads += 1;
    }

    /// Want the photo at `path` decoded at `longest` pixels on its
    /// longest side ([`longest`]), before anything of a lower `priority`
    /// ([`ranked`]), unless a texture of that size, or why the photo
    /// cannot be shown, is known since the photos were last read. A view
    /// wants each photo it draws before it draws them, and the photos it
    /// soon may, the best first.
    pub fn want(&mut self, path: &Path, longest: u32, priority: u32) {
        let (frame, reads) = (self.frame, self.reads);
        let slot = (path.to_path_buf(), longest);
        let held = self
            .held
            .get_mut(path)
            .and_then(|sizes| sizes.iter_mut().find(|held| held.longest == longest));
        let fresh = held.is_some_and(|held| {
            held.used = frame;
            held.reads == reads
        });
        let failed = self.failed.get(path).is_some_and(|&(_, at)| at == reads);
        if !fresh
            && !failed
            && let Some(decoder) = &mut self.decoder
        {
            decoder.want(slot.clone(), path, longest, priority);
        }
        self.wanted.insert(slot);
    }

    /// The thumbnail of the photo at `path` at `longest` pixels on its
    /// longest side, to draw in this frame: the texture of that size,
    /// decoded since the photos were last read; or why the photo cannot
    /// be shown, the last answer known, from before that read or since;
    /// or, while the texture wanted is on its way, another one of the
    /// photo held, the one of that size decoded before, or else the
    /// nearest larger, or else the largest smaller. It wants nothing
    /// decoded ([`want`](Self::want) does).
    pub fn thumbnail(&mut self, path: &Path, longest: u32) -> Thumbnail {
        let (frame, reads) = (self.frame, self.reads);
        let held = self.held.get_mut(path).and_then(|sizes| {
            let at = stand_in(sizes, longest)?;
            Some(&mut sizes[at])
        });
        let fresh = held
            .as_ref()
            .is_some_and(|held| held.longest == longest && held.reads == reads);
        let failure = self.failed.get(path).map(|&(failure, _)| failure);
        match held {
            Some(held) if fresh || failure.is_none() => {
                held.used = frame;
                Thumbnail::Ready {
                    texture: held.texture.id(),
                    size: held.texture.size(),
                }
            }
            _ => failure.map_or(Thumbnail::Pending, Thumbnail::Failed),
        }
    }

    /// End a frame, after everything is drawn: drop the requests of the
    /// photos not wanted in it, and let go of textures past the budget,
    /// those wanted least recently first, but never one wanted or drawn
    /// in this frame, so what is on screen and next to it stays even when
    /// it takes more.
    // Only an inline decoder, in the tests, needs the context here.
    pub fn end_frame(
        &mut self,
        #[cfg_attr(not(test), allow(unused_variables))] ctx: &egui::Context,
    ) {
        let wanted = &self.wanted;
        if let Some(decoder) = &mut self.decoder {
            decoder.retain(|slot| wanted.contains(slot));
            #[cfg(test)]
            if self.inline && decoder.pending() > 0 {
                ctx.request_repaint();
            }
        }
        self.evict();
        self.wanted.clear();
        self.frame += 1;
    }

    /// Let go of the textures past the budget, those wanted least
    /// recently first, but none wanted or drawn in this frame.
    fn evict(&mut self) {
        let mut total: usize = self.held.values().flatten().map(|held| held.bytes).sum();
        if total <= self.budget {
            return;
        }
        let frame = self.frame;
        let mut idle: Vec<(u64, PathBuf, u32)> = self
            .held
            .iter()
            .flat_map(|(path, sizes)| {
                sizes
                    .iter()
                    .filter(|held| held.used != frame)
                    .map(|held| (held.used, path.clone(), held.longest))
            })
            .collect();
        idle.sort_unstable();
        for (_, path, longest) in idle {
            if total <= self.budget {
                break;
            }
            let Some(sizes) = self.held.get_mut(&path) else {
                continue;
            };
            if let Some(at) = sizes.iter().position(|held| held.longest == longest) {
                total -= sizes.swap_remove(at).bytes;
            }
            if sizes.is_empty() {
                self.held.remove(&path);
            }
        }
    }
}

/// The photos of `order`, each with its tier ([`CURRENT`], [`ON_SCREEN`]
/// or [`AHEAD`]), the best first and the tiers never rising, each with a
/// priority of its own to want it at, from the highest down.
///
/// The decoder takes the highest priority queued first, and only among
/// equal priorities the one first wanted last; a worker woken by a want
/// takes what is queued at that moment, before the rest is wanted. So
/// each photo has a priority no other has, its tier above the bits that
/// rank it within the tier (`tier << RANK_BITS | MAX_RANK - rank`), and
/// a view wants them the best first: whatever a worker takes, at any
/// moment, is the best wanted so far.
pub fn ranked(order: impl IntoIterator<Item = (usize, u32)>) -> Vec<(usize, u32)> {
    let mut ranks = [0; CURRENT as usize + 1];
    order
        .into_iter()
        .map(|(index, tier)| {
            let rank = &mut ranks[tier as usize];
            let priority = tier << RANK_BITS | (MAX_RANK - (*rank).min(MAX_RANK)) as u32;
            *rank += 1;
            (index, priority)
        })
        .collect()
}

/// Which of `sizes`, the textures held of a photo, to show for one of
/// `longest` pixels: the one of that size, or else the nearest larger, or
/// else the largest smaller; `None` when none is held.
fn stand_in(sizes: &[Held], longest: u32) -> Option<usize> {
    let rank = |held: &Held| match held.longest.cmp(&longest) {
        Ordering::Equal => (0, 0),
        Ordering::Greater => (1, held.longest),
        Ordering::Less => (2, u32::MAX - held.longest),
    };
    (0..sizes.len()).min_by_key(|&at| rank(&sizes[at]))
}

/// A JPEG file's bytes, `width` by `height` pixels of one colour, for the
/// tests.
#[cfg(test)]
pub fn jpeg(width: u32, height: u32) -> Vec<u8> {
    use image::{ImageFormat, Rgb, RgbImage};
    let image = RgbImage::from_pixel(width, height, Rgb([200, 120, 40]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Jpeg).unwrap();
    bytes.into_inner()
}

/// The longest side, in pixels, to decode a thumbnail at that is drawn
/// at most `points` long on a screen of `pixels_per_point`: the smallest
/// of [`SIZES`] that covers it, or the largest when none does.
pub fn longest(points: f32, pixels_per_point: f32) -> u32 {
    let pixels = (points * pixels_per_point).ceil();
    let largest = SIZES[SIZES.len() - 1];
    SIZES
        .into_iter()
        .find(|&size| size as f32 >= pixels)
        .unwrap_or(largest)
}

/// Draw `thumbnail` in `room`: the texture as large as fits, its aspect
/// kept, in the middle; for a photo that cannot be shown, the icon that
/// says so, `icon_size` points, in the middle, dimmed; and nothing while
/// it is pending, as decoding takes moments and anything shown meanwhile
/// would only flicker. Why the photo cannot be shown, if it cannot.
pub fn paint(ui: &egui::Ui, thumbnail: Thumbnail, room: Rect, icon_size: f32) -> Option<Failure> {
    match thumbnail {
        Thumbnail::Pending => None,
        Thumbnail::Ready { texture, size } => {
            let size = vec2(size[0] as f32, size[1] as f32);
            let image = egui::Image::new(SizedTexture::new(texture, size));
            image.paint_at(ui, placed(thumbnail, room));
            None
        }
        Thumbnail::Failed(failure) => {
            let (name, fallback) = FAILED_ICON;
            let (glyph, font) = icons::glyph(name, icon_size, fallback);
            let colour = ui.visuals().weak_text_color();
            ui.painter()
                .text(room.center(), Align2::CENTER_CENTER, glyph, font, colour);
            Some(failure)
        }
    }
}

/// How far the outline that marks the current photo stands clear of its
/// thumbnail, or of the room the thumbnail goes in while there is none,
/// in points, in every view: the outline is drawn around what [`placed`]
/// gives, grown by this.
pub const MARK_GAP: f32 = 2.0;

/// Where [`paint`] draws `thumbnail` in `room`: the texture's own place,
/// as large as fits, its aspect kept, in the middle; or, with no texture
/// to draw, the whole room, for a view to mark the photo by.
pub fn placed(thumbnail: Thumbnail, room: Rect) -> Rect {
    match thumbnail {
        Thumbnail::Ready { size, .. } => fitted(vec2(size[0] as f32, size[1] as f32), room),
        Thumbnail::Pending | Thumbnail::Failed(_) => room,
    }
}

/// Where to draw a thumbnail of `size` in `room`: as large as fits, its
/// aspect kept, in the middle.
fn fitted(size: Vec2, room: Rect) -> Rect {
    let scale = (room.width() / size.x).min(room.height() / size.y);
    Rect::from_center_size(room.center(), size * scale)
}

/// The tooltip of a photo named `name`, its name drawn cut short when
/// `elided`: the name whole, where it is cut short, and why the photo
/// cannot be shown, where it cannot. `None` when there is nothing to
/// say.
pub fn tooltip(name: String, elided: bool, failure: Option<Failure>) -> Option<String> {
    match (elided, failure) {
        (true, Some(failure)) => Some(format!("{name}\n{failure}")),
        (true, None) => Some(name),
        (false, Some(failure)) => Some(failure.to_string()),
        (false, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// A temporary folder holding the 400 by 300 pixel JPEGs `a.jpg`,
    /// `b.jpg` and `c.jpg`, and `broken.jpg`, which starts as a JPEG does
    /// and ends there.
    fn folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.jpg", "b.jpg", "c.jpg"] {
            fs::write(dir.path().join(name), jpeg(400, 300)).unwrap();
        }
        fs::write(dir.path().join("broken.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        dir
    }

    /// How many requests the store's decoder holds.
    fn pending(store: &Textures) -> usize {
        store.decoder.as_ref().map_or(0, Decoder::pending)
    }

    /// A frame of the store's, in which the photos at `drawn` are wanted
    /// on the screen and drawn, at 256 pixels.
    fn frame(store: &mut Textures, ctx: &egui::Context, drawn: &[&Path]) -> Vec<Thumbnail> {
        store.begin_frame(ctx);
        for path in drawn {
            store.want(path, 256, ON_SCREEN);
        }
        let shown = drawn
            .iter()
            .map(|path| store.thumbnail(path, 256))
            .collect();
        store.end_frame(ctx);
        shown
    }

    /// The texture `thumbnail` shows, if any.
    fn texture_of(thumbnail: Thumbnail) -> Option<TextureId> {
        match thumbnail {
            Thumbnail::Ready { texture, .. } => Some(texture),
            _ => None,
        }
    }

    /// How many textures are held, of every photo and size.
    fn textures(store: &Textures) -> usize {
        store.held.values().map(Vec::len).sum()
    }

    /// A thumbnail is drawn as large as it fits, its aspect kept, in the
    /// middle of its room, smaller or larger than its pixels.
    #[test]
    fn a_thumbnail_is_fitted_in_the_middle_of_its_room() {
        let room = Rect::from_min_size(egui::pos2(10.0, 24.0), vec2(200.0, 186.0));
        let in_middle = |size| Rect::from_center_size(room.center(), size);
        assert_eq!(
            fitted(vec2(512.0, 256.0), room),
            in_middle(vec2(200.0, 100.0))
        );
        assert_eq!(
            fitted(vec2(186.0, 372.0), room),
            in_middle(vec2(93.0, 186.0))
        );
        assert_eq!(
            fitted(vec2(50.0, 50.0), room),
            in_middle(vec2(186.0, 186.0))
        );
    }

    /// A texture is placed where it is fitted in its room; with none to
    /// draw, pending or failed, the place is the whole room.
    #[test]
    fn a_thumbnail_is_placed_where_it_is_fitted_or_in_its_whole_room() {
        let room = Rect::from_min_size(egui::pos2(10.0, 22.0), vec2(100.0, 112.0));
        let ready = Thumbnail::Ready {
            texture: TextureId::Managed(1),
            size: [300, 200],
        };
        assert_eq!(placed(ready, room), fitted(vec2(300.0, 200.0), room));
        assert_ne!(placed(ready, room), room);
        assert_eq!(placed(Thumbnail::Pending, room), room);
        assert_eq!(placed(Thumbnail::Failed(Failure::Damaged), room), room);
    }

    #[test]
    fn the_icon_of_a_photo_that_cannot_be_shown_resolves() {
        let (name, _) = FAILED_ICON;
        assert!(icons::resolves(name), "{name}");
    }

    /// A photo's tooltip holds its name when it is cut short, and why
    /// the photo cannot be shown when it cannot, both on lines of their
    /// own; there is none when there is nothing to say.
    #[test]
    fn a_tooltip_says_the_whole_name_and_why_a_photo_cannot_be_shown() {
        let tip = |elided, failure| tooltip("DSC_0001.NEF".to_owned(), elided, failure);
        let damaged = Some(Failure::Damaged);
        assert_eq!(
            tip(true, damaged).as_deref(),
            Some("DSC_0001.NEF\nThe file is damaged")
        );
        assert_eq!(tip(true, None).as_deref(), Some("DSC_0001.NEF"));
        assert_eq!(tip(false, damaged).as_deref(), Some("The file is damaged"));
        assert_eq!(tip(false, None), None);
    }

    /// A thumbnail is decoded at the smallest size that covers it, and
    /// at the largest when none does.
    #[test]
    fn a_thumbnail_is_decoded_at_the_smallest_size_that_covers_it() {
        assert_eq!(longest(200.0, 1.0), 256);
        assert_eq!(longest(256.0, 1.0), 256);
        assert_eq!(longest(256.5, 1.0), 384);
        assert_eq!(longest(200.0, 1.5), 384);
        assert_eq!(longest(200.0, 2.0), 512);
        assert_eq!(longest(200.0, 3.0), 512);
        assert_eq!(longest(0.0, 2.0), 256);
    }

    /// A photo wanted is decoded, shown as its texture on the next frame,
    /// and not decoded again while that texture is held. Wanted at
    /// another size, it is decoded again at that size, the texture held
    /// shown meanwhile, and both sizes are held.
    #[test]
    fn each_size_of_a_photo_has_a_texture_of_its_own() {
        let dir = folder();
        let a = dir.path().join("a.jpg");
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        assert_eq!(frame(&mut store, &ctx, &[&a]), [Thumbnail::Pending]);
        let shown = frame(&mut store, &ctx, &[&a]);
        assert!(
            matches!(
                shown[..],
                [Thumbnail::Ready {
                    size: [256, 192],
                    ..
                }]
            ),
            "{shown:?}"
        );
        assert_eq!(pending(&store), 0);
        store.want(&a, 384, ON_SCREEN);
        assert_eq!(pending(&store), 1);
        assert_eq!(store.thumbnail(&a, 384), shown[0], "the 256 meanwhile");
        store.end_frame(&ctx);
        store.begin_frame(&ctx);
        let big = store.thumbnail(&a, 384);
        assert!(
            matches!(
                big,
                Thumbnail::Ready {
                    size: [384, 288],
                    ..
                }
            ),
            "{big:?}"
        );
        assert_eq!(store.thumbnail(&a, 256), shown[0]);
        assert_eq!(textures(&store), 2);
    }

    /// While the size wanted is on its way, the nearest larger size held
    /// stands in for it, or else the largest smaller one.
    #[test]
    fn another_size_stands_in_for_the_one_on_its_way() {
        let dir = folder();
        let a = dir.path().join("a.jpg");
        let ctx = egui::Context::default();
        // The texture of `a` at `longest` pixels, decoded for `store`.
        let decoded = |store: &mut Textures, longest| {
            store.begin_frame(&ctx);
            store.want(&a, longest, ON_SCREEN);
            store.end_frame(&ctx);
            store.begin_frame(&ctx);
            store.texture(&a, longest).unwrap()
        };
        let mut store = Textures::inline();
        let small = decoded(&mut store, 256);
        assert_eq!(texture_of(store.thumbnail(&a, 384)), Some(small));
        let large = decoded(&mut store, 512);
        assert_eq!(texture_of(store.thumbnail(&a, 384)), Some(large));
        assert_eq!(texture_of(store.thumbnail(&a, 256)), Some(small));
    }

    /// A photo that cannot be shown says why, and is not decoded again,
    /// frame after frame, until the folder's photos are read again; then
    /// it is decoded again, and says why meanwhile, with no gap.
    #[test]
    fn a_photo_that_cannot_be_shown_is_not_wanted_again_until_a_new_read() {
        let dir = folder();
        let broken = dir.path().join("broken.jpg");
        let failed = Thumbnail::Failed(previews::decode(&broken, 256).unwrap_err());
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        store.shown(1);
        frame(&mut store, &ctx, &[&broken]);
        for _ in 0..2 {
            assert_eq!(frame(&mut store, &ctx, &[&broken]), [failed]);
            assert_eq!(pending(&store), 0);
        }
        store.shown(1);
        store.want(&broken, 256, ON_SCREEN);
        assert_eq!(store.thumbnail(&broken, 256), failed);
        assert_eq!(pending(&store), 0);
        store.shown(2);
        store.want(&broken, 256, ON_SCREEN);
        assert_eq!(store.thumbnail(&broken, 256), failed);
        assert_eq!(pending(&store), 1);
        store.end_frame(&ctx);
        assert_eq!(frame(&mut store, &ctx, &[&broken]), [failed]);
        assert_eq!(pending(&store), 0);
    }

    /// Photos listed the best first, their tiers never rising, are each
    /// given a priority of their own, from the highest down, any of a
    /// higher tier above any of a lower one; past the last rank, photos
    /// of a tier share it.
    #[test]
    fn photos_listed_best_first_get_priorities_from_the_highest_down() {
        let order = [
            (7, CURRENT),
            (3, ON_SCREEN),
            (4, ON_SCREEN),
            (9, AHEAD),
            (1, AHEAD),
        ];
        let given = ranked(order);
        let indices: Vec<usize> = given.iter().map(|&(index, _)| index).collect();
        assert_eq!(indices, [7, 3, 4, 9, 1]);
        let priorities: Vec<u32> = given.iter().map(|&(_, priority)| priority).collect();
        assert!(
            priorities.windows(2).all(|pair| pair[0] > pair[1]),
            "{priorities:?}"
        );
        let top = MAX_RANK as u32;
        assert_eq!(priorities[0], CURRENT << RANK_BITS | top);
        assert_eq!(
            priorities[1..3],
            [
                ON_SCREEN << RANK_BITS | top,
                ON_SCREEN << RANK_BITS | (top - 1)
            ]
        );
        assert!(priorities[2] > (AHEAD << RANK_BITS | top));
        let many = ranked((0..MAX_RANK + 2).map(|index| (index, AHEAD)));
        assert_eq!(many[MAX_RANK].1, 0);
        assert_eq!(many[MAX_RANK + 1].1, 0);
    }

    /// The photos not wanted in a frame are not decoded: their requests
    /// are dropped as the frame ends.
    #[test]
    fn the_photos_not_wanted_in_a_frame_are_not_decoded() {
        let dir = folder();
        let [a, b, c] = ["a.jpg", "b.jpg", "c.jpg"].map(|name| dir.path().join(name));
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        store.begin_frame(&ctx);
        store.want(&a, 256, ON_SCREEN);
        store.want(&b, 256, AHEAD);
        store.want(&c, 256, AHEAD);
        store.end_frame(&ctx);
        assert_eq!(pending(&store), 3);
        // The answers are left to come, so the requests stay queued.
        store.want(&b, 256, AHEAD);
        store.end_frame(&ctx);
        assert_eq!(pending(&store), 1);
        store.begin_frame(&ctx);
        assert!(store.texture(&b, 256).is_some());
        assert_eq!(store.texture(&a, 256), None);
        assert_eq!(store.texture(&c, 256), None);
    }

    /// A new read of the photos, of the folder shown again or of another,
    /// drops what was wanted before it, so nothing decoded from a file as
    /// it was before the read is handed out.
    #[test]
    fn a_new_read_drops_what_was_wanted_before_it() {
        let dir = folder();
        let a = dir.path().join("a.jpg");
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        store.begin_frame(&ctx);
        store.shown(1);
        store.want(&a, 256, ON_SCREEN);
        store.shown(1);
        assert_eq!(pending(&store), 1);
        store.shown(2);
        assert_eq!(pending(&store), 0);
    }

    /// A texture decoded before the photos were read again is still shown,
    /// with no gap, but is decoded again, as the file may have changed,
    /// and replaced when the new one comes.
    #[test]
    fn a_texture_from_before_a_new_read_is_shown_until_it_is_decoded_again() {
        let dir = folder();
        let a = dir.path().join("a.jpg");
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        store.shown(1);
        frame(&mut store, &ctx, &[&a]);
        let [before] = frame(&mut store, &ctx, &[&a])[..] else {
            unreachable!()
        };
        assert!(texture_of(before).is_some(), "{before:?}");
        store.shown(2);
        assert_eq!(frame(&mut store, &ctx, &[&a]), [before]);
        let [after] = frame(&mut store, &ctx, &[&a])[..] else {
            unreachable!()
        };
        assert!(texture_of(after).is_some(), "{after:?}");
        assert_ne!(after, before);
        assert_eq!(textures(&store), 1);
        assert_eq!(pending(&store), 0);
    }

    /// A texture is counted at four bytes a pixel against the budget, and
    /// no more, as it has no mipmaps, which the renderer does not make.
    #[test]
    fn a_texture_is_counted_at_four_bytes_a_pixel() {
        let dir = folder();
        let a = dir.path().join("a.jpg");
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        frame(&mut store, &ctx, &[&a]);
        frame(&mut store, &ctx, &[&a]);
        assert_eq!(store.held[&a][0].bytes, 4 * 256 * 192);
    }

    /// Past the budget, the textures wanted least recently go first, and
    /// those wanted or drawn in the frame never do, even past it, a
    /// texture of another size drawn in place of the one wanted among
    /// them.
    #[test]
    fn past_the_budget_the_least_recently_wanted_textures_go_first() {
        let dir = folder();
        let [a, b, c] = ["a.jpg", "b.jpg", "c.jpg"].map(|name| dir.path().join(name));
        let ctx = egui::Context::default();
        let mut store = Textures::inline();
        frame(&mut store, &ctx, &[&a, &b, &c]);
        frame(&mut store, &ctx, &[&a, &b, &c]);
        let texture = 4 * 256 * 192;
        store.budget = 3 * texture;
        frame(&mut store, &ctx, &[&b]);
        frame(&mut store, &ctx, &[&c]);
        assert_eq!(textures(&store), 3);
        // Of the two not drawn, `b` was drawn last the longest ago.
        store.budget = 2 * texture;
        frame(&mut store, &ctx, &[&a]);
        assert!(!store.held.contains_key(&b));
        assert!(store.held.contains_key(&a) && store.held.contains_key(&c));
        store.budget = 1;
        frame(&mut store, &ctx, &[&a]);
        assert_eq!(store.held.keys().collect::<Vec<_>>(), [&a]);
        // `a` at 384 is on its way, its 256 drawn in its place: kept.
        store.begin_frame(&ctx);
        store.want(&a, 384, ON_SCREEN);
        assert!(texture_of(store.thumbnail(&a, 384)).is_some());
        store.end_frame(&ctx);
        assert_eq!(textures(&store), 1);
    }
}
