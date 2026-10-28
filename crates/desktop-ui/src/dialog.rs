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
///
/// `name` is the dialog's own: its id is made from the name alone, not
/// from where it is shown, so no other dialog of the window may share it.
pub fn show<R>(
    ctx: &egui::Context,
    name: impl std::hash::Hash + std::fmt::Debug,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    // egui puts a modal dialog in its foreground order, where drop-down
    // lists and menus open too, and of two layers in one order draws the
    // one last brought to the top above. A click that opens a list in a
    // dialog brings both the dialog it lands on and the new list to the
    // top; when the press and the release come in one frame, egui keeps
    // their earlier order, and a list opened in an earlier showing of the
    // dialog is drawn under it. In the middle order, where egui's windows
    // are, a dialog is always under its lists.
    let id = egui::Id::unique(name);
    egui::Modal::new(id)
        .area(egui::Modal::default_area(id).order(egui::Order::Middle))
        .frame(dialog_frame(ctx))
        .show(ctx, add_contents)
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
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

    /// A window with a dialog holding a drop-down list of two choices,
    /// shown while the dialog is open: whether it is open, and the choice.
    fn window_with_drop_down() -> Harness<'static, (bool, u8)> {
        Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui_state(
                |ui, (open, choice): &mut (bool, u8)| {
                    if !*open {
                        return;
                    }
                    let dialog = show(ui.ctx(), "dialog", |ui| {
                        let label = ui.label("Choice");
                        egui::ComboBox::from_id_salt("choice")
                            .selected_text(format!("Choice {choice}"))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(choice, 1, "One");
                                ui.selectable_value(choice, 2, "Two");
                            })
                            .response
                            .labelled_by(label.id);
                    });
                    if dialog.should_close() {
                        *open = false;
                    }
                },
                (true, 1),
            )
    }

    /// Click at `pos` as a quick click or a tap on a trackpad reaches the
    /// window: the pointer comes to rest there, then the press and the
    /// release arrive in one frame.
    fn quick_click<State>(harness: &mut Harness<'_, State>, pos: egui::Pos2) {
        harness.input_mut().events.push(Event::PointerMoved(pos));
        harness.run();
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        harness
            .input_mut()
            .events
            .extend([button(true), button(false)]);
        harness.run();
    }

    /// A dialog's drop-down list opens above the dialog, every time the
    /// dialog is opened, and also when the list was opened before.
    #[test]
    fn a_drop_down_list_opens_above_its_dialog_every_time() {
        let mut harness = window_with_drop_down();
        for (opening, pick) in [(1, "Two"), (2, "One")] {
            let list = harness.get_by_role(Role::ComboBox).rect().center();
            quick_click(&mut harness, list);
            let choice = harness.get_by_label(pick).rect().center();
            let on_top = harness.ctx.layer_id_at(choice);
            assert!(
                on_top.is_some_and(|layer| layer.id != egui::Id::unique("dialog")),
                "opening {opening}: the list is under the dialog",
            );
            quick_click(&mut harness, choice);
            harness.key_press(Key::Escape);
            harness.run();
            assert!(!harness.state().0);
            harness.state_mut().0 = true;
            harness.run();
        }
    }
}
