// SPDX-License-Identifier: GPL-3.0-or-later

//! The action bars: fixed columns of icon buttons along the window's sides,
//! each beside the dock whose contents it picks. Unlike a dock, an action
//! bar cannot be resized or hidden.
//!
//! A bar is filled like the docks and marked off from them by a line. Its
//! buttons hang from its top, with a group at its foot for what is apart
//! from the rest. A button's icon brightens under the pointer, and the
//! button that is on has a rounded square behind its icon.

use egui::{Align, Align2, Frame, Layout, Margin, Rect, Sense, vec2};

use crate::icons;
use crate::theme::Palette;

/// The width of an action bar.
pub const ACTION_BAR_WIDTH: f32 = 48.0;

/// The height of an action bar's button, which is as wide as the bar.
const BUTTON_HEIGHT: f32 = 40.0;

/// The room above the first button and below the last.
const END_MARGIN: i8 = 8;

/// The gap between two buttons.
const BUTTON_GAP: f32 = 6.0;

/// The side of the rounded square behind a button's icon.
const PLATE_SIZE: f32 = 32.0;

/// The radius of that square's corners, as a control's.
const PLATE_CORNER_RADIUS: f32 = 4.0;

/// The size of a button's icon.
const ICON_SIZE: f32 = 22.0;

/// The edge of the window an action bar runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The left edge.
    Left,
    /// The right edge.
    Right,
}

/// Show an action bar along `edge`, fixed at [`ACTION_BAR_WIDTH`], as
/// the panel `id`: `top` adds the buttons that hang from its top, and
/// `foot` the ones at its foot, from the bottom up.
pub fn show(
    ui: &mut egui::Ui,
    (id, edge): (&str, Edge),
    palette: &Palette,
    top: impl FnOnce(&mut egui::Ui),
    foot: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let id = egui::Id::new(id);
    let panel = match edge {
        Edge::Left => egui::Panel::left(id),
        Edge::Right => egui::Panel::right(id),
    };
    let frame = Frame::NONE.fill(palette.panel).inner_margin(Margin {
        top: END_MARGIN,
        bottom: END_MARGIN,
        ..Margin::ZERO
    });
    // egui's own line would take a point of the bar's width, off the
    // whole points the docks beside it start on, so the bar draws its own.
    panel
        .resizable(false)
        .show_separator_line(false)
        .exact_size(ACTION_BAR_WIDTH)
        .frame(frame)
        .show(ui, |ui| {
            let bar = ui.max_rect().expand2(vec2(0.0, f32::from(END_MARGIN)));
            let inner = match edge {
                Edge::Left => bar.right() - 0.5,
                Edge::Right => bar.left() + 0.5,
            };
            let line = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().vline(inner, bar.y_range(), line);
            ui.spacing_mut().item_spacing.y = BUTTON_GAP;
            ui.with_layout(Layout::top_down(Align::Center), top);
            ui.with_layout(Layout::bottom_up(Align::Center), foot);
        })
        .response
}

/// A button for an action bar, the bar's width and 40 points high,
/// showing only the Lucide icon `icon.0` (or the character `icon.1` if
/// the name does not resolve). While `selected` it has a rounded square
/// behind its icon in the theme's colour for a button that is on, and
/// under the pointer a fainter one, the theme's wash for a control under
/// the pointer. `label` is its tooltip and the name screen readers and the
/// tests know it by.
pub fn button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: (&str, char),
    label: &str,
    selected: bool,
) -> egui::Response {
    let size = vec2(ACTION_BAR_WIDTH, BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let square = Rect::from_center_size(rect.center(), egui::Vec2::splat(PLATE_SIZE));
    let plate = if selected {
        Some(palette.button_on)
    } else {
        response.hovered().then_some(palette.hover_overlay)
    };
    if let Some(fill) = plate {
        ui.painter().rect_filled(square, PLATE_CORNER_RADIUS, fill);
    }
    let colour = if selected || response.hovered() {
        palette.icon_active
    } else {
        palette.icon_inactive
    };
    let (glyph, font) = icons::glyph(icon.0, ICON_SIZE, icon.1);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, glyph, font, colour);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            response.enabled(),
            selected,
            label,
        )
    });
    response.on_hover_text(label)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use egui::accesskit::Toggled;
    use egui::{Event, Shape};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use super::*;
    use crate::theme::Theme;

    /// A left action bar with "Folders" at its top, selected when
    /// `selected`, and "Info" at its foot, never selected. The bar's
    /// rectangle goes into `bar`; the first frame installs the icon font,
    /// which egui uses from the next frame on, and draws nothing.
    fn bar(selected: bool, bar: Rc<Cell<Rect>>) -> Harness<'static> {
        let palette = Theme::Paper.palette();
        let mut fonts_installed = false;
        let mut harness = Harness::builder()
            .with_size(vec2(300.0, 400.0))
            .build_ui(move |ui| {
                if !fonts_installed {
                    icons::install(ui.ctx());
                    fonts_installed = true;
                    return;
                }
                let top = |ui: &mut egui::Ui| {
                    button(ui, &palette, ("folder", '🗀'), "Folders", selected);
                };
                let foot = |ui: &mut egui::Ui| {
                    button(ui, &palette, ("info", 'ℹ'), "Info", false);
                };
                let shown = show(ui, ("bar", Edge::Left), &palette, top, foot);
                bar.set(shown.rect);
            });
        harness.run();
        harness
    }

    /// The fills of the rectangles painted in the last frame.
    fn plates(harness: &Harness<'_>) -> Vec<(Rect, egui::Color32)> {
        harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(rect) => Some((rect.rect, rect.fill)),
                _ => None,
            })
            .collect()
    }

    /// The bar is 48 points wide, marked off by a line along its inner
    /// edge; its buttons are as wide, 40 points high, and stand 8 points
    /// in from its top and its foot.
    #[test]
    fn the_bar_holds_its_buttons_at_its_top_and_its_foot() {
        let rect = Rc::new(Cell::new(Rect::NOTHING));
        let harness = bar(true, Rc::clone(&rect));
        let bar = rect.get();
        assert_eq!(bar.width(), ACTION_BAR_WIDTH, "{bar:?}");
        let margin = f32::from(END_MARGIN);
        let folders = harness.get_by_label("Folders").rect();
        assert_eq!(folders.size(), vec2(ACTION_BAR_WIDTH, BUTTON_HEIGHT));
        assert_eq!(folders.left_top(), bar.left_top() + vec2(0.0, margin));
        let info = harness.get_by_label("Info").rect();
        assert_eq!(info.left_bottom(), bar.left_bottom() - vec2(0.0, margin));
        let inner_line = harness.output().shapes.iter().any(|clipped| {
            matches!(&clipped.shape, Shape::LineSegment { points, .. }
                if points[0].x == bar.right() - 0.5 && points[1].x == points[0].x)
        });
        assert!(inner_line, "no line along the bar's inner edge");
    }

    /// The button that is on has a rounded square behind its icon, in the
    /// theme's colour for a button that is on; the others have none until
    /// the pointer is on them, and then the wash for a control under the
    /// pointer.
    #[test]
    fn the_button_that_is_on_has_a_square_behind_its_icon() {
        let rect = Rc::new(Cell::new(Rect::NOTHING));
        let mut harness = bar(true, Rc::clone(&rect));
        let palette = Theme::Paper.palette();
        // Taken before the pointer is on Info, whose tooltip has its name too.
        let folders = harness.get_by_label("Folders").rect();
        let info = harness.get_by_label("Info").rect();
        let squares = |harness: &Harness<'_>, button: Rect| {
            plates(harness)
                .into_iter()
                .filter(|(plate, _)| button.contains_rect(*plate) && plate.width() == PLATE_SIZE)
                .map(|(_, fill)| fill)
                .collect::<Vec<_>>()
        };
        assert_eq!(squares(&harness, folders), [palette.button_on]);
        assert_eq!(squares(&harness, info), []);
        harness
            .input_mut()
            .events
            .push(Event::PointerMoved(info.center()));
        harness.run();
        assert_eq!(
            squares(&harness, info),
            [palette.hover_overlay],
            "under the pointer"
        );
    }

    #[test]
    fn a_button_is_named_by_its_label_and_says_whether_it_is_on() {
        for selected in [true, false] {
            let harness = bar(selected, Rc::new(Cell::new(Rect::NOTHING)));
            let node = harness.get_by_label("Folders").accesskit_node();
            assert_eq!(node.toggled(), Some(Toggled::from(selected)));
        }
    }
}
