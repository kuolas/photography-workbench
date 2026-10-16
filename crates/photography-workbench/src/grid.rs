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

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{FontId, Margin, Rect, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, vec2};

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

/// The grid of `photos`, filling `ui`, with the photo at `current`
/// outlined; it scrolls when the rows do not fit. As many tiles fit in a
/// row as the width allows, and at least one; the room left over stays
/// at the right. The click on a tile, if any.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    photos: &[Photo],
    current: Option<usize>,
) -> Option<Click> {
    let columns = columns(ui.available_width());
    let rows = photos.len().div_ceil(columns);
    let mut click = None;
    ui.scope(|ui| {
        // Rows 1 point apart: egui adds the gap between items to each
        // row's height.
        ui.spacing_mut().item_spacing.y = GAP;
        egui::ScrollArea::vertical()
            .id_salt("grid")
            .auto_shrink([false, false])
            .content_margin(Margin::same(MARGIN))
            .show_rows(ui, TILE, rows, |ui, range| {
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
    click
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
    use super::columns;

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
}
