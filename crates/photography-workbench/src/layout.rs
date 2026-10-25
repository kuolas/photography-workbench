// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of the window around a mode's centre: the tool rail along its
//! right edge, and the docks, each one there when the mode's [`Layout`]
//! has it.
//!
//! The left dock is where photos are browsed from: it holds the folder
//! tree, which the app draws in it. The tool rail picks what the right
//! dock shows, in Library one thing so far, the photo's information,
//! which the app draws in it. The bottom dock is the filmstrip's, which
//! only the viewer has: the grid shows the photos as thumbnails already.
//! The app draws the filmstrip in it, on the viewer's backdrop.
//!
//! The title bar holds the toggles for the side docks, and the toolbar,
//! beside the zoom, the filmstrip's.

use desktop_ui::action_bar::{self, Edge};
use desktop_ui::chrome;
use desktop_ui::dock::{self, Side};
use desktop_ui::theme::Palette;
use desktop_ui::toolbar;

use crate::keymap::{self, Action, Surface};
use crate::mode::Layout;

/// The width the left dock opens at the first time, and the widths it can
/// be dragged to. With both side docks at their widest the window's centre
/// keeps more than 300 points at the smallest the window can be.
const LEFT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (268.0, 200.0..=380.0);

/// The width the right dock opens at the first time, and the widths it can
/// be dragged to. With both side docks at their widest, at the smallest the
/// window can be, the toolbar between them still holds its controls: it is
/// about 440 points wide then.
const RIGHT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (270.0, 250.0..=410.0);

/// The height of the bottom dock, which holds the filmstrip: fixed, as
/// the filmstrip's cells are.
const FILMSTRIP_HEIGHT: f32 = 142.0;

/// What the right dock can show: the tool rail's button for it.
const INFO: ((&str, char), &str) = (("info", 'ℹ'), "Info");

/// The toolbar's toggle for the filmstrip: its Lucide icon, with the
/// character shown in its place if the name does not resolve, and its
/// name, which screen readers and the tests know it by.
const FILMSTRIP_TOGGLE: ((&str, char), &str) =
    (("panel-bottom", '▁'), "Show or hide the filmstrip");

/// Which docks are shown, saved between runs. A dock the mode does not
/// have is never shown, whatever it says here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Docks {
    /// Whether the left dock is shown.
    pub left: bool,
    /// Whether the right dock is shown.
    pub right: bool,
    /// Whether the bottom dock is shown.
    pub bottom: bool,
}

impl Default for Docks {
    /// Every dock shown.
    fn default() -> Self {
        Self {
            left: true,
            right: true,
            bottom: true,
        }
    }
}

impl Docks {
    /// The [`eframe::Storage`] keys each dock's state is saved under, as
    /// `"true"` or `"false"`: the left, right and bottom docks'.
    const KEYS: [&str; 3] = ["show_left_dock", "show_right_dock", "show_bottom_dock"];

    /// The docks' states saved in `storage`. A dock with none saved, or an
    /// unreadable one, is shown.
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let shown = |key| {
            storage
                .and_then(|storage| storage.get_string(key))
                .is_none_or(|value| value != "false")
        };
        let [left, right, bottom] = Self::KEYS;
        Self {
            left: shown(left),
            right: shown(right),
            bottom: shown(bottom),
        }
    }

    /// Hide the docks on `sides` that `layout` has, if any of them is
    /// shown, or else show them all, so one key shows or hides them
    /// together. A dock `layout` does not have is left as it is.
    pub fn toggle(&mut self, layout: Layout, sides: &[Side]) {
        let any_shown = sides
            .iter()
            .any(|&side| layout.has_dock(side) && self.shown(side));
        for &side in sides {
            if layout.has_dock(side) {
                *self.shown_mut(side) = !any_shown;
            }
        }
    }

    /// Whether the dock on `side` is shown.
    pub fn shown(self, side: Side) -> bool {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
            Side::Bottom => self.bottom,
        }
    }

    /// Whether the dock on `side` is shown, to read or change.
    fn shown_mut(&mut self, side: Side) -> &mut bool {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
            Side::Bottom => &mut self.bottom,
        }
    }

    /// Save the docks' states in `storage`.
    pub fn save(self, storage: &mut dyn eframe::Storage) {
        let states = [self.left, self.right, self.bottom];
        for (key, shown) in Self::KEYS.into_iter().zip(states) {
            storage.set_string(key, shown.to_string());
        }
    }
}

/// The title bar's toggles for the side docks `layout` has, in a
/// right-to-left layout, so they read left, right, as the docks lie, each
/// showing whether its dock is shown in `docks`. Each names in its tooltip
/// the key combination that shows or hides its dock on `surface`. The
/// filmstrip's toggle is on the toolbar ([`filmstrip_toggle`]). The side of
/// the dock whose toggle was clicked, if any.
pub fn dock_toggles(
    ui: &mut egui::Ui,
    palette: &Palette,
    layout: Layout,
    surface: Surface,
    docks: Docks,
) -> Option<Side> {
    let toggles = [
        (
            layout.right_dock,
            Side::Right,
            docks.right,
            "Show or hide the right dock",
        ),
        (
            layout.left_dock,
            Side::Left,
            docks.left,
            "Show or hide the left dock",
        ),
    ];
    let mut clicked = None;
    for (has, side, shown, label) in toggles {
        let combination = keymap::key_combination_for(surface, Action::Dock(side));
        if has && chrome::dock_toggle(ui, palette, side, shown, label, combination).clicked() {
            clicked = Some(side);
        }
    }
    clicked
}

/// The toolbar's toggle for the filmstrip, in a right-to-left layout, shown
/// selected while the filmstrip is `shown`. It names in its tooltip the key
/// combination that shows or hides the filmstrip on `surface`. Whether it
/// was clicked.
pub fn filmstrip_toggle(
    ui: &mut egui::Ui,
    palette: &Palette,
    surface: Surface,
    shown: bool,
) -> bool {
    let combination = keymap::key_combination_for(surface, Action::Dock(Side::Bottom));
    let (icon, label) = FILMSTRIP_TOGGLE;
    // An id of its own, as what comes before it on the toolbar comes and
    // goes, and the ids egui gives by position would shift with it.
    let scope = egui::UiBuilder::new()
        .id(egui::Id::new("filmstrip_toggle"))
        .layout(egui::Layout::right_to_left(egui::Align::Center));
    ui.scope_builder(scope, |ui| {
        toolbar::button(ui, palette, icon, label, shown, combination).clicked()
    })
    .inner
}

/// The tool rail. A button is shown selected while the right dock shows
/// what it picks; a click on the one that is on hides the dock, and a
/// click on a button while the dock is hidden shows the dock with what
/// the button picks. Whether the Info button was clicked.
pub fn tool_rail(ui: &mut egui::Ui, palette: &Palette, docks: Docks) -> bool {
    let mut clicked = false;
    let info = |ui: &mut egui::Ui| {
        let (icon, label) = INFO;
        // No key combination in its tooltip: F8 shows or hides the right
        // dock whatever it shows, where the button picks what it shows.
        clicked = action_bar::button(ui, palette, icon, label, docks.right).clicked();
    };
    let bar = ("tool_rail", Edge::Right);
    action_bar::show(ui, bar, palette, info, |_| {});
    clicked
}

/// The side docks `layout` has, each shown or hidden as `docks` says,
/// `left` adding what the left dock holds and `right` what the right
/// dock shows. They come before anything along the bottom, so they run
/// down to the window's bottom edge.
pub fn side_docks(
    ui: &mut egui::Ui,
    layout: Layout,
    docks: &mut Docks,
    left: impl FnOnce(&mut egui::Ui),
    right: impl FnOnce(&mut egui::Ui),
) {
    if layout.left_dock {
        let (width, widths) = LEFT_DOCK;
        let panel = egui::Panel::left("left_dock")
            .default_size(width)
            .size_range(widths);
        dock::show(panel, ui, &mut docks.left, left);
    }
    if layout.right_dock {
        let (width, widths) = RIGHT_DOCK;
        let panel = egui::Panel::right("right_dock")
            .default_size(width)
            .size_range(widths);
        dock::show(panel, ui, &mut docks.right, right);
    }
}

/// The bottom dock, if `layout` has it, shown or hidden as `docks` says,
/// `filmstrip` adding what it holds on the viewer's backdrop, so it reads
/// as part of the viewer above it. It comes after the side docks, so it
/// runs between them, under the centre only. Without it, the widgets
/// after it keep the ids they have with it, as they do when it is only
/// hidden.
pub fn bottom_dock(
    ui: &mut egui::Ui,
    palette: &Palette,
    layout: Layout,
    docks: &mut Docks,
    filmstrip: impl FnOnce(&mut egui::Ui),
) {
    if layout.bottom_dock {
        let panel = egui::Panel::bottom("bottom_dock")
            .resizable(false)
            .exact_size(FILMSTRIP_HEIGHT);
        dock::show(panel, ui, &mut docks.bottom, |ui| {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, palette.viewer_backdrop);
            filmstrip(ui);
        });
    } else {
        ui.skip_ahead_auto_ids(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{FILMSTRIP_TOGGLE, INFO};

    #[test]
    fn every_icon_resolves() {
        for ((name, _), label) in [INFO, FILMSTRIP_TOGGLE] {
            assert!(desktop_ui::icons::resolves(name), "{label}: {name}");
        }
    }
}
