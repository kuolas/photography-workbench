// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! dialogs.

use desktop_ui::chrome::{self, TITLE_BAR_HEIGHT};
use desktop_ui::dock::Side;
use desktop_ui::fonts;
use desktop_ui::style;
use desktop_ui::theme::{self, Theme};
use desktop_ui::toolbar;

use crate::dialogs;
use crate::keymap::{self, Action, Surface};
use crate::layout::{self, Docks};
use crate::menu::{self, MacMenu};
use crate::mode::{Layout, Mode};
use crate::view::View;

/// The Lucide icons of the title bar's settings gear and of its button
/// for the reference of keyboard shortcuts, and the text shown in place
/// of each if its name does not resolve.
const TITLE_BAR_ICONS: [(&str, char); 2] = [("settings", '⚙'), ("circle-question-mark", '?')];

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
    /// Which docks are shown, saved between runs.
    docks: Docks,
    /// How Library shows a folder's photos, saved between runs.
    view: View,
    /// Whether the Settings dialog is open.
    show_settings: bool,
    /// Whether the About dialog is open.
    show_about: bool,
    /// Whether the reference of keyboard shortcuts is open.
    show_shortcuts: bool,
    /// [`Self::icon`] as a texture, for the title bar and About; made the
    /// first time one of them is shown.
    icon_texture: Option<egui::TextureHandle>,
    /// The width of macOS's window buttons with their margins, measured at
    /// start-up, for the title bar to start past. `None` on other systems,
    /// or when the window could not be measured.
    mac_window_buttons: Option<f32>,
    /// The menu bar at the top of the screen, on macOS. `None` on other
    /// systems, and when there is no window to give it to.
    menu: Option<MacMenu>,
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
    /// style, and restores the saved theme choice, mode, docks and view;
    /// with none saved, or an unreadable one, takes the default.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        style::install(&cc.egui_ctx);
        // Quitting is the keymap's, like every other shortcut: egui's own
        // Ctrl+Q would close the window even with a dialog open. On macOS
        // the menu bar's Quit takes Cmd+Q before the window sees it.
        cc.egui_ctx.options_mut(|o| o.quit_shortcuts.clear());
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
            docks: Docks::restore(cc.storage),
            view,
            mac_window_buttons: measure_mac_window_buttons(cc),
            menu: MacMenu::install(cc),
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
    /// take, and the window controls, the toggles for the mode's docks, the
    /// settings gear and the button for the reference of keyboard
    /// shortcuts at the right.
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
                let (parts, surface) = (self.parts(), self.surface());
                if let Some(side) = layout::dock_toggles(ui, palette, parts, surface, self.docks) {
                    self.perform(ui.ctx(), Action::Dock(side));
                }
                let [gear, shortcuts] = TITLE_BAR_ICONS;
                let chord = keymap::chord_for(surface, Action::Settings);
                if chrome::icon_button(ui, palette, gear, "Settings", chord).clicked() {
                    self.perform(ui.ctx(), Action::Settings);
                }
                let label = "Keyboard shortcuts";
                let chord = keymap::chord_for(surface, Action::Shortcuts);
                if chrome::icon_button(ui, palette, shortcuts, label, chord).clicked() {
                    self.perform(ui.ctx(), Action::Shortcuts);
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

    /// Where the keyboard shortcuts apply: the view Library shows.
    fn surface(&self) -> Surface {
        match self.view {
            View::Grid => Surface::Grid,
            View::Viewer => Surface::Viewer,
        }
    }

    /// Do what the keyboard shortcuts pressed in this frame ask. They
    /// stand down while a dialog is open, all but the reference's own
    /// chord, which closes it, and while a text field has the keyboard,
    /// as what is typed there is the field's.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let surface = self.surface();
        let pressed = |action| keymap::pressed(ctx, surface, action);
        if self.show_shortcuts && pressed(Action::Shortcuts) {
            self.perform(ctx, Action::Shortcuts);
            return;
        }
        if ctx.memory(|m| m.top_modal_layer().is_some()) || ctx.text_edit_focused() {
            return;
        }
        // egui moves the keyboard focus to the next widget on Tab, and to
        // the one before on Shift+Tab; here those keys show and hide the
        // docks instead.
        if pressed(Action::SideDocks) || pressed(Action::AllDocks) {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        for action in keymap::pressed_actions(ctx, surface) {
            self.perform(ctx, action);
        }
    }

    /// Do what the menu bar's items clicked since the last frame ask.
    fn menu_clicks(&mut self, ctx: &egui::Context) {
        let clicked = self.menu.as_mut().map(MacMenu::clicked);
        self.perform_from_menu(ctx, clicked.unwrap_or_default());
    }

    /// Do the `actions` menu items asked for, each only if its item can
    /// be chosen when it is done. The items show what can be chosen as
    /// the last frame left the app, and an action done before this one
    /// can have changed that, by opening a dialog, for one.
    fn perform_from_menu(&mut self, ctx: &egui::Context, actions: Vec<Action>) {
        for action in actions {
            if self.menu_state().enabled(action) {
                self.perform(ctx, action);
            }
        }
    }

    /// What the menu bar shows of the app: the mode, the view and the
    /// docks shown, and whether a dialog is open.
    fn menu_state(&self) -> menu::State {
        menu::State {
            mode: self.mode,
            view: self.view,
            parts: self.parts(),
            docks: self.docks,
            dialog: self.show_settings || self.show_about || self.show_shortcuts,
            shortcuts: self.show_shortcuts,
        }
    }

    /// Do what `action` asks, whether a key, a button or a menu item asked
    /// for it, so each asks for the same thing in the same way (ADR010).
    fn perform(&mut self, ctx: &egui::Context, action: Action) {
        let parts = self.parts();
        match action {
            Action::Mode(mode) => {
                if mode.is_available() {
                    self.mode = mode;
                }
            }
            Action::View(view) => self.view = view,
            Action::SideDocks => self.docks.toggle(parts, &[Side::Left, Side::Right]),
            Action::AllDocks => {
                self.docks
                    .toggle(parts, &[Side::Left, Side::Right, Side::Bottom]);
            }
            Action::Dock(side) => self.docks.toggle(parts, &[side]),
            Action::Settings => self.show_settings = true,
            Action::Shortcuts => self.show_shortcuts = !self.show_shortcuts,
            Action::About => self.show_about = true,
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
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
                    let chord = keymap::chord_for(self.surface(), Action::View(view));
                    if toolbar::button(ui, palette, view.icon(), view.name(), selected, chord)
                        .clicked()
                    {
                        self.perform(ui.ctx(), Action::View(view));
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
            self.perform(ui.ctx(), Action::About);
        }
    }

    /// A button per mode, the current one selected. A mode not built yet
    /// is shown disabled.
    fn mode_switcher(&mut self, ui: &mut egui::Ui) {
        for mode in Mode::ALL {
            let chord = keymap::chord_for(self.surface(), Action::Mode(mode));
            let response = ui
                .add_enabled_ui(mode.is_available(), |ui| {
                    chrome::selectable_button(
                        ui,
                        mode.icon(),
                        mode.name(),
                        self.mode == mode,
                        chord,
                    )
                })
                .inner
                .on_disabled_hover_text("Coming in a later release");
            if response.clicked() {
                self.perform(ui.ctx(), Action::Mode(mode));
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
    /// Do what the menu bar's items ask. eframe calls this before every
    /// frame, and also while the window is minimized or hidden, when it
    /// draws no frame, so the menu bar acts then too: its Minimize leaves
    /// it showing, and its Quit must still close the window.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu_clicks(ctx);
        let state = self.menu_state();
        if let Some(menu) = &mut self.menu {
            menu.show(state);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The theme is installed only here, at the top of the frame, never
        // part-way through it, so a frame is painted in one theme. The root
        // `Ui` took its style from the context before this runs, and every
        // panel inherits it, so a newly installed theme is also put on the
        // root, to paint in this frame rather than the next.
        if self.reconcile_theme(ui.ctx()) {
            ui.reset_style();
        }

        // What the keys ask for, after the menu bar's items (`logic`),
        // before anything is drawn, so it shows in this frame.
        self.shortcuts(ui.ctx());

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
        if self.show_shortcuts && dialogs::shortcuts(ui.ctx(), &keymap::sections()) {
            self.show_shortcuts = false;
        }

        // The menu bar shows the app as this frame leaves it.
        let state = self.menu_state();
        if let Some(menu) = &mut self.menu {
            menu.show(state);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, theme::choice_name(self.theme).to_owned());
        storage.set_string(MODE_KEY, self.mode.name().to_owned());
        self.docks.save(storage);
        storage.set_string(VIEW_KEY, self.view.name().to_owned());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::{App as _, Storage as _};
    use egui::accesskit::{Role, Toggled};
    use egui::{Key, Modifiers, ViewportCommand, ViewportId};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::keys::Chord;
    use desktop_ui::{icons, theme};

    use super::{
        Action, Docks, MODE_KEY, Mode, PhotographyWorkbenchApp, Side, THEME_KEY, TITLE_BAR_ICONS,
        Theme, VIEW_KEY, View,
    };

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
    fn the_title_bar_holds_its_buttons_and_the_window_controls() {
        let harness = viewer_harness();
        let mut last = egui::Rect::NOTHING;
        let names = [
            "Library",
            "Organizer",
            "Develop",
            "Keyboard shortcuts",
            "Settings",
            LEFT_TOGGLE,
            BOTTOM_TOGGLE,
            RIGHT_TOGGLE,
        ];
        for name in names {
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

    /// The title bar's toggle for each dock.
    const LEFT_TOGGLE: &str = "Show or hide the left dock";
    const BOTTOM_TOGGLE: &str = "Show or hide the filmstrip";
    const RIGHT_TOGGLE: &str = "Show or hide the right dock";

    /// A dock toggle hides its dock, and shows it again, and says which.
    #[test]
    fn a_dock_toggle_hides_and_shows_its_dock() {
        let mut harness = viewer_harness();
        let info = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            harness.query_all_by_label("No photo selected").count()
        };
        assert_eq!(info(&harness), 1);
        harness.get_by_label(RIGHT_TOGGLE).click();
        harness.run();
        assert_eq!(info(&harness), 0);
        let toggle = harness.get_by_label(RIGHT_TOGGLE).accesskit_node();
        assert_eq!(toggle.toggled(), Some(Toggled::False));
        harness.get_by_label(RIGHT_TOGGLE).click();
        harness.run();
        assert_eq!(info(&harness), 1);

        harness.get_by_label(BOTTOM_TOGGLE).click();
        harness.get_by_label(LEFT_TOGGLE).click();
        harness.run();
        let docks = harness.state().docks;
        assert!(!docks.left && !docks.bottom && docks.right, "{docks:?}");
    }

    /// Press `key` with `modifiers` held, and let the app settle.
    fn press(harness: &mut Harness<'_, PhotographyWorkbenchApp>, modifiers: Modifiers, key: Key) {
        harness.key_press_modifiers(modifiers, key);
        harness.run();
    }

    #[test]
    fn g_and_e_switch_the_view_and_esc_goes_back_to_the_grid() {
        let mut harness = harness();
        press(&mut harness, Modifiers::NONE, Key::E);
        assert_eq!(harness.state().view, View::Viewer);
        press(&mut harness, Modifiers::NONE, Key::G);
        assert_eq!(harness.state().view, View::Grid);
        press(&mut harness, Modifiers::NONE, Key::E);
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert_eq!(harness.state().view, View::Grid);
    }

    /// Tab hides the side docks together while either is shown, and shows
    /// them both otherwise. It takes the keyboard focus nowhere, as egui's
    /// own Tab would.
    #[test]
    fn tab_shows_and_hides_the_side_docks() {
        let mut harness = viewer_harness();
        let docks = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            let docks = harness.state().docks;
            (docks.left, docks.right, docks.bottom)
        };
        press(&mut harness, Modifiers::NONE, Key::Tab);
        assert_eq!(docks(&harness), (false, false, true));
        assert_eq!(harness.ctx.memory(|m| m.focused()), None);
        press(&mut harness, Modifiers::NONE, Key::F7);
        assert_eq!(docks(&harness), (true, false, true));
        press(&mut harness, Modifiers::NONE, Key::Tab);
        assert_eq!(docks(&harness), (false, false, true));
        press(&mut harness, Modifiers::NONE, Key::Tab);
        assert_eq!(docks(&harness), (true, true, true));
    }

    /// Shift+Tab shows and hides every dock the view has: with the grid,
    /// which has no filmstrip, the side docks.
    #[test]
    fn shift_tab_shows_and_hides_every_dock_the_view_has() {
        let mut harness = viewer_harness();
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        assert_eq!(
            harness.state().docks,
            Docks {
                left: false,
                right: false,
                bottom: false
            }
        );
        assert_eq!(harness.ctx.memory(|m| m.focused()), None);
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        assert_eq!(harness.state().docks, Docks::default());

        let mut harness = self::harness();
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        assert_eq!(
            harness.state().docks,
            Docks {
                left: false,
                right: false,
                bottom: true
            }
        );
    }

    /// F6, F7 and F8 show and hide the filmstrip, the left dock and the
    /// right dock; F6 only with the viewer, which has the filmstrip.
    #[test]
    fn the_function_keys_show_and_hide_a_dock_each() {
        let mut harness = viewer_harness();
        press(&mut harness, Modifiers::NONE, Key::F6);
        assert_eq!(
            harness.state().docks,
            Docks {
                left: true,
                right: true,
                bottom: false
            }
        );
        press(&mut harness, Modifiers::NONE, Key::F7);
        press(&mut harness, Modifiers::NONE, Key::F8);
        assert_eq!(
            harness.state().docks,
            Docks {
                left: false,
                right: false,
                bottom: false
            }
        );

        let mut harness = self::harness();
        press(&mut harness, Modifiers::NONE, Key::F6);
        assert_eq!(harness.state().docks, Docks::default());
    }

    #[test]
    fn a_mode_not_built_yet_is_not_switched_to_from_the_keyboard() {
        let mut harness = harness();
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        for key in [Key::Num2, Key::Num3, Key::Num1] {
            press(&mut harness, ctrl_alt, key);
            assert_eq!(harness.state().mode, Mode::Library, "{key:?}");
        }
    }

    /// Ctrl+Alt+1 switches to Library from another mode.
    #[test]
    fn ctrl_alt_1_switches_to_library() {
        let mut harness = harness();
        harness.state_mut().mode = Mode::Develop;
        harness.run();
        assert_eq!(harness.state().mode, Mode::Develop);
        press(&mut harness, Modifiers::COMMAND | Modifiers::ALT, Key::Num1);
        assert_eq!(harness.state().mode, Mode::Library);
    }

    #[test]
    fn ctrl_comma_opens_settings() {
        let mut harness = harness();
        press(&mut harness, Modifiers::COMMAND, Key::Comma);
        assert!(harness.state().show_settings);
    }

    #[test]
    fn ctrl_q_closes_the_window() {
        let mut harness = harness();
        // In one frame, whose output holds what it sent the window: the
        // harness's own key presses run a frame per event.
        harness.input_mut().events.push(egui::Event::Key {
            key: Key::Q,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        });
        harness.step();
        let sent = harness
            .output()
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|viewport| viewport.commands.clone())
            .unwrap_or_default();
        assert!(sent.contains(&ViewportCommand::Close), "{sent:?}");
    }

    /// An action asked for directly, as a menu item asks for it, does what
    /// the key or button that asks for it does.
    #[test]
    fn an_action_does_what_its_key_or_button_does() {
        let ctx = egui::Context::default();
        let mut app = PhotographyWorkbenchApp {
            view: View::Viewer,
            ..PhotographyWorkbenchApp::default()
        };
        app.perform(&ctx, Action::About);
        assert!(app.show_about);
        app.perform(&ctx, Action::Settings);
        assert!(app.show_settings);
        app.perform(&ctx, Action::Shortcuts);
        assert!(app.show_shortcuts, "the reference opens");
        app.perform(&ctx, Action::Shortcuts);
        assert!(!app.show_shortcuts, "and closes");

        app.perform(&ctx, Action::Dock(Side::Bottom));
        assert_eq!(app.docks, docks(true, true, false));
        app.perform(&ctx, Action::SideDocks);
        assert_eq!(app.docks, docks(false, false, false));
        app.perform(&ctx, Action::AllDocks);
        assert_eq!(app.docks, docks(true, true, true));

        app.perform(&ctx, Action::View(View::Grid));
        assert_eq!(app.view, View::Grid);
        app.mode = Mode::Develop;
        app.perform(&ctx, Action::Mode(Mode::Library));
        assert_eq!(app.mode, Mode::Library);
        app.perform(&ctx, Action::Mode(Mode::Organizer));
        assert_eq!(app.mode, Mode::Library, "a mode not built yet");
    }

    /// The menu bar is shown the app as it is: the mode, the view, the
    /// window's parts and docks, and whether a dialog is open, and which.
    #[test]
    fn the_menu_bar_is_shown_the_app_as_it_is() {
        let mut app = PhotographyWorkbenchApp {
            view: View::Viewer,
            ..PhotographyWorkbenchApp::default()
        };
        app.docks.right = false;
        let state = app.menu_state();
        assert_eq!(state.mode, Mode::Library);
        assert_eq!(state.view, View::Viewer);
        assert!(state.parts.bottom_dock, "the viewer has the filmstrip");
        assert_eq!(state.docks, docks(true, false, true));
        assert!(!state.dialog && !state.shortcuts);
        for open in [
            |app: &mut PhotographyWorkbenchApp| app.show_settings = true,
            |app: &mut PhotographyWorkbenchApp| app.show_about = true,
        ] {
            let mut app = PhotographyWorkbenchApp::default();
            open(&mut app);
            let state = app.menu_state();
            assert!(state.dialog && !state.shortcuts);
        }
        app.show_shortcuts = true;
        let state = app.menu_state();
        assert!(state.dialog && state.shortcuts);
    }

    /// A menu item's action is done only if its item can be chosen when
    /// it is done: none while a dialog is open, even one opened by the
    /// action before it, but the reference's own, which closes it.
    #[test]
    fn a_menu_item_s_action_waits_while_a_dialog_is_open() {
        let ctx = egui::Context::default();
        let mut app = PhotographyWorkbenchApp::default();
        let viewer = Action::View(View::Viewer);
        app.perform_from_menu(&ctx, vec![Action::Settings, viewer]);
        assert!(app.show_settings);
        assert_eq!(app.view, View::Grid, "not behind the dialog");

        let mut app = PhotographyWorkbenchApp {
            show_shortcuts: true,
            ..PhotographyWorkbenchApp::default()
        };
        app.perform_from_menu(&ctx, vec![Action::About, Action::Shortcuts]);
        assert!(!app.show_about);
        assert!(!app.show_shortcuts, "the reference's own item closes it");
    }

    /// The docks shown: the left, right and bottom ones.
    fn docks(left: bool, right: bool, bottom: bool) -> Docks {
        Docks {
            left,
            right,
            bottom,
        }
    }

    /// Rest the pointer on `label` until a tooltip saying `tip` shows, or
    /// plainly too many frames go by; whether it showed. egui shows a
    /// tooltip once the pointer has rested a moment.
    fn tooltip_shows(
        harness: &mut Harness<'_, PhotographyWorkbenchApp>,
        label: &str,
        tip: &str,
    ) -> bool {
        harness.get_by_label(label).hover();
        (0..20).any(|_| {
            harness.step();
            harness.query_by_label(tip).is_some()
        })
    }

    /// The title bar's and the toolbar's buttons name in their tooltips
    /// the chord that does what they do, as this system spells it.
    #[test]
    fn a_button_s_tooltip_names_its_chord() {
        let mut harness = viewer_harness();
        let chord = |modifiers, key| Chord::new(modifiers, key).label();
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        let tips = [
            ("Library", chord(ctrl_alt, Key::Num1)),
            ("Keyboard shortcuts", chord(Modifiers::COMMAND, Key::Slash)),
            ("Settings", chord(Modifiers::COMMAND, Key::Comma)),
            (LEFT_TOGGLE, chord(Modifiers::NONE, Key::F7)),
            (BOTTOM_TOGGLE, chord(Modifiers::NONE, Key::F6)),
            (RIGHT_TOGGLE, chord(Modifiers::NONE, Key::F8)),
            ("Thumbnail grid", chord(Modifiers::NONE, Key::G)),
            ("Image viewer", chord(Modifiers::NONE, Key::E)),
        ];
        for (label, chord) in tips {
            let tip = format!("{label} ({chord})");
            assert!(tooltip_shows(&mut harness, label, &tip), "{tip}");
        }
    }

    /// The button beside the gear opens the reference of keyboard
    /// shortcuts.
    #[test]
    fn the_title_bar_s_button_opens_the_reference() {
        let mut harness = harness();
        harness.get_by_label("Keyboard shortcuts").click();
        harness.run();
        assert!(harness.state().show_shortcuts);
        assert!(
            harness
                .query_by_label("Back to the thumbnail grid")
                .is_some()
        );
    }

    #[test]
    fn every_title_bar_icon_resolves() {
        for (name, _) in TITLE_BAR_ICONS {
            assert!(icons::resolves(name), "{name}");
        }
    }

    /// Ctrl+/ opens the reference, which lists the shortcuts, and closes
    /// it again.
    #[test]
    fn ctrl_slash_opens_the_reference_and_closes_it() {
        let mut harness = harness();
        let row = "Back to the thumbnail grid";
        assert!(harness.query_by_label(row).is_none());
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        assert!(harness.state().show_shortcuts);
        assert!(harness.query_by_label(row).is_some());
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        assert!(!harness.state().show_shortcuts);
        assert!(harness.query_by_label(row).is_none());
    }

    /// While a dialog is open, the keys are the dialog's: G does not
    /// switch the view behind it, and Esc closes the dialog alone, leaving
    /// the viewer as it is.
    #[test]
    fn the_shortcuts_stand_down_while_a_dialog_is_open() {
        let mut harness = viewer_harness();
        press(&mut harness, Modifiers::COMMAND, Key::Comma);
        press(&mut harness, Modifiers::NONE, Key::G);
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        assert_eq!(harness.state().view, View::Viewer);
        assert!(!harness.state().show_shortcuts);
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert!(!harness.state().show_settings);
        assert_eq!(harness.state().view, View::Viewer);
    }

    /// Which docks are shown is saved, and restored on the next run. A
    /// dock with no state saved, or one that cannot be read, is shown.
    #[test]
    fn the_docks_are_saved_and_restored() {
        let mut storage = MemoryStorage::default();
        let mut harness = harness();
        let docks = Docks {
            left: false,
            right: true,
            bottom: false,
        };
        harness.state_mut().docks = docks;
        harness.state_mut().save(&mut storage);
        assert_eq!(harness_from(&storage).state().docks, docks);

        storage.set_string("show_left_dock", "no".to_owned());
        storage.remove_string("show_bottom_dock");
        assert_eq!(harness_from(&storage).state().docks, Docks::default());
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
    /// viewer. The grid has neither it nor its toggle: the centre runs down
    /// to the toolbar.
    #[test]
    fn the_filmstrip_comes_with_the_viewer_only() {
        let mut harness = viewer_harness();
        assert_eq!(panel(&harness, "bottom_dock").height(), 142.0);
        harness.get_by_label(BOTTOM_TOGGLE);
        harness.get_by_label("Thumbnail grid").click();
        harness.run();
        assert!(harness.query_by_label(BOTTOM_TOGGLE).is_none());
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
