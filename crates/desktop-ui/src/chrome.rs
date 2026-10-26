// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of a title bar the app draws itself: the bar's frame, the
//! region that drags the window, the buttons the bar holds, the window
//! controls, and the strips along the window's edges that resize it.
//!
//! On Windows and Linux the app turns the system's frame off, so it draws
//! the close, maximize and minimize buttons and handles resizing itself. On
//! macOS the system's frame stays, with its title bar hidden and the content
//! extended under it: the system's three buttons sit on the app's title bar
//! and its own edges resize the window, so [`window_controls`] and
//! [`window_resize_handles`] add nothing there, and [`title_bar_left_pad`]
//! keeps the bar's first control clear of those buttons. Dragging the bar
//! and double-clicking it work through [`title_bar_drag_region`] on every
//! system.

use egui::{
    Align2, CursorIcon, Frame, Margin, Rect, ResizeDirection, Sense, Stroke, StrokeKind,
    ViewportCommand, pos2, vec2,
};

use crate::dock::Side;
use crate::icons;
use crate::keys::{self, KeyCombination};
use crate::theme::Palette;

/// The height of the title bar, and of the window controls, which fill it.
pub const TITLE_BAR_HEIGHT: f32 = 42.0;

/// The width of the strips along the window's edges that resize it.
pub const RESIZE_BORDER: f32 = 8.0;

/// The width of macOS's close, minimize and zoom buttons with their margins,
/// in the system's points, which egui's zoom does not change, for when the
/// window cannot be measured: the buttons are 14 pt wide at x = 9, 32 and
/// 55, with the 9 pt margin repeated after the last.
pub const MAC_WINDOW_BUTTONS_WIDTH: f32 = 78.0;

/// The title bar's left margin, in points. There is none on the right, so
/// the close button reaches the window's corner.
const TITLE_BAR_LEFT_MARGIN: i8 = 8;

/// The width of each window control.
const WINDOW_CONTROL_WIDTH: f32 = 46.0;

/// The room an [`icon_button`] or a [`dock_toggle`] takes along the bar:
/// the square it takes the pointer over, and 6 points either side, so
/// neighbouring icons stand 40 points apart.
const ICON_SLOT_WIDTH: f32 = 40.0;

/// The side of the square an [`icon_button`] or a [`dock_toggle`] takes
/// the pointer over.
const ICON_HIT_SIZE: f32 = 28.0;

/// The size of an [`icon_button`]'s icon.
const ICON_SIZE: f32 = 18.0;

/// Whether the system draws the window's controls and resizes it at its
/// edges, which it does on macOS only.
const SYSTEM_FRAME: bool = cfg!(target_os = "macos");

/// The title bar's frame: its fill and its margins. [`title_bar_left_pad`]
/// counts on this frame's left margin.
pub fn title_bar_frame(palette: &Palette) -> Frame {
    Frame::NONE.fill(palette.panel).inner_margin(Margin {
        left: TITLE_BAR_LEFT_MARGIN,
        right: 0,
        top: 0,
        bottom: 0,
    })
}

/// The space to leave before the title bar's first control, inside
/// [`title_bar_frame`]. On macOS that clears the system's window buttons,
/// which float over the bar's left end, plus a gap so the first control
/// does not crowd them; elsewhere it is a small gap.
///
/// `measured` is the buttons' width with their margins as the system
/// reports it at start-up, unaffected by egui's zoom; `None` when it could
/// not be measured, which leaves [`MAC_WINDOW_BUTTONS_WIDTH`]. In full
/// screen the system hides the buttons, and the gap is the small one.
pub fn title_bar_left_pad(ctx: &egui::Context, measured: Option<f32>) -> f32 {
    let full_screen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    left_pad(SYSTEM_FRAME && !full_screen, measured, ctx.zoom_factor())
}

/// [`title_bar_left_pad`] once the system is known: `system_buttons` says
/// whether the system's window buttons are on the bar, `measured` is their
/// width in the system's points, and `zoom` is egui's zoom factor, which
/// turns those into egui's points.
fn left_pad(system_buttons: bool, measured: Option<f32>, zoom: f32) -> f32 {
    /// The gap between the system's buttons and the first control.
    const GAP: f32 = 8.0;
    if !system_buttons {
        return 2.0;
    }
    let buttons = measured.map_or(MAC_WINDOW_BUTTONS_WIDTH, |width| {
        width.max(MAC_WINDOW_BUTTONS_WIDTH)
    });
    buttons / zoom - f32::from(TITLE_BAR_LEFT_MARGIN) + GAP
}

/// Make `bar` drag the window, and maximize or restore it on a
/// double-click.
///
/// Call it before adding anything else to the bar: widgets added later
/// take the pointer first, so they keep their clicks, and the bar's empty
/// parts drag the window. The drag starts once the pointer has moved
/// farther than a click may, never on the press, nor when a press is only
/// held: a press on a button lands on the bar under it too, and starting
/// the drag there would take the pointer away from the button.
pub fn title_bar_drag_region(ui: &mut egui::Ui, bar: Rect) {
    let id = ui.id().with("title_bar_drag");
    let response = ui.interact(bar, id, Sense::click_and_drag());
    if response.double_clicked() {
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    // egui counts a press held still past its click time as a drag too, so
    // whether the pointer has moved is checked here, once per press.
    let started = id.with("started");
    if response.dragged_by(egui::PointerButton::Primary) {
        let max_click_dist = ui.ctx().options(|o| o.input_options.max_click_dist);
        let moved = ui.input(|i| {
            i.pointer
                .total_drag_delta()
                .is_some_and(|delta| delta.length() > max_click_dist)
        });
        if moved && !ui.data(|d| d.get_temp(started).unwrap_or(false)) {
            ui.data_mut(|d| d.insert_temp(started, true));
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    } else {
        ui.data_mut(|d| d.remove::<bool>(started));
    }
}

/// A title-bar button showing only the Lucide icon `icon.0`, or the
/// character `icon.1` if the name does not resolve. The icon brightens under
/// the pointer, with nothing drawn behind it. `label` is the name screen
/// readers and the tests know it by, and its tooltip, with the
/// `key_combination` that does the same, if any ([`keys::tooltip`]).
pub fn icon_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: (&str, char),
    label: &str,
    key_combination: Option<KeyCombination>,
) -> egui::Response {
    let response = title_icon(ui, label, None);
    let colour = if response.hovered() {
        palette.icon_active
    } else {
        palette.icon_inactive
    };
    let (glyph, font) = icons::glyph(icon.0, ICON_SIZE, icon.1);
    let centre = response.rect.center();
    ui.painter()
        .text(centre, Align2::CENTER_CENTER, glyph, font, colour);
    response.on_hover_text(keys::tooltip(label, key_combination))
}

/// The place of an [`icon_button`] or a [`dock_toggle`] on the bar: an
/// [`ICON_SLOT_WIDTH`] slot, the bar's height, with a square in its middle
/// that takes the pointer. Named `label` for screen readers and the tests,
/// and, when `selected` is given, said to be on or off.
fn title_icon(ui: &mut egui::Ui, label: &str, selected: Option<bool>) -> egui::Response {
    let (slot, _) = ui.allocate_exact_size(vec2(ICON_SLOT_WIDTH, TITLE_BAR_HEIGHT), Sense::hover());
    let square = Rect::from_center_size(slot.center(), egui::Vec2::splat(ICON_HIT_SIZE));
    let response = ui.interact(square, ui.id().with(("title_icon", label)), Sense::click());
    response.widget_info(|| match selected {
        Some(on) => {
            egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), on, label)
        }
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label),
    });
    response
}

/// A title-bar button that shows or hides the dock on `side`: a small window
/// whose strip on that side is filled while the dock is `shown`, and only
/// marked off by a line while it is hidden. It brightens while the dock is
/// shown and under the pointer. `label` is the name screen readers and the
/// tests know it by, and its tooltip, with the `key_combination` that does
/// the same, if any ([`keys::tooltip`]).
pub fn dock_toggle(
    ui: &mut egui::Ui,
    palette: &Palette,
    side: Side,
    shown: bool,
    label: &str,
    key_combination: Option<KeyCombination>,
) -> egui::Response {
    let response = title_icon(ui, label, Some(shown));
    let colour = if shown || response.hovered() {
        palette.icon_active
    } else {
        palette.icon_inactive
    };
    let (window, strip) = dock_glyph(response.rect.center(), side);
    let stroke = Stroke::new(1.2, colour);
    let painter = ui.painter();
    painter.rect_stroke(window, 1.0, stroke, StrokeKind::Inside);
    if shown {
        painter.rect_filled(strip, 0.0, colour);
    } else {
        let edge = match side {
            Side::Left => [strip.right_top(), strip.right_bottom()],
            Side::Right => [strip.left_top(), strip.left_bottom()],
            Side::Bottom => [strip.left_top(), strip.right_top()],
        };
        painter.line_segment(edge, stroke);
    }
    response.on_hover_text(keys::tooltip(label, key_combination))
}

/// The glyph of a [`dock_toggle`] centred on `centre`: the outline of a
/// window, 15 by 12 points, and the strip of it on `side` that stands for
/// the dock.
fn dock_glyph(centre: egui::Pos2, side: Side) -> (Rect, Rect) {
    let window = Rect::from_center_size(centre, vec2(15.0, 12.0));
    let strip = match side {
        Side::Left => Rect::from_min_max(
            window.left_top(),
            pos2(window.left() + 5.0, window.bottom()),
        ),
        Side::Right => Rect::from_min_max(
            pos2(window.right() - 5.0, window.top()),
            window.right_bottom(),
        ),
        Side::Bottom => Rect::from_min_max(
            pos2(window.left(), window.bottom() - 4.0),
            window.right_bottom(),
        ),
    };
    (window, strip)
}

/// A title-bar button with the Lucide icon `icon.0` (or the text `icon.1` if
/// the name does not resolve) before the word `label`, shown selected when
/// `selected`: one of a row of buttons choosing one of several things.
/// Screen readers and the tests know it by `label` alone. With a
/// `key_combination` that does the same, its tooltip names it
/// ([`keys::tooltip`]); without, it has none, as its word says what it is.
/// Disabled when added to a disabled `ui`.
pub fn selectable_button(
    ui: &mut egui::Ui,
    icon: (&str, &str),
    label: &str,
    selected: bool,
    key_combination: Option<KeyCombination>,
) -> egui::Response {
    let text = icons::labeled(ui, icon.0, icon.1, label);
    let response = ui.add(egui::Button::new(text).selected(selected));
    // egui names a button by all its text, the icon's character included.
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            response.enabled(),
            selected,
            label,
        )
    });
    match key_combination {
        Some(_) => response.on_hover_text(keys::tooltip(label, key_combination)),
        None => response,
    }
}

/// The close, maximize (or restore) and minimize buttons, in a
/// right-to-left layout, so close ends up at the far right as on Windows
/// and Linux. On macOS they add nothing: the system's own buttons are there.
pub fn window_controls(ui: &mut egui::Ui, palette: &Palette) {
    if !SYSTEM_FRAME {
        draw_window_controls(ui, palette);
    }
}

/// [`window_controls`], drawn whatever the system.
fn draw_window_controls(ui: &mut egui::Ui, palette: &Palette) {
    if window_control(ui, palette, ("x", '🗙'), "Close", true).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    let (icon, label) = if maximized {
        (("copy", '🗗'), "Restore")
    } else {
        (("square", '🗖'), "Maximize")
    };
    if window_control(ui, palette, icon, label, false).clicked() {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if window_control(ui, palette, ("minus", '🗕'), "Minimize", false).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

/// One window control, the title bar's full height: the Lucide icon
/// `icon.0`, or the character `icon.1` if it does not resolve. `danger`
/// turns its hover red, for the close button.
fn window_control(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: (&str, char),
    label: &str,
    danger: bool,
) -> egui::Response {
    let (hover_glyph, fill) = if danger {
        (palette.on_danger, palette.danger)
    } else {
        (palette.window_ctrl, palette.hover_overlay)
    };
    let colours = (palette.window_ctrl, hover_glyph);
    let size = vec2(WINDOW_CONTROL_WIDTH, TITLE_BAR_HEIGHT);
    painted_button(ui, size, (icon, 14.0), colours, fill, label)
}

/// A window control of `size` that paints its own icon: the Lucide icon
/// `icon.0.0`, or the character `icon.0.1` if it does not resolve, at
/// `icon.1` points. The icon is `colours.0`, and `colours.1` on `fill`
/// while the pointer is over the button. The icon is painted rather than
/// added as a widget, so the button gives screen readers and the tests
/// `label` as its name itself; `label` is its tooltip too.
fn painted_button(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    icon: ((&str, char), f32),
    colours: (egui::Color32, egui::Color32),
    fill: egui::Color32,
    label: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let colour = if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, fill);
        colours.1
    } else {
        colours.0
    };
    let ((name, fallback), points) = icon;
    let (glyph, font) = icons::glyph(name, points, fallback);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, glyph, font, colour);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(label)
}

/// Strips along the window's edges and corners that resize it, above the
/// window's panels and under its dialogs, popups and tooltips. They add
/// nothing while the window is maximized or full screen, nor on macOS,
/// where the system's frame resizes it.
pub fn window_resize_handles(ctx: &egui::Context) {
    if !SYSTEM_FRAME {
        draw_resize_handles(ctx);
    }
}

/// [`window_resize_handles`], whatever the system.
fn draw_resize_handles(ctx: &egui::Context) {
    let fills_screen = ctx.input(|i| {
        let viewport = i.viewport();
        viewport.maximized.unwrap_or(false) || viewport.fullscreen.unwrap_or(false)
    });
    if fills_screen {
        return;
    }
    let window = ctx.input(egui::InputState::viewport_rect);
    // The resize starts on the press, not once a drag is under way: from
    // there the system follows the pointer, so the edge moves with it from
    // the first pixel. The strips are above the window's panels, so no
    // panel under them needs the press; a dialog or a popup over a strip
    // is above it and takes the press itself.
    let pressed = ctx.input(|i| i.pointer.primary_pressed());
    // In egui's middle order, with the dialogs: above the window's panels,
    // in the background order, and under a dialog, which egui brings to
    // the top as it opens.
    egui::Area::new(egui::Id::new("window_resize_handles"))
        .order(egui::Order::Middle)
        .fixed_pos(window.min)
        .show(ctx, |ui| {
            ui.set_clip_rect(window);
            // egui says the pointer is over every strip under it, so a
            // corner would resize three ways at once: the last strip under
            // it, the corner where one overlaps the edges, is the one.
            let mut under_pointer = None;
            for (i, (rect, direction, cursor)) in resize_strips(window).into_iter().enumerate() {
                let response =
                    ui.interact(rect, egui::Id::new(("window_resize", i)), Sense::drag());
                if response.contains_pointer() {
                    under_pointer = Some((direction, cursor));
                }
            }
            if let Some((direction, cursor)) = under_pointer {
                ui.ctx().set_cursor_icon(cursor);
                if pressed {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
            }
        });
}

/// The eight strips of [`window_resize_handles`] for a window covering
/// `window`: the edges first, then the corners, which overlap them and must
/// come last.
fn resize_strips(window: Rect) -> [(Rect, ResizeDirection, CursorIcon); 8] {
    use CursorIcon as C;
    use ResizeDirection as D;
    let (l, r, t, b) = (window.left(), window.right(), window.top(), window.bottom());
    let w = RESIZE_BORDER;
    let strip = |x0, y0, x1, y1| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    [
        (strip(l, t, l + w, b), D::West, C::ResizeWest),
        (strip(r - w, t, r, b), D::East, C::ResizeEast),
        (strip(l, t, r, t + w), D::North, C::ResizeNorth),
        (strip(l, b - w, r, b), D::South, C::ResizeSouth),
        (strip(l, t, l + w, t + w), D::NorthWest, C::ResizeNorthWest),
        (strip(r - w, t, r, t + w), D::NorthEast, C::ResizeNorthEast),
        (strip(l, b - w, l + w, b), D::SouthWest, C::ResizeSouthWest),
        (strip(r - w, b - w, r, b), D::SouthEast, C::ResizeSouthEast),
    ]
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use egui::accesskit::Toggled;
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, ViewportId};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use super::*;
    use crate::theme::Theme;

    /// `add_contents` run headless in a window of `size`, maximized or not.
    /// The first frame installs the icon fonts, which egui uses from the
    /// next frame on, and draws nothing.
    fn harness(
        size: egui::Vec2,
        maximized: bool,
        mut add_contents: impl FnMut(&mut egui::Ui) + 'static,
    ) -> Harness<'static> {
        let mut fonts_installed = false;
        let mut harness = Harness::builder().with_size(size).build_ui(move |ui| {
            if fonts_installed {
                add_contents(ui);
            } else {
                icons::install(ui.ctx());
                fonts_installed = true;
            }
        });
        harness
            .input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .maximized = Some(maximized);
        harness.run();
        harness
    }

    /// The commands the last frame sent the window.
    fn commands(harness: &Harness<'_>) -> Vec<ViewportCommand> {
        harness
            .output()
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|viewport| viewport.commands.clone())
            .unwrap_or_default()
    }

    /// Run one frame with `events`, and return what it sent the window.
    /// (The harness's own `event` runs a frame for each event.)
    fn frame_with(harness: &mut Harness<'_>, events: Vec<Event>) -> Vec<ViewportCommand> {
        harness.input_mut().events.extend(events);
        harness.step();
        commands(harness)
    }

    /// The pointer moving to `pos`, and the primary button pressed there
    /// and, unless `hold`, released.
    fn press_at(pos: Pos2, hold: bool) -> Vec<Event> {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let mut events = vec![Event::PointerMoved(pos), button(true)];
        if !hold {
            events.push(button(false));
        }
        events
    }

    /// A 40 pt high title bar, 400 pt wide: the drag region over all of it,
    /// and the window controls at its right end.
    fn title_bar(maximized: bool) -> Harness<'static> {
        harness(vec2(400.0, TITLE_BAR_HEIGHT), maximized, |ui| {
            let bar = ui.max_rect();
            title_bar_drag_region(ui, bar);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                draw_window_controls(ui, &Theme::Paper.palette());
            });
        })
    }

    #[test]
    fn the_left_pad_clears_the_mac_buttons_only_when_they_are_on_the_bar() {
        assert_eq!(left_pad(false, Some(100.0), 1.0), 2.0);
        // The frame's 8 pt margin is part of the clearance; 8 pt more is
        // the gap after the buttons.
        assert_eq!(left_pad(true, None, 1.0), MAC_WINDOW_BUTTONS_WIDTH);
        assert_eq!(left_pad(true, Some(90.0), 1.0), 90.0);
        // A measurement narrower than the buttons can be is not trusted.
        assert_eq!(left_pad(true, Some(10.0), 1.0), MAC_WINDOW_BUTTONS_WIDTH);
    }

    /// egui's zoom leaves the system's buttons as they are, so zoomed in,
    /// they take fewer of egui's points, and zoomed out, more.
    #[test]
    fn the_left_pad_follows_the_zoom() {
        let half = MAC_WINDOW_BUTTONS_WIDTH / 2.0;
        assert_eq!(left_pad(true, None, 2.0), half);
        assert_eq!(left_pad(true, Some(10.0), 2.0), half);
        assert_eq!(left_pad(true, Some(90.0), 2.0), 45.0);
        assert_eq!(left_pad(true, None, 0.5), 2.0 * MAC_WINDOW_BUTTONS_WIDTH);
    }

    #[test]
    fn each_window_control_sends_its_command() {
        let cases = [
            ("Close", ViewportCommand::Close),
            ("Maximize", ViewportCommand::Maximized(true)),
            ("Minimize", ViewportCommand::Minimized(true)),
        ];
        for (label, command) in cases {
            let mut harness = title_bar(false);
            let centre = harness.get_by_label(label).rect().center();
            let sent = frame_with(&mut harness, press_at(centre, false));
            assert_eq!(sent, [command], "{label}");
        }
    }

    #[test]
    fn a_maximized_window_offers_restore() {
        let mut harness = title_bar(true);
        assert!(harness.query_by_label("Maximize").is_none());
        let centre = harness.get_by_label("Restore").rect().center();
        let sent = frame_with(&mut harness, press_at(centre, false));
        assert_eq!(sent, [ViewportCommand::Maximized(false)]);
    }

    #[test]
    fn double_clicking_the_bar_maximizes_the_window() {
        let mut harness = title_bar(false);
        let mut events = press_at(pos2(20.0, 20.0), false);
        events.extend(press_at(pos2(20.0, 20.0), false));
        let sent = frame_with(&mut harness, events);
        assert_eq!(sent, [ViewportCommand::Maximized(true)]);
    }

    #[test]
    fn dragging_the_bar_moves_the_window_and_a_click_does_not() {
        let mut harness = title_bar(false);
        let sent = frame_with(&mut harness, press_at(pos2(20.0, 20.0), false));
        assert!(sent.is_empty(), "a click on the bar sent {sent:?}");

        let sent = frame_with(&mut harness, press_at(pos2(20.0, 20.0), true));
        assert!(sent.is_empty(), "the press alone sent {sent:?}");
        let sent = frame_with(&mut harness, vec![Event::PointerMoved(pos2(60.0, 20.0))]);
        assert_eq!(sent, [ViewportCommand::StartDrag]);
    }

    #[test]
    fn the_window_draws_its_own_controls_except_on_macos() {
        let palette = Theme::Paper.palette();
        let mut harness = harness(vec2(400.0, 300.0), false, move |ui| {
            window_controls(ui, &palette);
            window_resize_handles(ui.ctx());
        });
        assert_eq!(harness.query_by_label("Close").is_some(), !SYSTEM_FRAME);
        let sent = frame_with(&mut harness, press_at(pos2(200.0, 298.0), false));
        assert_eq!(
            sent.is_empty(),
            SYSTEM_FRAME,
            "the bottom edge sent {sent:?}"
        );
    }

    #[test]
    fn holding_a_press_on_the_bar_does_not_move_the_window() {
        let mut harness = title_bar(false);
        let sent = frame_with(&mut harness, press_at(pos2(20.0, 20.0), true));
        assert!(sent.is_empty(), "the press sent {sent:?}");
        // Each frame is a quarter of a second: well past egui's click time.
        for _ in 0..8 {
            let sent = frame_with(&mut harness, vec![]);
            assert!(sent.is_empty(), "the held press sent {sent:?}");
        }
        let sent = frame_with(&mut harness, vec![Event::PointerMoved(pos2(60.0, 20.0))]);
        assert_eq!(sent, [ViewportCommand::StartDrag]);
        let sent = frame_with(&mut harness, vec![Event::PointerMoved(pos2(80.0, 20.0))]);
        assert!(sent.is_empty(), "a second move sent {sent:?}");
    }

    #[test]
    fn the_left_pad_is_small_in_full_screen() {
        let pad = |full_screen| {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input
                .viewports
                .entry(ViewportId::ROOT)
                .or_default()
                .fullscreen = Some(full_screen);
            ctx.begin_pass(input);
            let pad = title_bar_left_pad(&ctx, None);
            // No renderer here to upload the font atlas to.
            ctx.end_pass().textures_delta.clear();
            pad
        };
        assert_eq!(pad(true), 2.0);
        assert_eq!(pad(false), left_pad(SYSTEM_FRAME, None, 1.0));
    }

    #[test]
    fn an_icon_button_is_named_by_its_label_and_reports_its_click() {
        let clicks = Rc::new(Cell::new(0));
        let counted = Rc::clone(&clicks);
        let palette = Theme::Paper.palette();
        let mut harness = harness(vec2(200.0, TITLE_BAR_HEIGHT), false, move |ui| {
            if icon_button(ui, &palette, ("settings", '⚙'), "Settings", None).clicked() {
                counted.set(counted.get() + 1);
            }
        });
        harness.get_by_label("Settings").click();
        harness.run();
        assert_eq!(clicks.get(), 1);
    }

    /// An icon button alone in a bar 120 points wide.
    fn lone_icon_button() -> Harness<'static> {
        let palette = Theme::Paper.palette();
        harness(vec2(120.0, TITLE_BAR_HEIGHT), false, move |ui| {
            icon_button(ui, &palette, ("settings", '⚙'), "Settings", None);
        })
    }

    /// An icon takes the pointer over a 28-point square, and the next one
    /// stands 40 points along.
    #[test]
    fn icons_take_28_point_squares_40_points_apart() {
        let palette = Theme::Paper.palette();
        let harness = harness(vec2(200.0, TITLE_BAR_HEIGHT), false, move |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                icon_button(ui, &palette, ("settings", '⚙'), "Settings", None);
                dock_toggle(ui, &palette, Side::Left, true, "Left dock", None);
            });
        });
        let gear = harness.get_by_label("Settings").rect();
        let toggle = harness.get_by_label("Left dock").rect();
        for square in [gear, toggle] {
            assert_eq!(
                square.size(),
                egui::Vec2::splat(ICON_HIT_SIZE),
                "{square:?}"
            );
        }
        assert_eq!(toggle.center().x - gear.center().x, ICON_SLOT_WIDTH);
    }

    /// Under the pointer the icon brightens, with nothing painted behind it.
    #[test]
    fn an_icon_button_brightens_under_the_pointer_with_nothing_behind_it() {
        let palette = Theme::Paper.palette();
        let mut harness = lone_icon_button();
        let background = harness.ctx.global_style().visuals.panel_fill;
        let icon_colour = |harness: &Harness<'_>| {
            let mut colours = Vec::new();
            for clipped in &harness.output().shapes {
                match &clipped.shape {
                    egui::Shape::Text(text) => colours.push(text.fallback_color),
                    egui::Shape::Rect(rect) if rect.fill != background => {
                        panic!("a plate behind the icon: {rect:?}");
                    }
                    _ => {}
                }
            }
            colours
        };
        assert_eq!(icon_colour(&harness), [palette.icon_inactive]);
        let centre = harness.get_by_label("Settings").rect().center();
        frame_with(&mut harness, vec![Event::PointerMoved(centre)]);
        assert_eq!(icon_colour(&harness), [palette.icon_active]);
    }

    #[test]
    fn a_selectable_button_is_named_by_its_word_and_says_whether_it_is_selected() {
        let harness = harness(vec2(400.0, TITLE_BAR_HEIGHT), false, |ui| {
            ui.horizontal(|ui| {
                selectable_button(ui, ("images", "▦"), "Library", true, None);
                ui.add_enabled_ui(false, |ui| {
                    selectable_button(ui, ("import", "⇩"), "Organizer", false, None);
                });
            });
        });
        let library = harness.get_by_label("Library").accesskit_node();
        assert_eq!(library.toggled(), Some(Toggled::True));
        assert!(!library.is_disabled());
        let organizer = harness.get_by_label("Organizer").accesskit_node();
        assert_eq!(organizer.toggled(), Some(Toggled::False));
        assert!(organizer.is_disabled());
    }

    /// Rest the pointer on `label` until a tooltip saying `tip` shows, or
    /// plainly too many frames go by; whether it showed. egui shows a
    /// tooltip once the pointer has rested a moment.
    fn tooltip_shows(harness: &mut Harness<'_>, label: &str, tip: &str) -> bool {
        harness.get_by_label(label).hover();
        (0..20).any(|_| {
            harness.step();
            harness.query_by_label(tip).is_some()
        })
    }

    /// A button with a key combination names it in its tooltip, and keeps
    /// its label as its name.
    #[test]
    fn a_button_s_tooltip_names_its_combination() {
        let palette = Theme::Paper.palette();
        let combination = |key| Some(KeyCombination::new(Modifiers::NONE, key));
        let mut harness = harness(vec2(400.0, TITLE_BAR_HEIGHT), false, move |ui| {
            ui.horizontal(|ui| {
                icon_button(
                    ui,
                    &palette,
                    ("settings", '⚙'),
                    "Settings",
                    combination(Key::F1),
                );
                dock_toggle(ui, &palette, Side::Left, true, "Left", combination(Key::F7));
                selectable_button(ui, ("images", "▦"), "Library", true, combination(Key::G));
            });
        });
        let tips = [
            ("Settings", "Settings (F1)"),
            ("Left", "Left (F7)"),
            ("Library", "Library (G)"),
        ];
        for (label, tip) in tips {
            assert!(tooltip_shows(&mut harness, label, tip), "{label}");
        }
    }

    /// A selectable button's word says what it is, so without a key
    /// combination it has no tooltip to repeat it.
    #[test]
    fn a_selectable_button_without_a_combination_has_no_tooltip() {
        let mut harness = harness(vec2(400.0, TITLE_BAR_HEIGHT), false, |ui| {
            selectable_button(ui, ("images", "▦"), "Library", true, None);
        });
        harness.get_by_label("Library").hover();
        for _ in 0..20 {
            harness.step();
        }
        // A tooltip would be a second thing named "Library".
        assert_eq!(harness.query_all_by_label("Library").count(), 1);
    }

    /// A dock toggle for each side, the left and bottom docks shown and
    /// the right one hidden. A click on one flips its entry in `shown`.
    fn dock_toggles(shown: Rc<Cell<[bool; 3]>>) -> Harness<'static> {
        let palette = Theme::Paper.palette();
        harness(vec2(200.0, TITLE_BAR_HEIGHT), false, move |ui| {
            ui.horizontal(|ui| {
                let sides = [
                    (Side::Left, "Left dock"),
                    (Side::Bottom, "Bottom dock"),
                    (Side::Right, "Right dock"),
                ];
                let mut now = shown.get();
                for (i, (side, label)) in sides.into_iter().enumerate() {
                    if dock_toggle(ui, &palette, side, now[i], label, None).clicked() {
                        now[i] = !now[i];
                    }
                }
                shown.set(now);
            });
        })
    }

    #[test]
    fn a_dock_toggle_is_named_by_its_label_and_says_whether_the_dock_is_shown() {
        let shown = Rc::new(Cell::new([true, true, false]));
        let mut harness = dock_toggles(Rc::clone(&shown));
        let toggled =
            |harness: &Harness<'_>, label| harness.get_by_label(label).accesskit_node().toggled();
        assert_eq!(toggled(&harness, "Left dock"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Right dock"), Some(Toggled::False));
        harness.get_by_label("Right dock").click();
        harness.run();
        assert_eq!(shown.get(), [true, true, true]);
        assert_eq!(toggled(&harness, "Right dock"), Some(Toggled::True));
    }

    /// The glyph's strip is filled while its dock is shown, and only its
    /// inner edge is drawn while it is hidden.
    #[test]
    fn a_dock_toggle_fills_its_strip_only_while_the_dock_is_shown() {
        let palette = Theme::Paper.palette();
        let icon_colours = [palette.icon_active, palette.icon_inactive];
        let filled = |shown: [bool; 3]| {
            let harness = dock_toggles(Rc::new(Cell::new(shown)));
            harness
                .output()
                .shapes
                .iter()
                .filter(|clipped| match &clipped.shape {
                    egui::Shape::Rect(rect) => icon_colours.contains(&rect.fill),
                    _ => false,
                })
                .count()
        };
        assert_eq!(filled([true, true, false]), 2);
        assert_eq!(filled([false, false, false]), 0);
        assert_eq!(filled([true, true, true]), 3);
    }

    /// Each strip runs along its own side of the window outline.
    #[test]
    fn the_dock_glyph_marks_the_dock_s_side() {
        let centre = pos2(100.0, 100.0);
        let (window, left) = dock_glyph(centre, Side::Left);
        assert_eq!(window, Rect::from_center_size(centre, vec2(15.0, 12.0)));
        let column = |x: f32| Rect::from_x_y_ranges(x..=x + 5.0, window.y_range());
        assert_eq!(left, column(window.left()));
        let (_, right) = dock_glyph(centre, Side::Right);
        assert_eq!(right, column(window.right() - 5.0));
        let (_, bottom) = dock_glyph(centre, Side::Bottom);
        let floor = window.bottom() - 4.0..=window.bottom();
        assert_eq!(bottom, Rect::from_x_y_ranges(window.x_range(), floor));
    }

    /// A window 400 × 300 pt with nothing in it but the resize strips.
    fn resizable_window(maximized: bool) -> Harness<'static> {
        harness(vec2(400.0, 300.0), maximized, |ui| {
            draw_resize_handles(ui.ctx());
        })
    }

    #[test]
    fn a_press_on_an_edge_or_a_corner_resizes_the_window_that_way() {
        use ResizeDirection as D;
        let mut harness = resizable_window(false);
        let cases = [
            (pos2(2.0, 150.0), D::West),
            (pos2(398.0, 150.0), D::East),
            (pos2(200.0, 2.0), D::North),
            (pos2(200.0, 298.0), D::South),
            (pos2(2.0, 2.0), D::NorthWest),
            (pos2(398.0, 2.0), D::NorthEast),
            (pos2(2.0, 298.0), D::SouthWest),
            (pos2(398.0, 298.0), D::SouthEast),
        ];
        for (pos, direction) in cases {
            let sent = frame_with(&mut harness, press_at(pos, false));
            assert_eq!(sent, [ViewportCommand::BeginResize(direction)], "{pos:?}");
        }
        let sent = frame_with(&mut harness, press_at(pos2(200.0, 150.0), false));
        assert!(sent.is_empty(), "the middle of the window sent {sent:?}");
    }

    #[test]
    fn a_maximized_window_does_not_resize_at_its_edges() {
        let mut harness = resizable_window(true);
        let sent = frame_with(&mut harness, press_at(pos2(2.0, 150.0), false));
        assert!(sent.is_empty(), "the left edge sent {sent:?}");
    }

    #[test]
    fn a_full_screen_window_does_not_resize_at_its_edges() {
        let mut harness = resizable_window(false);
        harness
            .input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .fullscreen = Some(true);
        harness.run();
        let sent = frame_with(&mut harness, press_at(pos2(2.0, 150.0), false));
        assert!(sent.is_empty(), "the left edge sent {sent:?}");
    }

    /// A dialog is above the resize strips, as above everything else in
    /// the window: a press on an edge while it is open does not resize the
    /// window.
    #[test]
    fn a_dialog_is_above_the_resize_strips() {
        let mut harness = harness(vec2(400.0, 300.0), false, |ui| {
            draw_resize_handles(ui.ctx());
            crate::dialog::show(ui.ctx(), "dialog", |ui| ui.label("Inside"));
        });
        harness.run();
        let sent = frame_with(&mut harness, press_at(pos2(2.0, 150.0), false));
        assert!(sent.is_empty(), "the left edge sent {sent:?}");
    }
}
