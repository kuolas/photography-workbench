// SPDX-License-Identifier: GPL-3.0-or-later

//! Drop-down lists: a choice among a few values, each shown by its name,
//! all of one width so that a column of them, as in a dialog of settings,
//! lines up.

/// The width of a drop-down list, in points.
const WIDTH: f32 = 190.0;

/// A drop-down list of `choices`, in their order, each shown as `name`
/// gives it, with `choice`'s name on the closed list, cut short with "…"
/// where it is too long for the list's width. Picking one sets `choice`.
pub fn show<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id_salt: &str,
    choice: &mut T,
    choices: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) -> egui::Response {
    // egui takes the width as the least the list may have, and cuts a
    // name short at the room there is, so the list is given no more room
    // than its width.
    ui.scope(|ui| {
        ui.set_max_width(WIDTH);
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(name(*choice))
            .width(WIDTH)
            .truncate()
            .show_ui(ui, |ui| {
                for value in choices {
                    ui.selectable_value(choice, value, name(value));
                }
            })
            .response
    })
    .inner
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use super::{WIDTH, show};

    fn name(number: u8) -> &'static str {
        ["One", "Two", "Three"][usize::from(number) - 1]
    }

    /// The closed list shows the choice; the open one every value, and a
    /// click on one makes it the choice.
    #[test]
    fn a_drop_down_offers_every_choice_and_sets_the_one_picked() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, choice: &mut u8| {
                let label = ui.label("Number");
                show(ui, "numbers", choice, [1, 2, 3], name).labelled_by(label.id);
            },
            1,
        );
        let shown = |harness: &Harness<'_, u8>| {
            let list = harness.get_by_role(Role::ComboBox);
            list.accesskit_node().value()
        };
        assert_eq!(shown(&harness).as_deref(), Some("One"));
        assert!(harness.query_by_label("Three").is_none());
        harness.get_by_role(Role::ComboBox).click();
        harness.run();
        harness.get_by_label("Two");
        harness.get_by_label("Three").click();
        harness.run();
        assert_eq!(*harness.state(), 3);
        assert_eq!(shown(&harness).as_deref(), Some("Three"));
    }

    /// Every drop-down is as wide, whatever its names: one whose choice's
    /// name is far longer than the list is wide has it cut short.
    #[test]
    fn a_drop_down_is_as_wide_whatever_its_names() {
        let long = |_| "A name far longer than any drop-down list is wide, cut short to fit";
        let mut harness = Harness::builder().build_ui_state(
            |ui, choice: &mut u8| {
                let label = ui.label("Number");
                show(ui, "numbers", choice, [1, 2, 3], name).labelled_by(label.id);
                let label = ui.label("Long");
                show(ui, "long", choice, [1], long).labelled_by(label.id);
            },
            1,
        );
        harness.run();
        let widths: Vec<f32> = harness
            .query_all_by_role(Role::ComboBox)
            .map(|list| list.rect().width())
            .collect();
        assert_eq!(widths.len(), 2);
        for width in widths {
            assert!((width - WIDTH).abs() <= 1.0, "{width}");
        }
    }
}
