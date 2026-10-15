// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of the window around a mode's centre: the tool rail along its
//! right edge, and the docks, each one there when the mode's [`Layout`]
//! has it.
//!
//! The left dock is where photos are browsed from: it will hold the
//! folder tree, and is empty until then. The tool rail picks what the
//! right dock shows, in Library one thing so far, the photo's
//! information. The bottom dock is the filmstrip's, which only the viewer
//! has: the grid shows the photos as thumbnails already. It is empty so
//! far.

use desktop_ui::action_bar::{self, Edge};
use desktop_ui::chrome;
use desktop_ui::dock::{self, Side};
use desktop_ui::theme::Palette;

use crate::mode::Layout;

/// The width the left dock opens at the first time, and the widths it can
/// be dragged to. With both side docks at their widest the window's centre
/// keeps more than 300 points at the smallest the window can be.
const LEFT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (268.0, 200.0..=380.0);

/// The width the right dock opens at the first time, and the widths it can
/// be dragged to.
const RIGHT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (270.0, 250.0..=460.0);

/// The height of the bottom dock, which holds the filmstrip: fixed, as
/// the filmstrip's cells are.
const FILMSTRIP_HEIGHT: f32 = 142.0;

/// What the right dock can show: the tool rail's button for it.
const INFO: ((&str, char), &str) = (("info", 'ℹ'), "Info");

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
        let has = |side| match side {
            Side::Left => layout.left_dock,
            Side::Right => layout.right_dock,
            Side::Bottom => layout.bottom_dock,
        };
        let any_shown = sides.iter().any(|&side| has(side) && *self.shown_mut(side));
        for &side in sides {
            if has(side) {
                *self.shown_mut(side) = !any_shown;
            }
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

/// The title bar's toggles for the docks `layout` has, in a right-to-left
/// layout, so they read left, bottom, right, as the docks lie.
pub fn dock_toggles(ui: &mut egui::Ui, palette: &Palette, layout: Layout, docks: &mut Docks) {
    let toggles = [
        (
            layout.right_dock,
            Side::Right,
            &mut docks.right,
            "Show or hide the right dock",
        ),
        (
            layout.bottom_dock,
            Side::Bottom,
            &mut docks.bottom,
            "Show or hide the filmstrip",
        ),
        (
            layout.left_dock,
            Side::Left,
            &mut docks.left,
            "Show or hide the left dock",
        ),
    ];
    for (has, side, shown, label) in toggles {
        if has && chrome::dock_toggle(ui, palette, side, *shown, label, None).clicked() {
            *shown = !*shown;
        }
    }
}

/// The tool rail. A button is shown selected while the right dock shows
/// what it picks, and a click on it shows the right dock if it is hidden,
/// as what it picks has nowhere else to show.
pub fn tool_rail(ui: &mut egui::Ui, palette: &Palette, docks: &mut Docks) {
    let info = |ui: &mut egui::Ui| {
        let (icon, label) = INFO;
        if action_bar::button(ui, palette, icon, label, docks.right).clicked() {
            docks.right = true;
        }
    };
    let bar = ("tool_rail", Edge::Right);
    action_bar::show(ui, bar, palette, info, |_| {});
}

/// The side docks `layout` has, each shown or hidden as `docks` says.
/// They come before anything along the bottom, so they run down to the
/// window's bottom edge.
pub fn side_docks(ui: &mut egui::Ui, layout: Layout, docks: &mut Docks) {
    if layout.left_dock {
        let (width, widths) = LEFT_DOCK;
        let panel = egui::Panel::left("left_dock")
            .default_size(width)
            .size_range(widths);
        dock::show(panel, ui, &mut docks.left, |_| {});
    }
    if layout.right_dock {
        let (width, widths) = RIGHT_DOCK;
        let panel = egui::Panel::right("right_dock")
            .default_size(width)
            .size_range(widths);
        dock::show(panel, ui, &mut docks.right, info);
    }
}

/// The bottom dock, if `layout` has it, shown or hidden as `docks` says.
/// It comes after the side docks, so it runs between them, under the
/// centre only. Without it, the widgets after it keep the ids they have
/// with it, as they do when it is only hidden.
pub fn bottom_dock(ui: &mut egui::Ui, palette: &Palette, layout: Layout, docks: &mut Docks) {
    if layout.bottom_dock {
        let panel = egui::Panel::bottom("bottom_dock")
            .resizable(false)
            .exact_size(FILMSTRIP_HEIGHT);
        dock::show(panel, ui, &mut docks.bottom, |ui| filmstrip(ui, palette));
    } else {
        ui.skip_ahead_auto_ids(1);
    }
}

/// The bottom dock: the filmstrip, on the viewer's backdrop, so it reads
/// as part of the viewer above it.
fn filmstrip(ui: &mut egui::Ui, palette: &Palette) {
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, palette.viewer_backdrop);
}

/// The right dock showing Info: the selected photo's information, or a
/// message saying no photo is selected.
fn info(ui: &mut egui::Ui) {
    dock::empty_message(
        ui,
        "No photo selected",
        "Select a photo to see its information",
    );
}

#[cfg(test)]
mod tests {
    use super::INFO;

    #[test]
    fn every_icon_resolves() {
        let ((name, _), label) = INFO;
        assert!(desktop_ui::icons::resolves(name), "{label}: {name}");
    }
}
