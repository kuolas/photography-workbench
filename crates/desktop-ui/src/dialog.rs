// SPDX-License-Identifier: GPL-3.0-or-later

//! Dialogs: one frame for every dialog the window opens, so they read as
//! one family.
//!
//! A dialog sits over the window, which it dims, and closes as egui's
//! dialogs do: on Esc, or on a click outside it. A dialog never acts when
//! it closes, so closing one is always safe; one that changes something
//! applies each change as it is made.

/// The room between a dialog's edge and its contents, on every side, in
/// points: egui's own dialog frame hugs its contents.
const DIALOG_PADDING: f32 = 16.0;

/// A dialog's frame: the current theme's popup frame (its fill, edge and
/// shadow), with 16 points of room inside.
pub fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::popup(&ctx.style_of(ctx.theme())).inner_margin(DIALOG_PADDING)
}

/// Show a dialog in [`dialog_frame`], with `add_contents` inside it. Ask
/// the result's `should_close()` whether to stop showing it.
pub fn show<R>(
    ctx: &egui::Context,
    id_salt: impl egui::AsId,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    egui::Modal::new(egui::Id::new(id_salt))
        .frame(dialog_frame(ctx))
        .show(ctx, add_contents)
}

#[cfg(test)]
mod tests {
    use egui::{Event, Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// A window with a dialog holding a label, shown while it is open, and
    /// whether the dialog is open.
    fn window_with_dialog() -> Harness<'static, bool> {
        Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui_state(
                |ui, open: &mut bool| {
                    if *open && show(ui.ctx(), "dialog", |ui| ui.label("Inside")).should_close() {
                        *open = false;
                    }
                },
                true,
            )
    }

    #[test]
    fn a_dialog_has_room_inside_its_edge() {
        let ctx = egui::Context::default();
        let margin = dialog_frame(&ctx).inner_margin;
        assert_eq!(margin, egui::Margin::same(16));
    }

    #[test]
    fn esc_closes_a_dialog() {
        let mut harness = window_with_dialog();
        harness.get_by_label("Inside");
        harness.key_press(Key::Escape);
        harness.run();
        assert!(!*harness.state());
        assert!(harness.query_by_label("Inside").is_none());
    }

    #[test]
    fn a_click_outside_closes_a_dialog_and_one_inside_does_not() {
        let mut harness = window_with_dialog();
        harness.get_by_label("Inside").click();
        harness.run();
        assert!(*harness.state());

        let corner = egui::pos2(5.0, 5.0);
        let button = |pressed| Event::PointerButton {
            pos: corner,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        harness.input_mut().events.extend([
            Event::PointerMoved(corner),
            button(true),
            button(false),
        ]);
        harness.run();
        assert!(!*harness.state());
    }
}
