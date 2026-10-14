// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! menus.

use desktop_ui::chrome::{self, TITLE_BAR_HEIGHT};
use desktop_ui::icons;
use desktop_ui::theme::{self, Theme};

/// The [`eframe::Storage`] key the theme choice is saved under, as the
/// theme's [`Theme::name`], or `"System"` for none. The display name is the
/// saved value, so renaming a theme resets users who chose it to System.
const THEME_KEY: &str = "theme";

/// The application state, owned by eframe for the life of the window.
#[derive(Default)]
pub struct PhotographyWorkbenchApp {
    /// The theme chosen in View ▸ Theme, `None` for System, saved between runs.
    theme: Option<Theme>,
    /// The concrete theme last installed on the context: [`Self::theme`]
    /// resolved against the OS preference. `None` until the first install.
    resolved_theme: Option<Theme>,
}

impl PhotographyWorkbenchApp {
    /// The name shown in the title bar.
    pub const NAME: &'static str = "Photography Workbench";

    /// The smallest the window can be, in points, and its size on the first
    /// run: a handheld's 1280 × 800 screen, the smallest a photographer
    /// culls on. Every layout is designed and tested at this size (ADR010).
    pub const MIN_WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

    /// Called once, before the first frame. Installs the icon font and
    /// restores the saved theme choice; with none saved, or an unreadable
    /// one, takes the default.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        icons::install(&cc.egui_ctx);
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|name| Theme::ALL.into_iter().find(|t| t.name() == name));
        let mut app = Self {
            theme,
            ..Self::default()
        };
        app.reconcile_theme(&cc.egui_ctx);
        app
    }

    /// The width and height of [`Self::icon`], in pixels.
    pub const ICON_SIZE: u32 = if cfg!(target_os = "macos") { 1024 } else { 256 };

    /// The window icon, rendered from `assets/icon.svg`.
    ///
    /// macOS shows it in the Dock in place of the app bundle's icon while the
    /// app runs, so it gets the full-size image there.
    pub fn icon() -> egui::IconData {
        #[cfg(target_os = "macos")]
        let png = include_bytes!("../assets/icon-1024.png");
        #[cfg(not(target_os = "macos"))]
        let png = include_bytes!("../assets/icon-256.png");
        eframe::icon_data::from_png_bytes(png).expect("the icons in assets/ are valid PNGs")
    }

    /// Bring egui in line with the theme choice. Sets egui's theme
    /// preference to the choice's (see [`theme::egui_preference`]) every
    /// frame. Resolves the choice against the OS light/dark
    /// setting and, if the concrete theme changed (a new choice, the OS
    /// flipping while on `System`, or the first frame), reinstalls egui's
    /// visuals. Returns whether it reinstalled them. Cheap to call every
    /// frame.
    fn reconcile_theme(&mut self, ctx: &egui::Context) -> bool {
        // egui sends the window a new appearance only when the preference
        // changes, so setting it every frame is cheap; it also undoes
        // anything else that changed it (eframe restoring a saved one).
        ctx.set_theme(theme::egui_preference(self.theme));
        let want = theme::resolve(self.theme, ctx);
        if self.resolved_theme == Some(want) {
            return false;
        }
        self.resolved_theme = Some(want);
        want.apply(ctx);
        true
    }

    /// The title bar's contents: the menus at the left, past the space
    /// macOS's window buttons take, and the window controls at the right.
    /// The whole bar drags the window, so it is made a drag region before
    /// anything is put on it.
    fn title_bar(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        let bar = ui.max_rect();
        chrome::title_bar_drag_region(ui, bar);

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                chrome::window_controls(ui, palette);
            },
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.add_space(chrome::title_bar_left_pad(ui.ctx(), None));
                self.menus(ui);
            },
        );
    }

    /// The File and View menus.
    fn menus(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                ui.menu_button("Theme", |ui| {
                    // The click itself makes egui run another frame,
                    // whose top installs the new choice.
                    ui.selectable_value(&mut self.theme, None, "System");
                    for t in Theme::ALL {
                        ui.selectable_value(&mut self.theme, Some(t), t.name());
                    }
                });
            });
        });
    }
}

impl eframe::App for PhotographyWorkbenchApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The theme is installed only here, at the top of the frame, never
        // part-way through it, so a frame is painted in one theme. The root
        // `Ui` took its style from the context before this runs, and every
        // panel inherits it, so a newly installed theme is also put on the
        // root, to paint in this frame rather than the next.
        if self.reconcile_theme(ui.ctx()) {
            ui.reset_style();
        }

        let palette = theme::resolve(self.theme, ui.ctx()).palette();
        egui::Panel::top("title_bar")
            .exact_size(TITLE_BAR_HEIGHT)
            .show_separator_line(true)
            .frame(chrome::title_bar_frame(&palette))
            .show(ui, |ui| self.title_bar(ui, &palette));

        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("No folder open");
            });
        });

        // Last, so the strips are above everything else.
        chrome::window_resize_handles(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(
            THEME_KEY,
            self.theme.map_or("System", Theme::name).to_owned(),
        );
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::icons;

    use super::PhotographyWorkbenchApp;

    /// The app, run headless in a window of the smallest size it allows.
    fn harness() -> Harness<'static, PhotographyWorkbenchApp> {
        Harness::builder()
            .with_size(PhotographyWorkbenchApp::MIN_WINDOW)
            .build_eframe(|cc| PhotographyWorkbenchApp::new(cc))
    }

    #[test]
    fn first_frame_shows_menu_and_empty_state() {
        let harness = harness();
        harness.get_by_label("File");
        harness.get_by_label("No folder open");
    }

    #[test]
    fn the_title_bar_holds_the_menus_and_the_window_controls() {
        let harness = harness();
        for menu in ["File", "View"] {
            let rect = harness.get_by_label(menu).rect();
            assert!(rect.bottom() <= TITLE_BAR_HEIGHT, "{menu} at {rect:?}");
            // On macOS, clear of the system's window buttons.
            if cfg!(target_os = "macos") {
                assert!(
                    rect.left() >= MAC_WINDOW_BUTTONS_WIDTH,
                    "{menu} at {rect:?}"
                );
            }
        }
        let close = harness.query_by_label("Close");
        assert_eq!(close.is_some(), !cfg!(target_os = "macos"));
    }

    #[test]
    fn the_icon_font_is_installed_at_start() {
        let harness = harness();
        assert!(icons::resolves("x"));
        let (glyph, font) = icons::glyph("x", 14.0, '?');
        assert!(harness.ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)));
    }

    #[test]
    fn icon_decodes_to_a_square_of_icon_size() {
        let size = PhotographyWorkbenchApp::ICON_SIZE;
        let icon = PhotographyWorkbenchApp::icon();
        assert_eq!((icon.width, icon.height), (size, size));
        assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
    }
}
