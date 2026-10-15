// SPDX-License-Identifier: GPL-3.0-or-later

//! The About dialog: the app's icon above its name, what it is for, its
//! version, copyright and licence, and a link to its home page, centred in
//! the frame every dialog shares ([`crate::dialog`]).
//!
//! What it says arrives as an [`AboutInfo`], so this crate names no app.

/// What the About dialog shows.
pub struct AboutInfo<'a> {
    /// The app's icon, shown 96 points square.
    pub icon: &'a egui::TextureHandle,
    /// The app's name.
    pub name: &'a str,
    /// What the app is for, in a line.
    pub tagline: &'a str,
    /// The version, such as `"0.1.0"`.
    pub version: &'a str,
    /// The copyright notice.
    pub copyright: &'a str,
    /// The licence, and that there is no warranty.
    pub licence: &'a str,
    /// The app's home page.
    pub url: &'a str,
}

/// The width of the About dialog, in points.
const WIDTH: f32 = 340.0;

/// The side of the icon, in points.
const ICON_SIDE: f32 = 96.0;

/// Show the About dialog. Returns whether it is to close.
pub fn show(ctx: &egui::Context, info: &AboutInfo<'_>) -> bool {
    crate::dialog::show(ctx, "about", |ui| {
        ui.set_width(WIDTH);
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            let icon = egui::Image::new(info.icon).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE));
            ui.add(icon);
            ui.add_space(14.0);
            ui.heading(info.name);
            ui.add_space(4.0);
            ui.label(info.tagline);
            ui.add_space(10.0);
            ui.label(format!("Version {}", info.version));
            ui.add_space(6.0);
            ui.label(info.copyright);
            ui.add_space(6.0);
            ui.label(info.licence);
            ui.add_space(8.0);
            ui.hyperlink(info.url);
            ui.add_space(4.0);
        });
    })
    .should_close()
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    #[test]
    fn the_dialog_shows_every_line_and_links_the_home_page() {
        let mut icon = None;
        let mut harness = Harness::builder().build_ui(move |ui| {
            let icon = icon.get_or_insert_with(|| {
                let image = egui::ColorImage::filled([1, 1], egui::Color32::WHITE);
                ui.ctx()
                    .load_texture("icon", image, egui::TextureOptions::LINEAR)
            });
            show(
                ui.ctx(),
                &AboutInfo {
                    icon,
                    name: "Example",
                    tagline: "Does the thing.",
                    version: "1.2.3",
                    copyright: "Copyright (C) Someone",
                    licence: "Free, with no warranty.",
                    url: "https://example.invalid",
                },
            );
        });
        for line in [
            "Example",
            "Does the thing.",
            "Version 1.2.3",
            "Copyright (C) Someone",
            "Free, with no warranty.",
        ] {
            harness.get_by_label(line);
        }
        harness.get_by_label("https://example.invalid").click();
        harness.step();
        let opened: Vec<_> = harness
            .output()
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(opened, ["https://example.invalid"]);
    }
}
