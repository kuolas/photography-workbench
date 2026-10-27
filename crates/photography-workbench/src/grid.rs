// SPDX-License-Identifier: GPL-3.0-or-later

//! The thumbnail grid in Library's centre: a square tile for each of a
//! folder's photos, row after row, the current one outlined. A row holds
//! as many tiles as fit at the smallest they may be, and they stretch to
//! share its width, so the grid fills the centre whatever its width.
//!
//! A tile shows its photo's file name over its thumbnail, which sits
//! straight on the grid's backdrop with nothing behind it, or, for a
//! photo that cannot be shown, an icon there, and why in its tooltip. The
//! current photo's thumbnail is outlined, a little clear of its edge, or,
//! until there is one, the room it goes in. A click on a tile makes its
//! photo the current one, and a double-click opens it in the viewer too.
//! The tile, though nothing marks it, is still what the grid lays out
//! and brings into view.
//!
//! The grid draws a list of photos and the index of the current one,
//! whatever they were read from, and the thumbnails the app holds
//! ([`crate::textures`]). Only the rows in view are laid out, so a
//! folder of thousands of photos scrolls as smoothly as one of ten. Their
//! thumbnails are decoded in the order a photographer reads them, the
//! current photo's first, and after them those of a screen of rows above
//! and one below, the nearest first, so they are ready when scrolled to
//! ([`want_order`]).
//!
//! The tiles are laid out again whenever the grid's width changes, as
//! the window or a dock is resized, and when the size of the tiles does:
//! the rows are then another height, and may hold another number of
//! photos, so the grid is scrolled to where the photo at the top of the
//! view now is, and the view keeps its place ([`anchored`]).
//!
//! The grid brings the current photo's row into view, scrolled just far
//! enough, when the current photo changes, when the list does, or when
//! the grid comes back on screen, and then only if the photo's tile is
//! not wholly in view already ([`crate::follow`]). It glides there over
//! the next frames, as egui scrolls when asked to show something,
//! rather than jumping, so the eye follows where the photos went. A
//! click is the exception: the photo clicked on is where the pointer
//! is, and moving the grid under it would put another photo under the
//! second click of a double-click.
//!
//! The keys that move a row up or down ask the grid which photo is
//! above or below another ([`above`], [`below`]), for the number of
//! tiles in a row it last laid out.

use std::ops::Range;
use std::path::Path;

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{FontId, Margin, Rect, Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType, vec2};

use crate::follow;
use crate::textures::{self, AHEAD, CURRENT, ON_SCREEN, Textures, Thumbnail};

/// The smallest side of a tile: a row holds as many tiles at least this
/// wide as fit, and they stretch to share its width ([`side`]).
const TILE: f32 = 220.0;

/// The gap between two tiles, across and down.
const GAP: f32 = 1.0;

/// The room between the centre's edges and the tiles.
const MARGIN: i8 = 4;

/// Where a photo's name starts, from its tile's left edge; it ends as far
/// from the right edge.
const NAME_LEFT: f32 = 8.0;

/// Where the middle of a photo's name is, from its tile's top edge. The
/// name's text box, about 13 points high, ends some 18.5 points down,
/// above where the current photo's outline runs round a thumbnail as tall
/// as its room ([`IMAGE_INSET`]), from 20 to 22 points down.
const NAME_MIDDLE: f32 = 12.0;

/// The size of a photo's name.
const NAME_SIZE: f32 = 10.5;

/// The width of the outline around the current photo: around its
/// thumbnail, or around the room the thumbnail goes in while there is
/// none to draw, [`textures::MARK_GAP`] clear of either.
const OUTLINE: f32 = 2.0;

/// The room between a tile's edges and its thumbnail, which is fitted
/// in what is left; the photo's name is in the room above it. Nothing is
/// drawn behind the thumbnail: the grid's backdrop shows around it.
const IMAGE_INSET: Margin = Margin {
    left: 10,
    right: 10,
    top: 24,
    bottom: 10,
};

/// The size of the icon a tile shows when its photo cannot be shown.
const FAILED_ICON_SIZE: f32 = 32.0;

/// A click on a tile: which photo it was on, and whether it was a
/// double-click, which opens the photo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    /// The index of the photo clicked on.
    pub index: usize,
    /// Whether it was a double-click.
    pub open: bool,
}

/// How the grid was last laid out, kept in egui's memory from one frame
/// to the next: how many tiles to a row, and their side.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LaidOut {
    columns: usize,
    side: f32,
}

/// What the grid did in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    /// How many tiles it laid out in a row.
    pub columns: usize,
    /// The click on a tile, if any.
    pub click: Option<Click>,
}

/// The grid of `photos`, the photos of `folder`, filling `ui`, with the
/// photo at `current` outlined and the thumbnails in `textures`; it
/// scrolls when the rows do not fit. As many tiles fit in a row as the
/// width allows at their smallest, and at least one, and they stretch to
/// share the width; only the pixel fractions are left over, at the right.
/// `folder` tells one list from another, so the grid brings the current
/// photo into view when the list changes.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    textures: &mut Textures,
    photos: &[Photo],
    current: Option<usize>,
    folder: &Path,
) -> Shown {
    let columns = columns(ui.available_width());
    let side = side(ui.available_width(), columns, ui.pixels_per_point());
    let rows = photos.len().div_ceil(columns);
    let id = ui.make_persistent_id("grid");
    let laid_out = LaidOut { columns, side };
    let laid_out_id = id.with("laid out");
    let before = ui.ctx().data(|data| data.get_temp::<LaidOut>(laid_out_id));
    ui.ctx()
        .data_mut(|data| data.insert_temp(laid_out_id, laid_out));
    let list = egui::Id::new(folder);
    let follow = follow::follows(ui.ctx(), id, list, current);
    let mut click = None;
    ui.scope(|ui| {
        // Rows 1 point apart: egui adds the gap between items to each
        // row's height.
        ui.spacing_mut().item_spacing.y = GAP;
        let mut area = egui::ScrollArea::vertical()
            .id_salt("grid")
            .auto_shrink([false, false])
            .content_margin(Margin::same(MARGIN));
        // The scroll area keeps where it is scrolled to under the id egui
        // makes from its salt.
        let scroll_id = ui.make_persistent_id(egui::IdSalt::new("grid"));
        let state = egui::scroll_area::State::load(ui.ctx(), scroll_id);
        let offset = state.map_or(0.0, |state| state.offset.y);
        // Set before the rows are laid out, so the rows laid out are the
        // ones scrolled to, however far they are.
        if let Some(before) = before
            && before != laid_out
        {
            area = area.vertical_scroll_offset(anchored(offset, before, laid_out));
        }
        area.show_rows(ui, side, rows, |ui, range| {
            if follow && let Some(index) = current {
                // Asked from inside the scroll area, where it listens. It
                // glides there over the next frames, and lays out the rows
                // it passes as it goes, however far the row is.
                ui.scroll_to_rect(row_to_show(ui, range.start, index / columns, side), None);
            }
            // Every tile's thumbnail fits in the same room.
            let room = Vec2::splat(side) - IMAGE_INSET.sum();
            let longest = textures::longest(room.max_elem(), ui.pixels_per_point());
            for (index, priority) in want_order(range.clone(), rows, columns, photos.len(), current)
            {
                textures.want(photos[index].path(), longest, priority);
            }
            let mut outline = None;
            for row in range {
                let (_, rect) = ui.allocate_space(vec2(ui.available_width(), side));
                let first = row * columns;
                for (column, photo) in photos[first..].iter().take(columns).enumerate() {
                    let left_top = rect.left_top() + vec2(column as f32 * (side + GAP), 0.0);
                    let tile = Rect::from_min_size(left_top, egui::Vec2::splat(side));
                    let index = first + column;
                    let is_current = current == Some(index);
                    let thumbnail = textures.thumbnail(photo.path(), longest);
                    if let Some(open) = photo_tile(ui, palette, photo, thumbnail, tile, is_current)
                    {
                        click = Some(Click { index, open });
                    }
                    if is_current {
                        let place = textures::placed(thumbnail, tile - IMAGE_INSET);
                        outline = Some(place.expand(textures::MARK_GAP));
                    }
                }
            }
            // Last, so the tiles next to it do not cover it.
            if let Some(around) = outline {
                let stroke = Stroke::new(OUTLINE, palette.accent);
                ui.painter()
                    .rect_stroke(around, 0.0, stroke, StrokeKind::Outside);
            }
        });
    });
    if let Some(click) = click {
        follow::shown(ui.ctx(), id, list, Some(click.index));
    }
    Shown { columns, click }
}

/// Where to scroll a grid laid out `now`, so that the photo at the top of
/// its view is where it was when the grid was laid out `before` and
/// scrolled `offset` points down its rows: the photo that began the row
/// at the view's top, in the row it is in now, with the view as far into
/// that row as it was into the other. A grid scrolled to its top, where
/// the margin is, stays there.
fn anchored(offset: f32, before: LaidOut, now: LaidOut) -> f32 {
    let margin = f32::from(MARGIN);
    if offset <= margin {
        return offset;
    }
    let rows = (offset - margin) / (before.side + GAP);
    let row = rows.floor();
    let into = rows - row;
    let first = row as usize * before.columns.max(1);
    let row_now = (first / now.columns.max(1)) as f32;
    margin + (row_now + into) * (now.side + GAP)
}

/// Where the tiles of `row`, `side` points high, are, in `ui`, which lays
/// out the rows from `first`, with the room to leave between them and the
/// view's edge when the row is scrolled into view: 4 points, as the first
/// and last rows have when the grid is scrolled to its ends, less the gap
/// egui leaves of its own, which is the one between the rows. Scrolled
/// to, the row is brought just far enough that its tiles are whole: up
/// until they are 4 points below the view's top edge, when that edge cuts
/// them or they are above it, or down until they are 4 points above its
/// bottom edge, when that cuts them or they are below it, and not at all
/// when they are in view already.
fn row_to_show(ui: &egui::Ui, first: usize, row: usize, side: f32) -> Rect {
    let rows = row as f32 - first as f32;
    let top = ui.max_rect().top() + rows * (side + GAP);
    let tiles = Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=top + side);
    tiles.expand2(vec2(0.0, f32::from(MARGIN) - GAP))
}

/// The index of the photo above the one at `index`, in a grid of
/// `columns` to a row: the one in the same column of the row before, or
/// `index` itself in the first row.
pub fn above(index: usize, columns: usize) -> usize {
    index.checked_sub(columns.max(1)).unwrap_or(index)
}

/// The index of the photo below the one at `index`, in a grid of `len`
/// photos, `columns` to a row: the one in the same column of the row
/// after, or the last photo when that row is too short to reach the
/// column, as the last row can be, or `index` itself in the last row.
pub fn below(index: usize, len: usize, columns: usize) -> usize {
    let columns = columns.max(1);
    let last = len.saturating_sub(1);
    if index / columns < last / columns {
        (index + columns).min(last)
    } else {
        index
    }
}

/// The photos whose thumbnails a grid of `len` photos, `columns` to a
/// row in `rows` rows, wants while it shows the rows in `range`, the best
/// first, each with a priority of its own from the highest down
/// ([`textures::ranked`]): the current photo ([`CURRENT`]); then the
/// photos in view, as a photographer reads them; then those of the rows
/// ahead ([`rows_ahead`]), each row from its first photo to its last.
fn want_order(
    range: Range<usize>,
    rows: usize,
    columns: usize,
    len: usize,
    current: Option<usize>,
) -> Vec<(usize, u32)> {
    let photos_of = |row: usize| row * columns..((row + 1) * columns).min(len);
    let in_view = range
        .clone()
        .flat_map(photos_of)
        .map(|index| (index, ON_SCREEN));
    let ahead = rows_ahead(range, rows).into_iter().flat_map(photos_of);
    let mut order: Vec<(usize, u32)> = in_view.chain(ahead.map(|index| (index, AHEAD))).collect();
    if let Some(at) = order.iter().position(|&(index, _)| Some(index) == current) {
        let (index, _) = order.remove(at);
        order.insert(0, (index, CURRENT));
    }
    textures::ranked(order)
}

/// The rows wanted ahead of being scrolled to, of a grid of `rows` rows
/// showing those in `range`: as many as it shows above them, and as many
/// below, as far as there are rows; the nearest first, and of two as
/// near, the one below first, as a grid is mostly scrolled down.
fn rows_ahead(range: Range<usize>, rows: usize) -> Vec<usize> {
    let mut ahead = Vec::new();
    for distance in 1..=range.len() {
        let below = range.end - 1 + distance;
        if below < rows {
            ahead.push(below);
        }
        if let Some(above) = range.start.checked_sub(distance) {
            ahead.push(above);
        }
    }
    ahead
}

/// How many tiles at their smallest ([`TILE`]) fit in a row of a grid
/// `width` points wide: at least one, however narrow it is.
fn columns(width: f32) -> usize {
    let room = width - 2.0 * f32::from(MARGIN) + GAP;
    // A negative width is cast to none.
    ((room / (TILE + GAP)).floor() as usize).max(1)
}

/// The side of the tiles in a row of a grid `width` points wide that
/// holds `columns` of them: the room between the margins, less the gaps,
/// shared equally, and rounded down to whole pixels on a screen of
/// `pixels_per_point`, so that on a screen of a whole number of pixels a
/// point every tile's edges lie on the pixel grid and its thumbnail is
/// drawn sharp; what is left over, less than a pixel a tile, stays at
/// the right. At least a pixel, however narrow the grid.
fn side(width: f32, columns: usize, pixels_per_point: f32) -> f32 {
    let columns = columns.max(1) as f32;
    let room = width - 2.0 * f32::from(MARGIN) - (columns - 1.0) * GAP;
    let pixels = (room / columns * pixels_per_point).floor().max(1.0);
    pixels / pixels_per_point
}

/// The tile of `photo` at `rect`, showing `thumbnail` on the grid's
/// backdrop, outlined elsewhere when `is_current`. Whether it was
/// clicked, `Some(true)` for a double-click.
///
/// Its id comes from its photo's path, not from where it is, so a tile
/// keeps its state when photos before it come or go.
fn photo_tile(
    ui: &mut egui::Ui,
    palette: &Palette,
    photo: &Photo,
    thumbnail: Thumbnail,
    rect: Rect,
    is_current: bool,
) -> Option<bool> {
    let name = photo.name().into_owned();
    let id = egui::Id::new(("photo_tile", photo.path()));
    let response = ui.interact(rect, id, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, is_current, &name));

    let painter = ui.painter();
    let failure = textures::paint(ui, thumbnail, rect - IMAGE_INSET, FAILED_ICON_SIZE);
    // The name, cut short with "…" where it would run past its room.
    let mut job = LayoutJob::single_section(
        name.clone(),
        TextFormat::simple(FontId::proportional(NAME_SIZE), palette.text),
    );
    job.wrap = TextWrapping::truncate_at_width(rect.width() - 2.0 * NAME_LEFT);
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    painter.galley(
        rect.left_top() + vec2(NAME_LEFT, NAME_MIDDLE - galley.size().y / 2.0),
        galley,
        palette.text,
    );
    let response = match textures::tooltip(name, elided, failure) {
        Some(tooltip) => response.on_hover_text(tooltip),
        None => response,
    };

    // egui counts a click within twice the double-click time of the click
    // before last as a third, even when that one was elsewhere: so the
    // second click of a double-click on a tile just after a click on
    // another is a triple-click to it.
    if response.double_clicked() || response.triple_clicked() {
        Some(true)
    } else if response.clicked() {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{LaidOut, above, anchored, below, columns, rows_ahead, side, want_order};
    use crate::textures::{self, AHEAD, CURRENT, ON_SCREEN};

    /// A row holds as many tiles of 220 points at least, 1 point apart, as
    /// fit between 4 points of room at each side, and at least one.
    #[test]
    fn a_row_holds_as_many_tiles_as_fit() {
        assert_eq!(columns(8.0 + 220.0), 1);
        assert_eq!(columns(8.0 + 441.0 - 0.5), 1);
        assert_eq!(columns(8.0 + 441.0), 2);
        assert_eq!(columns(8.0 + 3.0 * 221.0 - 1.0), 3);
        assert_eq!(columns(100.0), 1);
        assert_eq!(columns(-5.0), 1);
    }

    /// The tiles of a row share its width, less the margins and the gaps,
    /// rounded down to whole pixels of the screen, and are a pixel at
    /// least.
    #[test]
    fn the_tiles_of_a_row_share_its_width() {
        assert_eq!(side(8.0 + 441.0, 2, 1.0), 220.0);
        assert_eq!(side(8.0 + 441.0 + 10.0, 2, 1.0), 225.0);
        assert_eq!(side(8.0 + 441.0 + 1.0, 2, 1.0), 220.0, "half a point");
        assert_eq!(side(8.0 + 441.0 + 1.0, 2, 2.0), 220.5, "a whole pixel");
        assert_eq!(side(8.0 + 3.0 * 221.0 - 1.0 + 2.0, 3, 1.0), 220.0);
        assert_eq!(side(8.0 + 3.0 * 221.0 - 1.0 + 3.0, 3, 1.0), 221.0);
        assert_eq!(side(100.0, 1, 1.0), 92.0, "a lone tile fills a narrow row");
        assert_eq!(side(-5.0, 1, 1.0), 1.0);
        assert_eq!(side(-5.0, 1, 2.0), 0.5);
        assert_eq!(
            side(8.0 + 441.0, 0, 1.0),
            441.0,
            "a row has a tile at least"
        );
    }

    /// Up goes to the same column of the row before, and stays put in
    /// the first row.
    #[test]
    fn the_photo_above_is_a_row_back() {
        assert_eq!(above(7, 3), 4);
        assert_eq!(above(3, 3), 0);
        assert_eq!(above(2, 3), 2);
        assert_eq!(above(0, 3), 0);
        assert_eq!(above(5, 1), 4);
        assert_eq!(above(5, 0), 4, "a grid has a column at least");
    }

    /// Down goes to the same column of the row after; to the last photo
    /// when the last row is too short to reach the column; and stays put
    /// in the last row. Of 8 photos 3 to a row, the last row holds 6 and
    /// 7.
    #[test]
    fn the_photo_below_is_a_row_on() {
        assert_eq!(below(0, 8, 3), 3);
        assert_eq!(below(1, 8, 3), 4);
        assert_eq!(below(3, 8, 3), 6);
        assert_eq!(below(4, 8, 3), 7);
        assert_eq!(below(5, 8, 3), 7, "the row below is too short");
        assert_eq!(below(6, 8, 3), 6, "the last row");
        assert_eq!(below(7, 8, 3), 7, "the last row");
        assert_eq!(below(2, 3, 3), 2, "one row");
        assert_eq!(below(0, 1, 3), 0, "one photo");
        assert_eq!(below(0, 0, 3), 0, "no photos");
        assert_eq!(below(2, 4, 1), 3);
        assert_eq!(below(2, 4, 0), 3, "a grid has a column at least");
    }

    /// A screen of rows above those in view is wanted ahead, and one
    /// below, as far as the grid has rows, the nearest first and, of two
    /// as near, the one below first.
    #[test]
    fn a_screen_of_rows_is_wanted_above_and_below_the_view() {
        assert_eq!(rows_ahead(5..8, 20), [8, 4, 9, 3, 10, 2]);
        assert_eq!(rows_ahead(0..3, 20), [3, 4, 5]);
        assert_eq!(rows_ahead(1..4, 5), [4, 0]);
        assert_eq!(rows_ahead(0..2, 2), Vec::<usize>::new());
        assert_eq!(rows_ahead(0..0, 0), Vec::<usize>::new());
    }

    /// The photos are wanted the best first, each at a priority of its
    /// own from the highest down, so a worker that takes one while the
    /// rest are still being wanted takes the best: the current photo,
    /// then those in view in reading order, then those of the rows
    /// ahead, the nearest first. Here 10 photos, 3 to a row, the second
    /// and third rows in view and the fifth photo current.
    #[test]
    fn the_photos_are_wanted_the_best_first_at_falling_priorities() {
        let order = want_order(1..3, 4, 3, 10, Some(4));
        let indices: Vec<usize> = order.iter().map(|&(index, _)| index).collect();
        assert_eq!(indices, [4, 3, 5, 6, 7, 8, 9, 0, 1, 2]);
        let tiers = [
            CURRENT, ON_SCREEN, ON_SCREEN, ON_SCREEN, ON_SCREEN, ON_SCREEN,
        ];
        let tiers = tiers.into_iter().chain([AHEAD; 4]);
        assert_eq!(order, textures::ranked(indices.into_iter().zip(tiers)));
        assert!(
            order.windows(2).all(|pair| pair[0].1 > pair[1].1),
            "{order:?}"
        );
        // The last row may be short, and the current photo ahead.
        let order = want_order(2..3, 4, 3, 10, Some(9));
        let indices: Vec<usize> = order.iter().map(|&(index, _)| index).collect();
        assert_eq!(indices, [9, 6, 7, 8, 3, 4, 5]);
    }

    /// Laid out again, the grid is scrolled to where the photo at the top
    /// of its view now is, as far into its row as the view was: taller
    /// tiles move the view down by the rows' growth, more to a row move it
    /// up to the row the photo is in now, and the grid's top stays its top.
    #[test]
    fn a_grid_laid_out_again_keeps_the_photo_at_the_top_of_its_view() {
        let laid = |columns, side| LaidOut { columns, side };
        // The view at the top of row 5 (4 + 5 * 221), the tiles 10 taller.
        assert_eq!(
            anchored(1109.0, laid(3, 220.0), laid(3, 230.0)),
            4.0 + 5.0 * 231.0
        );
        // Half a row into row 5.
        let half = anchored(1109.0 + 110.5, laid(3, 220.0), laid(3, 230.0));
        assert_eq!(half, 4.0 + 5.5 * 231.0);
        // Four to a row, then five: row 5 began with photo 20, now in row 4.
        assert_eq!(
            anchored(1109.0, laid(4, 220.0), laid(5, 220.0)),
            4.0 + 4.0 * 221.0
        );
        // Five to a row, then three: photo 25, now in row 8.
        assert_eq!(
            anchored(1109.0, laid(5, 220.0), laid(3, 220.0)),
            4.0 + 8.0 * 221.0
        );
        // At the top, or within the margin, the grid stays where it is.
        assert_eq!(anchored(0.0, laid(3, 220.0), laid(2, 330.0)), 0.0);
        assert_eq!(anchored(4.0, laid(3, 220.0), laid(2, 330.0)), 4.0);
        assert_eq!(
            anchored(1109.0, laid(0, 220.0), laid(0, 220.0)),
            1109.0,
            "a column at least"
        );
    }
}
