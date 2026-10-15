// SPDX-License-Identifier: GPL-3.0-or-later

//! The toolbar: the fixed row of controls under the window's centre, for
//! what the centre shows and how. Like an action bar, it cannot be resized
//! or hidden.
//!
//! It is filled like the docks and marked off from the centre by a line.
//! Its controls start from both its ends, and its buttons look like an
//! action bar's, smaller: the one that is on has a rounded square behind
//! its icon.

use egui::{Align, Frame, Layout, Margin, UiBuilder, vec2};

use crate::action_bar;
use crate::keys::Chord;
use crate::theme::Palette;

/// The height of the toolbar.
pub const TOOLBAR_HEIGHT: f32 = 48.0;

/// The room between the toolbar's ends and its controls.
const END_MARGIN: i8 = 12;

/// The side of a toolbar's button, as wide as the square behind an action
/// bar's icon.
const BUTTON_SIZE: f32 = action_bar::PLATE_SIZE;

/// The gap between two buttons of the group at the toolbar's start.
const START_GAP: f32 = 2.0;

/// The gap between two controls of the group at the toolbar's end.
const END_GAP: f32 = 6.0;

/// The size of a button's icon.
const ICON_SIZE: f32 = 20.0;

/// Show the toolbar along the bottom of `ui`, fixed at [`TOOLBAR_HEIGHT`],
/// as the panel `id`: `start` adds the controls that run from its left
/// end, and `end` the ones that run from its right end, leftwards.
pub fn show(
    ui: &mut egui::Ui,
    id: &str,
    palette: &Palette,
    start: impl FnOnce(&mut egui::Ui),
    end: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let frame = Frame::NONE.fill(palette.panel).inner_margin(Margin {
        left: END_MARGIN,
        right: END_MARGIN,
        ..Margin::ZERO
    });
    // egui's own line would take a point of the bar's height, off the
    // whole points the centre above it ends on, so the bar draws its own.
    egui::Panel::bottom(egui::Id::new(id))
        .resizable(false)
        .show_separator_line(false)
        .exact_size(TOOLBAR_HEIGHT)
        .frame(frame)
        .show(ui, |ui| {
            let inner = ui.max_rect();
            let bar = inner.expand2(vec2(f32::from(END_MARGIN), 0.0));
            let line = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().hline(bar.x_range(), bar.top() + 0.5, line);
            let at_start = UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::left_to_right(Align::Center));
            ui.scope_builder(at_start, |ui| {
                ui.spacing_mut().item_spacing.x = START_GAP;
                start(ui);
            });
            let at_end = UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::right_to_left(Align::Center));
            ui.scope_builder(at_end, |ui| {
                ui.spacing_mut().item_spacing.x = END_GAP;
                end(ui);
            });
        })
        .response
}

/// A button for the toolbar, 32 points square, showing only the Lucide
/// icon `icon.0` (or the character `icon.1` if the name does not
/// resolve). While `selected` it has a rounded square behind its icon, as
/// an action bar's button has. `label` is the name screen readers and the
/// tests know it by, and its tooltip, with the `chord` that does the same,
/// if any ([`crate::keys::tooltip`]).
pub fn button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: (&str, char),
    label: &str,
    selected: bool,
    chord: Option<Chord>,
) -> egui::Response {
    let size = (egui::Vec2::splat(BUTTON_SIZE), ICON_SIZE);
    action_bar::icon_button(ui, palette, size, (icon, label), selected, chord)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use egui::accesskit::Toggled;
    use egui::{Rect, Shape};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use super::*;
    use crate::icons;
    use crate::theme::Theme;

    /// A toolbar with "Grid" and "Viewer" at its start, "Grid" selected
    /// when `selected` and with G as its chord, and "Zoom" at its end. The
    /// bar's rectangle goes into `bar`; the first frame installs the icon
    /// font, which egui uses from the next frame on, and draws nothing.
    fn toolbar(selected: bool, bar: Rc<Cell<Rect>>) -> Harness<'static> {
        let palette = Theme::Paper.palette();
        let mut fonts_installed = false;
        let mut harness = Harness::builder()
            .with_size(vec2(400.0, 300.0))
            .build_ui(move |ui| {
                if !fonts_installed {
                    icons::install(ui.ctx());
                    fonts_installed = true;
                    return;
                }
                let start = |ui: &mut egui::Ui| {
                    let g = Chord::new(egui::Modifiers::NONE, egui::Key::G);
                    button(
                        ui,
                        &palette,
                        ("layout-grid", '▦'),
                        "Grid",
                        selected,
                        Some(g),
                    );
                    button(ui, &palette, ("image", '▣'), "Viewer", false, None);
                };
                let end = |ui: &mut egui::Ui| {
                    button(ui, &palette, ("zoom-in", '+'), "Zoom", false, None);
                };
                let shown = show(ui, "toolbar", &palette, start, end);
                bar.set(shown.rect);
            });
        harness.run();
        harness
    }

    /// The bar is 48 points high along the bottom, marked off by a line
    /// along its top; its buttons are 32 points square, centred on it, 12
    /// points in from its ends and 2 apart at its start.
    #[test]
    fn the_bar_holds_its_controls_at_both_ends() {
        let rect = Rc::new(Cell::new(Rect::NOTHING));
        let harness = toolbar(true, Rc::clone(&rect));
        let bar = rect.get();
        assert_eq!(bar.height(), TOOLBAR_HEIGHT, "{bar:?}");
        let margin = f32::from(END_MARGIN);
        let grid = harness.get_by_label("Grid").rect();
        assert_eq!(grid.size(), egui::Vec2::splat(BUTTON_SIZE));
        assert_eq!(grid.left(), bar.left() + margin, "{grid:?}");
        assert_eq!(grid.center().y, bar.center().y, "{grid:?}");
        let viewer = harness.get_by_label("Viewer").rect();
        assert_eq!(viewer.left(), grid.right() + START_GAP, "{viewer:?}");
        let zoom = harness.get_by_label("Zoom").rect();
        assert_eq!(zoom.right(), bar.right() - margin, "{zoom:?}");
        let top_line = harness.output().shapes.iter().any(|clipped| {
            matches!(&clipped.shape, Shape::LineSegment { points, .. }
                if points[0].y == bar.top() + 0.5 && points[1].y == points[0].y)
        });
        assert!(top_line, "no line along the bar's top");
    }

    #[test]
    fn a_button_s_tooltip_names_its_chord() {
        let mut harness = toolbar(false, Rc::new(Cell::new(Rect::NOTHING)));
        harness.get_by_label("Grid").hover();
        // egui shows a tooltip once the pointer has rested a moment.
        let shown = (0..20).any(|_| {
            harness.step();
            harness.query_by_label("Grid (G)").is_some()
        });
        assert!(shown, "no tooltip naming the chord");
    }

    #[test]
    fn a_button_is_named_by_its_label_and_says_whether_it_is_on() {
        for selected in [true, false] {
            let harness = toolbar(selected, Rc::new(Cell::new(Rect::NOTHING)));
            let node = harness.get_by_label("Grid").accesskit_node();
            assert_eq!(node.toggled(), Some(Toggled::from(selected)));
        }
    }
}
