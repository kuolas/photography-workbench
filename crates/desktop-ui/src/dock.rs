// SPDX-License-Identifier: GPL-3.0-or-later

//! The docks: the panels along the window's sides and bottom that hold a
//! mode's tools around its centre. A dock can be resized by dragging its
//! inner edge, and shown or hidden, sliding in and out as it does.
//!
//! A dock is filled to its edges, and what it holds sets its own room. A
//! dock with nothing to show says so in its middle ([`empty_message`]).

use egui::{Align, FontFamily, FontId, Frame, Label, Layout, RichText};

use crate::fonts;

/// The edge of the window a dock is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The left edge.
    Left,
    /// The bottom edge.
    Bottom,
    /// The right edge.
    Right,
}

/// Show `panel` as a dock, sliding it out of view while `shown` is false
/// and back in when it turns true. Returns `None` once the dock is out of
/// view.
///
/// A dock hidden this way keeps the ids of the widgets after it as they
/// are when it is shown, so they keep their state; leaving the panel out
/// instead would change those ids, and egui would flag a clash.
///
/// A `panel` that can be resized can also be hidden by dragging its edge
/// all the way in, which sets `shown` to false; hidden, it keeps a thin
/// handle along its edge, unseen until the pointer is on it, that drags it
/// back open or reopens it on a double-click. One of a fixed size has
/// neither.
///
/// The dock takes the size it opens at, or was last dragged to, however
/// little it holds.
///
/// The pointer resting on a resizable dock's edge turns into a resize
/// cursor but leaves the edge's line as it is. egui paints that line
/// bright while the pointer is over it, so merely crossing an edge would
/// flash a line the window's full height or width; the line still lights
/// up while the edge is being dragged.
pub fn show<R>(
    panel: egui::Panel,
    ui: &mut egui::Ui,
    shown: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<egui::InnerResponse<R>> {
    let bright = ui.visuals().widgets.hovered.fg_stroke;
    let dim = ui.visuals().widgets.noninteractive.bg_stroke;
    // egui paints the edge with the hovered stroke of the `ui` the dock is
    // shown in, so only that `ui` gets the dim one, and only for the
    // dock's own sake.
    ui.visuals_mut().widgets.hovered.fg_stroke = dim;
    // Filled to its edges: what it holds sets its own room.
    let panel = panel.frame(Frame::NONE.fill(ui.visuals().panel_fill));
    let response = panel.show_collapsible(ui, shown, |ui| {
        ui.visuals_mut().widgets.hovered.fg_stroke = bright;
        // egui sizes a panel to its contents, up to the size it is given,
        // so a dock holding little would shrink to its smallest.
        ui.set_min_size(ui.available_size());
        add_contents(ui)
    });
    ui.visuals_mut().widgets.hovered.fg_stroke = bright;
    response
}

/// What a dock with nothing to show says, in its middle: `title` in the
/// semibold weight over `body`, dimmer.
pub fn empty_message(ui: &mut egui::Ui, title: &str, body: &str) {
    let area = ui.available_rect_before_wrap();
    let font = FontId::new(18.0, FontFamily::Name(fonts::SEMIBOLD.into()));
    let builder = egui::UiBuilder::new()
        .max_rect(area)
        .layout(Layout::top_down(Align::Center));
    ui.scope_builder(builder, |ui| {
        // The title's middle 12 points above the dock's, the body's 14
        // below it.
        ui.add_space((area.height() / 2.0 - 23.0).max(0.0));
        ui.add(Label::new(RichText::new(title).font(font)).selectable(false));
        ui.add_space(3.0);
        ui.add(Label::new(RichText::new(body).weak()).selectable(false));
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use egui::{CursorIcon, Event, Id, Rect, Shape, Stroke, pos2};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// What a test's window saw in its last frame.
    #[derive(Default)]
    struct Seen {
        /// The dock's rectangle, `None` when it was out of view.
        dock: Option<Rect>,
        /// The hovered stroke inside the dock.
        inside: Stroke,
        /// The hovered stroke after the dock.
        after: Stroke,
        /// The id of the button after the dock.
        button: Option<Id>,
    }

    /// A window with a left dock, shown while `shown` holds true, and a
    /// button after it; `seen` records the last frame.
    fn window(shown: Rc<RefCell<bool>>, seen: Rc<RefCell<Seen>>) -> Harness<'static> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui(move |ui| {
                let mut seen = seen.borrow_mut();
                let panel = egui::Panel::left("dock").default_size(100.0);
                let dock = show(panel, ui, &mut shown.borrow_mut(), |ui| {
                    seen.inside = ui.visuals().widgets.hovered.fg_stroke;
                });
                seen.dock = dock.map(|dock| dock.response.rect);
                seen.after = ui.visuals().widgets.hovered.fg_stroke;
                seen.button = Some(ui.button("After").id);
            });
        harness.run();
        harness
    }

    /// The stroke of every line painted in the last frame.
    fn lines(harness: &Harness<'_>) -> Vec<Stroke> {
        fn collect(shape: &Shape, into: &mut Vec<Stroke>) {
            match shape {
                Shape::LineSegment { stroke, .. } => into.push(*stroke),
                Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, into)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        for clipped in &harness.output().shapes {
            collect(&clipped.shape, &mut found);
        }
        found
    }

    #[test]
    fn the_pointer_on_the_edge_shows_a_resize_cursor_and_no_bright_line() {
        let seen = Rc::new(RefCell::new(Seen::default()));
        let mut harness = window(Rc::new(RefCell::new(true)), Rc::clone(&seen));
        let bright = harness.ctx.global_style().visuals.widgets.hovered.fg_stroke;
        let dock = seen.borrow().dock.expect("the dock is shown");
        harness
            .input_mut()
            .events
            .push(Event::PointerMoved(pos2(dock.right(), dock.center().y)));
        harness.run();
        let cursor = harness.output().platform_output.cursor_icon;
        let resize = [
            CursorIcon::ResizeHorizontal,
            CursorIcon::ResizeEast,
            CursorIcon::ResizeWest,
        ];
        assert!(resize.contains(&cursor), "{cursor:?}");
        let lines = lines(&harness);
        assert!(!lines.is_empty() && !lines.contains(&bright), "{lines:?}");
        // The dock's contents and what comes after it keep the bright one.
        let seen = seen.borrow();
        assert_eq!((seen.inside, seen.after), (bright, bright));
    }

    /// A dock holding nothing still opens at the size it is given.
    #[test]
    fn an_empty_dock_opens_at_its_size() {
        let mut bottom = Rect::NOTHING;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui(|ui| {
                let panel = egui::Panel::bottom("dock")
                    .default_size(140.0)
                    .size_range(80.0..=200.0);
                if let Some(dock) = show(panel, ui, &mut true, |_| {}) {
                    bottom = dock.response.rect;
                }
            });
        harness.run();
        drop(harness);
        assert_eq!(bottom.height(), 140.0, "{bottom:?}");
    }

    #[test]
    fn hiding_the_dock_keeps_the_ids_after_it() {
        let shown = Rc::new(RefCell::new(true));
        let seen = Rc::new(RefCell::new(Seen::default()));
        let mut harness = window(Rc::clone(&shown), Rc::clone(&seen));
        let id = seen.borrow().button;
        *shown.borrow_mut() = false;
        harness.run();
        assert!(seen.borrow().dock.is_none(), "the dock is still in view");
        assert_eq!(seen.borrow().button, id);
    }

    /// `add_parts` run headless in a dock-sized window, once the fonts are
    /// in: the first frame installs them and draws nothing.
    fn parts(mut add_parts: impl FnMut(&mut egui::Ui) + 'static) -> Harness<'static> {
        let mut fonts_installed = false;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(260.0, 400.0))
            .build_ui(move |ui| {
                if fonts_installed {
                    add_parts(ui);
                } else {
                    crate::fonts::install(ui.ctx());
                    fonts_installed = true;
                }
            });
        harness.run();
        harness
    }

    /// A dock's contents start at its edges: they set their own room.
    #[test]
    fn a_dock_s_contents_reach_its_edges() {
        let seen = Rc::new(RefCell::new((Rect::NOTHING, Rect::NOTHING)));
        let record = Rc::clone(&seen);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui(move |ui| {
                let panel = egui::Panel::left("dock").default_size(100.0);
                let mut inside = Rect::NOTHING;
                let dock = show(panel, ui, &mut true, |ui| inside = ui.max_rect());
                if let Some(dock) = dock {
                    *record.borrow_mut() = (dock.response.rect, inside);
                }
            });
        harness.run();
        let (dock, inside) = *seen.borrow();
        assert_eq!(inside.left_top(), dock.left_top(), "{inside:?} in {dock:?}");
    }

    /// The empty message stands in the middle of the dock, its title over
    /// its body.
    #[test]
    fn the_empty_message_stands_in_the_middle() {
        let harness = parts(|ui| {
            empty_message(
                ui,
                "No photo selected",
                "Select a photo to see its information",
            );
        });
        let title = harness.get_by_label("No photo selected").rect();
        let body = harness
            .get_by_label("Select a photo to see its information")
            .rect();
        let middle = harness.ctx.content_rect().center();
        assert!(
            title.bottom() <= middle.y && middle.y <= body.top() + 8.0,
            "{title:?} {body:?}"
        );
        assert!((title.center().x - middle.x).abs() < 1.0, "{title:?}");
    }
}
