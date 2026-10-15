// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! menus.

use desktop_ui::chrome::{self, TITLE_BAR_HEIGHT};
use desktop_ui::icons;
use desktop_ui::style;
use desktop_ui::theme::{self, Theme};

use crate::mode::Mode;

/// The [`eframe::Storage`] key the theme choice is saved under, as the
/// theme's [`Theme::name`], or `"System"` for none. The display name is the
/// saved value, so renaming a theme resets users who chose it to System.
const THEME_KEY: &str = "theme";

/// The [`eframe::Storage`] key the mode is saved under, as its
/// [`Mode::name`].
const MODE_KEY: &str = "mode";

/// The application state, owned by eframe for the life of the window.
#[derive(Default)]
pub struct PhotographyWorkbenchApp {
    /// The theme chosen in View ▸ Theme, `None` for System, saved between runs.
    theme: Option<Theme>,
    /// The concrete theme last installed on the context: [`Self::theme`]
    /// resolved against the OS preference. `None` until the first install.
    resolved_theme: Option<Theme>,
    /// The mode the window shows, saved between runs.
    mode: Mode,
    /// The width of macOS's window buttons with their margins, measured at
    /// start-up, for the title bar to start past. `None` on other systems,
    /// or when the window could not be measured.
    mac_window_buttons: Option<f32>,
}

impl PhotographyWorkbenchApp {
    /// The name shown in the title bar.
    pub const NAME: &'static str = "Photography Workbench";

    /// The smallest the window can be, in points, and its size on the first
    /// run: a handheld's 1280 × 800 screen, the smallest a photographer
    /// culls on. Every layout is designed and tested at this size (ADR010).
    pub const MIN_WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

    /// Called once, before the first frame. Installs the icon font and the
    /// style, and restores the saved theme choice and mode; with none
    /// saved, or an unreadable one, takes the default.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        icons::install(&cc.egui_ctx);
        style::install(&cc.egui_ctx);
        let saved = |key| cc.storage.and_then(|storage| storage.get_string(key));
        let theme =
            saved(THEME_KEY).and_then(|name| Theme::ALL.into_iter().find(|t| t.name() == name));
        let mode = saved(MODE_KEY)
            .and_then(|name| Mode::from_saved(&name))
            .unwrap_or_default();
        let mut app = Self {
            theme,
            mode,
            mac_window_buttons: measure_mac_window_buttons(cc),
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

    /// The title bar's contents: the mode switcher and the menus at the
    /// left, past the space macOS's window buttons take, and the window
    /// controls at the right.
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
                let pad = chrome::title_bar_left_pad(ui.ctx(), self.mac_window_buttons);
                ui.add_space(pad);
                // The menus take the rest of the bar's width, so they come
                // last.
                self.mode_switcher(ui);
                self.menus(ui);
            },
        );
    }

    /// A button per mode, the current one selected. A mode not built yet
    /// is shown disabled.
    fn mode_switcher(&mut self, ui: &mut egui::Ui) {
        for mode in Mode::ALL {
            let response = ui
                .add_enabled_ui(mode.is_available(), |ui| {
                    chrome::selectable_button(ui, mode.icon(), mode.name(), self.mode == mode)
                })
                .inner
                .on_disabled_hover_text("Coming in a later release");
            if response.clicked() {
                self.mode = mode;
            }
        }
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

/// The width of macOS's window buttons with their margins, as the system
/// lays them out, unaffected by egui's zoom. `None` on other systems, and
/// when there is no window to measure, as in the headless tests.
fn measure_mac_window_buttons(cc: &eframe::CreationContext<'_>) -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::HasWindowHandle as _;
        let handle = cc.window_handle().ok()?;
        eframe::WindowChromeMetrics::from_window_handle(&handle.as_raw())
            .map(|metrics| metrics.traffic_lights_size.x)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cc;
        None
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
        storage.set_string(MODE_KEY, self.mode.name().to_owned());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::{App as _, Storage as _};
    use egui::accesskit::Toggled;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::icons;

    use super::{MODE_KEY, Mode, PhotographyWorkbenchApp};

    /// Settings kept in memory, in place of eframe's file.
    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    /// The app, run headless in a window of the smallest size it allows.
    fn harness() -> Harness<'static, PhotographyWorkbenchApp> {
        Harness::builder()
            .with_size(PhotographyWorkbenchApp::MIN_WINDOW)
            .build_eframe(|cc| PhotographyWorkbenchApp::new(cc))
    }

    /// [`harness`], with the settings in `storage` saved by an earlier run.
    fn harness_from(storage: &MemoryStorage) -> Harness<'_, PhotographyWorkbenchApp> {
        Harness::builder()
            .with_size(PhotographyWorkbenchApp::MIN_WINDOW)
            .build_eframe(|cc| {
                cc.storage = Some(storage);
                PhotographyWorkbenchApp::new(cc)
            })
    }

    #[test]
    fn first_frame_shows_menu_and_empty_state() {
        let harness = harness();
        harness.get_by_label("File");
        harness.get_by_label("No folder open");
    }

    /// At the window's smallest, everything on the title bar fits on it,
    /// clear of the system's window buttons on macOS, and of the window
    /// controls elsewhere.
    #[test]
    fn the_title_bar_holds_the_modes_the_menus_and_the_window_controls() {
        let harness = harness();
        let mut last = egui::Rect::NOTHING;
        for name in ["Library", "Organizer", "Develop", "File", "View"] {
            let rect = harness.get_by_label(name).rect();
            assert!(rect.bottom() <= TITLE_BAR_HEIGHT, "{name} at {rect:?}");
            assert!(
                rect.right() <= PhotographyWorkbenchApp::MIN_WINDOW.x,
                "{name} at {rect:?}"
            );
            if cfg!(target_os = "macos") {
                assert!(
                    rect.left() >= MAC_WINDOW_BUTTONS_WIDTH,
                    "{name} at {rect:?}"
                );
            }
            last = rect;
        }
        let minimize = harness.query_by_label("Minimize");
        assert_eq!(minimize.is_some(), !cfg!(target_os = "macos"));
        if let Some(minimize) = minimize {
            assert!(last.right() < minimize.rect().left(), "{last:?}");
        }
    }

    #[test]
    fn the_mode_switcher_offers_library_and_shows_the_others_disabled() {
        let harness = harness();
        let library = harness.get_by_label("Library").accesskit_node();
        assert_eq!(library.toggled(), Some(Toggled::True));
        assert!(!library.is_disabled());
        for name in ["Organizer", "Develop"] {
            let mode = harness.get_by_label(name).accesskit_node();
            assert_eq!(mode.toggled(), Some(Toggled::False), "{name}");
            assert!(mode.is_disabled(), "{name}");
        }
    }

    #[test]
    fn a_mode_not_built_yet_says_so_under_the_pointer() {
        let mut harness = harness();
        assert!(
            harness
                .query_by_label("Coming in a later release")
                .is_none()
        );
        harness.get_by_label("Organizer").hover();
        // egui shows a tooltip once the pointer has rested on the button for
        // a moment, so give it frames until it does, or plainly too many.
        let shown = (0..20).any(|_| {
            harness.step();
            harness
                .query_by_label("Coming in a later release")
                .is_some()
        });
        assert!(shown, "no tooltip after 20 frames");
    }

    /// The mode is saved, and restored on the next run, unless it is one
    /// not built yet, which starts in the default mode.
    #[test]
    fn the_mode_is_saved_and_restored_when_available() {
        let mut storage = MemoryStorage::default();
        harness().state_mut().save(&mut storage);
        assert_eq!(storage.get_string(MODE_KEY).as_deref(), Some("Library"));
        assert_eq!(harness_from(&storage).state().mode, Mode::Library);

        storage.set_string(MODE_KEY, "Organizer".to_owned());
        assert_eq!(harness_from(&storage).state().mode, Mode::Library);
    }

    #[test]
    fn the_icon_font_is_installed_at_start() {
        let harness = harness();
        assert!(icons::resolves("x"));
        let (glyph, font) = icons::glyph("x", 14.0, '?');
        assert!(harness.ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)));
    }

    #[test]
    fn the_style_is_installed_at_start() {
        let harness = harness();
        let padding = harness.ctx.global_style().spacing.button_padding;
        assert_eq!(padding, egui::vec2(10.0, 6.0));
    }

    #[test]
    fn icon_decodes_to_a_square_of_icon_size() {
        let size = PhotographyWorkbenchApp::ICON_SIZE;
        let icon = PhotographyWorkbenchApp::icon();
        assert_eq!((icon.width, icon.height), (size, size));
        assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
    }
}
