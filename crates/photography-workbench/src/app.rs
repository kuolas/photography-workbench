// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! dialogs.

use desktop_ui::chrome::{self, TITLE_BAR_HEIGHT};
use desktop_ui::dock::{self, Side};
use desktop_ui::fonts;
use desktop_ui::style;
use desktop_ui::theme::{self, Theme};
use desktop_ui::toolbar;

use crate::browse::{Browser, Photos};
use crate::dialogs;
use crate::filmstrip;
use crate::folder_tree;
use crate::grid;
use crate::keymap::{self, Action, Step, Surface};
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
    /// The folder tree in Library's left dock, fed from disk, and the
    /// selected folder's photos; what is expanded and selected in the
    /// tree is saved between runs.
    browser: Browser,
    /// How many tiles the grid laid out in a row the last time it was
    /// drawn, which the keys that move a row up or down go by. `None`
    /// until it is drawn, and from a change of mode, view or docks shown
    /// until it is drawn again.
    grid_columns: Option<usize>,
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
    /// style, and restores the saved theme choice, mode, docks and view,
    /// and what is expanded and selected in the folder tree; with none
    /// saved, or an unreadable one, takes the default.
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
            browser: Browser::restore(cc.storage),
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
        // egui moves the keyboard focus to the next widget on Tab, to the
        // one before on Shift+Tab, and, while a widget has it, to the one
        // beside it on an arrow key, each time the key goes down or the
        // system repeats it. Here those keys show and hide the docks and
        // move between photos instead, and none of them moves the focus,
        // even where it does nothing else, or held, as the docks' keys
        // ignore the repeats.
        if ctx.input(|i| i.events.iter().any(moves_focus)) {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let actions = keymap::pressed_actions(ctx, surface);
        // A row key pressed in the same frame as a key that changes the
        // grid's rows does nothing too, whichever of them the table puts
        // first.
        if actions.iter().copied().any(changes_rows) {
            self.grid_columns = None;
        }
        for action in actions {
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
            Action::Photo(step) => self.step_photo(step),
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
        // The grid is not laid out again until it is drawn, after the
        // keys: until then the keys that move a row do nothing, rather
        // than move by a row of the old length.
        if changes_rows(action) {
            self.grid_columns = None;
        }
    }

    /// Make the photo `step` asks for the current one, stopping at the
    /// folder's first and last photos. The photos above and below are
    /// the grid's as it was last laid out, and there are none before it
    /// is. Nothing while there are no photos.
    fn step_photo(&mut self, step: Step) {
        match step {
            Step::Previous => self.browser.step_photo(-1),
            Step::Next => self.browser.step_photo(1),
            Step::First => self.browser.first_photo(),
            Step::Last => self.browser.last_photo(),
            Step::Above | Step::Below => {
                let Photos::Read(list) = self.browser.photos() else {
                    return;
                };
                let (Some(current), Some(columns)) = (list.current(), self.grid_columns) else {
                    return;
                };
                let index = match step {
                    Step::Above => grid::above(current, columns),
                    _ => grid::below(current, list.len(), columns),
                };
                self.browser.select_photo(index);
            }
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

    /// The centre: in Library's grid the selected folder's photos as
    /// tiles, in its viewer the current photo, or a message where there
    /// is none to show. Nothing while a folder just chosen is being read.
    fn centre(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        if let Some((title, body)) = empty_state(self.browser.photos()) {
            dock::empty_message(ui, title, &body);
            return;
        }
        let Photos::Read(list) = self.browser.photos() else {
            return;
        };
        match self.view {
            View::Grid => {
                let shown = grid::show(ui, palette, list.photos(), list.current(), list.folder());
                self.grid_columns = Some(shown.columns);
                if let Some(click) = shown.click {
                    self.browser.select_photo(click.index);
                    if click.open {
                        self.perform(ui.ctx(), Action::View(View::Viewer));
                    }
                }
            }
            // The photo's name, in place of the photo until the app
            // decodes previews.
            View::Viewer => {
                if let Some(photo) = list.current_photo() {
                    let name = egui::RichText::new(photo.name())
                        .size(VIEWER_NAME_SIZE)
                        .color(palette.text);
                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Label::new(name).selectable(false));
                    });
                }
            }
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

/// The size of the current photo's name in the viewer.
const VIEWER_NAME_SIZE: f32 = 13.5;

/// The size of the toolbar's count of photos.
const COUNT_SIZE: f32 = 12.5;

/// What the centre says when it has no photos to show, as a title and
/// the line under it: no folder is selected, it could not be read, or it
/// holds no photos. `None` when there are photos to show, and while a
/// folder just chosen is being read, when it shows nothing.
fn empty_state(photos: &Photos) -> Option<(&'static str, String)> {
    match photos {
        Photos::None => Some((
            "No folder selected",
            "Choose a folder on the left to see its photos".to_owned(),
        )),
        Photos::Reading => None,
        Photos::Failed(kind) => Some((
            "This folder can't be read",
            folder_tree::why_unreadable(*kind),
        )),
        Photos::Read(list) if list.is_empty() => Some((
            "No photos in this folder",
            "Photos are JPEG files and the RAW files of Nikon and Sony cameras".to_owned(),
        )),
        Photos::Read(_) => None,
    }
}

/// Which of the photos is the current one, as the toolbar shows it:
/// "12 of 348". `None` when there are none.
fn position(photos: &Photos) -> Option<String> {
    let Photos::Read(list) = photos else {
        return None;
    };
    let current = list.current()?;
    Some(format!("{} of {}", current + 1, list.len()))
}

/// Whether egui would move the keyboard focus on `event`: Tab, alone or
/// with Shift, or an arrow key, going down or repeated by the system.
fn moves_focus(event: &egui::Event) -> bool {
    match event {
        egui::Event::Key {
            key: egui::Key::Tab,
            pressed: true,
            modifiers,
            ..
        } => !modifiers.any() || modifiers.shift_only(),
        egui::Event::Key {
            key:
                egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::ArrowLeft | egui::Key::ArrowRight,
            pressed: true,
            ..
        } => true,
        _ => false,
    }
}

/// Whether `action` can change the length of the grid's rows, which are
/// as long as fits between the parts of the window around the grid: a
/// change of mode, view or docks shown can change those parts.
fn changes_rows(action: Action) -> bool {
    matches!(
        action,
        Action::Mode(_) | Action::View(_) | Action::SideDocks | Action::AllDocks | Action::Dock(_)
    )
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
        // What the disk has read, before the tree is drawn.
        self.browser.update(ui.ctx());

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
        let folders = |ui: &mut egui::Ui| folder_tree::show(ui, &palette, &mut self.browser);
        layout::side_docks(ui, parts, &mut self.docks, folders);
        if parts.toolbar {
            let position = position(self.browser.photos());
            let start = |ui: &mut egui::Ui| self.mode_controls(ui, &palette);
            let end = |ui: &mut egui::Ui| {
                if let Some(position) = position {
                    let text = egui::RichText::new(position)
                        .size(COUNT_SIZE)
                        .color(palette.text);
                    ui.add(egui::Label::new(text).selectable(false));
                }
            };
            toolbar::show(ui, "toolbar", &palette, start, end);
        }
        // The same photos as the grid's, sharing the current one.
        let film = |ui: &mut egui::Ui| {
            if let Photos::Read(list) = self.browser.photos() {
                let photos = list.photos();
                let clicked = filmstrip::show(ui, &palette, photos, list.current(), list.folder());
                if let Some(index) = clicked {
                    self.browser.select_photo(index);
                }
            }
        };
        layout::bottom_dock(ui, &palette, parts, &mut self.docks, film);

        // The viewer shows its photo on a backdrop of its own. What the
        // centre holds sets its own room.
        let fill = match self.view {
            View::Grid => palette.panel,
            View::Viewer => palette.viewer_backdrop,
        };
        let centre = egui::Frame::central_panel(ui.style())
            .fill(fill)
            .inner_margin(egui::Margin::ZERO);
        egui::CentralPanel::default()
            .frame(centre)
            .show(ui, |ui| self.centre(ui, &palette));

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
        self.browser.save(storage);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use eframe::{App as _, Storage as _};
    use egui::accesskit::{Role, Toggled};
    use egui::{Key, Modifiers, ViewportCommand, ViewportId};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT as _, Queryable as _};

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::keys::Chord;
    use desktop_ui::{icons, theme};

    use catalog::folders::{Place, Root};

    use crate::disk::Disk;

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

    /// The app, run headless in a window of the smallest size it allows,
    /// with no folders to browse: no test reads the real home folder.
    fn harness() -> Harness<'static, PhotographyWorkbenchApp> {
        harness_on(None, Vec::new()).0
    }

    /// [`harness`], with the settings in `storage` saved by an earlier run.
    fn harness_from(storage: &MemoryStorage) -> Harness<'_, PhotographyWorkbenchApp> {
        harness_on(Some(storage), Vec::new()).0
    }

    /// The app, run headless in a window of the smallest size it allows,
    /// with the settings in `storage` saved by an earlier run, if any, and
    /// `roots` mounted. The disk is read at once, on the window's thread;
    /// the roots it reads are those the handle returned holds, which a
    /// test changes to mount and unmount volumes, the change showing on
    /// the next frame.
    fn harness_on(
        storage: Option<&MemoryStorage>,
        roots: Vec<Root>,
    ) -> (Harness<'_, PhotographyWorkbenchApp>, Arc<Mutex<Vec<Root>>>) {
        let roots = Arc::new(Mutex::new(roots));
        let disk = Disk::inline(Arc::clone(&roots));
        let harness = Harness::builder()
            .with_size(PhotographyWorkbenchApp::MIN_WINDOW)
            .build_eframe(|cc| {
                if let Some(storage) = storage {
                    cc.storage = Some(storage);
                }
                let mut app = PhotographyWorkbenchApp::new(cc);
                app.browser.use_disk(disk);
                app
            });
        (harness, roots)
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
        harness.get_by_label("No folder selected");
        harness.get_by_label("Choose a folder on the left to see its photos");
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
        harness.get_by_label("No folder selected");
        harness.get_by_label("Image viewer").click();
        harness.run();
        assert_eq!(harness.state().view, View::Viewer);
        assert_eq!(toggled(&harness, "Image viewer"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Thumbnail grid"), Some(Toggled::False));
        harness.get_by_label("No folder selected");
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

    /// Where the centre was laid out in the last frame: the rectangle its
    /// frame filled, the largest filled with the view's colour between
    /// the side docks.
    fn centre(harness: &Harness<'_, PhotographyWorkbenchApp>) -> egui::Rect {
        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        let fill = match harness.state().view {
            View::Grid => palette.panel,
            View::Viewer => palette.viewer_backdrop,
        };
        let between = egui::Rangef::new(
            panel(harness, "left_dock").right(),
            panel(harness, "right_dock").left(),
        );
        let filled = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == fill
                        && between.contains(rect.rect.left())
                        && between.contains(rect.rect.right()) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            });
        filled
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("no centre")
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

        let centre = centre(&harness);
        assert!(
            centre.width() >= 300.0 && centre.height() >= 300.0,
            "{centre:?}"
        );
        assert_eq!(centre.x_range(), under_the_centre, "{centre:?}");
        assert_eq!(centre.top(), title_bar.bottom(), "{centre:?}");
        assert_eq!(centre.bottom(), bottom_dock.top(), "{centre:?}");
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
        let centre = centre(&harness);
        assert_eq!(
            centre.bottom(),
            panel(&harness, "toolbar").top(),
            "{centre:?}"
        );
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
        let centre = egui::Rect::from_x_y_ranges(
            panel(&harness, "left_dock").right()..=panel(&harness, "right_dock").left(),
            panel(&harness, "title_bar").bottom()..=panel(&harness, "bottom_dock").top(),
        );
        assert!(filled.contains(&centre), "{filled:?}");
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
        let grid = harness
            .get_by_label("No folder selected")
            .accesskit_node()
            .id();
        harness.get_by_label("Image viewer").click();
        harness.run();
        let viewer = harness
            .get_by_label("No folder selected")
            .accesskit_node()
            .id();
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

    /// A temporary folder to browse, holding `DCIM`, with `100NIKON` in
    /// it, and `Exports`.
    fn photos_folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("DCIM/100NIKON")).unwrap();
        std::fs::create_dir(dir.path().join("Exports")).unwrap();
        dir
    }

    /// The home folder at `dir`, as the tree's root named Home.
    fn home(dir: &Path) -> Root {
        Root::new(Place::Home, "Home", dir)
    }

    /// Click on what is named `label`, and let the app settle.
    fn click(harness: &mut Harness<'_, PhotographyWorkbenchApp>, label: &str) {
        harness.get_by_label(label).click();
        harness.run();
    }

    /// Whether the row named `label` is shown selected, which egui tells
    /// AccessKit as the row being toggled on.
    fn is_selected(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let row = harness.get_by_label(label).accesskit_node();
        row.toggled() == Some(Toggled::True)
    }

    /// Run frames until `seconds` of egui's time have gone by, then let
    /// the app settle.
    fn wait(harness: &mut Harness<'_, PhotographyWorkbenchApp>, seconds: f64) {
        let until = harness.ctx.input(|i| i.time) + seconds;
        for _ in 0..1000 {
            if harness.ctx.input(|i| i.time) >= until {
                break;
            }
            harness.step();
        }
        harness.run();
    }

    /// The left dock shows the tree's roots. The arrow beside a folder
    /// expands it, showing its subfolders, and collapses it again.
    #[test]
    fn the_arrow_beside_a_folder_expands_it_and_collapses_it() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.get_by_label("Home");
        assert!(harness.query_by_label("DCIM").is_none());
        click(&mut harness, "Expand Home");
        harness.get_by_label("DCIM");
        harness.get_by_label("Exports");
        assert!(harness.query_by_label("100NIKON").is_none());
        click(&mut harness, "Collapse Home");
        assert!(harness.query_by_label("DCIM").is_none());
        harness.get_by_label("Expand Home");
    }

    /// The tree's rows fill the left dock's width, up to the line along
    /// its edge, 29 points high each, one under the other from 8 points
    /// below its top, a folder's arrow 16 points right of its parent's.
    #[test]
    fn the_tree_s_rows_fill_the_left_dock() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        let dock = panel(&harness, "left_dock");
        let home = harness.get_by_label("Home").rect();
        let dcim = harness.get_by_label("DCIM").rect();
        assert_eq!(home.top(), dock.top() + 8.0, "{home:?} in {dock:?}");
        assert_eq!(home.left(), dock.left(), "{home:?} in {dock:?}");
        assert_eq!(home.right(), dock.right() - 1.0, "{home:?} in {dock:?}");
        assert_eq!((home.height(), dcim.height()), (29.0, 29.0));
        assert_eq!(dcim.top(), home.bottom());
        let arrow = |label| harness.get_by_label(label).rect();
        let (home_arrow, dcim_arrow) = (arrow("Collapse Home"), arrow("Expand DCIM"));
        assert_eq!(home_arrow.size(), egui::vec2(16.0, 16.0));
        assert_eq!(home_arrow.center().x, dock.left() + 10.0);
        assert_eq!(dcim_arrow.center().x - home_arrow.center().x, 16.0);
    }

    /// A click on a folder selects it, and its row says so.
    #[test]
    fn a_click_on_a_folder_selects_it() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        assert!(!is_selected(&harness, "DCIM"));
        click(&mut harness, "DCIM");
        assert!(is_selected(&harness, "DCIM"));
        assert!(!is_selected(&harness, "Home"));
        let dcim = dir.path().join("DCIM");
        assert_eq!(
            harness.state().browser.tree().selected(),
            Some(dcim.as_path())
        );
        // Selecting leaves it collapsed.
        assert!(harness.query_by_label("100NIKON").is_none());
    }

    /// What is expanded and selected in the tree is saved, and restored on
    /// the next run.
    #[test]
    fn the_tree_s_expanded_and_selected_folders_are_saved_and_restored() {
        let dir = photos_folder();
        let mut storage = MemoryStorage::default();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        click(&mut harness, "Expand DCIM");
        click(&mut harness, "100NIKON");
        harness.state_mut().save(&mut storage);
        drop(harness);

        let (harness, _) = harness_on(Some(&storage), vec![home(dir.path())]);
        harness.get_by_label("Collapse DCIM");
        assert!(is_selected(&harness, "100NIKON"));
    }

    /// A volume mounted while the app runs shows once the disk sees the
    /// roots change, and goes when it is unmounted.
    #[test]
    fn a_volume_shows_when_it_is_mounted_and_goes_when_it_is_not() {
        let dir = photos_folder();
        let card = tempfile::tempdir().unwrap();
        let (mut harness, roots) = harness_on(None, vec![home(dir.path())]);
        assert!(harness.query_by_label("EOS_DIGITAL").is_none());
        let volume = Root::new(Place::Volume, "EOS_DIGITAL", card.path());
        roots.lock().unwrap().push(volume);
        harness.run();
        harness.get_by_label("EOS_DIGITAL");
        roots.lock().unwrap().pop();
        harness.run();
        assert!(harness.query_by_label("EOS_DIGITAL").is_none());
        harness.get_by_label("Home");
    }

    /// An idle window draws nothing: once the tree is shown and the app
    /// has settled, no frame is asked for, now or later, as the cards and
    /// drives are watched off the window's thread.
    #[test]
    fn an_idle_window_asks_for_no_frame() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        wait(&mut harness, 3.0);
        harness.get_by_label("DCIM");
        let output = &harness.output().viewport_output[&ViewportId::ROOT];
        assert_eq!(output.repaint_delay, std::time::Duration::MAX);
    }

    /// A folder made while the window was in the background shows once
    /// the window is back in front.
    #[test]
    fn coming_back_to_the_window_reads_the_tree_again() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        std::fs::create_dir(dir.path().join("Trip")).unwrap();
        wait(&mut harness, 2.0);
        assert!(harness.query_by_label("Trip").is_none());
        leave_and_come_back(&mut harness);
        harness.get_by_label("Trip");
    }

    /// Send the window to the background and bring it back to the front,
    /// then let the app settle.
    fn leave_and_come_back(harness: &mut Harness<'_, PhotographyWorkbenchApp>) {
        for focused in [false, true] {
            harness.input_mut().focused = focused;
            let event = egui::Event::WindowFocused(focused);
            harness.input_mut().events.push(event);
            harness.step();
        }
        harness.run();
    }

    /// [`photos_folder`], holding beside its folders the photos
    /// `DSC_0002.NEF` and `DSC_0010.jpg`, a note and a hidden photo, all
    /// empty: the list of photos does not open them.
    fn folder_of_photos() -> tempfile::TempDir {
        let dir = photos_folder();
        for name in ["DSC_0010.jpg", "notes.txt", "DSC_0002.NEF", ".hidden.jpg"] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        dir
    }

    /// The text of the toolbar's count, if it shows one.
    fn count(harness: &Harness<'_, PhotographyWorkbenchApp>) -> Option<String> {
        let toolbar = panel(harness, "toolbar");
        harness
            .query_all_by_label_contains(" of ")
            .filter(|node| toolbar.contains_rect(node.rect()))
            .find_map(|node| node.accesskit_node().value())
    }

    /// Selecting a folder shows its photos in the grid, in natural order
    /// of their names, the first one current, and nothing else in the
    /// folder; the toolbar shows which photo of how many is current.
    #[test]
    fn selecting_a_folder_shows_its_photos_in_the_grid() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        assert_eq!(count(&harness), None);
        click(&mut harness, "Home");
        let first = harness.get_by_label("DSC_0002.NEF").rect();
        let second = harness.get_by_label("DSC_0010.jpg").rect();
        assert!(first.right() < second.left(), "{first:?} {second:?}");
        assert!(is_selected(&harness, "DSC_0002.NEF"));
        assert!(!is_selected(&harness, "DSC_0010.jpg"));
        // Home is collapsed, so its folders are not in the tree either.
        for name in ["notes.txt", ".hidden.jpg", "DCIM", "Exports"] {
            assert!(harness.query_by_label(name).is_none(), "{name}");
        }
        assert_eq!(count(&harness).as_deref(), Some("1 of 2"));
    }

    /// A click on a tile makes its photo the current one.
    #[test]
    fn a_click_on_a_tile_makes_its_photo_current() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        click(&mut harness, "DSC_0010.jpg");
        assert!(is_selected(&harness, "DSC_0010.jpg"));
        assert!(!is_selected(&harness, "DSC_0002.NEF"));
        assert_eq!(count(&harness).as_deref(), Some("2 of 2"));
        assert_eq!(harness.state().view, View::Grid);
    }

    /// Double-click on what is named `label`, and let the app settle. The
    /// harness runs a frame for each event it is given, a quarter of a
    /// second apart, too slow for a double-click, so the clicks go in as
    /// the input of one frame.
    fn double_click(harness: &mut Harness<'_, PhotographyWorkbenchApp>, label: &str) {
        let pos = harness.get_by_label(label).rect().center();
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        for pressed in [true, false, true, false] {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        harness.step();
        harness.run();
    }

    /// Whether the viewer shows the name of the photo named `label` in the
    /// centre, where the photo goes: as a name, not as a tile of the grid.
    fn viewer_shows(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let centre = centre(harness);
        harness.query_all_by_label(label).any(|node| {
            centre.contains_rect(node.rect()) && node.accesskit_node().toggled().is_none()
        })
    }

    /// A double-click on a tile makes its photo the current one and opens
    /// it in the viewer, which shows its name; so does one just after a
    /// click on another tile.
    #[test]
    fn a_double_click_on_a_tile_opens_its_photo_in_the_viewer() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        wait(&mut harness, 1.0);
        double_click(&mut harness, "DSC_0010.jpg");
        assert_eq!(harness.state().view, View::Viewer);
        assert!(viewer_shows(&harness, "DSC_0010.jpg"));
        assert!(!viewer_shows(&harness, "DSC_0002.NEF"));
        assert_eq!(count(&harness).as_deref(), Some("2 of 2"));

        click(&mut harness, "Thumbnail grid");
        wait(&mut harness, 1.0);
        harness.get_by_label("DSC_0010.jpg").click();
        harness.step();
        assert_eq!(harness.state().view, View::Grid);
        double_click(&mut harness, "DSC_0002.NEF");
        assert_eq!(harness.state().view, View::Viewer);
        assert!(viewer_shows(&harness, "DSC_0002.NEF"));
        assert_eq!(count(&harness).as_deref(), Some("1 of 2"));
    }

    /// A folder with no photos says so, in the grid and in the viewer,
    /// and the toolbar shows no count.
    #[test]
    fn a_folder_with_no_photos_says_so() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        click(&mut harness, "Exports");
        let body = "Photos are JPEG files and the RAW files of Nikon and Sony cameras";
        for view in View::ALL {
            harness.state_mut().view = view;
            harness.run();
            harness.get_by_label("No photos in this folder");
            harness.get_by_label(body);
            assert_eq!(count(&harness), None);
        }
    }

    /// A folder that can no longer be read, deleted since it was listed,
    /// says so and why.
    #[test]
    fn a_folder_that_cannot_be_read_says_so() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        click(&mut harness, "DCIM");
        harness.get_by_label("No photos in this folder");
        std::fs::remove_dir_all(dir.path().join("DCIM")).unwrap();
        click(&mut harness, "DCIM");
        harness.get_by_label("This folder can't be read");
        harness.get_by_label("It is no longer there");
        assert_eq!(count(&harness), None);
    }

    /// The folder selected when the app last ran shows its photos as soon
    /// as its root is read, with no click.
    #[test]
    fn a_restored_folder_shows_its_photos() {
        let dir = folder_of_photos();
        let mut storage = MemoryStorage::default();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        harness.state_mut().save(&mut storage);
        drop(harness);

        let (harness, _) = harness_on(Some(&storage), vec![home(dir.path())]);
        harness.get_by_label("DSC_0002.NEF");
        harness.get_by_label("DSC_0010.jpg");
        assert_eq!(count(&harness).as_deref(), Some("1 of 2"));
    }

    /// A photo copied in while the window was in the background shows
    /// once the window is back in front, and the current photo stays the
    /// current one.
    #[test]
    fn coming_back_to_the_window_reads_the_photos_again() {
        let dir = folder_of_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        click(&mut harness, "DSC_0010.jpg");
        std::fs::write(dir.path().join("DSC_0001.NEF"), b"").unwrap();
        wait(&mut harness, 2.0);
        assert!(harness.query_by_label("DSC_0001.NEF").is_none());
        leave_and_come_back(&mut harness);
        harness.get_by_label("DSC_0001.NEF");
        assert!(is_selected(&harness, "DSC_0010.jpg"));
        assert_eq!(count(&harness).as_deref(), Some("3 of 3"));
    }

    /// The tiles are 220 points square, 1 point apart across and down,
    /// from 4 points in from the centre's left and top, as many to a row
    /// as fit, the room left over at the right. A photo's name starts 8
    /// points in from its tile's left and 12 down from its top, and the
    /// current photo's tile is outlined in the accent colour.
    #[test]
    fn the_tiles_are_laid_out_in_rows() {
        let dir = photos_folder();
        for i in 1..=8 {
            std::fs::write(dir.path().join(format!("DSC_000{i}.NEF")), b"").unwrap();
        }
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let centre = centre(&harness);
        let columns = ((centre.width() - 8.0 + 1.0) / 221.0).floor() as usize;
        assert!((2..8).contains(&columns), "{centre:?}");
        let tile = |i: usize| harness.get_by_label(&format!("DSC_000{i}.NEF")).rect();
        let first = tile(1);
        assert_eq!(first.size(), egui::Vec2::splat(220.0), "{first:?}");
        assert_eq!(first.left_top(), centre.left_top() + egui::vec2(4.0, 4.0));
        assert_eq!(tile(2).left_top(), first.right_top() + egui::vec2(1.0, 0.0));
        let last_across = tile(columns);
        assert!(
            last_across.right() <= centre.right() - 4.0,
            "{last_across:?}"
        );
        assert!(centre.right() - 4.0 - last_across.right() < 221.0);
        let below = tile(columns + 1);
        assert_eq!(below.left_top(), first.left_bottom() + egui::vec2(0.0, 1.0));

        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        let shapes = &harness.output().shapes;
        let outlined = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) => {
                rect.rect == first
                    && rect.stroke == egui::Stroke::new(2.0, palette.accent)
                    && rect.stroke_kind == egui::StrokeKind::Outside
            }
            _ => false,
        });
        assert!(outlined, "no outline around {first:?}");
        let name_at = first.left_top() + egui::vec2(8.0, 12.0);
        let named = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => text.pos == name_at && text.galley.text() == "DSC_0001.NEF",
            _ => false,
        });
        assert!(named, "no name at {name_at:?}");
    }

    /// [`photos_folder`], holding beside its folders the 30 photos
    /// `DSC_0001.NEF` to `DSC_0030.NEF`, all empty, more than the
    /// filmstrip shows at once.
    fn folder_of_30_photos() -> tempfile::TempDir {
        folder_of_numbered_photos(30)
    }

    /// [`photos_folder`], holding beside its folders the photos
    /// `DSC_0001.NEF` to the one numbered `count`, all empty.
    fn folder_of_numbered_photos(count: usize) -> tempfile::TempDir {
        let dir = photos_folder();
        for n in 1..=count {
            std::fs::write(dir.path().join(photo(n)), b"").unwrap();
        }
        dir
    }

    /// The name of the photo numbered `n` in [`folder_of_30_photos`] and
    /// [`folder_of_numbered_photos`].
    fn photo(n: usize) -> String {
        format!("DSC_{n:04}.NEF")
    }

    /// Library's viewer on the photos of `dir`, the home folder, selected,
    /// the first photo current.
    fn filmstrip_harness(dir: &Path) -> Harness<'static, PhotographyWorkbenchApp> {
        let (mut harness, _) = harness_on(None, vec![home(dir)]);
        click(&mut harness, "Home");
        click(&mut harness, "Image viewer");
        harness
    }

    /// Where the filmstrip's cell for the photo named `label` was laid
    /// out in the last frame, and whether it was shown current; `None`
    /// when it was not laid out.
    fn film_cell(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        label: &str,
    ) -> Option<(egui::Rect, bool)> {
        let dock = panel(harness, "bottom_dock");
        harness
            .query_all_by_label(label)
            .find(|node| dock.y_range().contains(node.rect().center().y))
            .map(|node| {
                let current = node.accesskit_node().toggled() == Some(Toggled::True);
                (node.rect(), current)
            })
    }

    /// Whether the filmstrip's cell for the photo named `label` was laid
    /// out in the last frame and is wholly in view.
    fn in_filmstrip_view(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let across = panel(harness, "bottom_dock").x_range();
        film_cell(harness, label)
            .is_some_and(|(cell, _)| across.contains(cell.left()) && across.contains(cell.right()))
    }

    /// Turn the mouse wheel by `delta` points with the pointer over the
    /// filmstrip, then let the app settle.
    fn wheel_over_filmstrip(harness: &mut Harness<'_, PhotographyWorkbenchApp>, delta: egui::Vec2) {
        let pos = panel(harness, "bottom_dock").center();
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        wait(harness, 1.0);
    }

    /// In the viewer, the filmstrip shows a cell for each photo, the
    /// current one shown current. A click on another makes it the current
    /// one: the viewer shows its name, and the toolbar's count follows.
    #[test]
    fn a_click_in_the_filmstrip_makes_its_photo_current() {
        let dir = folder_of_30_photos();
        let mut harness = filmstrip_harness(dir.path());
        let first = film_cell(&harness, &photo(1)).expect("no cell for the first photo");
        assert!(first.1, "the first photo is not shown current");
        let third = film_cell(&harness, &photo(3)).expect("no cell for the third photo");
        assert!(!third.1, "the third photo is shown current");

        click(&mut harness, &photo(3));
        assert_eq!(count(&harness).as_deref(), Some("3 of 30"));
        assert_eq!(
            film_cell(&harness, &photo(3)).map(|cell| cell.1),
            Some(true)
        );
        assert_eq!(
            film_cell(&harness, &photo(1)).map(|cell| cell.1),
            Some(false)
        );
        assert!(viewer_shows(&harness, &photo(3)));
        assert_eq!(harness.state().view, View::Viewer);
    }

    /// A click on a cell leaves the strip where it is, whether the cell is
    /// wholly in view or only partly, at the strip's end.
    #[test]
    fn a_click_in_the_filmstrip_does_not_scroll_it() {
        let dir = folder_of_30_photos();
        let mut harness = filmstrip_harness(dir.path());
        let (before, _) = film_cell(&harness, &photo(1)).unwrap();
        assert!(in_filmstrip_view(&harness, &photo(5)));
        click(&mut harness, &photo(5));
        assert_eq!(count(&harness).as_deref(), Some("5 of 30"));
        let first = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            film_cell(harness, &photo(1)).map(|cell| cell.0)
        };
        assert_eq!(first(&harness), Some(before));

        let (partly, _) = film_cell(&harness, &photo(6)).unwrap();
        let dock = panel(&harness, "bottom_dock");
        assert!(dock.x_range().contains(partly.left()) && partly.right() > dock.right());
        click(&mut harness, &photo(6));
        assert_eq!(count(&harness).as_deref(), Some("6 of 30"));
        assert_eq!(first(&harness), Some(before));
    }

    /// The filmstrip's cells are 120 points wide and run its height, all
    /// of it but the line along its top, side by side from its left edge.
    /// A cell's tile sits 10 points in from its sides, 22 from its top and
    /// 8 from its bottom. In the band above, their middles 10 points below
    /// the cell's top, are its photo's name without the extension, 8
    /// points in from the cell's left, and its format, ending 8 points in
    /// from its right. The current photo's tile is outlined in the accent
    /// colour.
    #[test]
    fn the_filmstrip_s_cells_are_laid_out_in_a_row() {
        let dir = folder_of_30_photos();
        let harness = filmstrip_harness(dir.path());
        let dock = panel(&harness, "bottom_dock");
        let (first, _) = film_cell(&harness, &photo(1)).unwrap();
        let (second, _) = film_cell(&harness, &photo(2)).unwrap();
        assert_eq!(first.width(), 120.0, "{first:?}");
        assert_eq!(first.left(), dock.left(), "{first:?} in {dock:?}");
        let below_the_line = egui::Rangef::new(dock.top() + 1.0, dock.bottom());
        assert_eq!(first.y_range(), below_the_line, "{first:?} in {dock:?}");
        assert_eq!(second.left_top(), first.right_top(), "{second:?}");
        assert_eq!(second.size(), first.size(), "{second:?}");

        let tile = egui::Rect::from_min_max(
            first.left_top() + egui::vec2(10.0, 22.0),
            first.right_bottom() - egui::vec2(10.0, 8.0),
        );
        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        let shapes = &harness.output().shapes;
        let filled = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) => rect.rect == tile && rect.fill == palette.chip,
            _ => false,
        });
        assert!(filled, "no tile at {tile:?}");
        let outlined = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) => {
                rect.rect == tile
                    && rect.stroke == egui::Stroke::new(1.5, palette.accent)
                    && rect.stroke_kind == egui::StrokeKind::Outside
            }
            _ => false,
        });
        assert!(outlined, "no outline around {tile:?}");
        // The text laid out where it says, as (its left, its right, its
        // middle), if it is there.
        let text_at = |said: &str| {
            shapes.iter().find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == said => {
                    let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                    Some((rect.left(), rect.right(), rect.center().y))
                }
                _ => None,
            })
        };
        let stem = photo(1).trim_end_matches(".NEF").to_owned();
        let (left, _, middle) = text_at(&stem).expect("no name without its extension");
        assert_eq!((left, middle), (first.left() + 8.0, first.top() + 10.0));
        let (_, right, middle) = text_at("NEF").expect("no format");
        assert_eq!((right, middle), (first.right() - 8.0, first.top() + 10.0));
    }

    /// Only the cells in view and the few next to them are laid out: with
    /// the strip at its start, the last photo's cell is not.
    #[test]
    fn the_filmstrip_lays_out_only_the_cells_near_the_view() {
        let dir = folder_of_30_photos();
        let harness = filmstrip_harness(dir.path());
        assert!(in_filmstrip_view(&harness, &photo(1)));
        assert!(film_cell(&harness, &photo(30)).is_none());
    }

    /// A photo made current far along the strip is brought into view, to
    /// the strip's middle.
    /// Scrolled away from it, the strip stays where it was scrolled, as
    /// the current photo has not changed since.
    #[test]
    fn the_filmstrip_follows_the_current_photo_but_not_its_own_scrolling() {
        let dir = folder_of_30_photos();
        let mut harness = filmstrip_harness(dir.path());
        assert!(!in_filmstrip_view(&harness, &photo(26)));
        harness.state_mut().browser.select_photo(25);
        harness.run();
        assert!(in_filmstrip_view(&harness, &photo(26)));
        let (cell, _) = film_cell(&harness, &photo(26)).unwrap();
        let middle = panel(&harness, "bottom_dock").center().x;
        assert!((cell.center().x - middle).abs() <= 0.5, "{cell:?} {middle}");

        wheel_over_filmstrip(&mut harness, egui::vec2(10_000.0, 0.0));
        assert!(in_filmstrip_view(&harness, &photo(1)));
        assert!(!in_filmstrip_view(&harness, &photo(26)));
        wait(&mut harness, 3.0);
        assert!(in_filmstrip_view(&harness, &photo(1)));
        assert!(!in_filmstrip_view(&harness, &photo(26)));
    }

    /// Opening the viewer with a photo far along the strip current shows
    /// it in the strip at once, and so does opening it again after the
    /// strip was scrolled away from it.
    #[test]
    fn the_filmstrip_shows_the_current_photo_when_the_viewer_opens() {
        let dir = folder_of_30_photos();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        harness.state_mut().browser.select_photo(25);
        harness.run();
        click(&mut harness, "Image viewer");
        assert!(in_filmstrip_view(&harness, &photo(26)));

        wheel_over_filmstrip(&mut harness, egui::vec2(10_000.0, 0.0));
        assert!(!in_filmstrip_view(&harness, &photo(26)));
        click(&mut harness, "Thumbnail grid");
        click(&mut harness, "Image viewer");
        assert!(in_filmstrip_view(&harness, &photo(26)));
    }

    /// Another folder's photos bring its current photo into view, though
    /// it is at the same place in its list as the one before.
    #[test]
    fn the_filmstrip_shows_the_current_photo_of_another_folder() {
        let dir = folder_of_30_photos();
        for n in 1..=30 {
            std::fs::write(dir.path().join("Exports").join(photo(n)), b"").unwrap();
        }
        let mut harness = filmstrip_harness(dir.path());
        wheel_over_filmstrip(&mut harness, egui::vec2(-10_000.0, 0.0));
        assert!(!in_filmstrip_view(&harness, &photo(1)));
        click(&mut harness, "Expand Home");
        click(&mut harness, "Exports");
        assert_eq!(count(&harness).as_deref(), Some("1 of 30"));
        assert!(in_filmstrip_view(&harness, &photo(1)));
    }

    /// A photo's name too long for its cell is cut short before its
    /// format, which stays whole, and the name shows whole in a tooltip.
    #[test]
    fn a_long_name_in_the_filmstrip_shows_whole_in_a_tooltip() {
        let dir = photos_folder();
        let long = "Z_holiday_photo_from_a_phone_camera.jpg";
        for name in ["DSC_0001.NEF", long] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        let mut harness = filmstrip_harness(dir.path());
        let (cell, _) = film_cell(&harness, long).unwrap();
        let laid_out = |shape: &egui::Shape| match shape {
            egui::Shape::Text(text) if cell.contains(text.pos) => Some((
                egui::Rect::from_min_size(text.pos, text.galley.size()),
                text.galley.elided,
                text.galley.text().to_owned(),
            )),
            _ => None,
        };
        let texts: Vec<_> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| laid_out(&clipped.shape))
            .collect();
        let name = texts.iter().find(|(_, _, text)| text.starts_with("Z_"));
        let format = texts.iter().find(|(_, _, text)| text == "JPEG");
        let (Some((name, cut, _)), Some((format, whole, _))) = (name, format) else {
            panic!("no name or no format in {texts:?}");
        };
        assert!(cut, "the long name is not cut short");
        assert!(!whole, "the format is cut short");
        assert!(
            name.right() <= format.left(),
            "{name:?} runs into {format:?}"
        );
        assert_eq!(harness.query_all_by_label(long).count(), 1);
        harness.get_by_label(long).hover();
        let shown = (0..20).any(|_| {
            harness.step();
            harness.query_all_by_label(long).count() == 2
        });
        assert!(shown, "no tooltip");
    }

    /// A mouse's vertical wheel scrolls the filmstrip sideways.
    #[test]
    fn a_vertical_wheel_scrolls_the_filmstrip_sideways() {
        let dir = folder_of_30_photos();
        let mut harness = filmstrip_harness(dir.path());
        let (before, _) = film_cell(&harness, &photo(1)).unwrap();
        wheel_over_filmstrip(&mut harness, egui::vec2(0.0, -300.0));
        let (after, _) = film_cell(&harness, &photo(1)).unwrap();
        assert_eq!(after.top(), before.top(), "{before:?} {after:?}");
        assert!(after.left() < before.left() - 120.0, "{before:?} {after:?}");
    }

    /// Library's grid on the photos of `dir`, the home folder, selected,
    /// the first photo current.
    fn grid_harness(dir: &Path) -> Harness<'static, PhotographyWorkbenchApp> {
        let (mut harness, _) = harness_on(None, vec![home(dir)]);
        click(&mut harness, "Home");
        harness
    }

    /// Where the grid's tile for the photo named `label` was laid out in
    /// the last frame; `None` when it was not laid out.
    fn grid_tile(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        label: &str,
    ) -> Option<egui::Rect> {
        harness.query_by_label(label).map(|node| node.rect())
    }

    /// Whether the grid's tile for the photo named `label` was laid out
    /// in the last frame and is wholly in view.
    fn in_grid_view(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let centre = centre(harness);
        grid_tile(harness, label).is_some_and(|tile| centre.contains_rect(tile))
    }

    /// How many tiles the grid lays out in a row, of the `count` photos
    /// numbered from 1: those level with the first.
    fn tiles_in_a_row(harness: &Harness<'_, PhotographyWorkbenchApp>, count: usize) -> usize {
        let top = grid_tile(harness, &photo(1)).expect("no tile").top();
        (1..=count)
            .filter(|&n| grid_tile(harness, &photo(n)).is_some_and(|tile| tile.top() == top))
            .count()
    }

    /// The toolbar's count, when the photo numbered `n` of `of` is the
    /// current one.
    fn nth(n: usize, of: usize) -> Option<String> {
        Some(format!("{n} of {of}"))
    }

    /// Make the photo numbered `n` the current one, from outside the
    /// views, and let the app settle.
    fn make_current(harness: &mut Harness<'_, PhotographyWorkbenchApp>, n: usize) {
        harness.state_mut().browser.select_photo(n - 1);
        harness.run();
    }

    /// Click at `pos`, and let the app settle.
    fn click_at(harness: &mut Harness<'_, PhotographyWorkbenchApp>, pos: egui::Pos2) {
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        harness.run();
    }

    /// In the grid, Right and Left make the next and the previous photo
    /// the current one, which the grid outlines and the toolbar counts,
    /// stopping at the first and the last photo.
    #[test]
    fn right_and_left_move_between_the_photos_in_the_grid() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(count(&harness), nth(2, 30));
        assert!(is_selected(&harness, &photo(2)));
        assert!(!is_selected(&harness, &photo(1)));
        press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
        assert_eq!(count(&harness), nth(1, 30));
        assert!(is_selected(&harness, &photo(1)));
        press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
        assert_eq!(count(&harness), nth(1, 30), "past the first");

        make_current(&mut harness, 30);
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(count(&harness), nth(30, 30), "past the last");
        press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
        assert_eq!(count(&harness), nth(29, 30));
    }

    /// In the grid, Down and Up move a row, in the same column. Up stays
    /// put in the first row and Down in the last, and Down into a last
    /// row too short to reach the column goes to the last photo.
    #[test]
    fn down_and_up_move_a_row_in_the_grid() {
        // 31 photos, a prime number of them, never fill the last row of
        // a grid of 2 or more to a row.
        let dir = folder_of_numbered_photos(31);
        let mut harness = grid_harness(dir.path());
        let columns = tiles_in_a_row(&harness, 31);
        assert!((2..31).contains(&columns), "{columns}");
        // The number of the last row's first photo.
        let last_row = 30 / columns * columns + 1;
        assert!(last_row + columns > 32, "{last_row}");

        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(1 + columns, 31));
        assert!(is_selected(&harness, &photo(1 + columns)));
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
        assert_eq!(count(&harness), nth(1, 31));
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
        assert_eq!(count(&harness), nth(1, 31), "above the first row");

        // The last of the row before the last, with no photo below it.
        make_current(&mut harness, last_row - 1);
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(31, 31), "into the short last row");
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(31, 31), "below the last row");
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
        assert_eq!(count(&harness), nth(31 - columns, 31));

        // The first of the row before the last, with a photo below it.
        make_current(&mut harness, last_row - columns);
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(last_row, 31));
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(last_row, 31), "below the last row");
    }

    /// In the grid, End makes the last photo the current one and Home the
    /// first, and the grid scrolls just far enough to show its tile
    /// whole: the last row 4 points above the grid's bottom, the first
    /// 4 points below its top, as when the grid is scrolled to its ends.
    /// A photo made current that is in view already, as the one above
    /// the last is, leaves the grid where it is.
    #[test]
    fn home_and_end_go_to_the_first_and_the_last_photo() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        let columns = tiles_in_a_row(&harness, 30);
        assert!(!in_grid_view(&harness, &photo(30)));
        press(&mut harness, Modifiers::NONE, Key::End);
        assert_eq!(count(&harness), nth(30, 30));
        assert!(in_grid_view(&harness, &photo(30)));
        assert!(!in_grid_view(&harness, &photo(1)));
        let centre = centre(&harness);
        let last = grid_tile(&harness, &photo(30)).unwrap();
        let gap = centre.bottom() - last.bottom();
        assert!((gap - 4.0).abs() <= 0.5, "{last:?} in {centre:?}");

        let above = 30 - columns;
        assert!(in_grid_view(&harness, &photo(above)));
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
        assert_eq!(count(&harness), nth(above, 30));
        assert_eq!(grid_tile(&harness, &photo(30)), Some(last), "scrolled");

        press(&mut harness, Modifiers::NONE, Key::Home);
        assert_eq!(count(&harness), nth(1, 30));
        assert!(in_grid_view(&harness, &photo(1)));
        let first = grid_tile(&harness, &photo(1)).unwrap();
        let gap = first.top() - centre.top();
        assert!((gap - 4.0).abs() <= 0.5, "{first:?} in {centre:?}");
    }

    /// Turn the mouse wheel by `delta` points with the pointer over the
    /// grid, then let the app settle.
    fn wheel_over_grid(harness: &mut Harness<'_, PhotographyWorkbenchApp>, delta: egui::Vec2) {
        let pos = centre(harness).center();
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        wait(harness, 1.0);
    }

    /// Scrolled away from the current photo, the grid stays where it was
    /// scrolled, as the current photo has not changed since; but it shows
    /// the current photo when it comes back on screen from the viewer,
    /// and when another photo is made current there.
    #[test]
    fn the_grid_follows_the_current_photo_but_not_its_own_scrolling() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        wheel_over_grid(&mut harness, egui::vec2(0.0, -10_000.0));
        assert!(!in_grid_view(&harness, &photo(1)));
        wait(&mut harness, 3.0);
        assert!(!in_grid_view(&harness, &photo(1)));

        click(&mut harness, "Image viewer");
        click(&mut harness, "Thumbnail grid");
        assert!(in_grid_view(&harness, &photo(1)));

        click(&mut harness, "Image viewer");
        make_current(&mut harness, 30);
        click(&mut harness, "Thumbnail grid");
        assert!(in_grid_view(&harness, &photo(30)));
    }

    /// A click on a tile the grid's edge cuts makes its photo the current
    /// one and leaves the grid where it is, so the second click of a
    /// double-click lands on the same photo.
    #[test]
    fn a_click_on_a_tile_does_not_scroll_the_grid() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        let centre = centre(&harness);
        let cut = (1..=30)
            .find(|&n| {
                grid_tile(&harness, &photo(n))
                    .is_some_and(|tile| tile.intersects(centre) && !centre.contains_rect(tile))
            })
            .expect("no tile cut by the grid's edge");
        let tile = grid_tile(&harness, &photo(cut)).unwrap();
        let first = grid_tile(&harness, &photo(1)).unwrap();
        click_at(&mut harness, tile.intersect(centre).center());
        assert_eq!(count(&harness), nth(cut, 30));
        assert_eq!(grid_tile(&harness, &photo(1)), Some(first), "scrolled");
    }

    /// In the viewer, Right and Left make the next and the previous photo
    /// the current one, which the viewer shows and the filmstrip brings
    /// into view. Up, Down, Home and End are the grid's, and do nothing
    /// there.
    #[test]
    fn right_and_left_move_between_the_photos_in_the_viewer() {
        let dir = folder_of_30_photos();
        let mut harness = filmstrip_harness(dir.path());
        wheel_over_filmstrip(&mut harness, egui::vec2(-10_000.0, 0.0));
        assert!(!in_filmstrip_view(&harness, &photo(2)));
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(count(&harness), nth(2, 30));
        assert!(viewer_shows(&harness, &photo(2)));
        assert!(in_filmstrip_view(&harness, &photo(2)));
        press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
        assert_eq!(count(&harness), nth(1, 30));
        assert!(viewer_shows(&harness, &photo(1)));

        make_current(&mut harness, 16);
        for key in [Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End] {
            press(&mut harness, Modifiers::NONE, key);
            assert_eq!(count(&harness), nth(16, 30), "{key:?}");
        }
        assert_eq!(harness.state().view, View::Viewer);
    }

    /// The reference lists the keys that move between photos, under
    /// Photos.
    #[test]
    fn the_reference_lists_the_keys_that_move_between_photos() {
        let mut harness = harness();
        press(&mut harness, Modifiers::COMMAND, Key::Slash);
        harness.get_by_label("Photos");
        let rows = [
            "Previous photo",
            "Next photo",
            "Photo above",
            "Photo below",
            "First photo",
            "Last photo",
        ];
        for row in rows {
            harness.get_by_label(row);
        }
    }

    /// While a dialog is open, the keys that move between photos are the
    /// dialog's, and move none behind it.
    #[test]
    fn the_photo_keys_stand_down_while_a_dialog_is_open() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        make_current(&mut harness, 16);
        press(&mut harness, Modifiers::COMMAND, Key::Comma);
        assert!(harness.state().show_settings);
        let keys = [
            Key::ArrowRight,
            Key::ArrowLeft,
            Key::ArrowDown,
            Key::ArrowUp,
            Key::End,
            Key::Home,
        ];
        for key in keys {
            press(&mut harness, Modifiers::NONE, key);
            assert_eq!(count(&harness), nth(16, 30), "{key:?}");
        }
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert!(!harness.state().show_settings);
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(count(&harness), nth(17, 30));
    }

    /// Down pressed with Tab, which hides the side docks and so widens
    /// the grid's rows, moves nothing, rather than by a row of the length
    /// the grid had; Down pressed next moves by a row of the wider grid.
    #[test]
    fn down_with_the_docks_hidden_in_the_same_frame_moves_nothing() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        let before = tiles_in_a_row(&harness, 30);
        let events = &mut harness.input_mut().events;
        for pressed in [true, false] {
            for key in [Key::Tab, Key::ArrowDown] {
                events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
        }
        harness.run();
        assert!(!harness.state().docks.left, "Tab not done");
        assert_eq!(count(&harness), nth(1, 30));
        let after = tiles_in_a_row(&harness, 30);
        assert!(after > before, "{before} to {after} a row");
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(1 + after, 30));
    }

    /// Down pressed in the frame after the menu bar hid the side docks,
    /// before the grid was drawn again, moves nothing either.
    #[test]
    fn down_after_the_menu_bar_hid_the_docks_moves_nothing() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        let ctx = harness.ctx.clone();
        let hide = vec![Action::SideDocks];
        harness.state_mut().perform_from_menu(&ctx, hide);
        // Into the next frame itself: a press from the harness comes a
        // frame later, after one that draws the grid.
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
        harness.run();
        assert!(!harness.state().docks.left, "the menu item not done");
        assert_eq!(count(&harness), nth(1, 30));
        let columns = tiles_in_a_row(&harness, 30);
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
        assert_eq!(count(&harness), nth(1 + columns, 30));
    }

    /// Tab or Shift+Tab held down, the system repeating it, takes the
    /// keyboard focus nowhere, and shows or hides its docks once.
    #[test]
    fn a_held_tab_takes_the_keyboard_focus_nowhere() {
        for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
            let mut harness = viewer_harness();
            let docks = harness.state().docks;
            for _ in 0..3 {
                let events = &mut harness.input_mut().events;
                events.push(egui::Event::ModifiersChanged(modifiers));
                events.push(egui::Event::Key {
                    key: Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                });
                harness.step();
                let focused = harness.ctx.memory(|m| m.focused());
                assert_eq!(focused, None, "{modifiers:?}");
            }
            assert_eq!(harness.state().docks.left, !docks.left, "{modifiers:?}");
        }
    }

    /// In the viewer, where Up and Down move to no photo, they leave the
    /// keyboard focus on the control that has it too.
    #[test]
    fn up_and_down_leave_the_keyboard_focus_where_it_is_in_the_viewer() {
        let mut harness = viewer_harness();
        harness.get_by_label("Thumbnail grid").focus();
        harness.run();
        let focused = harness.ctx.memory(|m| m.focused());
        assert!(focused.is_some(), "no control has the focus");
        for key in [Key::ArrowUp, Key::ArrowDown] {
            press(&mut harness, Modifiers::NONE, key);
            assert_eq!(harness.ctx.memory(|m| m.focused()), focused, "{key:?}");
        }
    }

    /// An arrow key moves between photos while a control has the keyboard
    /// focus, and leaves the focus on that control, where egui's own
    /// arrow keys would move it to the control beside it.
    #[test]
    fn an_arrow_key_leaves_the_keyboard_focus_where_it_is() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        harness.get_by_label("Thumbnail grid").focus();
        harness.run();
        let focused = harness.ctx.memory(|m| m.focused());
        assert!(focused.is_some(), "no control has the focus");
        assert!(harness.get_by_label("Thumbnail grid").is_focused());
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(count(&harness), nth(2, 30));
        assert_eq!(harness.ctx.memory(|m| m.focused()), focused);
    }

    #[test]
    fn icon_decodes_to_a_square_of_icon_size() {
        let size = PhotographyWorkbenchApp::ICON_SIZE;
        let icon = PhotographyWorkbenchApp::icon();
        assert_eq!((icon.width, icon.height), (size, size));
        assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
    }
}
