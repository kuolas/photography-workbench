// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of a title bar the app draws itself: the bar's frame, the
//! region that drags the window, the window controls, and the strips along
//! the window's edges that resize it.
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
    Align2, CursorIcon, Frame, Margin, Rect, ResizeDirection, Sense, ViewportCommand, pos2, vec2,
};

use crate::icons;
use crate::theme::Palette;

/// The height of the title bar, and of the window controls, which fill it.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;

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

/// Whether the system draws the window's controls and resizes it at its
/// edges, which it does on macOS only.
const SYSTEM_FRAME: bool = cfg!(target_os = "macos");

/// The title bar's frame: its fill and its margins. [`title_bar_left_pad`]
/// counts on this frame's left margin.
pub fn title_bar_frame(palette: &Palette) -> Frame {
    Frame::NONE.fill(palette.title_bar).inner_margin(Margin {
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
/// turns its hover red, for the close button. The icon is painted rather
/// than added as a widget, so the control gives screen readers and the
/// tests `label` as its name itself.
fn window_control(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: (&str, char),
    label: &str,
    danger: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(WINDOW_CONTROL_WIDTH, TITLE_BAR_HEIGHT), Sense::click());
    let hovered = response.hovered();
    let colour = match (hovered, danger) {
        (true, true) => {
            ui.painter().rect_filled(rect, 0.0, palette.danger);
            palette.on_danger
        }
        (true, false) => {
            ui.painter().rect_filled(rect, 0.0, palette.hover_overlay);
            palette.window_ctrl
        }
        (false, _) => palette.window_ctrl,
    };
    let (glyph, font) = icons::glyph(icon.0, 14.0, icon.1);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, glyph, font, colour);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(label)
}

/// Strips along the window's edges and corners that resize it, above
/// everything else in the window. They add nothing while the window is
/// maximized or full screen, nor on macOS, where the system's frame resizes
/// it.
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
    // the first pixel. The strips are above everything else, so nothing
    // under them needs the press.
    let pressed = ctx.input(|i| i.pointer.primary_pressed());
    egui::Area::new(egui::Id::new("window_resize_handles"))
        .order(egui::Order::Foreground)
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
    use egui::{Event, Modifiers, PointerButton, Pos2, ViewportId};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

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
}
