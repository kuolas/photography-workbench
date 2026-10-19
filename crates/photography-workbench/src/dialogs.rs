// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's dialogs, each shown while its flag on the app is set, in the
//! frame every dialog shares ([`desktop_ui::dialog`]).

use desktop_ui::about::{self, AboutInfo};
use desktop_ui::keys;
use desktop_ui::theme::{self, Theme};
use desktop_ui::{dialog, drop_down};

use crate::PhotographyWorkbenchApp as App;
use crate::info::{SizeUnits, TimeForm};

/// The smallest a dialog of settings is wide, in points.
const SETTINGS_WIDTH: f32 = 600.0;

/// The Settings dialog: the theme choice, `theme`, how capture times are
/// written, `times`, and how file sizes are counted, `sizes`, each taking
/// effect as it is picked. Returns whether the dialog is to close.
pub fn settings(
    ctx: &egui::Context,
    theme: &mut Option<Theme>,
    times: &mut TimeForm,
    sizes: &mut SizeUnits,
) -> bool {
    dialog::show(ctx, "settings", |ui| {
        ui.set_min_width(SETTINGS_WIDTH);
        ui.heading("Settings");
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);
        let label = ui.label("Theme");
        theme::picker(ui, "theme", theme).labelled_by(label.id);
        let label = ui.label("Capture time");
        drop_down::show(ui, "capture_time", times, TimeForm::ALL, TimeForm::name)
            .labelled_by(label.id);
        let label = ui.label("File sizes");
        drop_down::show(ui, "file_sizes", sizes, SizeUnits::ALL, SizeUnits::name)
            .labelled_by(label.id);
    })
    .should_close()
}

/// The width of the reference of keyboard shortcuts, in points, which its
/// descriptions wrap in.
const SHORTCUTS_WIDTH: f32 = 480.0;

/// The tallest the reference's list grows before it scrolls, in points.
const SHORTCUTS_HEIGHT: f32 = 460.0;

/// The reference of keyboard shortcuts, `sections`. Returns whether the
/// dialog is to close.
pub fn shortcuts(ctx: &egui::Context, sections: &[keys::Section<'_>]) -> bool {
    dialog::show(ctx, "shortcuts", |ui| {
        ui.set_width(SHORTCUTS_WIDTH);
        ui.heading("Keyboard shortcuts");
        ui.add_space(2.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(SHORTCUTS_HEIGHT)
            .show(ui, |ui| keys::reference(ui, sections));
    })
    .should_close()
}

/// The About dialog, with `icon` as the app's icon. Returns whether the
/// dialog is to close.
pub fn about(ctx: &egui::Context, icon: &egui::TextureHandle) -> bool {
    about::show(
        ctx,
        &AboutInfo {
            icon,
            name: App::NAME,
            tagline: App::TAGLINE,
            version: env!("CARGO_PKG_VERSION"),
            copyright: App::COPYRIGHT,
            licence: App::LICENCE,
            url: env!("CARGO_PKG_REPOSITORY"),
        },
    )
}
