// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! dialogs.

use desktop_ui::chrome::{self, TITLE_BAR_HEIGHT};
use desktop_ui::fonts;
use desktop_ui::style;
use desktop_ui::theme::{self, Theme};
use desktop_ui::toolbar;

use crate::dialogs;
use crate::layout::{self, Docks};
use crate::mode::{Layout, Mode};
use crate::view::View;

/// The [`eframe::Storage`] key the theme choice is saved under, as the
/// theme's [`Theme::name`], or `"System"` for none. The display name is the
/// saved value, so renaming a theme resets users who chose it to System.
const THEME_KEY: &str = "theme";

/// The [`eframe::Storage`] key the mode is saved under, as its
/// [`Mode::name`].
const MODE_KEY: &str = "mode";

/// The [`eframe::Storage`] key Library's view is saved under, as its
/// [`View::name`].
const VIEW_KEY: &str = "view";

/// The application state, owned by eframe for the life of the window.
#[derive(Default)]
pub struct PhotographyWorkbenchApp {
    /// The theme chosen in Settings, `None` for System, saved between runs.
    theme: Option<Theme>,
    /// The concrete theme last installed on the context: [`Self::theme`]
    /// resolved against the OS preference. `None` until the first install.
    resolved_theme: Option<Theme>,
    /// The mode the window shows, saved between runs.
    mode: Mode,
    /// Which docks are shown.
    docks: Docks,
    /// How Library shows a folder's photos, saved between runs.
    view: View,
    /// Whether the Settings dialog is open.
    show_settings: bool,
    /// Whether the About dialog is open.
    show_about: bool,
    /// [`Self::icon`] as a texture, for the title bar and About; made the
    /// first time one of them is shown.
    icon_texture: Option<egui::TextureHandle>,
    /// The width of macOS's window buttons with their margins, measured at
    /// start-up, for the title bar to start past. `None` on other systems,
    /// or when the window could not be measured.
    mac_window_buttons: Option<f32>,
}

impl PhotographyWorkbenchApp {
    /// The name shown in the title bar.
    pub const NAME: &'static str = "Photography Workbench";

    /// What the app is for, in a line, as About shows it.
    pub const TAGLINE: &'static str = "A fast desktop app for culling photos";

    /// The copyright notice About shows.
    pub const COPYRIGHT: &'static str =
        "Copyright (C) 2026 Alvaro Kuolas and the Photography Workbench contributors";

    /// The licence, as About states it, with the notice that the GPL asks an
    /// interactive program to show.
    pub const LICENCE: &'static str = "Free software under the GNU General Public License, \
        version 3 or later. It comes with absolutely no warranty.";

    /// The smallest the window can be, in points, and its size on the first
    /// run: a handheld's 1280 × 800 screen, the smallest a photographer
    /// culls on. Every layout is designed and tested at this size (ADR010).
    pub const MIN_WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

    /// Called once, before the first frame. Installs the fonts and the
    /// style, and restores the saved theme choice, mode and view;
    /// with none saved, or an unreadable one, takes the default.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        style::install(&cc.egui_ctx);
        let saved = |key| cc.storage.and_then(|storage| storage.get_string(key));
        let theme =
            saved(THEME_KEY).and_then(|name| Theme::ALL.into_iter().find(|t| t.name() == name));
        let mode = saved(MODE_KEY)
            .and_then(|name| Mode::from_saved(&name))
            .unwrap_or_default();
        let view = saved(VIEW_KEY)
            .and_then(|name| View::from_saved(&name))
            .unwrap_or_default();
        let mut app = Self {
            theme,
            mode,
            view,
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

    /// [`Self::icon`] as a texture, made the first time it is asked for.
    fn icon_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        self.icon_texture
            .get_or_insert_with(|| {
                let icon = Self::icon();
                let size = [icon.width as usize, icon.height as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
                // Mipmaps, so the icon stays smooth drawn far smaller than
                // it is.
                let options = egui::TextureOptions {
                    mipmap_mode: Some(egui::TextureFilter::Linear),
                    ..egui::TextureOptions::LINEAR
                };
                ctx.load_texture("app_icon", image, options)
            })
            .clone()
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

    /// The title bar's contents: the app's icon (except on macOS) and the
    /// mode switcher at the left, past the space macOS's window buttons
    /// take, and the window controls and the settings gear at the right.
    /// The icon opens About; on macOS, About belongs in the app menu at the
    /// top of the screen, and an icon beside the system's window buttons
    /// would look out of place. The whole bar drags the window, so it is
    /// made a drag region before anything is put on it.
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
                // The first icon's square ends 16 points from the window
                // controls, or from the window's edge on macOS.
                ui.add_space(10.0);
                let gear = ("settings", '⚙');
                if chrome::icon_button(ui, palette, gear, "Settings").clicked() {
                    self.show_settings = true;
                }
            },
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let pad = chrome::title_bar_left_pad(ui.ctx(), self.mac_window_buttons);
                ui.add_space(pad);
                if !cfg!(target_os = "macos") {
                    self.title_bar_icon(ui);
                    ui.add_space(6.0);
                }
                self.mode_switcher(ui);
            },
        );
    }

    /// The parts of the window shown around the centre: the mode's, but
    /// in Library's grid without the bottom dock, the filmstrip's, as the
    /// grid shows the photos as thumbnails already.
    fn parts(&self) -> Layout {
        let mut parts = self.mode.layout();
        if self.view == View::Grid {
            parts.bottom_dock = false;
        }
        parts
    }

    /// The mode's own controls at the start of the toolbar: in Library, a
    /// button per view, the current one selected.
    fn mode_controls(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        match self.mode {
            Mode::Library => {
                for view in View::ALL {
                    let selected = self.view == view;
                    if toolbar::button(ui, palette, view.icon(), view.name(), selected).clicked() {
                        self.view = view;
                    }
                }
            }
            Mode::Organizer | Mode::Develop => {}
        }
    }

    /// The app's icon, 20 points square, which opens About.
    fn title_bar_icon(&mut self, ui: &mut egui::Ui) {
        let about = format!("About {}", Self::NAME);
        let icon = egui::Image::new(&self.icon_texture(ui.ctx()))
            .fit_to_exact_size(egui::Vec2::splat(20.0))
            .sense(egui::Sense::click())
            .alt_text(&about);
        if ui.add(icon).on_hover_text(about).clicked() {
            self.show_about = true;
        }
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

        // Each panel takes its edge of what the ones before it left: the
        // tool rail the window's right edge, the side docks the rest of
        // the height, down to the window's bottom, then the toolbar the
        // bottom between them, and the bottom dock the room above it.
        let parts = self.parts();
        if parts.tool_rail {
            layout::tool_rail(ui, &palette, &mut self.docks);
        }
        layout::side_docks(ui, parts, &mut self.docks);
        if parts.toolbar {
            let start = |ui: &mut egui::Ui| self.mode_controls(ui, &palette);
            toolbar::show(ui, "toolbar", &palette, start, |_| {});
        }
        layout::bottom_dock(ui, &palette, parts, &mut self.docks);

        // The viewer shows its photo on a backdrop of its own.
        let (empty, fill) = match self.view {
            View::Grid => ("No folder open", palette.panel),
            View::Viewer => ("No photo open", palette.viewer_backdrop),
        };
        let centre = egui::Frame::central_panel(ui.style()).fill(fill);
        egui::CentralPanel::default().frame(centre).show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(empty);
            });
        });

        // Above everything but the dialogs.
        chrome::window_resize_handles(ui.ctx());

        // The dialogs come last, above everything else. A theme picked in
        // Settings is installed at the top of the next frame, which the
        // click itself makes egui run.
        if self.show_settings && dialogs::settings(ui.ctx(), &mut self.theme) {
            self.show_settings = false;
        }
        if self.show_about {
            let icon = self.icon_texture(ui.ctx());
            if dialogs::about(ui.ctx(), &icon) {
                self.show_about = false;
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, theme::choice_name(self.theme).to_owned());
        storage.set_string(MODE_KEY, self.mode.name().to_owned());
        storage.set_string(VIEW_KEY, self.view.name().to_owned());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::{App as _, Storage as _};
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::{icons, theme};

    use super::{Docks, MODE_KEY, Mode, PhotographyWorkbenchApp, THEME_KEY, Theme, VIEW_KEY, View};

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

    /// [`harness`], with Library showing the viewer, which has every part
    /// of the window the grid has and the filmstrip.
    fn viewer_harness() -> Harness<'static, PhotographyWorkbenchApp> {
        let mut harness = harness();
        harness.state_mut().view = View::Viewer;
        harness.run();
        harness
    }

    #[test]
    fn first_frame_shows_the_title_bar_and_the_empty_state() {
        let harness = harness();
        harness.get_by_label("Library");
        harness.get_by_label("No folder open");
    }

    /// The window has no menus: the title bar holds the app's commands.
    #[test]
    fn the_window_has_no_menus() {
        let harness = harness();
        for menu in ["File", "View"] {
            assert!(harness.query_by_label(menu).is_none(), "{menu}");
        }
    }

    /// At the window's smallest, everything on the title bar fits on it in
    /// order, clear of the system's window buttons on macOS, and of the
    /// window controls elsewhere.
    #[test]
    fn the_title_bar_holds_the_modes_the_gear_and_the_window_controls() {
        let harness = harness();
        let mut last = egui::Rect::NOTHING;
        for name in ["Library", "Organizer", "Develop", "Settings"] {
            let rect = harness.get_by_label(name).rect();
            assert!(rect.left() >= last.right(), "{name} at {rect:?}");
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

    /// Library's view buttons choose how the centre shows the photos, and
    /// show which is chosen.
    #[test]
    fn the_view_buttons_switch_the_centre() {
        let mut harness = harness();
        let toggled = |harness: &Harness<'_, PhotographyWorkbenchApp>, name| {
            harness.get_by_label(name).accesskit_node().toggled()
        };
        assert_eq!(toggled(&harness, "Thumbnail grid"), Some(Toggled::True));
        harness.get_by_label("No folder open");
        harness.get_by_label("Image viewer").click();
        harness.run();
        assert_eq!(harness.state().view, View::Viewer);
        assert_eq!(toggled(&harness, "Image viewer"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Thumbnail grid"), Some(Toggled::False));
        harness.get_by_label("No photo open");
    }

    /// The view is saved and restored on the next run; an unreadable one
    /// starts in the grid.
    #[test]
    fn the_view_is_saved_and_restored() {
        let mut storage = MemoryStorage::default();
        let mut harness = harness();
        harness.state_mut().view = View::Viewer;
        harness.state_mut().save(&mut storage);
        assert_eq!(
            storage.get_string(VIEW_KEY).as_deref(),
            Some("Image viewer")
        );
        assert_eq!(harness_from(&storage).state().view, View::Viewer);

        storage.set_string(VIEW_KEY, "Filmstrip".to_owned());
        assert_eq!(harness_from(&storage).state().view, View::Grid);
    }

    #[test]
    fn the_gear_opens_settings_and_esc_closes_it() {
        let mut harness = harness();
        assert!(harness.query_by_label("Theme").is_none());
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_label("Theme");
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.query_by_label("Theme").is_none());
    }

    /// A theme picked in Settings is installed on the next frame, and saved.
    #[test]
    fn a_theme_picked_in_settings_is_installed_and_saved() {
        let mut harness = harness();
        harness.get_by_label("Settings").click();
        harness.run();
        harness.get_by_role(Role::ComboBox).click();
        harness.run();
        harness.get_by_label("Nexus").click();
        harness.run();
        assert_eq!(harness.state().resolved_theme, Some(Theme::Nexus));

        let mut storage = MemoryStorage::default();
        harness.state_mut().save(&mut storage);
        assert_eq!(storage.get_string(THEME_KEY).as_deref(), Some("Nexus"));
    }

    /// The icon on the title bar opens About, except on macOS, where About
    /// belongs in the app menu and the bar has no icon.
    #[test]
    fn the_title_bar_icon_opens_about_except_on_macos() {
        let mut harness = harness();
        let about = "About Photography Workbench";
        let icon = harness.query_by_label(about);
        assert_eq!(icon.is_some(), !cfg!(target_os = "macos"));
        let Some(icon) = icon else { return };
        let rect = icon.rect();
        assert!(rect.bottom() <= TITLE_BAR_HEIGHT, "{rect:?}");
        assert!(rect.right() <= harness.get_by_label("Library").rect().left());
        icon.click();
        harness.run();
        harness.get_by_label(PhotographyWorkbenchApp::TAGLINE);
    }

    /// The icon itself, drawn on any system: it is named for About, and a
    /// click on it opens About.
    #[test]
    fn a_click_on_the_icon_opens_about() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, app: &mut PhotographyWorkbenchApp| app.title_bar_icon(ui),
            PhotographyWorkbenchApp::default(),
        );
        harness.get_by_label("About Photography Workbench").click();
        harness.run();
        assert!(harness.state().show_about);
    }

    #[test]
    fn about_shows_the_version_licence_and_home_page() {
        let mut harness = harness();
        harness.state_mut().show_about = true;
        harness.run();
        harness.get_by_label(&format!("Version {}", env!("CARGO_PKG_VERSION")));
        harness.get_by_label(PhotographyWorkbenchApp::COPYRIGHT);
        harness.get_by_label(PhotographyWorkbenchApp::LICENCE);
        harness.get_by_label(env!("CARGO_PKG_REPOSITORY"));
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(!harness.state().show_about);
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

    /// Where the panel `id` was laid out in the last frame.
    fn panel(harness: &Harness<'_, PhotographyWorkbenchApp>, id: &str) -> egui::Rect {
        egui::PanelState::load(&harness.ctx, egui::Id::new(id))
            .unwrap_or_else(|| panic!("no panel {id}"))
            .outer_rect
    }

    /// At the window's smallest, Library has every part in its place: the
    /// tool rail and the side docks from the title bar down to the window's
    /// bottom, the toolbar along the bottom between the side docks, holding
    /// the view buttons, and the bottom dock above it, leaving the centre
    /// room to work in.
    #[test]
    fn library_lays_out_its_bars_and_docks_around_the_centre() {
        let harness = viewer_harness();
        let window =
            egui::Rect::from_min_size(egui::Pos2::ZERO, PhotographyWorkbenchApp::MIN_WINDOW);
        let title_bar = panel(&harness, "title_bar");
        let between = egui::Rangef::new(title_bar.bottom(), window.bottom());
        let tool_rail = panel(&harness, "tool_rail");
        let left_dock = panel(&harness, "left_dock");
        let right_dock = panel(&harness, "right_dock");
        for rect in [tool_rail, left_dock, right_dock] {
            assert_eq!(rect.y_range(), between, "{rect:?}");
        }
        assert_eq!(left_dock.left(), window.left());
        assert_eq!(tool_rail.right(), window.right());
        assert_eq!((left_dock.width(), right_dock.width()), (268.0, 270.0));
        assert_eq!(right_dock.right(), tool_rail.left());

        // Info, what a culler looks at most, heads the tool rail.
        let info = harness.get_by_role_and_label(Role::Button, "Info").rect();
        assert!(info.top() - tool_rail.top() < info.height(), "{info:?}");

        let under_the_centre = egui::Rangef::new(left_dock.right(), right_dock.left());
        let toolbar = panel(&harness, "toolbar");
        assert_eq!(toolbar.x_range(), under_the_centre, "{toolbar:?}");
        assert_eq!(toolbar.bottom(), window.bottom(), "{toolbar:?}");
        for view in View::ALL {
            let button = harness.get_by_label(view.name()).rect();
            assert!(toolbar.contains_rect(button), "{button:?}");
        }

        let bottom_dock = panel(&harness, "bottom_dock");
        assert_eq!(bottom_dock.x_range(), under_the_centre, "{bottom_dock:?}");
        assert_eq!(bottom_dock.bottom(), toolbar.top(), "{bottom_dock:?}");

        let centre = harness.get_by_label("No photo open").rect();
        assert!(
            centre.width() >= 300.0 && centre.height() >= 300.0,
            "{centre:?}"
        );
        assert!(left_dock.right() <= centre.left() && centre.right() <= right_dock.left());
        assert!(title_bar.bottom() <= centre.top() && centre.bottom() <= bottom_dock.top());
    }

    /// The bottom dock, the filmstrip's, is 142 points high under the
    /// viewer. The grid has none: the centre runs down to the toolbar.
    #[test]
    fn the_filmstrip_comes_with_the_viewer_only() {
        let mut harness = viewer_harness();
        assert_eq!(panel(&harness, "bottom_dock").height(), 142.0);
        harness.get_by_label("Thumbnail grid").click();
        harness.run();
        let centre = harness.get_by_label("No folder open").rect();
        let below = panel(&harness, "toolbar").top() - centre.bottom();
        assert!((0.0..20.0).contains(&below), "{below}");
    }

    /// The viewer and its filmstrip are filled with the viewer's backdrop;
    /// the grid is not.
    #[test]
    fn the_viewer_and_the_filmstrip_sit_on_the_viewer_s_backdrop() {
        let on_backdrop = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
            harness
                .output()
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Rect(rect) if rect.fill == palette.viewer_backdrop => {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let mut harness = viewer_harness();
        let filled = on_backdrop(&harness);
        let centre = harness.get_by_label("No photo open").rect();
        assert!(
            filled.iter().any(|rect| rect.contains_rect(centre)),
            "{filled:?}"
        );
        let filmstrip = panel(&harness, "bottom_dock");
        // All of it but the line along its top.
        let fills_filmstrip =
            |rect: &egui::Rect| filmstrip.contains_rect(*rect) && rect.height() >= 141.0;
        assert!(filled.iter().any(fills_filmstrip), "{filled:?}");
        harness.get_by_label("Thumbnail grid").click();
        harness.run();
        assert_eq!(on_backdrop(&harness), Vec::new());
    }

    /// Switching between the grid and the viewer, which has the bottom
    /// dock, leaves the ids of the centre's widgets as they were, so they
    /// keep their state.
    #[test]
    fn the_centre_keeps_its_ids_between_the_views() {
        let mut harness = harness();
        let grid = harness.get_by_label("No folder open").accesskit_node().id();
        harness.get_by_label("Image viewer").click();
        harness.run();
        let viewer = harness.get_by_label("No photo open").accesskit_node().id();
        assert_eq!(grid, viewer);
    }

    /// With the side docks dragged as wide as they go, the centre keeps
    /// 300 points at the window's smallest.
    #[test]
    fn the_widest_side_docks_leave_the_centre_room() {
        let harness = side_docks_dragged_to(1000.0);
        let gap = panel(&harness, "right_dock").left() - panel(&harness, "left_dock").right();
        assert!(gap >= 300.0, "{gap}");
    }

    /// Dragged as narrow as they go, the side docks keep room for their
    /// contents: 200 points on the left, 250 on the right.
    #[test]
    fn the_narrowest_side_docks_keep_room_for_their_contents() {
        let harness = side_docks_dragged_to(10.0);
        let widths = (
            panel(&harness, "left_dock").width(),
            panel(&harness, "right_dock").width(),
        );
        assert_eq!(widths, (200.0, 250.0));
    }

    /// [`harness`], with both side docks as if dragged to `width`.
    fn side_docks_dragged_to(width: f32) -> Harness<'static, PhotographyWorkbenchApp> {
        let mut harness = harness();
        for id in ["left_dock", "right_dock"] {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 100.0));
            let state = egui::PanelState { outer_rect: rect };
            harness
                .ctx
                .data_mut(|d| d.insert_persisted(egui::Id::new(id), state));
        }
        harness.run();
        harness
    }

    /// The right dock shows only what the tool rail picked, with no title
    /// over it: Info says when there is no photo to show the information
    /// of.
    #[test]
    fn the_right_dock_shows_only_the_photo_s_information() {
        let harness = harness();
        harness.get_by_label("No photo selected");
        harness.get_by_label("Select a photo to see its information");
        // The tool rail's button alone is named Info.
        assert_eq!(harness.query_all_by_label("Info").count(), 1);
    }

    /// A click on an action bar's button shows the dock it picks for, if
    /// that dock is hidden. The button is shown selected while its dock
    /// shows what it picks.
    #[test]
    fn an_action_bar_button_shows_its_dock() {
        let mut harness = harness();
        harness.state_mut().docks = Docks {
            left: false,
            right: false,
            bottom: false,
        };
        harness.run();
        let shown = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            harness.query_by_label("No photo selected").is_some()
        };
        let selected = |harness: &Harness<'_, PhotographyWorkbenchApp>, label| {
            let button = harness.get_by_role_and_label(Role::Button, label);
            button.accesskit_node().toggled() == Some(Toggled::True)
        };
        assert!(!shown(&harness));
        assert!(!selected(&harness, "Info"));
        harness.get_by_role_and_label(Role::Button, "Info").click();
        harness.run();
        assert!(shown(&harness));
        assert!(selected(&harness, "Info"));
        let docks = harness.state().docks;
        assert!(!docks.left && docks.right && !docks.bottom, "{docks:?}");
    }

    #[test]
    fn the_icon_font_is_installed_at_start() {
        let harness = harness();
        assert!(icons::resolves("x"));
        let (glyph, font) = icons::glyph("x", 14.0, '?');
        assert!(harness.ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)));
    }

    /// The window's text is in Inter, and its headings in Inter's
    /// semibold weight.
    #[test]
    fn the_typeface_is_installed_at_start() {
        let harness = harness();
        let first = harness.ctx.fonts(|fonts| {
            fonts.definitions().families[&egui::FontFamily::Proportional][0].clone()
        });
        assert_eq!(first, "Inter");
        let heading = egui::TextStyle::Heading.resolve(&harness.ctx.global_style());
        assert_eq!(
            heading.family,
            egui::FontFamily::Name(desktop_ui::fonts::SEMIBOLD.into())
        );
    }

    #[test]
    fn the_style_is_installed_at_start() {
        let harness = harness();
        let padding = harness.ctx.global_style().spacing.button_padding;
        assert_eq!(padding, egui::vec2(8.0, 3.0));
    }

    #[test]
    fn icon_decodes_to_a_square_of_icon_size() {
        let size = PhotographyWorkbenchApp::ICON_SIZE;
        let icon = PhotographyWorkbenchApp::icon();
        assert_eq!((icon.width, icon.height), (size, size));
        assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
    }
}
