// SPDX-License-Identifier: GPL-3.0-or-later

//! The filmstrip in the viewer's bottom dock: a cell for each of a
//! folder's photos, side by side in one row, the current one outlined.
//!
//! A cell holds the photo's name and format over an empty tile, where
//! the photo's thumbnail goes once the app decodes previews. A click on a
//! cell makes its photo the current one.
//!
//! The strip draws a list of photos and the index of the current one,
//! whatever they were read from. Only the cells near the view are laid
//! out, so a folder of thousands of photos scrolls as smoothly as one of
//! ten. A mouse's wheel, which turns up and down, scrolls it sideways.
//!
//! The strip brings the current photo into view when the current photo
//! changes, when the list does, or when the strip comes back on screen,
//! and then only if the photo is not in view already. Nothing else moves
//! it: not time passing, and not the strip being scrolled, so a
//! photographer looking along the folder is not pulled back
//! ([`crate::follow`]).

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

/// The width of a cell. Cells run the strip's height and touch.
const CELL: f32 = 120.0;

/// The room between a cell's edges and its tile, where its photo's
/// thumbnail goes; the photo's name is in the band above the tile.
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

/// The width of the outline around the current photo's tile.
const OUTLINE: f32 = 1.5;

/// How many cells are laid out past each end of the view, so the strip
/// has them ready as it scrolls.
const NEAR: usize = 4;

/// The filmstrip of `photos`, the photos of `folder`, filling `ui`, with
/// the photo at `current` outlined; it scrolls sideways when the cells do
/// not fit. `folder` tells one list from another, so the strip brings
/// the current photo into view when the list changes. Nothing is drawn
/// when there are no photos. The index of the photo clicked on, if any.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
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
                let first =
                    ((viewport.left() / CELL).floor().max(0.0) as usize).saturating_sub(NEAR);
                let end =
                    ((viewport.right() / CELL).ceil().max(0.0) as usize + NEAR).min(photos.len());
                let mut outline = None;
                for (index, photo) in photos.iter().enumerate().take(end).skip(first) {
                    let rect = cell(index);
                    let is_current = current == Some(index);
                    if photo_cell(ui, palette, photo, rect, is_current) {
                        click = Some(index);
                    }
                    if is_current {
                        outline = Some(rect - TILE_INSET);
                    }
                }
                // Last, so the cells next to it do not cover it.
                if let Some(tile) = outline {
                    let stroke = Stroke::new(OUTLINE, palette.accent);
                    ui.painter()
                        .rect_stroke(tile, 0.0, stroke, StrokeKind::Outside);
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

/// The cell of `photo` at `rect`, its tile outlined elsewhere when
/// `is_current`. Whether it was clicked.
///
/// Its id comes from its photo's path, not from where it is, so a cell
/// keeps its state when photos before it come or go.
fn photo_cell(
    ui: &mut egui::Ui,
    palette: &Palette,
    photo: &Photo,
    rect: Rect,
    is_current: bool,
) -> bool {
    let name = photo.name().into_owned();
    let id = egui::Id::new(("filmstrip_cell", photo.path()));
    let response = ui.interact(rect, id, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, is_current, &name));

    let tile = rect - TILE_INSET;
    let painter = ui.painter();
    painter.rect_filled(tile, 0.0, palette.chip);
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
    let response = if elided {
        response.on_hover_text(name)
    } else {
        response
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

    use super::{format_of, stem};

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
