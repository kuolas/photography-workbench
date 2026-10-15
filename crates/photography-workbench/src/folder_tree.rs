// SPDX-License-Identifier: GPL-3.0-or-later

//! The folder tree in Library's left dock: a row for each root and each
//! folder shown below one, indented by its depth, with the arrow that
//! expands or collapses it, an icon saying what it is, and its name.
//!
//! A click on the arrow expands or collapses the folder; a click anywhere
//! else on the row selects it. Only the rows in view are laid out, so a
//! folder with thousands of subfolders scrolls as smoothly as one with
//! ten.

use std::io;
use std::path::{Path, PathBuf};

use catalog::folders::{Place, Row, Subfolders};
use desktop_ui::icons;
use desktop_ui::theme::Palette;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align2, FontId, Rect, Sense, WidgetInfo, WidgetType, pos2, vec2};

use crate::browse::Browser;

/// The height of a row.
const ROW_HEIGHT: f32 = 29.0;

/// How far a row's contents move right for each level of depth.
const INDENT: f32 = 16.0;

/// The room above the first row.
const TOP_SPACE: f32 = 8.0;

/// How far the rounded rectangle behind a selected row, or one under the
/// pointer, stands in from the row's sides.
const INSET: f32 = 8.0;

/// The radius of that rectangle's corners.
const CORNER_RADIUS: f32 = 4.0;

/// The side of the arrow's and of the icon's square.
const ICON_SIZE: f32 = 16.0;

/// Where the arrow's square is centred, from the row's left edge before
/// indenting.
const ARROW_CENTRE: f32 = 10.0;

/// Where the icon's square starts, from the row's left edge before
/// indenting.
const ICON_LEFT: f32 = 18.0;

/// Where the name starts, from the row's left edge before indenting.
const NAME_LEFT: f32 = 42.0;

/// The size of a folder's name.
const NAME_SIZE: f32 = 13.5;

/// The Lucide icons of the arrow, for a folder collapsed and expanded,
/// and the text shown in place of each if its name does not resolve.
const ARROWS: [(&str, char); 2] = [("chevron-right", '▸'), ("chevron-down", '▾')];

/// The Lucide icons of the home folder, of a volume and of any other
/// folder, and the text shown in place of each if its name does not
/// resolve.
const ICONS: [(&str, char); 3] = [("house", '⌂'), ("hard-drive", '▭'), ("folder", '🗀')];

/// What a click on a row asked for, done once every row is drawn, as the
/// rows borrow the tree.
enum Click {
    /// Expand or collapse the folder.
    Toggle(PathBuf),
    /// Select the folder.
    Select(PathBuf),
}

/// The folder tree, filling `ui`, with 8 points of room above its first
/// row; it scrolls when its rows do not fit. Expands, collapses and
/// selects folders in `browser` as they are clicked.
pub fn show(ui: &mut egui::Ui, palette: &Palette, browser: &mut Browser) {
    let rows = browser.tree().rows();
    let selected = browser.tree().selected();
    let mut click = None;
    ui.add_space(TOP_SPACE);
    ui.scope(|ui| {
        // Rows abut: egui adds the gap between items to each row's height.
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .id_salt("folder_tree")
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                for row in &rows[range] {
                    if let Some(clicked) = folder_row(ui, palette, row, selected) {
                        click = Some(clicked);
                    }
                }
            });
    });
    drop(rows);
    match click {
        Some(Click::Toggle(path)) => browser.toggle(&path),
        Some(Click::Select(path)) => browser.select(&path),
        None => {}
    }
}

/// A row of the tree, as wide as `ui`, `selected` the selected folder's
/// path. What a click on it asked for, if anything.
///
/// Its ids come from its folder's path and depth, not from where it is,
/// so a row keeps its state when rows above it come or go. The depth
/// tells apart a folder shown twice, at two depths: on Windows the home
/// folder is a root of its own and also a folder two levels below `C:`.
fn folder_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    row: &Row<'_>,
    selected: Option<&Path>,
) -> Option<Click> {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), ROW_HEIGHT));
    let is_selected = selected == Some(row.path);
    let indent = INDENT * row.depth as f32;
    let name = row.name.as_ref();

    let response = ui.interact(
        rect,
        egui::Id::new(("folder_row", row.path, row.depth)),
        Sense::click(),
    );
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, is_selected, name));
    // A folder that may have subfolders has an arrow, after the row so it
    // takes the clicks on it.
    let expandable = matches!(row.subfolders, Subfolders::Unknown | Subfolders::Present);
    let arrow = expandable.then(|| {
        let centre = pos2(rect.left() + ARROW_CENTRE + indent, rect.center().y);
        let square = Rect::from_center_size(centre, egui::Vec2::splat(ICON_SIZE));
        let id = egui::Id::new(("folder_arrow", row.path, row.depth));
        let arrow = ui.interact(square, id, Sense::click());
        let label = if row.expanded {
            format!("Collapse {name}")
        } else {
            format!("Expand {name}")
        };
        arrow.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
        arrow
    });

    let painter = ui.painter();
    let inner = rect.shrink2(vec2(INSET, 0.0));
    if is_selected {
        painter.rect_filled(inner, CORNER_RADIUS, palette.viewer_backdrop);
    } else if response.contains_pointer() {
        painter.rect_filled(inner, CORNER_RADIUS, palette.hover_overlay);
    }

    if let Some(arrow) = &arrow {
        let [collapsed, expanded] = ARROWS;
        let (name, fallback) = if row.expanded { expanded } else { collapsed };
        let colour = if arrow.hovered() {
            palette.text
        } else {
            palette.icon_inactive
        };
        let (glyph, font) = icons::glyph(name, ICON_SIZE, fallback);
        let centre = arrow.rect.center();
        painter.text(centre, Align2::CENTER_CENTER, glyph, font, colour);
    }

    let [home, volume, folder] = ICONS;
    let (icon, fallback) = match row.place {
        Some(Place::Home) => home,
        Some(Place::Volume) => volume,
        None => folder,
    };
    let colour = if is_selected {
        palette.text
    } else {
        palette.icon_inactive
    };
    let (glyph, font) = icons::glyph(icon, ICON_SIZE, fallback);
    let centre = pos2(
        rect.left() + ICON_LEFT + indent + ICON_SIZE / 2.0,
        rect.center().y,
    );
    painter.text(centre, Align2::CENTER_CENTER, glyph, font, colour);

    // The name, cut short with "…" where it would run past the rounded
    // rectangle's right edge.
    let left = rect.left() + NAME_LEFT + indent;
    let unreadable = match row.subfolders {
        Subfolders::Unreadable(kind) => Some(kind),
        _ => None,
    };
    let colour = if unreadable.is_some() {
        ui.visuals().weak_text_color()
    } else {
        palette.text
    };
    let mut job = LayoutJob::single_section(
        name.to_owned(),
        TextFormat::simple(FontId::proportional(NAME_SIZE), colour),
    );
    job.wrap = TextWrapping::truncate_at_width((inner.right() - left).max(0.0));
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    let top = rect.center().y - galley.size().y / 2.0;
    painter.galley(pos2(left, top), galley, colour);

    let tip = match (elided, unreadable) {
        (false, None) => None,
        (true, None) => Some(name.to_owned()),
        (false, Some(kind)) => Some(format!("Can't be read. {}.", why_unreadable(kind))),
        (true, Some(kind)) => Some(format!("{name}\nCan't be read. {}.", why_unreadable(kind))),
    };
    let response = match tip {
        Some(tip) => response.on_hover_text(tip),
        None => response,
    };

    if arrow.is_some_and(|arrow| arrow.clicked()) {
        Some(Click::Toggle(row.path.to_path_buf()))
    } else if response.clicked() {
        Some(Click::Select(row.path.to_path_buf()))
    } else {
        None
    }
}

/// Why a folder could not be read, in words for the user, with no full
/// stop. The system words the two errors a photographer meets most, a
/// card taken out and a folder another user owns, for programmers
/// ("entity not found"), so those two have words of their own; any other
/// is in the system's words.
pub fn why_unreadable(kind: io::ErrorKind) -> String {
    match kind {
        io::ErrorKind::NotFound => "It is no longer there".to_owned(),
        io::ErrorKind::PermissionDenied => "You do not have permission to open it".to_owned(),
        other => {
            let text = other.to_string();
            let mut chars = text.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{ARROWS, ICONS, why_unreadable};

    #[test]
    fn why_a_folder_cannot_be_read_is_said_for_the_user() {
        let why = |kind| why_unreadable(kind);
        assert_eq!(why(io::ErrorKind::NotFound), "It is no longer there");
        assert_eq!(
            why(io::ErrorKind::PermissionDenied),
            "You do not have permission to open it"
        );
        assert_eq!(why(io::ErrorKind::TimedOut), "Timed out");
    }

    #[test]
    fn every_icon_resolves() {
        for (name, _) in ARROWS.into_iter().chain(ICONS) {
            assert!(desktop_ui::icons::resolves(name), "{name}");
        }
    }
}
