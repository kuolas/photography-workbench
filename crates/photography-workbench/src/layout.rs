// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of the window around a mode's centre: the tool rail along its
//! right edge, and the docks, each one there when the mode's [`Layout`]
//! has it.
//!
//! The left dock is where photos are browsed from: it will hold the
//! folder tree, and is empty until then. The tool rail picks what the
//! right dock shows, in Library one thing so far, the photo's
//! information.

use desktop_ui::action_bar::{self, Edge};
use desktop_ui::dock;
use desktop_ui::theme::Palette;

use crate::mode::Layout;

/// The width the left dock opens at the first time, and the widths it can
/// be dragged to. With both side docks at their widest the window's centre
/// keeps more than 300 points at the smallest the window can be.
const LEFT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (268.0, 200.0..=380.0);

/// The width the right dock opens at the first time, and the widths it can
/// be dragged to.
const RIGHT_DOCK: (f32, std::ops::RangeInclusive<f32>) = (270.0, 250.0..=460.0);

/// What the right dock can show: the tool rail's button for it.
const INFO: ((&str, char), &str) = (("info", 'ℹ'), "Info");

/// Which docks are shown. A dock the mode does not have is never shown,
/// whatever it says here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Docks {
    /// Whether the left dock is shown.
    pub left: bool,
    /// Whether the right dock is shown.
    pub right: bool,
}

impl Default for Docks {
    /// Every dock shown.
    fn default() -> Self {
        Self {
            left: true,
            right: true,
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
