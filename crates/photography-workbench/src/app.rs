// SPDX-License-Identifier: GPL-3.0-or-later

//! The top-level eframe application: the window's title bar, panels and
//! dialogs.

use catalog::photos::Photo;
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
use crate::info::{self, SizeUnits, TimeForm};
use crate::keymap::{self, Action, Step, Surface};
use crate::layout::{self, Docks};
use crate::menu::{self, MacMenu};
use crate::mode::{Layout, Mode};
use crate::overlay;
use crate::textures::Textures;
use crate::view::{self, View};
use crate::viewer;
use crate::zoom::{Framing, Stage, Zoom, Zooming};

/// The Lucide icons of the title bar's settings gear and of its button
/// for the reference of keyboard shortcuts, and the text shown in place
/// of each if its name does not resolve.
const TITLE_BAR_ICONS: [(&str, char); 2] = [("settings", '⚙'), ("circle-question-mark", '?')];

/// The [`eframe::Storage`] key the theme choice is saved under, as the
/// theme's [`Theme::name`], or `"System"` for none. The display name is the
/// saved value, so renaming a theme resets users who chose it to System.
const THEME_KEY: &str = "theme";

/// The [`eframe::Storage`] key how capture times are written is saved
/// under, as its [`TimeForm::name`].
const CAPTURE_TIME_KEY: &str = "capture_time";

/// The [`eframe::Storage`] key how file sizes are counted is saved under,
/// as its [`SizeUnits::name`].
const FILE_SIZES_KEY: &str = "file_sizes";

/// The [`eframe::Storage`] key the mode is saved under, as its
/// [`Mode::name`].
const MODE_KEY: &str = "mode";

/// The [`eframe::Storage`] key Library's view is saved under, as its
/// [`View::name`].
const VIEW_KEY: &str = "view";

/// The [`eframe::Storage`] key whether the viewer shows the photo's
/// information over its photo is saved under, as `"true"` or `"false"`.
const INFO_OVERLAY_KEY: &str = "show_info_overlay";

/// The application state, owned by eframe for the life of the window.
#[derive(Default)]
pub struct PhotographyWorkbenchApp {
    /// The theme chosen in Settings, `None` for System, saved between runs.
    theme: Option<Theme>,
    /// The concrete theme last installed on the context: [`Self::theme`]
    /// resolved against the OS preference. `None` until the first install.
    resolved_theme: Option<Theme>,
    /// How the Info dock, and the panel over the viewer's photo, write a
    /// capture time, chosen in Settings and saved between runs.
    capture_times: TimeForm,
    /// How the Info dock, and the panel over the viewer's photo, count a
    /// file's size, chosen in Settings and saved between runs.
    file_sizes: SizeUnits,
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
    /// The photos' thumbnails, which the grid, the filmstrip and the
    /// viewer draw, and the viewer's pictures, decoded off the window's
    /// thread.
    textures: Textures,
    /// What the viewer keeps between frames: how large it shows the photo
    /// and which part of it, kept from one photo to the next and read by
    /// the toolbar's zoom controls too, and how long it has waited for its
    /// picture, for it to say when the picture takes a while. Not saved.
    viewer: viewer::State,
    /// Whether Library's viewer shows the current photo's information on
    /// a panel over the photo, saved between runs.
    info_overlay: bool,
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
    /// style, and restores the saved theme choice, how capture times are
    /// written and file sizes counted, mode, docks and view, whether the
    /// viewer shows the photo's information over its photo, and what is
    /// expanded and selected in the folder tree; with none saved, or an
    /// unreadable one, takes the default, the photo's information hidden.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install(&cc.egui_ctx);
        style::install(&cc.egui_ctx);
        // Quitting is the keymap's, like every other shortcut: egui's own
        // Ctrl+Q would close the window even with a dialog open. On macOS
        // the menu bar's Quit takes Cmd+Q before the window sees it.
        cc.egui_ctx.options_mut(|o| o.quit_shortcuts.clear());
        // Ctrl+=, Ctrl+- and Ctrl+0 zoom the viewer's photo, as they zoom
        // a photo in the established photography tools. egui's own would
        // zoom the whole window with them wherever the viewer's keys do
        // not apply, as in the grid.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let saved = |key| cc.storage.and_then(|storage| storage.get_string(key));
        let theme =
            saved(THEME_KEY).and_then(|name| Theme::ALL.into_iter().find(|t| t.name() == name));
        let capture_times = saved(CAPTURE_TIME_KEY)
            .and_then(|name| TimeForm::from_saved(&name))
            .unwrap_or_default();
        let file_sizes = saved(FILE_SIZES_KEY)
            .and_then(|name| SizeUnits::from_saved(&name))
            .unwrap_or_default();
        let mode = saved(MODE_KEY)
            .and_then(|name| Mode::from_saved(&name))
            .unwrap_or_default();
        let view = saved(VIEW_KEY)
            .and_then(|name| View::from_saved(&name))
            .unwrap_or_default();
        let info_overlay = saved(INFO_OVERLAY_KEY).is_some_and(|value| value == "true");
        let mut app = Self {
            theme,
            capture_times,
            file_sizes,
            mode,
            docks: Docks::restore(cc.storage),
            view,
            info_overlay,
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
                let combination = keymap::key_combination_for(surface, Action::Settings);
                if chrome::icon_button(ui, palette, gear, "Settings", combination).clicked() {
                    self.perform(ui.ctx(), Action::Settings);
                }
                let label = "Keyboard shortcuts";
                let combination = keymap::key_combination_for(surface, Action::Shortcuts);
                if chrome::icon_button(ui, palette, shortcuts, label, combination).clicked() {
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

    /// Do what the keyboard shortcuts pressed in this frame ask. They stand
    /// down while a dialog is open, all but the reference's own key
    /// combination, which closes it, and while a text field has the
    /// keyboard, as what is typed there is the field's.
    ///
    /// The keys of the shortcuts that apply here are taken out of the
    /// frame's input before anything is drawn, held down as well as
    /// pressed. egui takes Enter and Space for a click on the control that
    /// has the keyboard focus, which assistive technology can give any
    /// control, so one press would otherwise do two things: Enter in the
    /// grid, with the gear focused, would open both the viewer and
    /// Settings. The keys of shortcuts standing down are left to the
    /// dialog or the field.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let surface = self.surface();
        let pressed = |action| keymap::take_pressed(ctx, surface, action);
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
        // ignore the repeats. This looks at the keys before they are
        // taken, below.
        if ctx.input(|i| i.events.iter().any(moves_focus)) {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let actions = keymap::take_pressed_actions(ctx, surface);
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
            if self.menu_state(ctx).enabled(action) {
                self.perform(ctx, action);
            }
        }
    }

    /// What the menu bar shows of the app: the mode, the view and the
    /// docks shown, which changes of the zoom can be asked for, as the
    /// toolbar's zoom controls show them, whether the viewer shows the
    /// photo's information over its photo, and whether a dialog is open.
    fn menu_state(&self, ctx: &egui::Context) -> menu::State {
        let zoom = self.shows_viewer().then(|| self.zoom_shown(ctx)).flatten();
        menu::State {
            mode: self.mode,
            view: self.view,
            parts: self.parts(),
            docks: self.docks,
            zooms: menu::Zooms::of(zoom),
            info_overlay: self.info_overlay,
            dialog: self.show_settings || self.show_about || self.show_shortcuts,
            shortcuts: self.show_shortcuts,
        }
    }

    /// Whether the centre shows Library's viewer, which alone has the
    /// zoom controls and shows the photo's information over its photo.
    fn shows_viewer(&self) -> bool {
        view::shows_viewer(self.mode, self.view)
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
            Action::Zoom(zooming) => self.zoom(ctx, zooming),
            Action::PhotoInformation => self.info_overlay = !self.info_overlay,
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

    /// The current photo, if there is one.
    fn current_photo(&self) -> Option<&Photo> {
        let Photos::Read(list) = self.browser.photos() else {
            return None;
        };
        list.photos().get(list.current()?)
    }

    /// Zoom the viewer's photo as `zooming` asks, in the room the viewer
    /// was drawn in during the frame before. Nothing outside the viewer,
    /// without a current photo, before its 100 % is known, or when the
    /// viewer was not drawn in the frame before, until it is drawn again.
    fn zoom(&mut self, ctx: &egui::Context, zooming: Zooming) {
        if self.view != View::Viewer {
            return;
        }
        let Some(path) = self.current_photo().map(|photo| photo.path().to_path_buf()) else {
            return;
        };
        let largest = self.textures.largest(&path);
        viewer::zoom(&mut self.viewer, ctx, &path, largest, zooming);
    }

    /// The zoom the toolbar shows in Library's viewer: the framing the
    /// viewer draws the current photo with, and the stage it zooms it
    /// on. `None` while the keys and the toolbar zoom nothing
    /// ([`viewer::State::stage`]).
    fn zoom_shown(&self, ctx: &egui::Context) -> Option<(Framing, Stage)> {
        let largest = self.textures.largest(self.current_photo()?.path());
        let stage = self.viewer.stage(ctx, largest)?;
        Some((self.viewer.framing(), stage))
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
                    let combination =
                        keymap::key_combination_for(self.surface(), Action::View(view));
                    if toolbar::button(ui, palette, view.icon(), view.name(), selected, combination)
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
    /// tiles, in its viewer the current photo, with its information over
    /// it while that is on and the viewer drew some texture of it, or a
    /// message where there is none to show. Nothing while a folder just
    /// chosen is being read.
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
                let shown = grid::show(
                    ui,
                    palette,
                    &mut self.textures,
                    list.photos(),
                    list.current(),
                    list.folder(),
                );
                self.grid_columns = Some(shown.columns);
                if let Some(click) = shown.click {
                    self.browser.select_photo(click.index);
                    if click.open {
                        self.perform(ui.ctx(), Action::View(View::Viewer));
                    }
                }
            }
            View::Viewer => {
                if let Some(current) = list.current() {
                    let (textures, state) = (&mut self.textures, &mut self.viewer);
                    let drew = viewer::show(ui, palette, textures, state, list.photos(), current);
                    // Once the viewer drew some texture of the photo, what a
                    // decode told of its file is known; the panel says only
                    // that, and wants nothing decoded.
                    let photo = &list.photos()[current];
                    if drew
                        && self.info_overlay
                        && let Some(details) = self.textures.details(photo.path())
                    {
                        let (times, sizes) = (self.capture_times, self.file_sizes);
                        overlay::show(ui, palette, photo, details, times, sizes);
                    }
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
            let combination = keymap::key_combination_for(self.surface(), Action::Mode(mode));
            let response = ui
                .add_enabled_ui(mode.is_available(), |ui| {
                    chrome::selectable_button(
                        ui,
                        mode.icon(),
                        mode.name(),
                        self.mode == mode,
                        combination,
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

/// The size of the toolbar's count of photos, and of its zoom.
const COUNT_SIZE: f32 = 12.5;

/// The toolbar's zoom buttons in Library's viewer, from the left: what
/// each asks for, its name, which screen readers and its tooltip give,
/// and its Lucide icon, with the text shown in its place if the name does
/// not resolve. The zoom shown sits between the first two.
const ZOOM_BUTTONS: [(Zooming, &str, (&str, char)); 4] = [
    (Zooming::Out, "Zoom out", ("zoom-out", '−')),
    (Zooming::In, "Zoom in", ("zoom-in", '+')),
    (Zooming::Fit, "Zoom to fit", ("maximize-2", '⤢')),
    (Zooming::Full, "Zoom to 100 %", ("maximize", '1')),
];

/// The width of the zoom the toolbar shows, with room to spare for its
/// widest, "800 %", so the controls beside it stay where they are as it
/// changes.
const ZOOM_LABEL_WIDTH: f32 = 52.0;

/// The room between the toolbar's count and the zoom controls, on top of
/// the toolbar's gap between two controls, so the two read apart.
const ZOOM_GAP: f32 = 6.0;

/// The zoom controls at the toolbar's end in Library's viewer, from the
/// left: zoom out, the zoom shown ([`viewer::zoom_label`]), zoom in, fit
/// and 100 %, drawn from the right, as the toolbar's end runs. `shown` is
/// the framing the viewer draws its photo with and the stage it zooms it
/// on, `None` while nothing can be zoomed, when every control is disabled
/// and no zoom is shown. The zoom shown is a whole percent, fitted too,
/// the fit's for the room the viewer was last drawn in, so it changes as
/// the window or the docks do. Zooming in and out are disabled where they
/// would change nothing ([`viewer::can_zoom`]); fit is shown selected
/// while the photo is fitted, and 100 % while the zoom shown reads 100 %,
/// fitted or not. What was clicked, if anything.
fn zoom_controls(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    shown: Option<(Framing, Stage)>,
) -> Option<Zooming> {
    let percent = shown.map(|(framing, stage)| stage.percent(framing.zoom));
    let mut clicked = None;
    let mut button = |ui: &mut egui::Ui, (zooming, name, icon): (Zooming, &str, (&str, char))| {
        let enabled = viewer::can_zoom(shown, zooming);
        let selected = match zooming {
            Zooming::Fit => shown.is_some_and(|(framing, _)| framing.zoom == Zoom::Fit),
            Zooming::Full => percent == Some(100),
            _ => false,
        };
        let combination = keymap::key_combination_for(Surface::Viewer, Action::Zoom(zooming));
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                toolbar::button(ui, palette, icon, name, selected, combination)
            })
            .inner;
        if response.clicked() {
            clicked = Some(zooming);
        }
    };
    // An id of its own, as the count beside it comes and goes, and the
    // ids egui gives by position would shift with it.
    let cluster = egui::UiBuilder::new()
        .id(egui::Id::new("zoom_controls"))
        .layout(egui::Layout::right_to_left(egui::Align::Center));
    ui.scope_builder(cluster, |ui| {
        let [out, zoom_in, fit, full] = ZOOM_BUTTONS;
        button(ui, full);
        button(ui, fit);
        button(ui, zoom_in);
        let text = egui::RichText::new(viewer::zoom_label(percent))
            .size(COUNT_SIZE)
            .color(palette.text);
        let size = egui::vec2(ZOOM_LABEL_WIDTH, ui.available_height());
        ui.add_enabled_ui(shown.is_some(), |ui| {
            ui.add_sized(size, egui::Label::new(text).selectable(false));
        });
        button(ui, out);
    });
    clicked
}

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
        let state = self.menu_state(ctx);
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
        // The thumbnails and pictures decoded since the last frame,
        // before any is drawn, and the read of photos they are wanted for.
        self.textures.begin_frame(ui.ctx());
        if let Some(read) = self.browser.photos_read() {
            self.textures.shown(read);
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
        // The current photo as the frame began, for the right dock: the
        // folder tree, drawn beside it, holds the photos while it is drawn.
        let current = self.current_photo().cloned();
        let folders = |ui: &mut egui::Ui| folder_tree::show(ui, &palette, &mut self.browser);
        let textures = &mut self.textures;
        let (times, sizes) = (self.capture_times, self.file_sizes);
        let photo_info = |ui: &mut egui::Ui| {
            info::show(ui, &palette, textures, current.as_ref(), times, sizes);
        };
        layout::side_docks(ui, parts, &mut self.docks, folders, photo_info);
        if parts.toolbar {
            let position = position(self.browser.photos());
            // The mode's own controls hold the app while the toolbar is
            // drawn, so the zoom controls read the viewer's zoom before it,
            // and a click on one is done after it, still before the centre.
            let zoom_shown = self.shows_viewer().then(|| self.zoom_shown(ui.ctx()));
            let mut zoomed = None;
            let start = |ui: &mut egui::Ui| self.mode_controls(ui, &palette);
            let end = |ui: &mut egui::Ui| {
                if let Some(position) = position {
                    let text = egui::RichText::new(position)
                        .size(COUNT_SIZE)
                        .color(palette.text);
                    ui.add(egui::Label::new(text).selectable(false));
                }
                if let Some(shown) = zoom_shown {
                    ui.add_space(ZOOM_GAP);
                    zoomed = zoom_controls(ui, &palette, shown);
                }
            };
            toolbar::show(ui, "toolbar", &palette, start, end);
            if let Some(zooming) = zoomed {
                self.perform(ui.ctx(), Action::Zoom(zooming));
            }
        }
        // The same photos as the grid's, sharing the current one and the
        // thumbnails.
        let film = |ui: &mut egui::Ui| {
            if let Photos::Read(list) = self.browser.photos() {
                let (photos, current) = (list.photos(), list.current());
                let textures = &mut self.textures;
                let clicked =
                    filmstrip::show(ui, &palette, textures, photos, current, list.folder());
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
        // Once every view of the photos is drawn, the filmstrip in the
        // bottom dock and the grid or the viewer in the centre, the Info
        // dock, if shown, wants what it needs of the photo it showed,
        // knowing what they wanted; then the textures none of them wanted
        // in this frame are not decoded.
        if parts.right_dock && self.docks.right {
            info::want(&mut self.textures, current.as_ref());
        }
        self.textures.end_frame(ui.ctx());

        // Above everything but the dialogs.
        chrome::window_resize_handles(ui.ctx());

        // The dialogs come last, above everything else. A theme picked in
        // Settings is installed at the top of the next frame, and the Info
        // dock writes its values as picked there in the next frame, which
        // the click itself makes egui run.
        if self.show_settings
            && dialogs::settings(
                ui.ctx(),
                &mut self.theme,
                &mut self.capture_times,
                &mut self.file_sizes,
            )
        {
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
        let state = self.menu_state(ui.ctx());
        if let Some(menu) = &mut self.menu {
            menu.show(state);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, theme::choice_name(self.theme).to_owned());
        storage.set_string(CAPTURE_TIME_KEY, self.capture_times.name().to_owned());
        storage.set_string(FILE_SIZES_KEY, self.file_sizes.name().to_owned());
        storage.set_string(MODE_KEY, self.mode.name().to_owned());
        self.docks.save(storage);
        storage.set_string(VIEW_KEY, self.view.name().to_owned());
        storage.set_string(INFO_OVERLAY_KEY, self.info_overlay.to_string());
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
    use egui_kittest::kittest::{NodeT as _, Queryable as _};
    use egui_kittest::wgpu::WgpuTestRenderer;
    use egui_kittest::{Harness, HarnessBuilder};

    use desktop_ui::chrome::{MAC_WINDOW_BUTTONS_WIDTH, TITLE_BAR_HEIGHT};
    use desktop_ui::keys::KeyCombination;
    use desktop_ui::{icons, theme};

    use catalog::folders::{Place, Root};

    use crate::disk::Disk;

    use super::{
        Action, CAPTURE_TIME_KEY, Docks, FILE_SIZES_KEY, Framing, INFO_OVERLAY_KEY, MODE_KEY, Mode,
        PhotographyWorkbenchApp, Side, SizeUnits, THEME_KEY, TITLE_BAR_ICONS, Textures, Theme,
        TimeForm, VIEW_KEY, View, ZOOM_BUTTONS, Zooming,
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
    /// the next frame. The thumbnails are decoded on the window's thread
    /// too, as the frame after the one that wants them begins, so a view
    /// that scrolls to another place takes a frame more to settle there,
    /// and the app is let run up to 8 frames to settle, not egui's 4.
    fn harness_on(
        storage: Option<&MemoryStorage>,
        roots: Vec<Root>,
    ) -> (Harness<'_, PhotographyWorkbenchApp>, Arc<Mutex<Vec<Root>>>) {
        harness_at(storage, roots, 1.0)
    }

    /// [`harness_on`], on a screen of `pixels_per_point` pixels a point.
    fn harness_at(
        storage: Option<&MemoryStorage>,
        roots: Vec<Root>,
        pixels_per_point: f32,
    ) -> (Harness<'_, PhotographyWorkbenchApp>, Arc<Mutex<Vec<Root>>>) {
        let builder = Harness::builder().with_pixels_per_point(pixels_per_point);
        harness_built(builder, storage, roots)
    }

    /// [`harness_on`], built by `builder`, which says the screen's pixels
    /// a point and what draws the window, if anything does.
    fn harness_built(
        builder: HarnessBuilder<PhotographyWorkbenchApp>,
        storage: Option<&MemoryStorage>,
        roots: Vec<Root>,
    ) -> (Harness<'_, PhotographyWorkbenchApp>, Arc<Mutex<Vec<Root>>>) {
        let roots = Arc::new(Mutex::new(roots));
        let disk = Disk::inline(Arc::clone(&roots));
        let harness = builder
            .with_size(PhotographyWorkbenchApp::MIN_WINDOW)
            .with_max_steps(8)
            .build_eframe(|cc| {
                if let Some(storage) = storage {
                    cc.storage = Some(storage);
                }
                let mut app = PhotographyWorkbenchApp::new(cc);
                app.browser.use_disk(disk);
                app.textures = Textures::inline();
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

        app.perform(&ctx, Action::PhotoInformation);
        assert!(app.info_overlay, "the photo's information shows");
        app.perform(&ctx, Action::PhotoInformation);
        assert!(!app.info_overlay, "and hides");

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
        let ctx = egui::Context::default();
        let state = app.menu_state(&ctx);
        assert_eq!(state.mode, Mode::Library);
        assert_eq!(state.view, View::Viewer);
        assert!(state.parts.bottom_dock, "the viewer has the filmstrip");
        assert_eq!(state.docks, docks(true, false, true));
        assert!(!state.info_overlay);
        assert!(!state.dialog && !state.shortcuts);
        app.info_overlay = true;
        assert!(app.menu_state(&ctx).info_overlay);
        for open in [
            |app: &mut PhotographyWorkbenchApp| app.show_settings = true,
            |app: &mut PhotographyWorkbenchApp| app.show_about = true,
        ] {
            let mut app = PhotographyWorkbenchApp::default();
            open(&mut app);
            let state = app.menu_state(&ctx);
            assert!(state.dialog && !state.shortcuts);
        }
        app.show_shortcuts = true;
        let state = app.menu_state(&ctx);
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

    /// The title bar's and the toolbar's buttons name in their tooltips the
    /// key combination that does what they do, as this system spells it.
    #[test]
    fn a_button_s_tooltip_names_its_combination() {
        let mut harness = viewer_harness();
        let combination = |modifiers, key| KeyCombination::new(modifiers, key).label();
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        let tips = [
            ("Library", combination(ctrl_alt, Key::Num1)),
            (
                "Keyboard shortcuts",
                combination(Modifiers::COMMAND, Key::Slash),
            ),
            ("Settings", combination(Modifiers::COMMAND, Key::Comma)),
            (LEFT_TOGGLE, combination(Modifiers::NONE, Key::F7)),
            (BOTTOM_TOGGLE, combination(Modifiers::NONE, Key::F6)),
            (RIGHT_TOGGLE, combination(Modifiers::NONE, Key::F8)),
            ("Thumbnail grid", combination(Modifiers::NONE, Key::G)),
            ("Image viewer", combination(Modifiers::NONE, Key::E)),
        ];
        for (label, combination) in tips {
            let tip = format!("{label} ({combination})");
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
        harness
            .get_by_role_and_label(Role::ComboBox, "Theme")
            .click();
        harness.run();
        harness.get_by_label("Nexus").click();
        harness.run();
        assert_eq!(harness.state().resolved_theme, Some(Theme::Nexus));

        let mut storage = MemoryStorage::default();
        harness.state_mut().save(&mut storage);
        assert_eq!(storage.get_string(THEME_KEY).as_deref(), Some("Nexus"));
    }

    /// How capture times are written and file sizes counted is saved, and
    /// restored on the next run; with none saved, or an unreadable one,
    /// each takes its default.
    #[test]
    fn the_capture_time_form_and_the_file_size_units_are_saved_and_restored() {
        let mut storage = MemoryStorage::default();
        let mut harness = harness();
        assert_eq!(harness.state().capture_times, TimeForm::Numeric);
        assert_eq!(harness.state().file_sizes, SizeUnits::Decimal);
        harness.state_mut().capture_times = TimeForm::Long;
        harness.state_mut().file_sizes = SizeUnits::Binary;
        harness.state_mut().save(&mut storage);
        let saved = |key| storage.get_string(key);
        assert_eq!(saved(CAPTURE_TIME_KEY).as_deref(), Some("Long"));
        assert_eq!(saved(FILE_SIZES_KEY).as_deref(), Some("Binary"));
        {
            let restored = harness_from(&storage);
            assert_eq!(restored.state().capture_times, TimeForm::Long);
            assert_eq!(restored.state().file_sizes, SizeUnits::Binary);
        }

        storage.set_string(CAPTURE_TIME_KEY, "Short".to_owned());
        storage.set_string(FILE_SIZES_KEY, "Metric".to_owned());
        let restored = harness_from(&storage);
        assert_eq!(restored.state().capture_times, TimeForm::Numeric);
        assert_eq!(restored.state().file_sizes, SizeUnits::Decimal);
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

    /// Whether the right dock showed something named `label` in the last
    /// frame.
    fn in_right_dock(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let dock = panel(harness, "right_dock");
        harness
            .query_all_by_label(label)
            .any(|node| dock.contains_rect(node.rect()))
    }

    /// The size on disk of the file at `path`, as the Info dock writes it
    /// in `units`.
    fn on_disk(path: &Path, units: SizeUnits) -> String {
        crate::info::file_size(std::fs::metadata(path).unwrap().len(), units)
    }

    /// Info shows a JPEG file's name, size on disk and format, when it was
    /// taken, unknown, its size and which way it is laid out, in boxes
    /// headed by what they hold, and says the camera's data is read from
    /// RAW files only.
    #[test]
    fn info_shows_what_a_jpeg_file_holds() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let size_on_disk = format!("Size on disk: {}", on_disk(&photo, SizeUnits::Decimal));
        let shown = [
            "File Information",
            "File name: DSC_0001.jpg",
            &size_on_disk,
            "Format: JPEG",
            "Image Information",
            "Capture time: —",
            "Image size: 300 × 200",
            "Orientation: Landscape",
            "Shooting Information",
            "Camera data is read from RAW files only.",
        ];
        for label in shown {
            assert!(in_right_dock(&harness, label), "{label}");
        }
        let not_shown = ["No photo selected", "Reading…", "ISO: —"];
        for label in not_shown {
            assert!(!in_right_dock(&harness, label), "{label}");
        }
        let raw_only = ["Preview size: ", "Previews: "];
        for label in raw_only {
            let dock = panel(&harness, "right_dock");
            let shown = harness
                .query_all_by_label_contains(label)
                .any(|node| dock.contains_rect(node.rect()));
            assert!(!shown, "{label}");
        }
    }

    /// A value too long for its row is cut short with "…", and shows whole
    /// under the pointer.
    #[test]
    fn info_cuts_a_long_value_short_and_shows_it_whole_under_the_pointer() {
        let dir = photos_folder();
        let name = "DSC_0001_on_the_long_walk_along_the_coast_at_dawn_with_friends.jpg";
        std::fs::write(dir.path().join(name), crate::textures::jpeg(300, 200)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let dock = panel(&harness, "right_dock");
        let cut_short = harness
            .output()
            .shapes
            .iter()
            .any(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => {
                    text.galley.text() == name && dock.contains(text.pos) && text.galley.elided
                }
                _ => false,
            });
        assert!(cut_short, "not cut short");
        // The grid's tile is named so too.
        let named = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            harness.query_all_by_label(name).count()
        };
        let before = named(&harness);
        harness.get_by_label(&format!("File name: {name}")).hover();
        let shown = (0..20).any(|_| {
            harness.step();
            named(&harness) > before
        });
        assert!(shown, "no tooltip");
    }

    /// The bytes of a NEF file as small as raw-files reads one: a TIFF
    /// header and IFD0, holding the Make a Nikon camera writes, `model`,
    /// and `preview`, a JPEG file's bytes, as the file's one preview, and
    /// pointing at an Exif IFD, which says the photo was taken at
    /// 10:20:30.85 on 1 May 2024 by a clock two hours ahead of UTC.
    fn nef(model: &str, preview: &[u8]) -> Vec<u8> {
        let make = b"NIKON CORPORATION\0".to_vec();
        let model = [model.as_bytes(), b"\0"].concat();
        let taken = b"2024:05:01 10:20:30\0".to_vec();
        let offset = b"+02:00\0".to_vec();
        // The header, then IFD0's count, its 5 entries of 12 bytes and
        // the offset of the next IFD, none; then the Exif IFD, with 3
        // entries; then the values.
        let exif_at = 8 + 2 + 5 * 12 + 4;
        let make_at = exif_at + 2 + 3 * 12 + 4;
        let model_at = make_at + make.len();
        let taken_at = model_at + model.len();
        let offset_at = taken_at + taken.len();
        let preview_at = offset_at + offset.len();
        let long = |n: usize| u32::try_from(n).unwrap().to_le_bytes();
        let mut bytes = b"II*\0".to_vec();
        bytes.extend(long(8));
        // Each entry's tag, in rising order, its type (2 for text, 4 for
        // a 32-bit number), how many values, and the value, where it fits
        // in 4 bytes, or else where it is.
        let ifd0 = [
            (0x010F_u16, 2_u16, make.len(), long(make_at)),
            (0x0110, 2, model.len(), long(model_at)),
            (0x0201, 4, 1, long(preview_at)),
            (0x0202, 4, 1, long(preview.len())),
            (0x8769, 4, 1, long(exif_at)),
        ];
        // DateTimeOriginal, OffsetTimeOriginal and SubSecTimeOriginal,
        // the hundredths of a second.
        let exif = [
            (0x9003, 2, taken.len(), long(taken_at)),
            (0x9011, 2, offset.len(), long(offset_at)),
            (0x9291, 2, 3, *b"85\0\0"),
        ];
        for ifd in [&ifd0[..], &exif] {
            bytes.extend(u16::try_from(ifd.len()).unwrap().to_le_bytes());
            for &(tag, kind, count, value) in ifd {
                bytes.extend(tag.to_le_bytes());
                bytes.extend(kind.to_le_bytes());
                bytes.extend(long(count));
                bytes.extend(value);
            }
            bytes.extend(long(0));
        }
        for value in [&make[..], &model, &taken, &offset, preview] {
            bytes.extend(value);
        }
        bytes
    }

    /// Info shows what the camera recorded of a RAW file: its make and
    /// model, as recorded, "—" for what it does not record, and the
    /// largest preview the file holds, which 100 % shows, and how many it
    /// holds; with no image size recorded, the image's size is the
    /// largest preview's.
    #[test]
    fn info_shows_what_a_raw_file_holds() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.NEF");
        let preview = crate::textures::jpeg(640, 480);
        std::fs::write(&photo, nef("NIKON Z 9", &preview)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let size_on_disk = format!("Size on disk: {}", on_disk(&photo, SizeUnits::Decimal));
        let shown = [
            "File name: DSC_0001.NEF",
            &size_on_disk,
            "Format: NEF",
            "Capture time: 2024-05-01 10:20:30.85 UTC+02:00",
            "Image size: 640 × 480",
            "Preview size: 640 × 480 (100 %)",
            "Previews: 1",
            "Orientation: Landscape",
            "Shooting Information",
            "Make: Nikon Corporation",
            "Model: Nikon Z 9",
            "Lens: —",
            "Focal length: —",
            "Exposure: —",
            "ISO: —",
            "Exposure bias: —",
        ];
        for label in shown {
            assert!(in_right_dock(&harness, label), "{label}");
        }
        let jpeg_only = "Camera data is read from RAW files only.";
        assert!(!in_right_dock(&harness, jpeg_only));
    }

    /// Opens Settings and picks `choice` in its drop-down labelled
    /// `setting`.
    fn pick_in_settings(
        harness: &mut Harness<'_, PhotographyWorkbenchApp>,
        setting: &str,
        choice: &str,
    ) {
        click(harness, "Settings");
        harness
            .get_by_role_and_label(Role::ComboBox, setting)
            .click();
        harness.run();
        click(harness, choice);
    }

    /// File sizes picked in Settings are how Info counts the size on disk
    /// at once.
    #[test]
    fn info_counts_sizes_as_picked_in_settings_at_once() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let row = |units| format!("Size on disk: {}", on_disk(&photo, units));
        assert!(in_right_dock(&harness, &row(SizeUnits::Decimal)));
        pick_in_settings(&mut harness, "File sizes", "Binary");
        assert_eq!(harness.state().file_sizes, SizeUnits::Binary);
        assert!(in_right_dock(&harness, &row(SizeUnits::Binary)));
        assert!(!in_right_dock(&harness, &row(SizeUnits::Decimal)));
    }

    /// The form of capture time picked in Settings is how Info writes a
    /// RAW file's at once.
    #[test]
    fn info_writes_capture_times_as_picked_in_settings_at_once() {
        let dir = photos_folder();
        let preview = crate::textures::jpeg(640, 480);
        std::fs::write(dir.path().join("DSC_0001.NEF"), nef("NIKON Z 9", &preview)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let numeric = "Capture time: 2024-05-01 10:20:30.85 UTC+02:00";
        let long = "Capture time: 1 May 2024 at 10:20:30.85 (UTC+02:00)";
        assert!(in_right_dock(&harness, numeric));
        pick_in_settings(&mut harness, "Capture time", "Long");
        assert_eq!(harness.state().capture_times, TimeForm::Long);
        assert!(in_right_dock(&harness, long));
        assert!(!in_right_dock(&harness, numeric));
    }

    /// Info names a RAW file's format by the file's own extension, and a
    /// JPEG file's "JPEG" by either of its extensions.
    #[test]
    fn info_names_the_format_by_the_file_s_extension() {
        for (name, format) in [("DSCN0001.nrw", "NRW"), ("IMG_0001.jpeg", "JPEG")] {
            let dir = photos_folder();
            std::fs::write(dir.path().join(name), b"not a photo").unwrap();
            let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
            click(&mut harness, "Home");
            let shown = format!("Format: {format}");
            assert!(in_right_dock(&harness, &shown), "{name}: {shown}");
        }
    }

    /// Until a decode of the current photo has told what its file holds,
    /// Info says it is reading it, or, once the photo cannot be shown,
    /// why, under File Information, which shows the file's name and
    /// format all along; the other boxes wait for the details.
    #[test]
    fn info_says_it_is_reading_until_it_knows_or_why_it_cannot() {
        let dir = photos_folder();
        std::fs::write(dir.path().join("DSC_0001.NEF"), b"not a photo").unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        let why = "This is not a kind of photo the app can show";
        let file = ["File name: DSC_0001.NEF", "Size on disk: —", "Format: NEF"];
        for label in file.iter().chain(&["Reading…"]) {
            assert!(in_right_dock(&harness, label), "{label}");
        }
        assert!(!in_right_dock(&harness, why));
        harness.state_mut().textures.stall(false);
        harness.run();
        for label in file.iter().chain(&[why]) {
            assert!(in_right_dock(&harness, label), "{label}");
        }
        let gone = ["Reading…", "Image Information", "Shooting Information"];
        for label in gone {
            assert!(!in_right_dock(&harness, label), "{label}");
        }
    }

    /// A photo whose file can no longer be shown when it is decoded again,
    /// as the photos are read again, has Info say why, as the grid shows,
    /// rather than what the decode before told of it.
    #[test]
    fn info_says_why_once_a_photo_read_again_cannot_be_shown() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        assert!(in_right_dock(&harness, "Image size: 300 × 200"));
        std::fs::write(&photo, b"not a photo").unwrap();
        let why = previews::decode(&photo, 256).unwrap_err().to_string();
        leave_and_come_back(&mut harness);
        assert!(in_right_dock(&harness, &why), "{why}");
        assert!(in_right_dock(&harness, "File name: DSC_0001.jpg"));
        let gone = [
            "Image size: 300 × 200",
            "Image Information",
            "Shooting Information",
        ];
        for label in gone {
            assert!(!in_right_dock(&harness, label), "{label}");
        }
    }

    /// On a screen of two pixels a point the grid wants the current
    /// photo's thumbnail at 512 pixels, and Info, wanting after the grid,
    /// has nothing more decoded: the grid's decode brings the details.
    #[test]
    fn info_has_nothing_decoded_of_its_own_for_a_photo_a_view_wants() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(600, 400)).unwrap();
        let (mut harness, _) = harness_at(None, vec![home(dir.path())], 2.0);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        assert_eq!(harness.state().textures.pending(), 1);
        harness.state_mut().textures.stall(false);
        harness.run();
        assert!(in_right_dock(&harness, "Image size: 600 × 400"));
        let textures = &harness.state().textures;
        assert!(textures.texture(&photo, 512).is_some());
        assert_eq!(textures.texture(&photo, 256), None);
    }

    /// Info brings the details of a photo no view draws, as the current
    /// photo once the grid is scrolled far from it, by wanting the photo
    /// decoded itself.
    #[test]
    fn info_brings_the_details_of_a_photo_no_view_draws() {
        let dir = photos_folder();
        for n in 1..=200 {
            let (width, height) = if n == 1 { (300, 200) } else { (30, 20) };
            let name = format!("DSC_{n:04}.jpg");
            std::fs::write(dir.path().join(name), crate::textures::jpeg(width, height)).unwrap();
        }
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        wheel_over_grid(&mut harness, egui::vec2(0.0, -100_000.0));
        assert!(grid_tile(&harness, "DSC_0001.jpg").is_none());
        harness.state_mut().textures.stall(false);
        harness.run();
        assert!(grid_tile(&harness, "DSC_0001.jpg").is_none());
        assert!(in_right_dock(&harness, "Image size: 300 × 200"));
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
        assert_eq!(home_arrow.center().x, dock.left() + 16.0);
        assert_eq!(dcim_arrow.center().x - home_arrow.center().x, 16.0);
    }

    /// An arrow lies whole inside the left dock, as far from its edge as
    /// the fill behind its row, whatever the folder's depth.
    #[test]
    fn the_tree_s_arrows_lie_inside_the_left_dock() {
        let dir = photos_folder();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Expand Home");
        let dock = panel(&harness, "left_dock");
        let fill = harness
            .get_by_label("Home")
            .rect()
            .shrink2(egui::vec2(8.0, 0.0));
        for label in ["Collapse Home", "Expand DCIM"] {
            let arrow = harness.get_by_label(label).rect();
            assert!(dock.contains_rect(arrow), "{label}: {arrow:?} in {dock:?}");
            assert!(
                arrow.left() >= fill.left(),
                "{label}: {arrow:?}, fill {fill:?}"
            );
        }
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

    /// Where the viewer put the photo named `label` in the last frame, if
    /// it showed it, found as the image screen readers are told of: where
    /// the photo was drawn, or, with none to draw, the room it goes in.
    fn viewer_photo(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        label: &str,
    ) -> Option<egui::Rect> {
        harness
            .query_all_by_label(label)
            .find(|node| node.accesskit_node().role() == Role::Image)
            .map(|node| node.rect())
    }

    /// Whether the viewer shows the photo named `label` in the centre.
    fn viewer_shows(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        let centre = centre(harness);
        viewer_photo(harness, label).is_some_and(|place| centre.contains_rect(place))
    }

    /// A double-click on a tile makes its photo the current one and opens
    /// it in the viewer, which shows it; so does one just after a click
    /// on another tile.
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
    /// points in from its tile's left, its middle 12 down from its top. Nothing
    /// is filled behind a tile, and the current photo, an empty file that
    /// cannot be shown, has the room its thumbnail would take outlined in
    /// the accent colour, 2 points clear of it.
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

        assert!(!chip_filled_in(&harness, first), "a fill in {first:?}");
        let room = tile_room(&harness, "DSC_0001.NEF");
        assert_eq!(current_outline(&harness, 2.0), Some(room.expand(2.0)));
        let shapes = &harness.output().shapes;
        let name_at = first.left_top() + egui::vec2(8.0, 12.0);
        let named = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => {
                let middle = text.pos.y + text.galley.size().y / 2.0;
                text.galley.text() == "DSC_0001.NEF"
                    && text.pos.x == name_at.x
                    && middle == name_at.y
            }
            _ => false,
        });
        assert!(named, "no name with its middle at {name_at:?}");
    }

    /// The textures drawn in the last frame with their middle in `room`,
    /// each with where it was drawn.
    fn textures_in(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        room: egui::Rect,
    ) -> Vec<(egui::TextureId, egui::Rect)> {
        let textured = |shape: &egui::Shape| match shape {
            egui::Shape::Rect(rect)
                if rect.fill_texture_id() != egui::TextureId::default()
                    && room.contains(rect.rect.center()) =>
            {
                Some((rect.fill_texture_id(), rect.rect))
            }
            _ => None,
        };
        let shapes = &harness.output().shapes;
        shapes
            .iter()
            .filter_map(|clipped| textured(&clipped.shape))
            .collect()
    }

    /// The one texture drawn in the last frame in `room`, and where;
    /// fails unless exactly one was.
    fn texture_in(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        room: egui::Rect,
    ) -> (egui::TextureId, egui::Rect) {
        let drawn = textures_in(harness, room);
        let [drawn] = drawn[..] else {
            panic!("{} textures drawn in {room:?}", drawn.len());
        };
        drawn
    }

    /// Whether the icon of a photo that cannot be shown, `size` points,
    /// was drawn in the last frame in the middle of `room`.
    fn failure_icon_in(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        room: egui::Rect,
        size: f32,
    ) -> bool {
        let (glyph, _) = icons::glyph("image-off", size, '?');
        harness
            .output()
            .shapes
            .iter()
            .any(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => {
                    let drawn = egui::Rect::from_min_size(text.pos, text.galley.size());
                    text.galley.text() == glyph.to_string()
                        && (drawn.center() - room.center()).length() <= 1.0
                }
                _ => false,
            })
    }

    /// Check that `drawn`, where a 3:2 thumbnail was drawn, is fitted in
    /// `room`: as wide as it, in its middle, give or take a pixel.
    fn assert_fitted_3_2(drawn: egui::Rect, room: egui::Rect) {
        assert_eq!(drawn.width(), room.width(), "{drawn:?} in {room:?}");
        assert!(
            (drawn.height() - room.width() * 2.0 / 3.0).abs() <= 1.0,
            "{drawn:?} in {room:?}"
        );
        assert!(
            (drawn.center() - room.center()).length() <= 1.0,
            "{drawn:?} in {room:?}"
        );
    }

    /// Where the grid's tile for the photo named `label` keeps its
    /// thumbnail: the tile less 10 points at each side, 24 at the top and
    /// 10 at the bottom.
    fn tile_room(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> egui::Rect {
        let tile = harness.get_by_label(label).rect();
        egui::Rect::from_min_max(
            tile.min + egui::vec2(10.0, 24.0),
            tile.max - egui::vec2(10.0, 10.0),
        )
    }

    /// Whether a rect filled with the theme's `chip` colour was painted
    /// in the last frame with its middle in `area`.
    fn chip_filled_in(harness: &Harness<'_, PhotographyWorkbenchApp>, area: egui::Rect) -> bool {
        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        harness
            .output()
            .shapes
            .iter()
            .any(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect) => {
                    rect.fill == palette.chip && area.contains(rect.rect.center())
                }
                _ => false,
            })
    }

    /// Around what the outline of the current photo, `width` points in
    /// the accent colour, was drawn in the last frame, if it was.
    fn current_outline(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        width: f32,
    ) -> Option<egui::Rect> {
        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        let stroke = egui::Stroke::new(width, palette.accent);
        harness
            .output()
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect)
                    if rect.stroke == stroke && rect.stroke_kind == egui::StrokeKind::Outside =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
    }

    /// Check that `outline` goes around `drawn` 2 points clear of it, give
    /// or take a pixel.
    fn assert_clear_around(outline: egui::Rect, drawn: egui::Rect) {
        let around = drawn.expand(2.0);
        let apart = (outline.min - around.min).length() + (outline.max - around.max).length();
        assert!(apart <= 1.0, "{outline:?} around {drawn:?}");
    }

    /// A tile has nothing behind its thumbnail, or behind the icon of a
    /// photo that cannot be shown: the grid's backdrop shows around it.
    /// The current photo's outline goes 2 points clear of its thumbnail
    /// as drawn, or of the room the thumbnail goes in when there is none
    /// to draw.
    #[test]
    fn a_tile_has_nothing_behind_its_thumbnail_and_its_outline_hugs_it() {
        let dir = photos_folder();
        std::fs::write(
            dir.path().join("DSC_0001.jpg"),
            crate::textures::jpeg(300, 200),
        )
        .unwrap();
        std::fs::write(dir.path().join("DSC_0002.NEF"), b"not a photo").unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        for label in ["DSC_0001.jpg", "DSC_0002.NEF"] {
            let tile = harness.get_by_label(label).rect();
            assert!(!chip_filled_in(&harness, tile), "a fill in {label}'s tile");
        }

        let (_, drawn) = texture_in(&harness, tile_room(&harness, "DSC_0001.jpg"));
        let outline = current_outline(&harness, 2.0).expect("no outline");
        assert_clear_around(outline, drawn);

        make_current(&mut harness, 2);
        let room = tile_room(&harness, "DSC_0002.NEF");
        assert_eq!(current_outline(&harness, 2.0), Some(room.expand(2.0)));
    }

    /// A photo's name, its middle 12 points below its tile's top, ends
    /// above the current photo's outline round a portrait thumbnail, as
    /// tall as its room, so the outline never runs through the name, its
    /// descenders and underscores included.
    #[test]
    fn a_tile_s_name_ends_above_the_outline_of_a_portrait_thumbnail() {
        let dir = photos_folder();
        let name = "DSC_0001_gyp.jpg";
        let jpeg = crate::textures::jpeg(200, 300);
        std::fs::write(dir.path().join(name), jpeg).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let room = tile_room(&harness, name);
        let (_, drawn) = texture_in(&harness, room);
        assert_eq!(drawn.height(), room.height(), "{drawn:?} in {room:?}");
        let outline = current_outline(&harness, 2.0).expect("no outline");
        // The outline's 2-point line runs outside the rect it goes round.
        let line_top = outline.top() - 2.0;
        let shapes = &harness.output().shapes;
        // The tile's, not the one in the Info dock's File row.
        let centre = centre(&harness);
        let name_box = shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.text() == name && centre.contains(text.pos) => {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        });
        let name_box = name_box.expect("no name");
        assert!(
            name_box.bottom() < line_top,
            "{name_box:?} runs into the outline at {line_top}"
        );
    }

    /// A tile shows its photo's thumbnail once it is decoded, fitted in
    /// the room under the name, its aspect kept; a photo that cannot be
    /// shown shows an icon there instead, and says why under the pointer.
    #[test]
    fn a_tile_shows_its_photo_s_thumbnail_or_why_it_cannot_be_shown() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        std::fs::write(dir.path().join("DSC_0002.NEF"), b"not a photo").unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");

        let (photo_room, broken_room) = (
            tile_room(&harness, "DSC_0001.jpg"),
            tile_room(&harness, "DSC_0002.NEF"),
        );
        let (texture, drawn) = texture_in(&harness, photo_room);
        assert_eq!(harness.state().textures.texture(&photo, 256), Some(texture));
        assert_fitted_3_2(drawn, photo_room);
        assert!(textures_in(&harness, broken_room).is_empty());
        assert!(failure_icon_in(&harness, broken_room, 32.0), "no icon");
        assert!(!failure_icon_in(&harness, photo_room, 32.0), "an icon");
        let why = "This is not a kind of photo the app can show";
        assert!(
            tooltip_shows(&mut harness, "DSC_0002.NEF", why),
            "no tooltip"
        );
    }

    /// A thumbnail's texture has no mipmaps, which the renderer does not
    /// make.
    #[test]
    fn a_thumbnail_s_texture_has_no_mipmaps() {
        let dir = photos_folder();
        std::fs::write(
            dir.path().join("DSC_0001.jpg"),
            crate::textures::jpeg(300, 200),
        )
        .unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let (texture, _) = texture_in(&harness, tile_room(&harness, "DSC_0001.jpg"));
        let manager = harness.ctx.tex_manager();
        let options = manager.read().meta(texture).unwrap().options;
        assert_eq!(options.mipmap_mode, None);
    }

    /// A photo changed while the window was in the background, as one
    /// copied in again or still being copied then, shows its thumbnail
    /// as it is now once the window is back in front.
    #[test]
    fn a_photo_changed_while_away_shows_its_new_thumbnail() {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.jpg");
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let room = tile_room(&harness, "DSC_0001.jpg");
        let (before, drawn) = texture_in(&harness, room);
        assert!(drawn.width() > drawn.height(), "{drawn:?}");

        std::fs::write(&photo, crate::textures::jpeg(200, 300)).unwrap();
        leave_and_come_back(&mut harness);
        let (after, drawn) = texture_in(&harness, room);
        assert_ne!(after, before);
        assert!(drawn.height() > drawn.width(), "{drawn:?}");
    }

    /// The number egui gave the texture `id` when it was made: a texture
    /// made later has a greater one.
    fn made(id: egui::TextureId) -> u64 {
        match id {
            egui::TextureId::Managed(number) => number,
            egui::TextureId::User(_) => panic!("{id:?} is not egui's"),
        }
    }

    /// The grid's thumbnails are decoded the current photo's first, then
    /// in reading order: the inline decoder decodes them in the order a
    /// worker would take them, and each is made a texture as it comes.
    #[test]
    fn the_grid_s_thumbnails_are_decoded_the_current_first_then_in_reading_order() {
        let dir = photos_folder();
        for n in 1..=8 {
            let name = format!("DSC_000{n}.jpg");
            std::fs::write(dir.path().join(name), crate::textures::jpeg(60, 40)).unwrap();
        }
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        make_current(&mut harness, 5);
        // Read again, so every thumbnail is decoded again, in the order
        // wanted with the fifth photo current.
        leave_and_come_back(&mut harness);
        let mut by_age: Vec<(u64, usize)> = (1..=8)
            .map(|n| {
                let room = tile_room(&harness, &format!("DSC_000{n}.jpg"));
                (made(texture_in(&harness, room).0), n)
            })
            .collect();
        by_age.sort_unstable();
        let order: Vec<usize> = by_age.into_iter().map(|(_, n)| n).collect();
        assert_eq!(order, [5, 1, 2, 3, 4, 6, 7, 8]);
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
    /// one: the viewer shows it, and the toolbar's count follows.
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
    /// from its right. Nothing is filled behind the tile; the current
    /// photo, an empty file that cannot be shown, has its tile outlined in
    /// the accent colour, 2 points clear of it.
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
        assert!(!chip_filled_in(&harness, first), "a fill in {first:?}");
        assert_eq!(current_outline(&harness, 1.5), Some(tile.expand(2.0)));
        let shapes = &harness.output().shapes;
        // The text laid out in the first cell where it says, as (its
        // left, its right, its middle), if it is there.
        let text_at = |said: &str| {
            shapes.iter().find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text)
                    if text.galley.text() == said && first.contains(text.pos) =>
                {
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

    /// A filmstrip cell's tile shows its photo's thumbnail, the same
    /// texture the grid showed, fitted in the tile, its aspect kept; a
    /// photo that cannot be shown shows an icon there instead, and says
    /// why under the pointer.
    #[test]
    fn a_film_cell_shows_its_photo_s_thumbnail_or_why_it_cannot_be_shown() {
        let dir = photos_folder();
        let (photo, broken) = (
            dir.path().join("DSC_0001.jpg"),
            dir.path().join("DSC_0002.NEF"),
        );
        std::fs::write(&photo, crate::textures::jpeg(300, 200)).unwrap();
        std::fs::write(&broken, b"not a photo").unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        let texture = harness.state().textures.texture(&photo, 256);
        let texture = texture.expect("no texture for the photo");
        click(&mut harness, "Image viewer");

        let (photo_tile, broken_tile) = (
            film_tile(&harness, "DSC_0001.jpg"),
            film_tile(&harness, "DSC_0002.NEF"),
        );
        let (drawn_texture, drawn) = texture_in(&harness, photo_tile);
        assert_eq!(
            drawn_texture, texture,
            "the grid's, of the same size, is not shared"
        );
        assert_fitted_3_2(drawn, photo_tile);
        assert!(textures_in(&harness, broken_tile).is_empty());
        assert!(failure_icon_in(&harness, broken_tile, 24.0), "no icon");
        assert!(!failure_icon_in(&harness, photo_tile, 24.0), "an icon");
        let why = "This is not a kind of photo the app can show";
        assert!(
            tooltip_shows(&mut harness, "DSC_0002.NEF", why),
            "no tooltip"
        );
    }

    /// A film cell has nothing behind its thumbnail, or behind the icon of
    /// a photo that cannot be shown: the strip's backdrop shows around
    /// it. The current photo's outline goes 2 points clear of its
    /// thumbnail as drawn, or of its tile when there is none to draw.
    #[test]
    fn a_film_cell_has_nothing_behind_its_thumbnail_and_its_outline_hugs_it() {
        let dir = photos_folder();
        std::fs::write(
            dir.path().join("DSC_0001.jpg"),
            crate::textures::jpeg(300, 200),
        )
        .unwrap();
        std::fs::write(dir.path().join("DSC_0002.NEF"), b"not a photo").unwrap();
        let mut harness = filmstrip_harness(dir.path());
        for label in ["DSC_0001.jpg", "DSC_0002.NEF"] {
            let (cell, _) = film_cell(&harness, label).expect("no cell");
            assert!(!chip_filled_in(&harness, cell), "a fill in {label}'s cell");
        }

        let (_, drawn) = texture_in(&harness, film_tile(&harness, "DSC_0001.jpg"));
        let outline = current_outline(&harness, 1.5).expect("no outline");
        assert_clear_around(outline, drawn);

        make_current(&mut harness, 2);
        let broken_tile = film_tile(&harness, "DSC_0002.NEF");
        assert_eq!(
            current_outline(&harness, 1.5),
            Some(broken_tile.expand(2.0))
        );
    }

    /// Where the filmstrip's cell for the photo named `label` keeps its
    /// thumbnail, its tile: the cell less 10 points at each side, 22 at
    /// the top and 8 at the bottom.
    fn film_tile(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> egui::Rect {
        let (cell, _) = film_cell(harness, label).expect("no cell");
        egui::Rect::from_min_max(
            cell.min + egui::vec2(10.0, 22.0),
            cell.max - egui::vec2(10.0, 8.0),
        )
    }

    /// The width and height in pixels of the texture `id`.
    fn texture_size(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
        id: egui::TextureId,
    ) -> [usize; 2] {
        let manager = harness.ctx.tex_manager();
        manager.read().meta(id).expect("no such texture").size
    }

    /// On a screen of two pixels a point, the grid draws a texture of
    /// 512 pixels, and the filmstrip, whose tiles are smaller, one of 256
    /// made for it, not the grid's drawn at half its size.
    #[test]
    fn each_view_draws_a_texture_made_for_its_size() {
        let dir = photos_folder();
        let jpeg = crate::textures::jpeg(800, 600);
        std::fs::write(dir.path().join("DSC_0001.jpg"), jpeg).unwrap();
        let (mut harness, _) = harness_at(None, vec![home(dir.path())], 2.0);
        click(&mut harness, "Home");
        let (grid, _) = texture_in(&harness, tile_room(&harness, "DSC_0001.jpg"));
        assert_eq!(texture_size(&harness, grid), [512, 384]);
        click(&mut harness, "Image viewer");
        let (film, _) = texture_in(&harness, film_tile(&harness, "DSC_0001.jpg"));
        assert_eq!(texture_size(&harness, film), [256, 192]);
    }

    /// The filmstrip's thumbnails are decoded the current photo's first,
    /// then the nearest it first, of two as near the one after it first.
    #[test]
    fn the_filmstrip_s_thumbnails_are_decoded_nearest_the_current_photo_first() {
        let dir = photos_folder();
        for n in 1..=5 {
            let name = format!("DSC_000{n}.jpg");
            std::fs::write(dir.path().join(name), crate::textures::jpeg(60, 40)).unwrap();
        }
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        click(&mut harness, "Image viewer");
        make_current(&mut harness, 3);
        for n in 1..=5 {
            let name = format!("DSC_000{n}.jpg");
            assert!(in_filmstrip_view(&harness, &name), "{name}");
        }
        // Read again, so every thumbnail is decoded again, in the order
        // wanted with the third photo current.
        leave_and_come_back(&mut harness);
        let mut by_age: Vec<(u64, usize)> = (1..=5)
            .map(|n| {
                let tile = film_tile(&harness, &format!("DSC_000{n}.jpg"));
                (made(texture_in(&harness, tile).0), n)
            })
            .collect();
        by_age.sort_unstable();
        let order: Vec<usize> = by_age.into_iter().map(|(_, n)| n).collect();
        assert_eq!(order, [3, 4, 2, 5, 1]);
    }

    /// A photo's name too long for its cell is cut short before its
    /// format, which stays whole, and the name shows whole in a tooltip.
    #[test]
    fn a_long_name_in_the_filmstrip_shows_whole_in_a_tooltip() {
        let dir = photos_folder();
        let long = "Z_holiday_photo_from_a_phone_camera.jpg";
        std::fs::write(dir.path().join("DSC_0001.NEF"), b"").unwrap();
        // A photo that can be shown, so its tooltip holds its name alone.
        let jpeg = crate::textures::jpeg(300, 200);
        std::fs::write(dir.path().join(long), jpeg).unwrap();
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

    /// Holding Right goes on through the photos, a photo each time the
    /// system repeats the key, and Tab held with it shows or hides the
    /// side docks once.
    #[test]
    fn a_held_arrow_key_goes_on_through_the_photos() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        let docks = harness.state().docks;
        for _ in 0..3 {
            // egui counts a key that goes down again while it is down as
            // the system repeating it.
            for key in [Key::ArrowRight, Key::Tab] {
                harness.input_mut().events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            harness.step();
        }
        harness.run();
        assert_eq!(count(&harness), nth(4, 30));
        assert_ne!(harness.state().docks, docks, "Tab not done");
        assert_eq!(harness.state().docks.left, !docks.left, "Tab done twice");
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

    /// In the grid, Enter opens the current photo in the viewer, as a
    /// double-click on its tile does, and Esc goes back to the grid with
    /// the same photo current. Enter just after a click on a view button
    /// opens the viewer and does nothing else: egui takes Enter for a
    /// click on the control that has the keyboard focus, and a click
    /// gives a button none.
    #[test]
    fn enter_opens_the_current_photo_in_the_viewer_and_esc_goes_back() {
        let dir = folder_of_30_photos();
        let mut harness = grid_harness(dir.path());
        make_current(&mut harness, 3);
        press(&mut harness, Modifiers::NONE, Key::Enter);
        assert_eq!(harness.state().view, View::Viewer);
        assert!(viewer_shows(&harness, &photo(3)));
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert_eq!(harness.state().view, View::Grid);
        assert_eq!(count(&harness), nth(3, 30));

        click(&mut harness, "Thumbnail grid");
        let docks = harness.state().docks;
        press(&mut harness, Modifiers::NONE, Key::Enter);
        let app = harness.state();
        assert_eq!(app.view, View::Viewer);
        assert_eq!((app.mode, app.docks), (Mode::Library, docks));
        assert!(!app.show_settings && !app.show_about && !app.show_shortcuts);
        assert_eq!(count(&harness), nth(3, 30));
    }

    /// Give the control named `label` the keyboard focus, as assistive
    /// technology gives it, and let the app settle.
    fn focus(harness: &mut Harness<'_, PhotographyWorkbenchApp>, label: &str) {
        harness.get_by_label(label).focus();
        harness.run();
        assert!(harness.get_by_label(label).is_focused(), "{label}");
    }

    /// Enter in the grid opens the viewer and does nothing else, whichever
    /// control has the keyboard focus, though egui takes Enter for a
    /// click on it: the gear opens no Settings, a dock's toggle hides no
    /// dock, and the grid's button does not switch back to the grid.
    #[test]
    fn enter_in_the_grid_does_not_also_press_the_focused_control() {
        let dir = folder_of_photos();
        for label in ["Settings", LEFT_TOGGLE, "Thumbnail grid"] {
            let mut harness = grid_harness(dir.path());
            focus(&mut harness, label);
            let docks = harness.state().docks;
            press(&mut harness, Modifiers::NONE, Key::Enter);
            let app = harness.state();
            assert_eq!(app.view, View::Viewer, "{label}");
            assert_eq!((app.mode, app.docks), (Mode::Library, docks), "{label}");
            let dialog = app.show_settings || app.show_about || app.show_shortcuts;
            assert!(!dialog, "{label}");
        }
    }

    /// A key that does nothing where it is pressed still reaches the
    /// control that has the keyboard focus: in the viewer, where Enter
    /// opens nothing, Enter on the gear opens Settings.
    #[test]
    fn a_key_with_no_shortcut_here_presses_the_focused_control() {
        let mut harness = viewer_harness();
        focus(&mut harness, "Settings");
        press(&mut harness, Modifiers::NONE, Key::Enter);
        assert!(harness.state().show_settings);
        assert_eq!(harness.state().view, View::Viewer);
    }

    /// Esc goes back to the grid from the viewer with a control focused,
    /// and egui still takes the keyboard focus away from it, as it does
    /// on Esc before the keys are looked at.
    #[test]
    fn esc_goes_back_to_the_grid_and_still_takes_the_focus_away() {
        let mut harness = viewer_harness();
        focus(&mut harness, "Settings");
        press(&mut harness, Modifiers::NONE, Key::Escape);
        assert_eq!(harness.state().view, View::Grid);
        assert!(!harness.state().show_settings);
        assert_eq!(harness.ctx.memory(|m| m.focused()), None);
    }

    /// The room the viewer fits its photo in: the centre less 24 points
    /// on every side.
    fn viewer_room(harness: &Harness<'_, PhotographyWorkbenchApp>) -> egui::Rect {
        centre(harness).shrink(24.0)
    }

    /// Check that `drawn`, where a photo `aspect` times as wide as it is
    /// high was drawn, is fitted in `room`: as wide as it or as high, in
    /// its middle, its aspect kept, give or take a pixel.
    fn assert_fitted(drawn: egui::Rect, room: egui::Rect, aspect: f32) {
        let near = |a: f32, b: f32| (a - b).abs() <= 1.0;
        let fits = room.expand(0.5).contains_rect(drawn);
        let reaches = near(drawn.width(), room.width()) || near(drawn.height(), room.height());
        assert!(fits && reaches, "{drawn:?} in {room:?}");
        assert!(near(drawn.width(), drawn.height() * aspect), "{drawn:?}");
        assert!(
            (drawn.center() - room.center()).length() <= 1.0,
            "{drawn:?} in {room:?}"
        );
    }

    /// The viewer draws the current photo's picture as large as the
    /// centre allows less 24 points on every side, enlarged from its
    /// pixels, its aspect kept, in the middle: a wide photo across the
    /// room, a tall one from its top to its bottom. Right steps to the
    /// next photo, which it then draws.
    #[test]
    fn the_viewer_shows_the_current_photo_as_large_as_the_centre_allows() {
        let dir = photos_folder();
        let (wide, tall) = (
            dir.path().join("DSC_0001.jpg"),
            dir.path().join("DSC_0002.jpg"),
        );
        std::fs::write(&wide, crate::textures::jpeg(300, 200)).unwrap();
        std::fs::write(&tall, crate::textures::jpeg(200, 300)).unwrap();
        let mut harness = filmstrip_harness(dir.path());
        let room = viewer_room(&harness);
        assert!(room.width() > 512.0, "{room:?}");

        let place = viewer_photo(&harness, "DSC_0001.jpg").expect("no photo");
        assert_fitted(place, room, 1.5);
        let (texture, drawn) = texture_in(&harness, room);
        let side = picture_side(&harness);
        assert_eq!(harness.state().textures.texture(&wide, side), Some(texture));
        assert_fitted(drawn, room, 1.5);
        assert!(drawn.width() > 300.0, "not enlarged: {drawn:?}");

        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert!(viewer_photo(&harness, "DSC_0001.jpg").is_none());
        let place = viewer_photo(&harness, "DSC_0002.jpg").expect("no photo");
        assert_fitted(place, room, 2.0 / 3.0);
        let (texture, drawn) = texture_in(&harness, room);
        assert_eq!(harness.state().textures.texture(&tall, side), Some(texture));
        assert_fitted(drawn, room, 2.0 / 3.0);
    }

    /// A photo that cannot be shown shows an icon in the middle of the
    /// viewer, and says why under the pointer.
    #[test]
    fn the_viewer_says_why_a_photo_cannot_be_shown() {
        let dir = photos_folder();
        std::fs::write(dir.path().join("broken.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        let mut harness = filmstrip_harness(dir.path());
        let room = viewer_room(&harness);
        assert!(textures_in(&harness, room).is_empty());
        assert!(failure_icon_in(&harness, room, 48.0), "no icon");
        let why = "The file is damaged";
        // The Info dock says why too, from the start.
        let tooltip = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            let dock = panel(harness, "right_dock");
            harness
                .query_all_by_label(why)
                .any(|node| !dock.contains_rect(node.rect()))
        };
        assert!(!tooltip(&harness));
        harness
            .query_all_by_label("broken.jpg")
            .find(|node| node.accesskit_node().role() == Role::Image)
            .expect("no photo")
            .hover();
        let shown = (0..20).any(|_| {
            harness.step();
            tooltip(&harness)
        });
        assert!(shown, "no tooltip");
    }

    /// The longest side of the picture the viewer wants: its room's long
    /// side in pixels, rounded up to a step of 256 and no longer than the
    /// largest texture egui takes, which in the harness is egui's own
    /// default of 2048.
    fn picture_side(harness: &Harness<'_, PhotographyWorkbenchApp>) -> u32 {
        let points = viewer_room(harness).size().max_elem();
        let max_side = harness.ctx.input(|i| i.max_texture_side);
        crate::viewer::picture_side(points, harness.ctx.pixels_per_point(), max_side)
    }

    /// [`photos_folder`], holding beside its folders the photos
    /// `DSC_0001.jpg` to `DSC_0003.jpg`, JPEGs `width` by `height` pixels,
    /// and their paths.
    fn folder_of_jpegs(width: u32, height: u32) -> (tempfile::TempDir, [std::path::PathBuf; 3]) {
        let dir = photos_folder();
        let paths = [1, 2, 3].map(|n| dir.path().join(format!("DSC_000{n}.jpg")));
        for path in &paths {
            std::fs::write(path, crate::textures::jpeg(width, height)).unwrap();
        }
        (dir, paths)
    }

    /// The viewer draws the current photo's picture, decoded as long as
    /// its room in pixels, rounded up to a step, not a thumbnail; until
    /// the picture comes, a thumbnail of the photo held stands in for it.
    /// Once it has come, the next photo's picture is decoded too, so Right
    /// shows that photo sharp at once, with nothing more decoded.
    #[test]
    fn the_viewer_draws_the_photo_s_picture_and_a_thumbnail_until_it_comes() {
        let (dir, photos) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        click(&mut harness, "Home");
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Image viewer");
        let room = viewer_room(&harness);
        let (texture, _) = texture_in(&harness, room);
        let grid_s = harness.state().textures.texture(&photos[0], 256);
        assert_eq!(grid_s, Some(texture), "not the grid's thumbnail");

        harness.state_mut().textures.stall(false);
        harness.run();
        let side = picture_side(&harness);
        let (texture, drawn) = texture_in(&harness, room);
        let textures = &harness.state().textures;
        assert_eq!(textures.texture(&photos[0], side), Some(texture));
        assert_eq!(texture_size(&harness, texture)[0], side as usize);
        assert_fitted(drawn, room, 1.5);
        let next = textures.texture(&photos[1], side).expect("no next picture");

        harness.state_mut().textures.stall(true);
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert_eq!(texture_in(&harness, room).0, next);
    }

    /// The grid draws thumbnails, never a picture the viewer held: back
    /// from the viewer on a screen of two pixels a point, a photo whose
    /// thumbnail of the grid's size is on its way shows the smaller one
    /// the filmstrip held, not its picture, nearer that size, which the
    /// viewer held as the next photo's.
    #[test]
    fn the_grid_draws_a_thumbnail_never_a_picture() {
        let (dir, photos) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_at(None, vec![home(dir.path())], 2.0);
        harness.state_mut().view = View::Viewer;
        click(&mut harness, "Home");
        let side = picture_side(&harness);
        let textures = &harness.state().textures;
        assert!(textures.texture(&photos[1], side).is_some(), "no picture");
        assert_eq!(textures.texture(&photos[1], 512), None);
        let small = textures.texture(&photos[1], 256).expect("no thumbnail");

        harness.state_mut().textures.stall(true);
        press(&mut harness, Modifiers::NONE, Key::Escape);
        let room = tile_room(&harness, "DSC_0002.jpg");
        assert_eq!(texture_in(&harness, room).0, small);
    }

    /// A current photo whose picture cannot be decoded still has the next
    /// photo's picture, and the one before's, made ready, as soon as that
    /// is known.
    #[test]
    fn the_neighbours_are_made_ready_when_the_picture_cannot_be_decoded() {
        let (dir, photos) = folder_of_jpegs(600, 400);
        let fourth = dir.path().join("DSC_0004.jpg");
        std::fs::write(&fourth, crate::textures::jpeg(600, 400)).unwrap();
        let mut harness = filmstrip_harness(dir.path());
        let side = picture_side(&harness);
        assert_eq!(harness.state().textures.texture(&fourth, side), None);
        let ctx = harness.ctx.clone();
        let damaged = previews::Failure::Damaged;
        harness
            .state_mut()
            .textures
            .fail(&ctx, &photos[2], side, damaged);
        make_current(&mut harness, 3);
        let textures = &harness.state().textures;
        assert_eq!(textures.texture(&photos[2], side), None, "decoded");
        assert!(textures.texture(&fourth, side).is_some(), "no next picture");
        assert!(textures.texture(&photos[1], side).is_some(), "no previous");
    }

    /// A photo whose thumbnails and whose picture at one size cannot be
    /// decoded keeps saying why, and says nothing of loading however long
    /// it waits, while its picture is decoded at another size, as once the
    /// window is resized; and the pictures of its neighbours at that size
    /// are made ready meanwhile.
    #[test]
    fn a_photo_that_cannot_be_shown_says_why_while_another_size_is_tried() {
        let (dir, photos) = folder_of_jpegs(600, 400);
        std::fs::write(&photos[1], [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        let mut harness = filmstrip_harness(dir.path());
        make_current(&mut harness, 2);
        assert!(failure_icon_in(&harness, viewer_room(&harness), 48.0));
        let before = picture_side(&harness);

        harness.state_mut().textures.stall(true);
        harness.set_size(PhotographyWorkbenchApp::MIN_WINDOW + egui::vec2(600.0, 400.0));
        let start = harness.ctx.input(|i| i.time) + 1.0;
        for later in [0.0, 0.5, 2.0] {
            frame_at(&mut harness, start + later);
            let room = viewer_room(&harness);
            assert!(failure_icon_in(&harness, room, 48.0), "{later}: no icon");
            assert!(!says_loading(&harness), "{later}");
        }
        let side = picture_side(&harness);
        assert_ne!(side, before);
        harness.state_mut().textures.stall(false);
        harness.step();
        let textures = &harness.state().textures;
        assert!(textures.texture(&photos[0], side).is_some(), "no previous");
        assert!(textures.texture(&photos[2], side).is_some(), "no next");
    }

    /// Whether the viewer says its picture is loading.
    fn says_loading(harness: &Harness<'_, PhotographyWorkbenchApp>) -> bool {
        harness.query_by_label("Loading…").is_some()
    }

    /// Run a frame at `time` seconds of egui's time, which stands still
    /// from then on between the frames a test runs.
    fn frame_at(harness: &mut Harness<'_, PhotographyWorkbenchApp>, time: f64) {
        harness.input_mut().time = Some(time);
        harness.step();
    }

    /// How long after the last frame the window asks for the next one.
    fn repaint_delay(harness: &Harness<'_, PhotographyWorkbenchApp>) -> std::time::Duration {
        harness.output().viewport_output[&ViewportId::ROOT].repaint_delay
    }

    /// The viewer on the third of [`folder_of_jpegs`], made current with
    /// the decoder stalled, so its picture stays on its way, the time
    /// standing still at the time returned, when it was made current.
    fn viewer_on_the_third_stalled(dir: &Path) -> (Harness<'static, PhotographyWorkbenchApp>, f64) {
        let mut harness = filmstrip_harness(dir);
        harness.state_mut().textures.stall(true);
        harness.state_mut().browser.select_photo(2);
        let now = harness.ctx.input(|i| i.time) + 1.0;
        frame_at(&mut harness, now);
        (harness, now)
    }

    /// When the picture of the photo stepped to takes half a second, the
    /// viewer says it is loading, under the photo, and not before; it asks
    /// for a frame for when it will, so an idle window says it on time.
    /// Once the picture comes, it says so no longer, and asks for no
    /// frame.
    #[test]
    fn the_viewer_says_it_is_loading_when_the_picture_takes_a_while() {
        let (dir, photos) = folder_of_jpegs(600, 400);
        let (mut harness, start) = viewer_on_the_third_stalled(dir.path());
        let room = viewer_room(&harness);
        let stand_in = harness.state().textures.texture(&photos[2], 256);
        assert_eq!(Some(texture_in(&harness, room).0), stand_in);
        assert!(!says_loading(&harness));
        // egui asks for the frame a frame's time early, which is a
        // quarter of a second in the harness.
        frame_at(&mut harness, start);
        assert_eq!(repaint_delay(&harness).as_secs_f64(), 0.5 - 0.25);
        frame_at(&mut harness, start + 0.375);
        assert!(!says_loading(&harness));
        frame_at(&mut harness, start + 0.5);
        assert!(says_loading(&harness));
        let label = harness.get_by_label("Loading…").rect();
        assert!(label.top() >= room.bottom(), "{label:?} over {room:?}");
        assert!(
            (label.center().x - room.center().x).abs() <= 1.0,
            "{label:?}"
        );

        harness.state_mut().textures.stall(false);
        frame_at(&mut harness, start + 0.75);
        harness.run();
        assert!(!says_loading(&harness));
        let picture = harness
            .state()
            .textures
            .texture(&photos[2], picture_side(&harness));
        assert_eq!(Some(texture_in(&harness, room).0), picture);
        assert_eq!(repaint_delay(&harness), std::time::Duration::MAX);
    }

    /// When the picture of the photo cannot be decoded while a thumbnail
    /// of it is held, the viewer shows the thumbnail, however long it
    /// waits says nothing of loading and asks for no frame for it, and
    /// shows no icon of a photo that cannot be shown.
    #[test]
    fn the_viewer_says_nothing_of_loading_when_the_picture_cannot_be_decoded() {
        let (dir, photos) = folder_of_jpegs(600, 400);
        let mut harness = filmstrip_harness(dir.path());
        harness.state_mut().textures.stall(true);
        let ctx = harness.ctx.clone();
        let side = picture_side(&harness);
        let damaged = previews::Failure::Damaged;
        harness
            .state_mut()
            .textures
            .fail(&ctx, &photos[2], side, damaged);
        harness.state_mut().browser.select_photo(2);
        let start = harness.ctx.input(|i| i.time) + 1.0;
        for later in [0.0, 0.5, 2.0] {
            frame_at(&mut harness, start + later);
            assert!(!says_loading(&harness), "{later}");
        }
        assert_eq!(repaint_delay(&harness), std::time::Duration::MAX);
        let room = viewer_room(&harness);
        let stand_in = harness.state().textures.texture(&photos[2], 256);
        assert_eq!(Some(texture_in(&harness, room).0), stand_in);
        assert!(!failure_icon_in(&harness, room, 48.0), "an icon");
    }

    /// The viewer on the photos of [`folder_of_jpegs`], JPEGs 2400 by 1600
    /// pixels, on a screen of `pixels_per_point`, the first photo current
    /// and its picture and its neighbour's decoded, and their paths.
    fn zoom_harness(
        pixels_per_point: f32,
    ) -> (
        Harness<'static, PhotographyWorkbenchApp>,
        tempfile::TempDir,
        [std::path::PathBuf; 3],
    ) {
        let (dir, photos) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_at(None, vec![home(dir.path())], pixels_per_point);
        harness.state_mut().view = View::Viewer;
        click(&mut harness, "Home");
        (harness, dir, photos)
    }

    /// The one texture the viewer drew in the last frame, and where,
    /// wherever the zoom put it: the one drawn showing only the room.
    fn viewer_texture(
        harness: &Harness<'_, PhotographyWorkbenchApp>,
    ) -> (egui::TextureId, egui::Rect) {
        let room = viewer_room(harness).expand(0.5);
        let drawn: Vec<_> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect)
                    if rect.fill_texture_id() != egui::TextureId::default()
                        && room.contains_rect(clipped.clip_rect) =>
                {
                    Some((rect.fill_texture_id(), rect.rect))
                }
                _ => None,
            })
            .collect();
        let [drawn] = drawn[..] else {
            panic!("{} textures drawn in the viewer", drawn.len());
        };
        drawn
    }

    /// Whether `a` and `b` are the same within `by`.
    fn near(a: egui::Vec2, b: egui::Vec2, by: f32) -> bool {
        (a - b).length() <= by
    }

    /// The point of the photo drawn at `drawn` under `at`, in fractions of
    /// its width and height.
    fn under(drawn: egui::Rect, at: egui::Pos2) -> egui::Vec2 {
        (at - drawn.min) / drawn.size()
    }

    /// The input of a click at `pos`, `clicks` times over, for one frame.
    fn push_clicks(
        harness: &mut Harness<'_, PhotographyWorkbenchApp>,
        pos: egui::Pos2,
        clicks: usize,
    ) {
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        for _ in 0..clicks {
            for pressed in [true, false] {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
        }
    }

    /// Turn the wheel by `delta` in `unit`, with `modifiers` held and the
    /// pointer at `pos`, then let the smoothing run out.
    fn wheel_at(
        harness: &mut Harness<'_, PhotographyWorkbenchApp>,
        pos: egui::Pos2,
        unit: egui::MouseWheelUnit,
        delta: egui::Vec2,
        modifiers: Modifiers,
    ) {
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(pos));
        events.push(egui::Event::MouseWheel {
            unit,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers,
        });
        wait(harness, 1.0);
    }

    /// Drag from `from` by `by`, in four moves of a frame each, and let
    /// the app settle.
    fn drag(harness: &mut Harness<'_, PhotographyWorkbenchApp>, from: egui::Pos2, by: egui::Vec2) {
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(from));
        events.push(button(from, true));
        harness.step();
        for step in 1..=4 {
            let pos = from + by * step as f32 / 4.0;
            harness
                .input_mut()
                .events
                .push(egui::Event::PointerMoved(pos));
            harness.step();
        }
        harness.input_mut().events.push(button(from + by, false));
        harness.run();
    }

    /// A click on the photo shows it at 100 %, each of its pixels on a
    /// pixel of the screen, the point clicked at the room's middle and its
    /// corner on a whole pixel, on a screen of one pixel a point and of
    /// two; a click on it then fits it again, gliding there over the
    /// time a click's move takes.
    #[test]
    fn a_click_shows_the_photo_at_100_percent_and_another_fits_it() {
        for ppp in [1.0, 2.0] {
            let (mut harness, _dir, _) = zoom_harness(ppp);
            let room = viewer_room(&harness);
            let (_, fitted) = viewer_texture(&harness);
            assert_fitted(fitted, room, 1.5);
            let at = room.center() + egui::vec2(40.0, 30.0);
            let point = under(fitted, at);
            click_at(&mut harness, at);
            let (_, full) = viewer_texture(&harness);
            let size = egui::vec2(2400.0, 1600.0) / ppp;
            assert!(near(full.size(), size, 0.01), "{ppp}: {full:?}");
            assert!(
                near(under(full, room.center()), point, 1.0 / size.y),
                "{ppp}: {full:?}"
            );
            let corner = full.min.to_vec2() * ppp;
            assert_eq!(corner, corner.round(), "{ppp}: {full:?}");

            let start = harness.ctx.input(|i| i.time) + 1.0;
            push_clicks(&mut harness, room.center(), 1);
            frame_at(&mut harness, start);
            assert!(near(viewer_texture(&harness).1.size(), size, 0.01), "{ppp}");
            frame_at(&mut harness, start + crate::zoom::GLIDE / 2.0);
            let (_, half) = viewer_texture(&harness);
            let between = (half.width() - (fitted.width() + size.x) / 2.0).abs() <= 1.0;
            assert!(between, "{ppp}: {half:?}");
            frame_at(&mut harness, start + crate::zoom::GLIDE);
            assert_fitted(viewer_texture(&harness).1, room, 1.5);
        }
    }

    /// A double-click on the photo zooms it once, whether its clicks come
    /// in frames of their own or in one: in, and in again from fitted,
    /// never in and straight back out.
    #[test]
    fn a_double_click_zooms_once() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let full = egui::vec2(2400.0, 1600.0);
        let start = harness.ctx.input(|i| i.time) + 1.0;
        push_clicks(&mut harness, room.center(), 1);
        frame_at(&mut harness, start);
        push_clicks(&mut harness, room.center(), 1);
        frame_at(&mut harness, start + 0.1);
        frame_at(&mut harness, start + 0.5);
        assert!(near(viewer_texture(&harness).1.size(), full, 0.01));
        push_clicks(&mut harness, room.center(), 2);
        frame_at(&mut harness, start + 2.0);
        frame_at(&mut harness, start + 2.5);
        assert_fitted(viewer_texture(&harness).1, room, 1.5);
    }

    /// A drag moves a photo larger than the room with the pointer, never
    /// so far that its edge comes inside the room's, and a fitted photo
    /// not at all.
    #[test]
    fn a_drag_moves_a_zoomed_photo_within_its_bounds() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let (_, fitted) = viewer_texture(&harness);
        drag(&mut harness, room.center(), egui::vec2(-100.0, -60.0));
        assert_eq!(viewer_texture(&harness).1, fitted);

        click_at(&mut harness, room.center());
        let (_, before) = viewer_texture(&harness);
        drag(&mut harness, room.center(), egui::vec2(-100.0, -60.0));
        let (_, after) = viewer_texture(&harness);
        let moved = after.min - before.min;
        assert!(near(moved, egui::vec2(-100.0, -60.0), 1.0), "{moved:?}");
        drag(&mut harness, room.center(), egui::vec2(5000.0, 5000.0));
        let (_, far) = viewer_texture(&harness);
        assert!(
            near(far.min.to_vec2(), room.min.to_vec2(), 0.75),
            "{far:?} in {room:?}"
        );
        assert!(far.contains_rect(room), "{far:?} in {room:?}");
    }

    /// A notch of the mouse wheel, three of them here, zooms the photo by
    /// about 1.22 each about the pointer, the point of the photo under it
    /// staying there, smoothed over a few frames rather than at once.
    #[test]
    fn the_mouse_wheel_zooms_about_the_pointer() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let (_, fitted) = viewer_texture(&harness);
        let at = room.center() + egui::vec2(60.0, 40.0);
        let point = under(fitted, at);
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(at));
        events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 3.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        harness.step();
        let factor = 0.6_f32.exp();
        let (_, first) = viewer_texture(&harness);
        assert!(
            first.width() > fitted.width() && first.width() < fitted.width() * factor - 1.0,
            "not smoothed: {first:?}"
        );
        wait(&mut harness, 1.0);
        let (_, zoomed) = viewer_texture(&harness);
        assert!(
            near(zoomed.size(), fitted.size() * factor, 0.5),
            "{zoomed:?}"
        );
        assert!(near(under(zoomed, at), point, 0.002), "{zoomed:?}");
    }

    /// The wheel with Ctrl or Cmd held zooms the photo as much as the
    /// wheel alone, counted once, and so does a trackpad's pinch, by as
    /// much as it says; neither touches egui's zoom of the whole window.
    #[test]
    fn ctrl_and_the_wheel_or_a_pinch_zoom_the_photo_once_and_not_the_window() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let (_, fitted) = viewer_texture(&harness);
        let line = egui::MouseWheelUnit::Line;
        wheel_at(
            &mut harness,
            room.center(),
            line,
            egui::vec2(0.0, 1.0),
            Modifiers::COMMAND,
        );
        let (_, zoomed) = viewer_texture(&harness);
        let factor = 0.2_f32.exp();
        assert!(
            near(zoomed.size(), fitted.size() * factor, 0.5),
            "{zoomed:?}"
        );
        assert_eq!(harness.ctx.zoom_factor(), 1.0);

        harness.input_mut().events.push(egui::Event::Zoom(1.5));
        harness.run();
        let (_, pinched) = viewer_texture(&harness);
        assert!(
            near(pinched.size(), zoomed.size() * 1.5, 0.5),
            "{pinched:?}"
        );
        assert_eq!(harness.ctx.zoom_factor(), 1.0);
    }

    /// The wheel's events are read while the photo's 100 % is not known
    /// yet, so the scrolling egui smooths over the frames after them is
    /// taken for what it is once it is: a mouse's notches turned just
    /// before then zoom the photo.
    #[test]
    fn the_wheel_is_read_before_the_photo_s_100_percent_is_known() {
        let (dir, photos) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        click(&mut harness, "Image viewer");
        assert!(
            viewer_photo(&harness, "DSC_0001.jpg").is_some(),
            "no viewer"
        );
        let room = viewer_room(&harness);
        let events = &mut harness.input_mut().events;
        events.push(egui::Event::PointerMoved(room.center()));
        events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 3.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        harness.step();
        assert_eq!(harness.state().textures.largest(&photos[0]), None);
        harness.state_mut().textures.stall(false);
        wait(&mut harness, 1.0);
        let (_, drawn) = viewer_texture(&harness);
        let fitted = room.width().min(room.height() * 1.5);
        assert!(
            drawn.width() > fitted + 1.0,
            "not zoomed: {drawn:?} in {room:?}"
        );
    }

    /// A trackpad's two fingers, scrolling in points, move a zoomed photo
    /// with them, and leave a fitted one where it is.
    #[test]
    fn a_trackpad_s_scrolling_moves_a_zoomed_photo_and_not_a_fitted_one() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let (_, fitted) = viewer_texture(&harness);
        let point = egui::MouseWheelUnit::Point;
        let delta = egui::vec2(-30.0, -20.0);
        wheel_at(&mut harness, room.center(), point, delta, Modifiers::NONE);
        assert_eq!(viewer_texture(&harness).1, fitted);

        click_at(&mut harness, room.center());
        let (_, before) = viewer_texture(&harness);
        wheel_at(&mut harness, room.center(), point, delta, Modifiers::NONE);
        let (_, after) = viewer_texture(&harness);
        assert_eq!(after.size(), before.size());
        let moved = after.min - before.min;
        assert!(near(moved, delta, 1.0), "{moved:?}");
    }

    /// Zoomed past its picture, the photo is decoded at its largest, here
    /// as long as the largest texture egui takes in the harness, 2048
    /// pixels, and drawn from that; fitted again, it is drawn from its
    /// picture.
    #[test]
    fn zoomed_past_its_picture_the_photo_is_drawn_at_its_largest() {
        let (mut harness, _dir, photos) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let side = picture_side(&harness);
        assert!(side < 2048, "{side}");
        let picture = harness.state().textures.texture(&photos[0], side);
        assert_eq!(Some(viewer_texture(&harness).0), picture);
        assert_eq!(harness.state().textures.texture(&photos[0], 2048), None);
        click_at(&mut harness, room.center());
        let largest = harness.state().textures.texture(&photos[0], 2048);
        assert!(largest.is_some(), "not decoded at its largest");
        assert_eq!(Some(viewer_texture(&harness).0), largest);
        assert_eq!(texture_size(&harness, largest.unwrap()), [2048, 1365]);
        assert_eq!(harness.state().textures.texture(&photos[1], 2048), None);
        click_at(&mut harness, room.center());
        assert_eq!(Some(viewer_texture(&harness).0), picture);
    }

    /// While the photo at its largest is on its way, its picture stands
    /// in, and half a second after it was first wanted, as the click's
    /// move grew the photo past its picture, the viewer says it is
    /// loading; once it comes, it is drawn, and the viewer says so no
    /// longer.
    #[test]
    fn the_viewer_says_it_is_loading_while_the_largest_takes_a_while() {
        let (mut harness, _dir, photos) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let picture = harness
            .state()
            .textures
            .texture(&photos[0], picture_side(&harness));
        harness.state_mut().textures.stall(true);
        let start = harness.ctx.input(|i| i.time) + 1.0;
        push_clicks(&mut harness, room.center(), 1);
        frame_at(&mut harness, start);
        // The click's move is over, and the largest wanted from then.
        let wanted = start + crate::zoom::GLIDE;
        frame_at(&mut harness, wanted);
        frame_at(&mut harness, wanted + 0.375);
        assert_eq!(Some(viewer_texture(&harness).0), picture);
        assert!(!says_loading(&harness));
        frame_at(&mut harness, wanted + 0.5);
        assert!(says_loading(&harness));
        harness.state_mut().textures.stall(false);
        frame_at(&mut harness, wanted + 0.75);
        frame_at(&mut harness, wanted + 1.0);
        assert!(!says_loading(&harness));
        let largest = harness.state().textures.texture(&photos[0], 2048);
        assert!(largest.is_some(), "not decoded at its largest");
        assert_eq!(Some(viewer_texture(&harness).0), largest);
    }

    /// Zoomed, the photo at its largest stays drawn when the photos are
    /// read again, as the window comes back to the front, while it and the
    /// picture are decoded again, rather than the picture standing in.
    #[test]
    fn the_largest_stays_drawn_while_the_photos_are_read_again() {
        let (mut harness, _dir, photos) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        click_at(&mut harness, room.center());
        let largest = harness.state().textures.texture(&photos[0], 2048);
        assert!(largest.is_some(), "not decoded at its largest");
        harness.state_mut().textures.stall(true);
        leave_and_come_back(&mut harness);
        assert_eq!(Some(viewer_texture(&harness).0), largest);
    }

    /// When the photo cannot be decoded at its largest, its picture stays
    /// shown, zoomed, and the viewer says nothing of loading.
    #[test]
    fn a_largest_that_cannot_be_decoded_leaves_the_picture_shown() {
        let (mut harness, _dir, photos) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let picture = harness
            .state()
            .textures
            .texture(&photos[0], picture_side(&harness));
        let ctx = harness.ctx.clone();
        let damaged = previews::Failure::Damaged;
        harness
            .state_mut()
            .textures
            .fail(&ctx, &photos[0], 2048, damaged);
        let start = harness.ctx.input(|i| i.time) + 1.0;
        push_clicks(&mut harness, room.center(), 1);
        for later in [0.0, 0.5, 2.0] {
            frame_at(&mut harness, start + later);
            assert!(!says_loading(&harness), "{later}");
        }
        let (texture, drawn) = viewer_texture(&harness);
        assert_eq!(Some(texture), picture);
        assert!(
            near(drawn.size(), egui::vec2(2400.0, 1600.0), 0.01),
            "{drawn:?}"
        );
        assert_eq!(repaint_delay(&harness), std::time::Duration::MAX);
    }

    /// Right keeps the zoom and the pan on the next photo, which is drawn
    /// at 100 % where the one before was, and at its largest.
    #[test]
    fn right_keeps_the_zoom_on_the_next_photo() {
        let (mut harness, _dir, photos) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        click_at(&mut harness, room.center() + egui::vec2(-80.0, 50.0));
        let (_, before) = viewer_texture(&harness);
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
        assert!(viewer_shows(&harness, "DSC_0002.jpg"));
        let (texture, after) = viewer_texture(&harness);
        assert_eq!(after, before);
        let largest = harness.state().textures.texture(&photos[1], 2048);
        assert_eq!(Some(texture), largest);
    }

    /// What the toolbar's zoom says: the text between its zoom out and
    /// zoom in buttons.
    fn zoom_shown(harness: &Harness<'_, PhotographyWorkbenchApp>) -> String {
        let out = harness.get_by_label("Zoom out").rect();
        let zoom_in = harness.get_by_label("Zoom in").rect();
        harness
            .query_all_by_role(Role::Label)
            .filter(|node| {
                let rect = node.rect();
                rect.left() >= out.right() && rect.right() <= zoom_in.left()
            })
            .find_map(|node| node.accesskit_node().value())
            .expect("no zoom shown")
    }

    /// Whether the control named `label` is shown on, as the toolbar's
    /// fit and 100 % are when the photo is fitted or at 100 %.
    fn is_on(harness: &Harness<'_, PhotographyWorkbenchApp>, label: &str) -> bool {
        harness.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True)
    }

    /// The scale the photos of [`zoom_harness`] fit at in the viewer's
    /// room as it was last laid out.
    fn fit(harness: &Harness<'_, PhotographyWorkbenchApp>) -> f32 {
        let room = viewer_room(harness);
        let ppp = harness.ctx.pixels_per_point();
        ppp * (room.width() / 2400.0).min(room.height() / 1600.0)
    }

    /// What the toolbar says of the photos of [`zoom_harness`] fitted: the
    /// fit's whole percent.
    fn fitted_shown(harness: &Harness<'_, PhotographyWorkbenchApp>) -> String {
        format!("{} %", (fit(harness) * 100.0).round())
    }

    /// The scale the first step past the fit of the photos of
    /// [`zoom_harness`] goes to, which the toolbar reads in whole percent.
    fn first_step(harness: &Harness<'_, PhotographyWorkbenchApp>) -> f32 {
        let shown = (fit(harness) * 100.0).round() / 100.0;
        crate::zoom::STEPS
            .into_iter()
            .find(|&step| step > shown)
            .expect("a step past the fit")
    }

    /// The input of `key` pressed and let go, with nothing held, for one
    /// frame.
    fn push_key(harness: &mut Harness<'_, PhotographyWorkbenchApp>, key: Key) {
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
    }

    /// Z and Space each show the fitted photo at 100 %, each of its pixels
    /// a pixel of the screen, its middle at the room's middle, and fit it
    /// again, gliding there as after a click; the toolbar says which, and
    /// shows its fit or its 100 % on.
    #[test]
    fn z_and_space_switch_between_fitted_and_100_percent() {
        for ppp in [1.0, 2.0] {
            let (mut harness, _dir, _) = zoom_harness(ppp);
            let room = viewer_room(&harness);
            let (_, fitted) = viewer_texture(&harness);
            assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
            assert!(is_on(&harness, "Zoom to fit"));
            let size = egui::vec2(2400.0, 1600.0) / ppp;
            for key in [Key::Z, Key::Space] {
                let start = harness.ctx.input(|i| i.time) + 1.0;
                push_key(&mut harness, key);
                frame_at(&mut harness, start);
                frame_at(&mut harness, start + crate::zoom::GLIDE / 2.0);
                let (_, half) = viewer_texture(&harness);
                let between = half.width() > fitted.width() + 1.0 && half.width() < size.x - 1.0;
                assert!(between, "{ppp} {key:?}: not gliding: {half:?}");
                assert_eq!(
                    zoom_shown(&harness),
                    "100 %",
                    "{ppp} {key:?}: not the target"
                );
                frame_at(&mut harness, start + crate::zoom::GLIDE);
                let (_, full) = viewer_texture(&harness);
                assert!(near(full.size(), size, 0.01), "{ppp} {key:?}: {full:?}");
                let middle = under(full, room.center());
                assert!(near(middle, egui::vec2(0.5, 0.5), 0.001), "{ppp} {key:?}");
                assert!(is_on(&harness, "Zoom to 100 %"));
                assert!(!is_on(&harness, "Zoom to fit"));
                push_key(&mut harness, key);
                frame_at(&mut harness, start + 1.0);
                frame_at(&mut harness, start + 1.0 + crate::zoom::GLIDE);
                assert_fitted(viewer_texture(&harness).1, room, 1.5);
                assert_eq!(
                    zoom_shown(&harness),
                    fitted_shown(&harness),
                    "{ppp} {key:?}"
                );
            }
        }
    }

    /// Fitted, the toolbar says the fit's whole percent for the room the
    /// photo is shown in, and says another when the room changes, as when
    /// the right dock is hidden.
    #[test]
    fn the_fitted_zoom_reads_the_fit_s_percent_for_the_room() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        // The scale the photo is drawn at, read from where it is drawn, in
        // whole percent.
        let drawn_at = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            let drawn = viewer_photo(harness, "DSC_0001.jpg").expect("no photo");
            let scale = drawn.width() * harness.ctx.pixels_per_point() / 2400.0;
            format!("{} %", (scale * 100.0).round())
        };
        let before = zoom_shown(&harness);
        assert_eq!(before, drawn_at(&harness));
        press(&mut harness, Modifiers::NONE, Key::F8);
        let after = zoom_shown(&harness);
        assert_eq!(after, drawn_at(&harness));
        assert_ne!(after, before);
        assert!(is_on(&harness, "Zoom to fit"));
    }

    /// Ctrl+= zooms the fitted photo in to the first step past the fit,
    /// keeping its middle at the room's middle, and so does Ctrl++, typed
    /// with Shift on the `=` key as a US keyboard types it; Ctrl+- zooms
    /// it out to fitted again. The toolbar says so.
    #[test]
    fn ctrl_equals_and_ctrl_minus_zoom_in_and_out_by_a_step() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let step = first_step(&harness);
        let percent = format!("{} %", (step * 100.0).round());
        // `key` typed on the keyboard's key `physical` with `modifiers`
        // held, going down and coming back up.
        let typed = |key, physical, modifiers| {
            [true, false].map(|pressed| egui::Event::Key {
                key,
                physical_key: Some(physical),
                pressed,
                repeat: false,
                modifiers,
            })
        };
        let ctrl = Modifiers::COMMAND;
        let zoom_in = [
            typed(Key::Equals, Key::Equals, ctrl),
            typed(Key::Plus, Key::Equals, ctrl | Modifiers::SHIFT),
        ];
        for (n, events) in zoom_in.into_iter().enumerate() {
            harness.input_mut().events.extend(events);
            harness.run();
            assert_eq!(zoom_shown(&harness), percent, "{n}");
            let (_, zoomed) = viewer_texture(&harness);
            let size = egui::vec2(2400.0, 1600.0) * step;
            assert!(near(zoomed.size(), size, 0.01), "{n}: {zoomed:?}");
            let middle = under(zoomed, room.center());
            assert!(near(middle, egui::vec2(0.5, 0.5), 0.001), "{n}: {zoomed:?}");
            press(&mut harness, Modifiers::COMMAND, Key::Minus);
            assert_eq!(zoom_shown(&harness), fitted_shown(&harness), "{n}");
            assert_fitted(viewer_texture(&harness).1, room, 1.5);
        }
    }

    /// Ctrl+Alt+0 shows the photo at 100 % and Ctrl+0 fits it, whatever
    /// the zoom.
    #[test]
    fn ctrl_alt_0_shows_100_percent_and_ctrl_0_fits() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        press(&mut harness, ctrl_alt, Key::Num0);
        assert_eq!(zoom_shown(&harness), "100 %");
        let full = egui::vec2(2400.0, 1600.0);
        assert!(near(viewer_texture(&harness).1.size(), full, 0.01));
        press(&mut harness, Modifiers::COMMAND, Key::Equals);
        assert_eq!(zoom_shown(&harness), "200 %");
        press(&mut harness, Modifiers::COMMAND, Key::Num0);
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
        assert_fitted(viewer_texture(&harness).1, room, 1.5);
        press(&mut harness, Modifiers::COMMAND, Key::Num0);
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
    }

    /// The toolbar's buttons zoom as the keys do: in by a step and out
    /// again, to 100 % and to fit; zooming out is disabled while the photo
    /// is fitted, and zooming in at 800 %.
    #[test]
    fn the_toolbar_s_zoom_buttons_zoom_as_the_keys_do() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let room = viewer_room(&harness);
        let disabled = |harness: &Harness<'_, PhotographyWorkbenchApp>, label| {
            harness.get_by_label(label).accesskit_node().is_disabled()
        };
        assert!(disabled(&harness, "Zoom out"));
        let step = first_step(&harness);
        click(&mut harness, "Zoom in");
        assert_eq!(
            zoom_shown(&harness),
            format!("{} %", (step * 100.0).round())
        );
        assert!(!disabled(&harness, "Zoom out"));
        click(&mut harness, "Zoom out");
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
        click(&mut harness, "Zoom to 100 %");
        assert_eq!(zoom_shown(&harness), "100 %");
        assert!(is_on(&harness, "Zoom to 100 %"));
        let full = egui::vec2(2400.0, 1600.0);
        assert!(near(viewer_texture(&harness).1.size(), full, 0.01));
        for _ in 0..3 {
            click(&mut harness, "Zoom in");
        }
        assert_eq!(zoom_shown(&harness), "800 %");
        assert!(disabled(&harness, "Zoom in"));
        click(&mut harness, "Zoom to fit");
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
        assert!(is_on(&harness, "Zoom to fit"));
        assert_fitted(viewer_texture(&harness).1, room, 1.5);
    }

    /// The menu bar's zoom items can be chosen exactly when the toolbar's
    /// controls that do the same are enabled: none in the grid, before the
    /// viewer is shown or after, nor in the viewer before the photo's
    /// 100 % is known; zooming out not while the photo is fitted, nor
    /// zooming in at 800 %.
    #[test]
    fn the_zoom_s_menu_items_follow_its_controls() {
        let zoomings = [Zooming::In, Zooming::Out, Zooming::Fit, Zooming::Full];
        let labels = ["Zoom in", "Zoom out", "Zoom to fit", "Zoom to 100 %"];
        let items = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            let state = harness.state().menu_state(&harness.ctx);
            zoomings.map(|zooming| state.enabled(Action::Zoom(zooming)))
        };
        let controls = |harness: &Harness<'_, PhotographyWorkbenchApp>| {
            labels.map(|label| !harness.get_by_label(label).accesskit_node().is_disabled())
        };
        let (dir, _) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        assert_eq!(items(&harness), [false; 4], "the grid");
        click(&mut harness, "Image viewer");
        assert_eq!(items(&harness), [false; 4], "100 % not known");
        assert_eq!(controls(&harness), [false; 4], "100 % not known");

        harness.state_mut().textures.stall(false);
        harness.run();
        assert_eq!(items(&harness), [true, false, true, true], "fitted");
        assert_eq!(controls(&harness), items(&harness), "fitted");
        // A step at a time, past the last step, which does nothing.
        for _ in crate::zoom::STEPS {
            click(&mut harness, "Zoom in");
        }
        assert_eq!(zoom_shown(&harness), "800 %");
        assert_eq!(items(&harness), [false, true, true, true], "800 %");
        assert_eq!(controls(&harness), items(&harness), "800 %");
        click(&mut harness, "Thumbnail grid");
        assert_eq!(items(&harness), [false; 4], "back in the grid");
    }

    /// A zoom item of the menu bar zooms exactly as its key does, one
    /// change after another.
    #[test]
    fn a_zoom_menu_item_zooms_as_its_key_does() {
        let (mut keyed, _dir, _) = zoom_harness(1.0);
        let (mut chosen, _other_dir, _) = zoom_harness(1.0);
        let changes = [
            (Zooming::In, Modifiers::COMMAND, Key::Equals),
            (Zooming::In, Modifiers::COMMAND, Key::Equals),
            (Zooming::Out, Modifiers::COMMAND, Key::Minus),
            (
                Zooming::Full,
                Modifiers::COMMAND | Modifiers::ALT,
                Key::Num0,
            ),
            (Zooming::In, Modifiers::COMMAND, Key::Equals),
            (Zooming::Fit, Modifiers::COMMAND, Key::Num0),
        ];
        for (zooming, modifiers, key) in changes {
            press(&mut keyed, modifiers, key);
            let ctx = chosen.ctx.clone();
            chosen
                .state_mut()
                .perform_from_menu(&ctx, vec![Action::Zoom(zooming)]);
            chosen.run();
            let framing =
                |harness: &Harness<'_, PhotographyWorkbenchApp>| harness.state().viewer.framing();
            assert_eq!(framing(&chosen), framing(&keyed), "{zooming:?}");
            assert_eq!(zoom_shown(&chosen), zoom_shown(&keyed), "{zooming:?}");
            assert_eq!(viewer_texture(&chosen).1, viewer_texture(&keyed).1);
        }
        assert_eq!(zoom_shown(&chosen), fitted_shown(&chosen));
    }

    /// The zoom controls are the viewer's: the grid has none.
    #[test]
    fn the_zoom_controls_come_with_the_viewer_only() {
        let (dir, _) = folder_of_jpegs(600, 400);
        let mut harness = grid_harness(dir.path());
        for label in ["Zoom out", "Zoom in", "Zoom to fit", "Zoom to 100 %"] {
            assert!(harness.query_by_label(label).is_none(), "{label}");
        }
        click(&mut harness, "Image viewer");
        for label in ["Zoom out", "Zoom in", "Zoom to fit", "Zoom to 100 %"] {
            assert!(harness.query_by_label(label).is_some(), "{label}");
        }
    }

    /// Until a decode of the photo has told its 100 %, the zoom controls
    /// are disabled, the toolbar shows no zoom, and the keys zoom
    /// nothing; once one has, they zoom.
    #[test]
    fn the_zoom_waits_for_the_photo_s_100_percent() {
        let (dir, _) = folder_of_jpegs(2400, 1600);
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        click(&mut harness, "Image viewer");
        for label in ["Zoom out", "Zoom in", "Zoom to fit", "Zoom to 100 %"] {
            let node = harness.get_by_label(label).accesskit_node();
            assert!(node.is_disabled(), "{label}");
            assert_ne!(node.toggled(), Some(Toggled::True), "{label}");
        }
        assert_eq!(zoom_shown(&harness), "");
        press(&mut harness, Modifiers::NONE, Key::Z);
        press(&mut harness, Modifiers::COMMAND, Key::Equals);
        assert_eq!(harness.state().viewer.framing(), Framing::default());

        harness.state_mut().textures.stall(false);
        harness.run();
        assert!(
            !harness
                .get_by_label("Zoom in")
                .accesskit_node()
                .is_disabled()
        );
        press(&mut harness, Modifiers::NONE, Key::Z);
        assert_eq!(zoom_shown(&harness), "100 %");
    }

    /// The keys and the toolbar zoom in the room the viewer was drawn in
    /// during the frame before, and in none once a frame has gone by
    /// without it: back from the grid, the window resized meanwhile, they
    /// zoom nothing until the viewer is drawn again, and then zoom in its
    /// new room.
    #[test]
    fn the_zoom_waits_for_the_viewer_to_be_drawn_again() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let before = viewer_room(&harness);
        press(&mut harness, Modifiers::NONE, Key::G);
        assert_eq!(harness.state().view, View::Grid);
        let size = PhotographyWorkbenchApp::MIN_WINDOW + egui::vec2(200.0, 100.0);
        harness.set_size(size);
        harness.run();

        // Back in the viewer, before it is drawn.
        harness.state_mut().view = View::Viewer;
        let ctx = harness.ctx.clone();
        assert_eq!(harness.state().zoom_shown(&ctx), None, "an old room");
        harness
            .state_mut()
            .perform(&ctx, Action::Zoom(crate::zoom::Zooming::Full));
        assert_eq!(harness.state().viewer.framing(), Framing::default());

        harness.run();
        let room = viewer_room(&harness);
        assert_ne!(room, before);
        let largest = previews::Size {
            width: 2400,
            height: 1600,
        };
        let (_, stage) = harness.state().zoom_shown(&ctx).expect("no zoom");
        assert_eq!(stage, crate::zoom::Stage::new(largest, room, 1.0));
        press(&mut harness, Modifiers::NONE, Key::Z);
        assert_eq!(zoom_shown(&harness), "100 %");
        let (_, full) = viewer_texture(&harness);
        let middle = under(full, room.center());
        assert!(near(middle, egui::vec2(0.5, 0.5), 0.001), "{full:?}");
    }

    /// Back in the viewer from the menu bar, in a frame with no input of
    /// its own and none asked for after it, the toolbar shows the zoom and
    /// its controls can be chosen at once: the viewer has the pass run
    /// again when its room is new to the toolbar. Once it is drawn, with
    /// its room the same, a frame runs a pass only.
    #[test]
    fn the_zoom_shows_at_once_back_in_the_viewer_from_the_menu_bar() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        press(&mut harness, Modifiers::NONE, Key::G);
        assert_eq!(harness.state().view, View::Grid);
        let ctx = harness.ctx.clone();
        harness
            .state_mut()
            .perform_from_menu(&ctx, vec![Action::View(View::Viewer)]);
        harness.try_run().expect("the window never settles");
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
        let zoom_in = harness.get_by_label("Zoom in").accesskit_node();
        assert!(!zoom_in.is_disabled());
        let before = ctx.cumulative_pass_nr();
        harness.step();
        assert_eq!(ctx.cumulative_pass_nr(), before + 1, "run again");
    }

    /// Whether egui painted its warning that passes were run again in many
    /// frames in a row, in the last frame.
    fn warns_of_passes_run_again(harness: &Harness<'_, PhotographyWorkbenchApp>) -> bool {
        harness
            .output()
            .shapes
            .iter()
            .any(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => text.galley.text().contains("PERF WARNING"),
                _ => false,
            })
    }

    /// While the window is resized, the viewer's room changing in every
    /// frame, each frame runs a single pass, and egui has nothing to warn
    /// of.
    #[test]
    fn a_resize_runs_each_frame_once() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let ctx = harness.ctx.clone();
        for step in 1..=6 {
            let grown = egui::vec2(10.0, 5.0) * step as f32;
            harness.set_size(PhotographyWorkbenchApp::MIN_WINDOW + grown);
            let before = ctx.cumulative_pass_nr();
            harness.step();
            assert_eq!(ctx.cumulative_pass_nr(), before + 1, "{step}");
            assert!(!warns_of_passes_run_again(&harness), "{step}");
        }
    }

    /// The window resized on an idle viewer has the toolbar show the new
    /// fit's zoom once it settles: the frame that finds the room changed
    /// asks for the next one.
    #[test]
    fn a_resized_idle_viewer_shows_the_new_fit() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let before = zoom_shown(&harness);
        // Nothing decoded asks for a frame meanwhile.
        harness.state_mut().textures.stall(true);
        harness.set_size(PhotographyWorkbenchApp::MIN_WINDOW + egui::vec2(400.0, 300.0));
        harness.run();
        let after = zoom_shown(&harness);
        assert_eq!(after, fitted_shown(&harness));
        assert_ne!(after, before);
    }

    /// A dock shown or hidden from the menu bar on an idle viewer, its
    /// room changing, has the toolbar show the new fit's zoom once the
    /// window settles.
    #[test]
    fn a_dock_toggled_from_the_menu_bar_shows_the_new_fit() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let before = zoom_shown(&harness);
        // Nothing decoded asks for a frame meanwhile.
        harness.state_mut().textures.stall(true);
        let ctx = harness.ctx.clone();
        harness
            .state_mut()
            .perform_from_menu(&ctx, vec![Action::Dock(Side::Right)]);
        harness.run();
        let drawn = viewer_photo(&harness, "DSC_0001.jpg").expect("no photo");
        let scale = drawn.width() * harness.ctx.pixels_per_point() / 2400.0;
        let after = zoom_shown(&harness);
        assert_eq!(after, format!("{} %", (scale * 100.0).round()));
        assert_ne!(after, before);
    }

    /// Space in the viewer switches between fitted and 100 % once, and
    /// does not also press the control that has the keyboard focus,
    /// though egui takes Space for a click on it: zooming in does not zoom
    /// in, and the grid's button does not switch to the grid.
    #[test]
    fn space_does_not_also_press_the_focused_control() {
        for label in ["Zoom in", "Thumbnail grid"] {
            let (mut harness, _dir, _) = zoom_harness(1.0);
            focus(&mut harness, label);
            press(&mut harness, Modifiers::NONE, Key::Space);
            assert_eq!(harness.state().view, View::Viewer, "{label}");
            assert_eq!(zoom_shown(&harness), "100 %", "{label}");
        }
    }

    /// Space held down, the system repeating it, switches between fitted
    /// and 100 % once.
    #[test]
    fn a_held_space_switches_once() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        for frame in 0..3 {
            // egui counts a key that goes down again while it is down as
            // repeating.
            harness.input_mut().events.push(egui::Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            harness.step();
            assert_eq!(zoom_shown(&harness), "100 %", "{frame}");
        }
    }

    /// The zoom buttons name in their tooltips the key combination that does
    /// what they do, as this system spells it.
    #[test]
    fn a_zoom_button_s_tooltip_names_its_combination() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let combination = |modifiers, key| KeyCombination::new(modifiers, key).label();
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        press(&mut harness, Modifiers::COMMAND, Key::Equals);
        let tips = [
            ("Zoom out", combination(Modifiers::COMMAND, Key::Minus)),
            ("Zoom in", combination(Modifiers::COMMAND, Key::Equals)),
            ("Zoom to fit", combination(Modifiers::COMMAND, Key::Num0)),
            ("Zoom to 100 %", combination(ctrl_alt, Key::Num0)),
        ];
        for (label, combination) in tips {
            let tip = format!("{label} ({combination})");
            assert!(tooltip_shows(&mut harness, label, &tip), "{tip}");
        }
    }

    /// The zoom shown takes the same width whatever it says, so the
    /// controls beside it stay where they are as it changes.
    #[test]
    fn the_zoom_controls_stay_put_as_the_zoom_changes() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        let out = harness.get_by_label("Zoom out").rect();
        let zoom_in = harness.get_by_label("Zoom in").rect();
        assert_eq!(zoom_in.left() - out.right(), 52.0 + 2.0 * 6.0);
        for _ in 0..5 {
            press(&mut harness, Modifiers::COMMAND, Key::Equals);
            assert_eq!(harness.get_by_label("Zoom out").rect(), out);
        }
        assert_eq!(zoom_shown(&harness), "800 %");
    }

    /// With both side docks at their widest, at the window's smallest, the
    /// toolbar holds the view buttons, the zoom controls and a count of a
    /// thousand photos, none over another.
    #[test]
    fn the_zoom_controls_fit_the_narrowest_toolbar() {
        let dir = folder_of_numbered_photos(1000);
        let mut harness = filmstrip_harness(dir.path());
        for id in ["left_dock", "right_dock"] {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 100.0));
            let state = egui::PanelState { outer_rect: rect };
            harness
                .ctx
                .data_mut(|d| d.insert_persisted(egui::Id::new(id), state));
        }
        harness.run();
        let toolbar = panel(&harness, "toolbar");
        let viewer = harness.get_by_label("Image viewer").rect();
        let out = harness.get_by_label("Zoom out").rect();
        let full = harness.get_by_label("Zoom to 100 %").rect();
        let count = harness.get_by_label("1 of 1000").rect();
        assert!(viewer.right() < out.left(), "{viewer:?} {out:?}");
        assert!(full.right() < count.left(), "{full:?} {count:?}");
        assert!(count.right() <= toolbar.right(), "{count:?} {toolbar:?}");
    }

    /// Ctrl+=, Ctrl+- and Ctrl+0 leave the whole window's size alone where
    /// they zoom no photo, as in the grid, though egui's own keys zoom it.
    #[test]
    fn the_zoom_keys_never_zoom_the_window() {
        let (dir, _) = folder_of_jpegs(600, 400);
        let mut harness = grid_harness(dir.path());
        for key in [Key::Equals, Key::Minus, Key::Equals, Key::Num0] {
            press(&mut harness, Modifiers::COMMAND, key);
            assert_eq!(harness.ctx.zoom_factor(), 1.0, "{key:?}");
        }
    }

    #[test]
    fn every_zoom_icon_resolves() {
        for (_, label, (name, _)) in ZOOM_BUTTONS {
            assert!(icons::resolves(name), "{label}: {name}");
        }
    }

    /// The node of the panel over the viewer's photo in the last frame;
    /// `None` while it is not shown.
    fn info_overlay_node<'h>(
        harness: &'h Harness<'_, PhotographyWorkbenchApp>,
    ) -> Option<egui_kittest::Node<'h>> {
        let id = crate::overlay::id().accesskit_id();
        harness
            .query_all_by_role(Role::Label)
            .find(|node| node.accesskit_node().locate().0 == id)
    }

    /// What the panel over the viewer's photo said in the last frame, a
    /// row of it to a line; `None` while it is not shown.
    fn info_overlay(harness: &Harness<'_, PhotographyWorkbenchApp>) -> Option<String> {
        info_overlay_node(harness).and_then(|node| node.accesskit_node().value())
    }

    /// [`photos_folder`], holding beside its folders `DSC_0001.NEF`, a RAW
    /// file of the camera `model` ([`nef`]) whose one preview is 640 by
    /// 480 pixels, and the file's path.
    fn nef_folder(model: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = photos_folder();
        let photo = dir.path().join("DSC_0001.NEF");
        let preview = crate::textures::jpeg(640, 480);
        std::fs::write(&photo, nef(model, &preview)).unwrap();
        (dir, photo)
    }

    /// What the panel over the viewer's photo says of the file at `path`,
    /// of the Nikon Z 9 ([`nef_folder`]), a row to a line: when it was
    /// taken, the camera, and the image's size, its format and its size on
    /// disk.
    fn nef_information(path: &Path) -> String {
        let bytes = on_disk(path, SizeUnits::Decimal);
        format!(
            "Taken: 2024-05-01 10:20:30.85 UTC+02:00\nCamera: Nikon Z 9\n\
             Image: 640 × 480 · NEF · {bytes}"
        )
    }

    /// In the viewer, I shows the current photo's information over it,
    /// as a panel screen readers read as its rows, and I again hides it.
    #[test]
    fn i_shows_and_hides_the_photo_s_information_in_the_viewer() {
        let (dir, photo) = nef_folder("NIKON Z 9");
        let mut harness = filmstrip_harness(dir.path());
        assert!(!harness.state().info_overlay, "on at first");
        assert_eq!(info_overlay(&harness), None);
        press(&mut harness, Modifiers::NONE, Key::I);
        assert!(harness.state().info_overlay);
        assert_eq!(info_overlay(&harness), Some(nef_information(&photo)));
        press(&mut harness, Modifiers::NONE, Key::I);
        assert!(!harness.state().info_overlay);
        assert_eq!(info_overlay(&harness), None);
    }

    /// In the grid, I does nothing, and the photo's information is never
    /// shown over the photos, even while it is on.
    #[test]
    fn the_grid_shows_no_photo_s_information() {
        let (dir, _photo) = nef_folder("NIKON Z 9");
        let mut harness = grid_harness(dir.path());
        press(&mut harness, Modifiers::NONE, Key::I);
        assert!(!harness.state().info_overlay);
        assert_eq!(info_overlay(&harness), None);
        harness.state_mut().info_overlay = true;
        harness.run();
        assert_eq!(info_overlay(&harness), None);
        click(&mut harness, "Image viewer");
        assert!(info_overlay(&harness).is_some(), "not shown in the viewer");
    }

    /// Whether the photo's information is shown is saved, and restored on
    /// the next run; with nothing saved, as on the first run, or anything
    /// but "true", it is hidden.
    #[test]
    fn whether_the_photo_s_information_is_shown_is_saved_and_restored() {
        let mut storage = MemoryStorage::default();
        let mut harness = harness();
        assert!(!harness.state().info_overlay, "on at the first run");
        harness.state_mut().info_overlay = true;
        harness.state_mut().save(&mut storage);
        assert_eq!(
            storage.get_string(INFO_OVERLAY_KEY).as_deref(),
            Some("true")
        );
        assert!(harness_from(&storage).state().info_overlay);
        harness.state_mut().info_overlay = false;
        harness.state_mut().save(&mut storage);
        assert_eq!(
            storage.get_string(INFO_OVERLAY_KEY).as_deref(),
            Some("false")
        );
        assert!(!harness_from(&storage).state().info_overlay);

        storage.set_string(INFO_OVERLAY_KEY, "yes".to_owned());
        assert!(!harness_from(&storage).state().info_overlay);
        storage.remove_string(INFO_OVERLAY_KEY);
        assert!(!harness_from(&storage).state().info_overlay);
    }

    /// While nothing of the photo is decoded, the viewer draws nothing and
    /// the photo's information waits; once some texture of it is decoded
    /// and drawn, the information shows.
    #[test]
    fn the_photo_s_information_waits_for_the_photo_to_be_decoded() {
        let (dir, photo) = nef_folder("NIKON Z 9");
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().view = View::Viewer;
        harness.state_mut().info_overlay = true;
        harness.state_mut().textures.stall(true);
        click(&mut harness, "Home");
        assert!(textures_in(&harness, viewer_room(&harness)).is_empty());
        assert_eq!(info_overlay(&harness), None);
        harness.state_mut().textures.stall(false);
        harness.run();
        assert_eq!(info_overlay(&harness), Some(nef_information(&photo)));
    }

    /// A photo that cannot be shown, which the viewer shows as an icon,
    /// has no information over it.
    #[test]
    fn a_photo_that_cannot_be_shown_has_no_information_over_it() {
        let dir = photos_folder();
        std::fs::write(dir.path().join("broken.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        let (mut harness, _) = harness_on(None, vec![home(dir.path())]);
        harness.state_mut().info_overlay = true;
        click(&mut harness, "Home");
        click(&mut harness, "Image viewer");
        assert!(failure_icon_in(&harness, viewer_room(&harness), 48.0));
        assert_eq!(info_overlay(&harness), None);
    }

    /// The panel's top-left corner stands 15 points across and 7 down from
    /// the viewer's, filled with the panel's colour, its corners rounded,
    /// marked off by its border and holding its rows in the panel's
    /// colour of text; it stays there however the photo is zoomed.
    #[test]
    fn the_photo_s_information_sits_at_the_viewer_s_corner() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        harness.state_mut().info_overlay = true;
        harness.run();
        let palette = theme::resolve(harness.state().theme, &harness.ctx).palette();
        let panel = info_overlay_node(&harness).expect("not shown").rect();
        let corner = centre(&harness).min + egui::vec2(15.0, 7.0);
        assert_eq!(panel.min, corner, "{panel:?}");
        let painted = harness.output().shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Rect(rect)
                if rect.rect == panel
                    && rect.fill == palette.overlay_fill
                    && rect.stroke == egui::Stroke::new(1.0, palette.overlay_border)
                    && rect.stroke_kind == egui::StrokeKind::Inside
                    && rect.corner_radius == egui::CornerRadius::same(4))
        });
        assert!(painted, "no panel painted at {panel:?}");
        let text = harness.output().shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Text(text)
                if panel.contains_rect(text.visual_bounding_rect())
                    && text.fallback_color == palette.overlay_text)
        });
        assert!(text, "no text in the panel");
        press(&mut harness, Modifiers::COMMAND | Modifiers::ALT, Key::Num0);
        assert_eq!(zoom_shown(&harness), "100 %");
        let zoomed = info_overlay_node(&harness).expect("not shown").rect();
        assert_eq!(zoomed, panel);
    }

    /// A value longer than the viewer has room for, less 15 points on each
    /// side, is cut short with "…", its label left whole, and the panel
    /// stays inside that room.
    #[test]
    fn a_long_value_of_the_photo_s_information_is_cut_short() {
        let model = "NIKON Z 9 ".repeat(20);
        let (dir, _photo) = nef_folder(&model);
        let mut harness = filmstrip_harness(dir.path());
        harness.state_mut().info_overlay = true;
        harness.run();
        let panel = info_overlay_node(&harness).expect("not shown").rect();
        // As far as the room goes, less what the last character cut off
        // would have taken.
        let room = centre(&harness).right() - 15.0;
        assert!(panel.right() <= room, "{panel:?} past {room}");
        assert!(panel.right() > room - 12.0, "{panel:?} short of {room}");
        let texts: Vec<_> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if panel.contains(text.pos) => Some(text),
                _ => None,
            })
            .collect();
        let cut_short = texts.iter().any(|text| {
            text.galley.elided && text.galley.text().starts_with("Nikon Z 9 Nikon Z 9 ")
        });
        assert!(cut_short, "the camera's model not cut short");
        let label = texts
            .iter()
            .any(|text| !text.galley.elided && text.galley.text() == "Camera");
        assert!(label, "the camera's label not whole");
    }

    /// The photo's information takes nothing from the photo under it: the
    /// pointer over both shows what a click on the photo does, and the
    /// click zooms the photo.
    #[test]
    fn a_click_on_the_photo_under_its_information_still_zooms() {
        let (mut harness, _dir, _) = zoom_harness(1.0);
        harness.state_mut().info_overlay = true;
        // At 100 % the photo fills the room.
        press(&mut harness, Modifiers::COMMAND | Modifiers::ALT, Key::Num0);
        assert_eq!(zoom_shown(&harness), "100 %");
        let panel = info_overlay_node(&harness).expect("not shown").rect();
        let both = panel.intersect(viewer_room(&harness));
        assert!(both.width() > 4.0 && both.height() > 4.0, "{both:?}");
        let at = both.center();
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(at));
        harness.run();
        let cursor = harness.output().platform_output.cursor_icon;
        assert_eq!(cursor, egui::CursorIcon::Grab);
        click_at(&mut harness, at);
        assert_eq!(zoom_shown(&harness), fitted_shown(&harness));
        assert!(is_on(&harness, "Zoom to fit"));
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

    /// How many times [`settle`] lets the app run until it asks for no
    /// further frame, up to 8 frames each time.
    const SETTLE_RUNS: usize = 20;

    /// Run frames until no decode is still to come and the app asks for no
    /// further frame, or [`SETTLE_RUNS`] runs have gone by, so the
    /// thumbnails and pictures wanted are drawn.
    fn settle(harness: &mut Harness<'_, PhotographyWorkbenchApp>) {
        for _ in 0..SETTLE_RUNS {
            let still = harness.run_ok().is_some();
            if still && harness.state().textures.pending() == 0 {
                return;
            }
        }
    }

    /// The app, drawn by the graphics card at one pixel a point, with the
    /// settings in `storage` saved by an earlier run, and the home folder
    /// at `folder` selected and settled.
    fn drawn_harness<'a>(
        storage: &'a MemoryStorage,
        folder: &Path,
    ) -> Harness<'a, PhotographyWorkbenchApp> {
        let builder = Harness::builder()
            .with_pixels_per_point(1.0)
            .renderer(WgpuTestRenderer::default());
        let (mut harness, _) = harness_built(builder, Some(storage), vec![home(folder)]);
        harness.get_by_label("Home").click();
        // The pointer leaves the window once it has clicked, so neither it
        // nor a hover is drawn.
        harness.remove_cursor();
        settle(&mut harness);
        harness
    }

    /// Write what `harness` shows to `<theme>-<scene>.png` in `folder`,
    /// the theme's name in lower case.
    fn screenshot(
        harness: &mut Harness<'_, PhotographyWorkbenchApp>,
        folder: &Path,
        theme: Theme,
        scene: &str,
    ) {
        let image = harness.render().expect("the window is drawn");
        let name = format!("{}-{scene}.png", theme.name().to_lowercase());
        let path = folder.join(name);
        image.save(&path).unwrap();
        println!("wrote {}", path.display());
    }

    /// [`photos_folder`], holding beside its folders a few JPEGs of one
    /// colour each, in the shapes photos come in: landscape, portrait,
    /// square and wide.
    fn generated_photos() -> tempfile::TempDir {
        let dir = photos_folder();
        let sizes = [
            (3000, 2000),
            (2000, 3000),
            (2400, 2400),
            (3200, 1800),
            (1500, 1000),
            (1000, 1500),
        ];
        for (index, (width, height)) in sizes.into_iter().enumerate() {
            let name = format!("DSC_{:04}.jpg", index + 1);
            let jpeg = crate::textures::jpeg(width, height);
            std::fs::write(dir.path().join(name), jpeg).unwrap();
        }
        dir
    }

    /// The window, drawn by the graphics card at its smallest size, one
    /// pixel a point, in every theme and scene, written as PNG files for
    /// people to review the look of every surface. It runs only when asked
    /// for, with `PW_SCREENSHOTS` naming the folder to write them to:
    ///
    /// ```sh
    /// PW_SCREENSHOTS=/path/to/folder cargo test -p photography-workbench \
    ///     --lib screenshots -- --ignored
    /// ```
    ///
    /// Each file is named `<theme>-<scene>.png`, the theme's name in lower
    /// case. The scenes: `grid`, the grid with the home folder selected
    /// and Info showing its first photo; `viewer`, that photo in the
    /// viewer, with the filmstrip and Info; `overlay`, that photo in the
    /// viewer with its information over it and the side docks hidden;
    /// `settings`, `about` and `shortcuts`, each dialog open over the grid;
    /// and `empty`, a folder with no photos selected. The home folder is the one
    /// `PW_SCREENSHOT_PHOTOS` names, to show real photos, or else holds a
    /// few JPEGs of one colour each, made for the run.
    #[test]
    #[ignore = "needs PW_SCREENSHOTS, a folder to write the screenshots to"]
    fn screenshots() {
        let Some(out) = std::env::var_os("PW_SCREENSHOTS") else {
            println!("skipped: PW_SCREENSHOTS names no folder");
            return;
        };
        let out = Path::new(&out);
        std::fs::create_dir_all(out).unwrap();
        let generated;
        let photos = match std::env::var_os("PW_SCREENSHOT_PHOTOS") {
            Some(folder) => std::path::PathBuf::from(folder),
            None => {
                generated = generated_photos();
                generated.path().to_path_buf()
            }
        };
        let empty = photos_folder();
        type Shown = fn(&mut PhotographyWorkbenchApp) -> &mut bool;
        let dialogs: [(&str, Shown); 3] = [
            ("settings", |app| &mut app.show_settings),
            ("about", |app| &mut app.show_about),
            ("shortcuts", |app| &mut app.show_shortcuts),
        ];
        for theme in Theme::ALL {
            let mut storage = MemoryStorage::default();
            storage.set_string(THEME_KEY, theme.name().to_owned());

            let mut harness = drawn_harness(&storage, &photos);
            screenshot(&mut harness, out, theme, "grid");
            for (scene, shown) in dialogs {
                *shown(harness.state_mut()) = true;
                settle(&mut harness);
                screenshot(&mut harness, out, theme, scene);
                *shown(harness.state_mut()) = false;
                settle(&mut harness);
            }
            harness.state_mut().view = View::Viewer;
            settle(&mut harness);
            screenshot(&mut harness, out, theme, "viewer");
            let ctx = harness.ctx.clone();
            harness.state_mut().perform(&ctx, Action::SideDocks);
            harness.state_mut().info_overlay = true;
            settle(&mut harness);
            screenshot(&mut harness, out, theme, "overlay");

            let mut harness = drawn_harness(&storage, empty.path());
            screenshot(&mut harness, out, theme, "empty");
        }
    }
}
