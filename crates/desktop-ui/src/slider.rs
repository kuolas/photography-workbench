// SPDX-License-Identifier: GPL-3.0-or-later

//! Sliders: a control that picks a number from a range by a place along a
//! line, the smallest at its left end and the largest at its right.
//!
//! A slider is a thin line across its width, the track, in the colour of
//! an icon that is off, and on it a small upright bar, the knob, in the
//! colour of one that is on, which stands at the value's place along the
//! track. A drag moves the value to the pointer. A click only says the
//! value at the pointer, and leaves it to the caller to move the value
//! there, as most sliders do, or to step it that way, as a slider whose
//! steps lie unevenly along the track does. A slider has no tooltip of
//! its own, takes no keys and never holds the keyboard focus; screen
//! readers know it by its label and read out its value.

use std::ops::RangeInclusive;

use egui::emath::GuiRounding as _;
use egui::{Rect, Sense, Stroke, WidgetInfo, pos2, vec2};

use crate::theme::Palette;

/// The size of a slider.
pub const SLIDER_SIZE: egui::Vec2 = vec2(110.0, 20.0);

/// The thickness of the track.
const TRACK_THICKNESS: f32 = 2.0;

/// The size of the knob, narrow and upright.
const KNOB_SIZE: egui::Vec2 = vec2(3.0, 14.0);

/// The radius of the knob's corners.
const KNOB_CORNER_RADIUS: u8 = 1;

/// What a slider did in a frame.
pub struct Shown {
    /// Its response, marked changed when a drag changed the value.
    pub response: egui::Response,
    /// The value at the pointer's place along the slider when it was
    /// clicked, for the caller to set, or to step the value towards.
    pub click: Option<f32>,
}

/// Show a slider [`SLIDER_SIZE`] in size for `value` within `range`: its
/// knob stands at `value`'s place along the track, at the start when the
/// range is empty or `value` lies below it, and at the end when `value`
/// lies above it. A drag sets `value` from the pointer's place along the
/// slider, and the response is marked changed when `value` changes; a
/// click leaves `value` as it is and says the value at the pointer's
/// place instead. An empty range has no place to pick, so a click or a
/// drag does nothing. While `ui` is disabled, the slider is drawn faded,
/// as egui fades every disabled widget, and ignores the pointer. `label`
/// is the name screen readers and the tests know it by; a tooltip, if
/// any, is the caller's to add.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
) -> Shown {
    let (min, max) = range.into_inner();
    // Clicks and drags, without the focus `Sense::click_and_drag` would add:
    // the slider takes no keys, so the focus would have nothing to do here.
    let (rect, mut response) = ui.allocate_exact_size(SLIDER_SIZE, Sense::CLICK | Sense::DRAG);
    let mut click = None;
    if max > min
        && (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let fraction = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let picked = min + fraction * (max - min);
        if response.clicked() {
            click = Some(picked);
        } else if picked != *value {
            *value = picked;
            response.mark_changed();
        }
    }
    // A disabled `ui`'s painter fades what it paints, as for every widget.
    let painter = ui.painter();
    let middle = rect.center().y;
    let track = Stroke::new(TRACK_THICKNESS, palette.icon_inactive);
    painter.hline(rect.x_range(), middle, track);
    let x = rect.left() + fraction_of(*value, min, max) * rect.width();
    // On the pixel grid, so the knob's edges are drawn sharp wherever it
    // stands.
    let knob =
        Rect::from_center_size(pos2(x, middle), KNOB_SIZE).round_to_pixels(ui.pixels_per_point());
    painter.rect_filled(knob, KNOB_CORNER_RADIUS, palette.icon_active);
    let value = f64::from(*value);
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), value, label));
    Shown { response, click }
}

/// How far along `min..=max` `value` lies, from 0 at `min` to 1 at `max`,
/// clamped to them; 0 when the range is empty.
fn fraction_of(value: f32, min: f32, max: f32) -> f32 {
    if max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use egui::accesskit::Action;
    use egui::epaint::RectShape;
    use egui::{Color32, CornerRadius, Pos2, Shape};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use super::*;
    use crate::theme::Theme;

    /// The range the tests' slider picks from.
    const RANGE: RangeInclusive<f32> = 90.0..=480.0;

    /// What a test's slider reports: its value, how many times its
    /// response said the value changed, and the values it said clicks
    /// were at.
    #[derive(Clone, Default)]
    struct Record {
        value: Rc<Cell<f32>>,
        changes: Rc<Cell<u32>>,
        clicks: Rc<RefCell<Vec<f32>>>,
    }

    /// A slider named "Size" for a value that starts at `start`, within
    /// `range`, in a `ui` enabled when `enabled`.
    fn slider(start: f32, range: RangeInclusive<f32>, enabled: bool) -> (Harness<'static>, Record) {
        let palette = Theme::Paper.palette();
        let record = Record::default();
        record.value.set(start);
        let shown = record.clone();
        let mut harness = Harness::builder()
            .with_size(vec2(300.0, 100.0))
            .build_ui(move |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let mut value = shown.value.get();
                    let slider = show(ui, &palette, "Size", &mut value, range.clone());
                    shown.value.set(value);
                    if slider.response.changed() {
                        shown.changes.set(shown.changes.get() + 1);
                    }
                    if let Some(click) = slider.click {
                        shown.clicks.borrow_mut().push(click);
                    }
                });
            });
        harness.run();
        (harness, record)
    }

    /// The slider's rectangle, as screen readers know it.
    fn bounds(harness: &Harness<'_>) -> Rect {
        harness.get_by_label("Size").rect()
    }

    /// The point `fraction` of the way along the slider, at its middle.
    fn along(harness: &Harness<'_>, fraction: f32) -> Pos2 {
        let rect = bounds(harness);
        pos2(rect.left() + fraction * rect.width(), rect.center().y)
    }

    /// A click at `pos`.
    fn click_at(harness: &mut Harness<'_>, pos: Pos2) {
        harness.hover_at(pos);
        harness.drag_at(pos);
        harness.run();
        harness.drop_at(pos);
        harness.run();
    }

    /// The knobs painted in the last frame: the rectangles on the slider,
    /// or overhanging its ends by a few points, other than the backdrop the
    /// harness fills behind its contents, as large as the slider.
    fn knobs(harness: &Harness<'_>) -> Vec<RectShape> {
        let slider = bounds(harness);
        harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Rect(rect)
                    if slider.expand(4.0).contains_rect(rect.rect) && rect.rect != slider =>
                {
                    Some(rect.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// The straight lines painted in the last frame: their ends, thickness
    /// and colour.
    fn lines(harness: &Harness<'_>) -> Vec<([Pos2; 2], f32, Color32)> {
        harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::LineSegment { points, stroke } => {
                    Some((*points, stroke.width, stroke.color))
                }
                _ => None,
            })
            .collect()
    }

    /// The slider is 110 × 20 points, named by its label, and screen
    /// readers read out its value; it never takes the keyboard focus.
    #[test]
    fn the_slider_is_named_by_its_label_and_carries_its_value() {
        let (harness, _) = slider(220.0, RANGE, true);
        let node = harness.get_by_label("Size");
        assert_eq!(node.rect().size(), vec2(110.0, 20.0));
        let node = node.accesskit_node();
        assert_eq!(node.numeric_value(), Some(220.0));
        let focusable = node.data().supports_action(Action::Focus);
        assert!(!focusable, "the slider takes the focus");
    }

    /// The track is a 2-point line across the slider's width, at its
    /// middle, in the colour of an icon that is off; the knob is a 3 × 14
    /// rounded rectangle in the colour of one that is on, centred on the
    /// value's place along the track, to the nearest pixel.
    #[test]
    fn the_knob_stands_at_the_value_s_place_on_the_track() {
        let palette = Theme::Paper.palette();
        let (harness, _) = slider(220.0, RANGE, true);
        let rect = bounds(&harness);
        let track = [rect.left_center(), rect.right_center()];
        assert_eq!(lines(&harness), [(track, 2.0, palette.icon_inactive)]);
        let knobs = knobs(&harness);
        let [knob] = knobs.as_slice() else {
            panic!("not one knob: {knobs:?}");
        };
        let fraction = (220.0 - 90.0) / (480.0 - 90.0);
        let centre = pos2(rect.left() + fraction * rect.width(), rect.center().y);
        // A third of the way along 110 points: 36.667, which no pixel
        // grid holds.
        assert!((knob.rect.center().x - centre.x).abs() < 0.5, "{knob:?}");
        assert_ne!(knob.rect.center().x, centre.x, "{knob:?}");
        assert_eq!(knob.rect.center().y, centre.y, "{knob:?}");
        assert_eq!(knob.rect.left().fract(), 0.0, "{knob:?}");
        assert_eq!(knob.rect.size(), vec2(3.0, 14.0), "{knob:?}");
        assert_eq!(knob.fill, palette.icon_active);
        assert_eq!(knob.corner_radius, CornerRadius::same(1));
    }

    /// A click says the value at the click's place along the slider, and
    /// leaves the value as it is, unchanged.
    #[test]
    fn a_click_says_the_value_at_its_place() {
        let (mut harness, record) = slider(220.0, RANGE, true);
        let quarter = along(&harness, 0.25);
        click_at(&mut harness, quarter);
        assert_eq!(*record.clicks.borrow(), [90.0 + 0.25 * 390.0]);
        assert_eq!(record.value.get(), 220.0);
        assert_eq!(record.changes.get(), 0);
        let end = along(&harness, 1.0);
        click_at(&mut harness, end);
        assert_eq!(*record.clicks.borrow(), [90.0 + 0.25 * 390.0, 480.0]);
        assert_eq!(record.value.get(), 220.0);
        assert_eq!(record.changes.get(), 0);
    }

    /// A drag moves the value with the pointer, and past either end of the
    /// slider holds it at that end of the range; the response says the
    /// value changed, and no click is said.
    #[test]
    fn a_drag_follows_the_pointer_up_to_the_ends() {
        let (mut harness, record) = slider(220.0, RANGE, true);
        let rect = bounds(&harness);
        let start = along(&harness, 0.5);
        harness.hover_at(start);
        harness.drag_at(start);
        harness.run();
        for (fraction, expected) in [(0.75, 90.0 + 0.75 * 390.0), (0.25, 90.0 + 0.25 * 390.0)] {
            harness.hover_at(along(&harness, fraction));
            harness.run();
            assert_eq!(record.value.get(), expected, "at {fraction}");
        }
        harness.hover_at(pos2(rect.right() + 40.0, rect.center().y));
        harness.run();
        assert_eq!(record.value.get(), 480.0, "past the right end");
        let left = pos2(rect.left() - 40.0, rect.center().y);
        harness.hover_at(left);
        harness.run();
        assert_eq!(record.value.get(), 90.0, "past the left end");
        harness.drop_at(left);
        harness.run();
        assert_eq!(record.value.get(), 90.0, "after the drop");
        assert!(record.changes.get() >= 3, "{}", record.changes.get());
        assert!(
            record.clicks.borrow().is_empty(),
            "{:?}",
            record.clicks.borrow()
        );
    }

    /// With an empty range, the knob stands at the start of the track, and
    /// a click says nothing and leaves the value as it was, whether it is
    /// the range's one value or lies outside it.
    #[test]
    fn an_empty_range_holds_the_knob_at_the_start() {
        let (mut harness, record) = slider(220.0, 220.0..=220.0, true);
        let rect = bounds(&harness);
        let knobs = knobs(&harness);
        let [knob] = knobs.as_slice() else {
            panic!("not one knob: {knobs:?}");
        };
        // At the start of the track, to the nearest pixel.
        let start = rect.left_center();
        assert!((knob.rect.center().x - start.x).abs() <= 0.5, "{knob:?}");
        assert_eq!(knob.rect.center().y, start.y, "{knob:?}");
        let middle = along(&harness, 0.5);
        click_at(&mut harness, middle);
        assert_eq!(record.value.get(), 220.0);
        assert_eq!(record.changes.get(), 0);
        assert!(
            record.clicks.borrow().is_empty(),
            "{:?}",
            record.clicks.borrow()
        );
        let (mut harness, record) = slider(300.0, 220.0..=220.0, true);
        let middle = along(&harness, 0.5);
        click_at(&mut harness, middle);
        assert_eq!(record.value.get(), 300.0, "outside the empty range");
        assert_eq!(record.changes.get(), 0);
    }

    /// A disabled slider is drawn faded, as egui fades every disabled
    /// widget, and a click says nothing and changes nothing.
    #[test]
    fn a_disabled_slider_is_faded_and_ignores_clicks() {
        let palette = Theme::Paper.palette();
        let (mut harness, record) = slider(220.0, RANGE, false);
        let faded = |colour| harness.ctx.global_style().visuals.disable(colour);
        let lines = lines(&harness);
        let knobs = knobs(&harness);
        assert_eq!(lines[0].2, faded(palette.icon_inactive), "{lines:?}");
        assert_eq!(knobs[0].fill, faded(palette.icon_active), "{knobs:?}");
        let quarter = along(&harness, 0.25);
        click_at(&mut harness, quarter);
        assert_eq!(record.value.get(), 220.0);
        assert_eq!(record.changes.get(), 0);
        assert!(
            record.clicks.borrow().is_empty(),
            "{:?}",
            record.clicks.borrow()
        );
    }
}
