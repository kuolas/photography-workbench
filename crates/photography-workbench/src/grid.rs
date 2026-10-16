// SPDX-License-Identifier: GPL-3.0-or-later

//! The thumbnail grid in Library's centre: a square tile for each of a
//! folder's photos, row after row, the current one outlined.
//!
//! A tile shows its photo's file name, where the photo's thumbnail goes
//! once the app decodes previews. A click on a tile makes its photo the
//! current one, and a double-click opens it in the viewer too.
//!
//! The grid draws a list of photos and the index of the current one,
//! whatever they were read from. Only the rows in view are laid out, so
//! a folder of thousands of photos scrolls as smoothly as one of ten.
//!
//! The grid brings the current photo's row into view, scrolled just far
//! enough, when the current photo changes, when the list does, or when
//! the grid comes back on screen, and then only if the photo's tile is
//! not wholly in view already ([`crate::follow`]). A click is the
//! exception: the photo clicked on is where the pointer is, and moving
//! the grid under it would put another photo under the second click of
//! a double-click.
//!
//! The keys that move a row up or down ask the grid which photo is
//! above or below another ([`above`], [`below`]), for the number of
//! tiles in a row it last laid out.

use std::path::Path;

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{FontId, Margin, Rect, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, vec2};

use crate::follow;

/// The side of a tile.
const TILE: f32 = 220.0;

/// The gap between two tiles, across and down.
const GAP: f32 = 1.0;

/// The room between the centre's edges and the tiles.
const MARGIN: i8 = 4;

/// Where a photo's name starts, from its tile's left edge; it ends as far
/// from the right edge.
const NAME_LEFT: f32 = 8.0;

/// Where the top of a photo's name is, from its tile's top edge.
const NAME_TOP: f32 = 12.0;

/// The size of a photo's name.
const NAME_SIZE: f32 = 10.5;

/// The width of the outline around the current photo's tile.
const OUTLINE: f32 = 2.0;

/// A click on a tile: which photo it was on, and whether it was a
/// double-click, which opens the photo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    /// The index of the photo clicked on.
    pub index: usize,
    /// Whether it was a double-click.
    pub open: bool,
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
/// photo at `current` outlined; it scrolls when the rows do not fit. As
/// many tiles fit in a row as the width allows, and at least one; the
/// room left over stays at the right. `folder` tells one list from
/// another, so the grid brings the current photo into view when the
/// list changes.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    photos: &[Photo],
    current: Option<usize>,
    folder: &Path,
) -> Shown {
    let columns = columns(ui.available_width());
    let rows = photos.len().div_ceil(columns);
    let id = ui.make_persistent_id("grid");
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
        if follow && let Some(index) = current {
            // The scroll area keeps where it is scrolled to under the id
            // egui makes from its salt. Its view is the height left here:
            // its one scroll bar runs down its side and takes none of it.
            let scroll_id = ui.make_persistent_id(egui::IdSalt::new("grid"));
            let state = egui::scroll_area::State::load(ui.ctx(), scroll_id);
            let offset = state.map_or(0.0, |state| state.offset.y);
            let height = ui.available_height();
            // Set before the rows are laid out, so the rows laid out are
            // the ones scrolled to, however far they are.
            if let Some(offset) = scrolled_to_show(index / columns, offset, height) {
                area = area.vertical_scroll_offset(offset);
            }
        }
        area.show_rows(ui, TILE, rows, |ui, range| {
            let mut outline = None;
            for row in range {
                let (_, rect) = ui.allocate_space(vec2(ui.available_width(), TILE));
                let first = row * columns;
                for (column, photo) in photos[first..].iter().take(columns).enumerate() {
                    let left_top = rect.left_top() + vec2(column as f32 * (TILE + GAP), 0.0);
                    let tile = Rect::from_min_size(left_top, egui::Vec2::splat(TILE));
                    let index = first + column;
                    let is_current = current == Some(index);
                    if let Some(open) = photo_tile(ui, palette, photo, tile, is_current) {
                        click = Some(Click { index, open });
                    }
                    if is_current {
                        outline = Some(tile);
                    }
                }
            }
            // Last, so the tiles next to it do not cover it.
            if let Some(tile) = outline {
                let stroke = Stroke::new(OUTLINE, palette.accent);
                ui.painter()
                    .rect_stroke(tile, 0.0, stroke, StrokeKind::Outside);
            }
        });
    });
    if let Some(click) = click {
        follow::shown(ui.ctx(), id, list, Some(click.index));
    }
    Shown { columns, click }
}

/// Where to scroll a grid scrolled `offset` points down its rows, with
/// `height` points of them in view, so the tiles of `row` are wholly in
/// view, scrolled as little as it takes: up until they are 4 points
/// below the view's top edge, when that edge cuts them or they are above
/// it, or down until they are 4 points above its bottom edge, when that
/// cuts them or they are below it, as the first and last rows are when
/// the grid is scrolled to its ends. A view too short for a whole tile
/// shows its top. `None` when they are wholly in view already.
fn scrolled_to_show(row: usize, offset: f32, height: f32) -> Option<f32> {
    let margin = f32::from(MARGIN);
    let top = margin + row as f32 * (TILE + GAP);
    let bottom = top + TILE;
    if offset <= top && bottom <= offset + height {
        None
    } else if top < offset {
        Some(top - margin)
    } else {
        Some((bottom + margin - height).min(top - margin))
    }
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

/// How many tiles fit in a row of a grid `width` points wide: at least
/// one, however narrow it is.
fn columns(width: f32) -> usize {
    let room = width - 2.0 * f32::from(MARGIN) + GAP;
    // A negative width is cast to none.
    ((room / (TILE + GAP)).floor() as usize).max(1)
}

/// The tile of `photo` at `rect`, outlined elsewhere when `is_current`.
/// Whether it was clicked, `Some(true)` for a double-click.
///
/// Its id comes from its photo's path, not from where it is, so a tile
/// keeps its state when photos before it come or go.
fn photo_tile(
    ui: &mut egui::Ui,
    palette: &Palette,
    photo: &Photo,
    rect: Rect,
    is_current: bool,
) -> Option<bool> {
    let name = photo.name().into_owned();
    let id = egui::Id::new(("photo_tile", photo.path()));
    let response = ui.interact(rect, id, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, is_current, &name));

    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.chip);
    // The name, cut short with "…" where it would run past its room.
    let mut job = LayoutJob::single_section(
        name.clone(),
        TextFormat::simple(FontId::proportional(NAME_SIZE), palette.text),
    );
    job.wrap = TextWrapping::truncate_at_width(TILE - 2.0 * NAME_LEFT);
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    painter.galley(
        rect.left_top() + vec2(NAME_LEFT, NAME_TOP),
        galley,
        palette.text,
    );
    let response = if elided {
        response.on_hover_text(name)
    } else {
        response
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
    use super::{above, below, columns, scrolled_to_show};

    /// A row holds as many 220-point tiles, 1 point apart, as fit between
    /// 4 points of room at each side, and at least one.
    #[test]
    fn a_row_holds_as_many_tiles_as_fit() {
        assert_eq!(columns(8.0 + 220.0), 1);
        assert_eq!(columns(8.0 + 441.0 - 0.5), 1);
        assert_eq!(columns(8.0 + 441.0), 2);
        assert_eq!(columns(8.0 + 3.0 * 221.0 - 1.0), 3);
        assert_eq!(columns(100.0), 1);
        assert_eq!(columns(-5.0), 1);
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

    /// A row is scrolled into view just far enough: down until its tiles
    /// end 4 points above the view's bottom, up until they start 4 points
    /// below its top, and not at all when they are wholly in view. Rows
    /// are 221 points apart, from 4 points below the grid's top.
    #[test]
    fn a_row_is_scrolled_just_into_view() {
        // In a view 500 points high, at the top: rows 0 and 1 are
        // wholly in view, row 2 (446 to 666) is cut by the bottom edge.
        assert_eq!(scrolled_to_show(0, 0.0, 500.0), None);
        assert_eq!(scrolled_to_show(1, 0.0, 500.0), None);
        assert_eq!(scrolled_to_show(2, 0.0, 500.0), Some(666.0 + 4.0 - 500.0));
        assert_eq!(scrolled_to_show(9, 0.0, 500.0), Some(2213.0 + 4.0 - 500.0));
        // Scrolled 300 points down: row 1 (225 to 445) is cut by the top
        // edge, and row 0 is above it.
        assert_eq!(scrolled_to_show(1, 300.0, 500.0), Some(221.0));
        assert_eq!(scrolled_to_show(0, 300.0, 500.0), Some(0.0));
        assert_eq!(scrolled_to_show(2, 300.0, 500.0), None);
        // A view shorter than a tile shows its top.
        assert_eq!(scrolled_to_show(3, 0.0, 100.0), Some(663.0));
        assert_eq!(scrolled_to_show(0, 0.0, 100.0), Some(0.0));
    }
}
