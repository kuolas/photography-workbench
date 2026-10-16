// SPDX-License-Identifier: GPL-3.0-or-later

//! The filmstrip in the viewer's bottom dock: a cell for each of a
//! folder's photos, side by side in one row, the current one outlined.
//!
//! A cell holds the photo's name and format over its thumbnail, which
//! sits straight on the strip's backdrop with nothing behind it, or, for
//! a photo that cannot be shown, an icon there, and why in its tooltip.
//! The current photo's thumbnail is outlined, a little clear of its
//! edge, or, until there is one, the room it goes in. A click on a cell makes its photo the current
//! one.
//!
//! The strip draws a list of photos and the index of the current one,
//! whatever they were read from, and the thumbnails the app holds
//! ([`crate::thumbnails`]). Only the cells near the view are laid out,
//! so a folder of thousands of photos scrolls as smoothly as one of ten;
//! their thumbnails are decoded the current photo's first, then those in
//! view, then those past its ends, each the nearest the current photo
//! first ([`want_order`]). A mouse's wheel, which turns up and down,
//! scrolls it sideways.
//!
//! The strip brings the current photo into view when the current photo
//! changes, when the list does, or when the strip comes back on screen,
//! and then only if the photo is not in view already. Nothing else moves
//! it: not time passing, and not the strip being scrolled, so a
//! photographer looking along the folder is not pulled back
//! ([`crate::follow`]).

use std::cmp::Reverse;
use std::ops::Range;
use std::path::Path;

use catalog::photos::{Kind, Photo};
use desktop_ui::fonts;
use desktop_ui::theme::Palette;
use egui::style::ScrollAnimation;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Align2, FontFamily, FontId, Margin, Rect, Sense, Stroke, StrokeKind, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::follow;
use crate::thumbnails::{self, AHEAD, CURRENT, ON_SCREEN, Thumbnail, Thumbnails};

/// The width of a cell. Cells run the strip's height and touch.
const CELL: f32 = 120.0;

/// The room between a cell's edges and its tile, the room its photo's
/// thumbnail is fitted in, which has no fill of its own: the strip's
/// backdrop shows around the thumbnail. The photo's name is in the band
/// above the tile.
const TILE_INSET: Margin = Margin {
    left: 10,
    right: 10,
    top: 22,
    bottom: 8,
};

/// How far a photo's name stands in from its cell's left edge, and its
/// format from the right edge.
const LABEL_SIDE: f32 = 8.0;

/// Where the middle of a photo's name and format is, from its cell's top
/// edge: in the band above the tile.
const LABEL_MIDDLE: f32 = 10.0;

/// The size of a photo's name.
const NAME_SIZE: f32 = 10.0;

/// The size of a photo's format, in the semibold weight.
const FORMAT_SIZE: f32 = 8.5;

/// The room kept between the end of a name cut short and the format.
const LABEL_GAP: f32 = 4.0;

/// The width of the outline around the current photo: around its
/// thumbnail, or around its tile while there is none to draw,
/// [`thumbnails::MARK_GAP`] clear of either.
const OUTLINE: f32 = 1.5;

/// How many cells are laid out past each end of the view, so the strip
/// has them ready as it scrolls.
const NEAR: usize = 4;

/// The size of the icon a tile shows when its photo cannot be shown.
const FAILED_ICON_SIZE: f32 = 24.0;

/// The filmstrip of `photos`, the photos of `folder`, filling `ui`, with
/// the photo at `current` outlined and the thumbnails in `thumbnails`;
/// it scrolls sideways when the cells do not fit. `folder` tells one
/// list from another, so the strip brings the current photo into view
/// when the list changes. Nothing is drawn when there are no photos. The
/// index of the photo clicked on, if any.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    thumbnails: &mut Thumbnails,
    photos: &[Photo],
    current: Option<usize>,
    folder: &Path,
) -> Option<usize> {
    if photos.is_empty() {
        return None;
    }
    let id = ui.make_persistent_id("filmstrip");
    let follow = follow::follows(ui.ctx(), id, egui::Id::new(folder), current);
    let mut click = None;
    ui.scope(|ui| {
        // A vertical wheel, the one most mice have, scrolls the strip
        // sideways, without Shift held.
        ui.style_mut().always_scroll_the_only_direction = true;
        egui::ScrollArea::horizontal()
            .id_salt(id)
            .auto_shrink([false, false])
            .content_margin(Margin::ZERO)
            .show_viewport(ui, |ui, viewport| {
                let height = ui.available_height();
                let (_, strip) = ui.allocate_space(vec2(photos.len() as f32 * CELL, height));
                // `viewport` is measured from the strip's left edge.
                let cell = |index: usize| {
                    let left_top = strip.left_top() + vec2(index as f32 * CELL, 0.0);
                    Rect::from_min_size(left_top, vec2(CELL, height))
                };

                if follow && let Some(index) = current {
                    let left = index as f32 * CELL;
                    let in_view = viewport.left() <= left && left + CELL <= viewport.right();
                    if !in_view {
                        let centre = Some(Align::Center);
                        ui.scroll_to_rect_animation(cell(index), centre, ScrollAnimation::none());
                    }
                }

                // The cells in view, and NEAR more at each end.
                let in_view = in_view(viewport, photos.len());
                let first = in_view.start.saturating_sub(NEAR);
                let end = (in_view.end + NEAR).min(photos.len());
                // Every cell's thumbnail fits in the same tile.
                let tile = (cell(0) - TILE_INSET).size();
                let longest = thumbnails::longest(tile.max_elem(), ui.pixels_per_point());
                for (index, priority) in want_order(first..end, in_view, current) {
                    thumbnails.want(photos[index].path(), longest, priority);
                }
                let mut outline = None;
                for (index, photo) in photos.iter().enumerate().take(end).skip(first) {
                    let rect = cell(index);
                    let is_current = current == Some(index);
                    let thumbnail = thumbnails.thumbnail(photo.path(), longest);
                    if photo_cell(ui, photo, thumbnail, rect, is_current) {
                        click = Some(index);
                    }
                    if is_current {
                        let place = thumbnails::placed(thumbnail, rect - TILE_INSET);
                        outline = Some(place.expand(thumbnails::MARK_GAP));
                    }
                }
                // Last, so the cells next to it do not cover it.
                if let Some(around) = outline {
                    let stroke = Stroke::new(OUTLINE, palette.accent);
                    ui.painter()
                        .rect_stroke(around, 0.0, stroke, StrokeKind::Outside);
                }
            });
    });
    // The photo clicked on is where the pointer is: the strip does not
    // move under it, even when the cell is only partly in view.
    if let Some(index) = click {
        follow::shown(ui.ctx(), id, egui::Id::new(folder), Some(index));
    }
    click
}

/// The cells of a strip of `len` photos in its `viewport`, measured from
/// the strip's left edge, however little of each; not one that only
/// touches the viewport's edge.
fn in_view(viewport: Rect, len: usize) -> Range<usize> {
    let start = (viewport.left() / CELL).floor().max(0.0) as usize;
    let end = ((viewport.right() / CELL).ceil().max(0.0) as usize).min(len);
    start.min(end)..end
}

/// The photos whose thumbnails the strip wants while it lays out the
/// cells in `laid_out`, those in `in_view` in its view, the best first,
/// each with a priority of its own from the highest down
/// ([`thumbnails::ranked`]).
///
/// A photographer in the viewer steps from the current photo to the
/// next or the one before, so the cells nearest the current photo are
/// the ones looked at next: the current photo comes first ([`CURRENT`]),
/// then the cells in view, then those past the view's ends, each the
/// nearest the current photo first and, of two as near, the one after
/// it, as a photographer mostly moves on. With no current photo, the
/// first in view stands in for it.
fn want_order(
    laid_out: Range<usize>,
    in_view: Range<usize>,
    current: Option<usize>,
) -> Vec<(usize, u32)> {
    let from = current.unwrap_or(in_view.start);
    let tier = |index| {
        if current == Some(index) {
            CURRENT
        } else if in_view.contains(&index) {
            ON_SCREEN
        } else {
            AHEAD
        }
    };
    let mut order: Vec<usize> = laid_out.collect();
    order.sort_by_key(|&index| (Reverse(tier(index)), index.abs_diff(from), index < from));
    thumbnails::ranked(order.into_iter().map(|index| (index, tier(index))))
}

/// The cell of `photo` at `rect`, its tile showing `thumbnail` on the
/// strip's backdrop, outlined elsewhere when `is_current`. Whether it was
/// clicked.
///
/// Its id comes from its photo's path, not from where it is, so a cell
/// keeps its state when photos before it come or go.
fn photo_cell(
    ui: &mut egui::Ui,
    photo: &Photo,
    thumbnail: Thumbnail,
    rect: Rect,
    is_current: bool,
) -> bool {
    let name = photo.name().into_owned();
    let id = egui::Id::new(("filmstrip_cell", photo.path()));
    let response = ui.interact(rect, id, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, is_current, &name));

    let painter = ui.painter();
    let failure = thumbnails::paint(ui, thumbnail, rect - TILE_INSET, FAILED_ICON_SIZE);
    // Above the tile, dimmer than the photo they name: the photo's format
    // at the right, and its name without the extension at the left, cut
    // short with "…" where it would run into the format, so the end of a
    // long name gives way and what kind of file it is stays in sight.
    let colour = ui.visuals().weak_text_color();
    let middle = rect.top() + LABEL_MIDDLE;
    let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
    let format = painter.text(
        pos2(rect.right() - LABEL_SIDE, middle),
        Align2::RIGHT_CENTER,
        format_of(photo.kind()),
        FontId::new(FORMAT_SIZE, semibold),
        colour,
    );
    let left = rect.left() + LABEL_SIDE;
    let mut job = LayoutJob::single_section(
        stem(&name).to_owned(),
        TextFormat::simple(FontId::proportional(NAME_SIZE), colour),
    );
    job.wrap = TextWrapping::truncate_at_width((format.left() - LABEL_GAP - left).max(0.0));
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    painter.galley(pos2(left, middle - galley.size().y / 2.0), galley, colour);
    let response = match thumbnails::tooltip(name, elided, failure) {
        Some(tooltip) => response.on_hover_text(tooltip),
        None => response,
    };
    response.clicked()
}

/// The photo's format as a photographer names it, whatever the file's
/// extension: "JPEG", or a RAW format's main extension in capitals,
/// "NEF" for an `.nrw` file as for a `.nef` one.
fn format_of(kind: Kind) -> String {
    match kind {
        Kind::Jpeg => "JPEG".to_owned(),
        Kind::Raw(format) => format
            .extensions()
            .first()
            .map_or_else(|| format.name().to_owned(), |main| main.to_uppercase()),
    }
}

/// `name` without its extension: what is before its last dot, if there
/// is anything before it.
fn stem(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use catalog::photos::Kind;

    use egui::{Rect, pos2};

    use super::{format_of, in_view, stem, want_order};
    use crate::thumbnails::{self, AHEAD, CURRENT, ON_SCREEN};

    /// A cell is in view however little of it is, and not when it only
    /// touches the view's edge, and as far as there are photos. Cells are
    /// 120 points wide.
    #[test]
    fn a_cell_is_in_view_however_little_of_it_is() {
        let viewport = Rect::from_x_y_ranges(250.0..=490.0, 0.0..=142.0);
        assert_eq!(in_view(viewport, 6), 2..5);
        let edges = Rect::from_min_max(pos2(240.0, 0.0), pos2(480.0, 142.0));
        assert_eq!(in_view(edges, 6), 2..4);
        assert_eq!(in_view(viewport, 3), 2..3);
        assert_eq!(in_view(viewport, 1), 1..1);
    }

    /// The cells are wanted the best first, each at a priority of its own
    /// from the highest down, so a worker that takes one while the rest
    /// are still being wanted takes the best: the current photo, then the
    /// cells in view, then those past the view's ends, each the nearest
    /// the current photo first, and of two as near the one after it.
    #[test]
    fn the_cells_are_wanted_the_best_first_at_falling_priorities() {
        let order = want_order(0..8, 2..6, Some(3));
        let indices: Vec<usize> = order.iter().map(|&(index, _)| index).collect();
        assert_eq!(indices, [3, 4, 2, 5, 1, 6, 0, 7]);
        let tiers = [
            CURRENT, ON_SCREEN, ON_SCREEN, ON_SCREEN, AHEAD, AHEAD, AHEAD, AHEAD,
        ];
        assert_eq!(order, thumbnails::ranked(indices.into_iter().zip(tiers)));
        assert!(
            order.windows(2).all(|pair| pair[0].1 > pair[1].1),
            "{order:?}"
        );
        // The current photo at the view's left edge: the cells in view
        // before the nearer ones past the edge.
        let indices: Vec<usize> = want_order(0..10, 4..8, Some(4))
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        assert_eq!(indices, [4, 5, 6, 7, 3, 2, 1, 8, 0, 9]);
        // With no current photo, from the first in view.
        let indices: Vec<usize> = want_order(0..4, 1..3, None)
            .into_iter()
            .map(|(index, _)| index)
            .collect();
        assert_eq!(indices, [1, 2, 0, 3]);
    }

    /// A photo's format is named the way photographers name it, whatever
    /// its file's extension.
    #[test]
    fn a_photo_s_format_is_named_by_its_kind() {
        let format = |name: &str| format_of(Kind::of(Path::new(name)).unwrap());
        assert_eq!(format("DSC_0001.NEF"), "NEF");
        assert_eq!(format("DSCN0001.nrw"), "NEF");
        assert_eq!(format("DSC00001.ARW"), "ARW");
        assert_eq!(format("DSC00001.sr2"), "ARW");
        assert_eq!(format("IMG_0001.jpg"), "JPEG");
        assert_eq!(format("IMG_0001.JPEG"), "JPEG");
    }

    /// A name loses its extension, but a name that is all extension, or
    /// has none, is kept whole.
    #[test]
    fn a_name_s_stem_is_what_comes_before_its_last_dot() {
        assert_eq!(
            stem("Z9-20261004-133504-Z9K_2517.JPG"),
            "Z9-20261004-133504-Z9K_2517"
        );
        assert_eq!(stem("trip.2026.NEF"), "trip.2026");
        assert_eq!(stem(".NEF"), ".NEF");
        assert_eq!(stem("README"), "README");
    }
}
