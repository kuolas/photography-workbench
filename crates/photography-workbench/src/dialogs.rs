// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's dialogs, each shown while its flag on the app is set, in the
//! frame every dialog shares ([`desktop_ui::dialog`]).

use desktop_ui::about::{self, AboutInfo};
use desktop_ui::dialog;
use desktop_ui::theme::{self, Theme};

use crate::PhotographyWorkbenchApp as App;

/// The smallest a dialog of settings is wide, in points.
const SETTINGS_WIDTH: f32 = 600.0;

/// The Settings dialog: the theme choice, `theme`, which takes effect as it
/// is picked. Returns whether the dialog is to close.
pub fn settings(ctx: &egui::Context, theme: &mut Option<Theme>) -> bool {
    dialog::show(ctx, "settings", |ui| {
        ui.set_min_width(SETTINGS_WIDTH);
        ui.heading("Settings");
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);
        let label = ui.label("Theme");
        theme::picker(ui, "theme", theme).labelled_by(label.id);
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
