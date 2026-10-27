// SPDX-License-Identifier: GPL-3.0-or-later

//! The menu bar at the top of the screen on macOS (ADR010). The window
//! has no menu bar of its own on any system, but a Mac app without one at
//! the top of the screen is out of place there, so on macOS the app gives
//! the system one: the app menu with About, Settings and Quit, View with
//! the views, the viewer's zoom, the grid's thumbnail size, the photo's
//! information over the viewer's photo and the docks, Window with the
//! modes, and Help with the reference of keyboard shortcuts.
//!
//! Its items ask for the app's actions, which the app does just as it
//! does them for a key or a button. Which items can be chosen and which
//! are checked is worked out from the app's [`State`] on every system, so
//! the tests check it everywhere; only [`MacMenu`], which hands the menus
//! to the system, is macOS's. Elsewhere it is a stand-in that is never
//! made.
//!
//! An item carries its action's key combination as its key equivalent only
//! when the combination holds Cmd. The system can hand a key equivalent to
//! the menu before the window sees it, and the keys without Cmd belong to
//! the window, which leaves them to an open dialog, or to a text field that
//! has the keyboard. Nor does an item that shows or hides something carry
//! one: the system repeats a key equivalent while its keys are held, and a
//! toggle done again undoes itself, where the window's keys act on a key's
//! first press only.
//!
//! The system gives a key equivalent to its item before the window sees the
//! key, so a key combination an item carries is done once, by the item, and
//! never by the window's keys too. It does so even while the item cannot be
//! chosen, when the key does nothing. The zoom's items carry the viewer's
//! combinations, as they can be chosen only there; in the grid, and wherever
//! else they cannot be chosen, the window's keys would zoom nothing either.
//! Held down, zooming in or out goes on by a step as the system repeats the
//! key, until it can go no further, where the window's keys zoom once.
//!
//! Quit closes the window, as closing it any other way does. The menu bar
//! winit, the windowing library, makes by default has a Quit that would
//! end the app without closing it, so the app turns that one off
//! (`main.rs`).

use desktop_ui::dock::Side;

use crate::grid;
use crate::keymap::{self, Action, Sizing, Surface};
use crate::layout::Docks;
use crate::mode::{Layout, Mode};
use crate::view::{self, View};
use crate::viewer;
use crate::zoom::{Framing, Stage, Zooming};

/// What the menu bar shows of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// The mode the window shows.
    pub mode: Mode,
    /// How Library shows a folder's photos.
    pub view: View,
    /// The parts of the window shown around the centre.
    pub parts: Layout,
    /// Which docks are shown.
    pub docks: Docks,
    /// Which of the zoom's items can be chosen.
    pub zooms: Zooms,
    /// Which of the thumbnail size's items can be chosen.
    pub thumbnails: Thumbnails,
    /// Whether the viewer shows the photo's information over its photo,
    /// when it is shown.
    pub info_overlay: bool,
    /// Whether a dialog is open.
    pub dialog: bool,
    /// Whether the dialog open is the reference of keyboard shortcuts.
    pub shortcuts: bool,
}

impl State {
    /// Whether the item that asks for `action` can be chosen. While a
    /// dialog is open none can, as the keys stand down then, but the
    /// reference's own item, which closes it. A mode not built yet cannot
    /// be chosen, nor a dock the window does not have, nor a change of the
    /// zoom the toolbar's control for it is disabled for, nor a change of
    /// the thumbnails' size anywhere but in Library's grid or past the
    /// size's end, nor the photo's information anywhere but in Library's
    /// viewer, which alone shows it.
    pub fn enabled(&self, action: Action) -> bool {
        if self.dialog {
            return self.shortcuts && action == Action::Shortcuts;
        }
        match action {
            Action::Mode(mode) => mode.is_available(),
            Action::Dock(side) => self.parts.has_dock(side),
            Action::Zoom(zooming) => self.zooms.can(zooming),
            Action::Thumbnails(sizing) => self.thumbnails.can(sizing),
            Action::PhotoInformation => self.shows_viewer(),
            _ => true,
        }
    }

    /// Whether the item that asks for `action` is checked: the mode and
    /// the view shown, every dock shown, and the photo's information while
    /// Library's viewer shows it.
    // Off macOS no menu shows it, but the tests check it on every system.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn checked(&self, action: Action) -> bool {
        match action {
            Action::Mode(mode) => self.mode == mode,
            Action::View(view) => self.view == view,
            Action::Dock(side) => self.parts.has_dock(side) && self.docks.shown(side),
            Action::PhotoInformation => self.shows_viewer() && self.info_overlay,
            _ => false,
        }
    }

    /// Whether the centre shows Library's viewer.
    fn shows_viewer(&self) -> bool {
        view::shows_viewer(self.mode, self.view)
    }
}

/// Which of the zoom's items can be chosen, in the order of [`ZOOMS`]:
/// each while the change of the viewer's zoom it asks for can be, as the
/// toolbar's control for it is enabled ([`viewer::can_zoom`]); none
/// outside the viewer, nor before the photo's 100 % is known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Zooms([bool; ZOOMS.len()]);

impl Zooms {
    /// The changes that can be asked for of the zoom `shown`: the framing
    /// the viewer draws its photo with and the stage it zooms it on, `None`
    /// while nothing can be zoomed.
    pub fn of(shown: Option<(Framing, Stage)>) -> Self {
        Self(ZOOMS.map(|(zooming, _)| viewer::can_zoom(shown, zooming)))
    }

    /// Whether `zooming` can be asked for from its item; never when no
    /// item asks for it, as for the switch between fitted and 100 %.
    fn can(self, zooming: Zooming) -> bool {
        ZOOMS
            .iter()
            .zip(self.0)
            .any(|(&(of, _), can)| of == zooming && can)
    }
}

/// Which of the thumbnail size's items can be chosen, in the order of
/// [`THUMBNAILS`]: in Library's grid, the one for larger thumbnails while
/// they are smaller than [`grid::LARGEST_SIZE`], and the one for smaller
/// while they are larger than [`grid::SMALLEST_SIZE`]; neither anywhere
/// else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Thumbnails([bool; THUMBNAILS.len()]);

impl Thumbnails {
    /// The changes that can be asked for of the grid's thumbnails at
    /// `size`, the smallest side of its tiles; `None` outside Library's
    /// grid.
    pub fn of(size: Option<f32>) -> Self {
        Self(THUMBNAILS.map(|(sizing, _)| {
            size.is_some_and(|size| match sizing {
                Sizing::Larger => size < grid::LARGEST_SIZE,
                Sizing::Smaller => size > grid::SMALLEST_SIZE,
            })
        }))
    }

    /// Whether `sizing` can be asked for from its item.
    fn can(self, sizing: Sizing) -> bool {
        THUMBNAILS
            .iter()
            .zip(self.0)
            .any(|(&(of, _), can)| of == sizing && can)
    }
}

/// The key equivalent of the item that asks for `action`, as the menu
/// library spells it ("Cmd+Alt+1"): the key combination that does `action`
/// wherever its item can be chosen, everywhere or, for the zoom, in the
/// viewer, when it holds Cmd and the action is no toggle. `None` for any
/// other.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn key_equivalent(action: Action) -> Option<String> {
    let toggles = matches!(
        action,
        Action::Shortcuts
            | Action::SideDocks
            | Action::AllDocks
            | Action::Dock(_)
            | Action::Zoom(Zooming::Toggle)
            | Action::PhotoInformation
    );
    if toggles {
        return None;
    }
    let combination = match action {
        Action::Zoom(_) => keymap::key_combination_for(Surface::Viewer, action),
        _ => keymap::key_combination_everywhere(action),
    }?;
    let held = combination.modifiers;
    if !held.command {
        return None;
    }
    let mut spelled = String::from("Cmd");
    if held.alt {
        spelled.push_str("+Alt");
    }
    if held.shift {
        spelled.push_str("+Shift");
    }
    spelled.push('+');
    // The menu library knows `=` as "Equal", where egui names it "Equals".
    spelled.push_str(match combination.key {
        egui::Key::Equals => "Equal",
        key => key.name(),
    });
    Some(spelled)
}

/// The names of the items for the zoom, in the View menu's order: the
/// toolbar's names for its controls that do the same.
const ZOOMS: [(Zooming, &str); 4] = [
    (Zooming::In, "Zoom in"),
    (Zooming::Out, "Zoom out"),
    (Zooming::Fit, "Zoom to fit"),
    (Zooming::Full, "Zoom to 100 %"),
];

/// The names of the items for the thumbnail size, in the View menu's
/// order: the reference's descriptions of the keys that do the same.
const THUMBNAILS: [(Sizing, &str); 2] = [
    (Sizing::Larger, "Larger thumbnails"),
    (Sizing::Smaller, "Smaller thumbnails"),
];

/// The name of the item that shows or hides the photo's information over
/// the viewer's photo.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const PHOTO_INFORMATION: &str = "Photo information";

/// The names of the items for the docks, in the View menu's order.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const DOCKS: [(Side, &str); 3] = [
    (Side::Left, "Left dock"),
    (Side::Right, "Right dock"),
    (Side::Bottom, "Filmstrip"),
];

/// The menu bar, handed to the system. Its items' clicks reach the app
/// through [`Self::clicked`], and what they show follows the app through
/// [`Self::show`].
#[cfg(target_os = "macos")]
pub struct MacMenu {
    /// The menu bar, kept for as long as the system shows it.
    _bar: muda::Menu,
    /// The items without a check mark, with the actions they ask for.
    items: Vec<(muda::MenuItem, Action)>,
    /// The items with a check mark, with the actions they ask for.
    checks: Vec<(muda::CheckMenuItem, Action)>,
    /// The action each item asks for, by the item's id.
    actions: std::collections::HashMap<muda::MenuId, Action>,
    /// The clicks on the items, sent as the system reports them.
    clicks: std::sync::mpsc::Receiver<muda::MenuId>,
    /// What the items show, as last set; `None` when it must be set again.
    shown: Option<State>,
}

#[cfg(target_os = "macos")]
impl MacMenu {
    /// Make the menu bar and hand it to the system, for the window `cc`
    /// starts. `None` without a window, as in the headless tests, which
    /// also run where the system's menus cannot be made, off the main
    /// thread; or when a menu cannot be made, which it logs, and the app
    /// then runs without one.
    pub fn install(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        use raw_window_handle::HasWindowHandle as _;

        cc.window_handle().ok()?;
        let menu = Self::build(cc);
        if menu.is_none() {
            log::warn!("the menu bar could not be made; the app runs without one");
        }
        menu
    }

    /// Make the menu bar and hand it to the system. `None` when a menu
    /// cannot be made.
    fn build(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

        let name = crate::PhotographyWorkbenchApp::NAME;
        let key = |action| key_equivalent(action).and_then(|keys| keys.parse().ok());
        let mut items = Vec::new();
        let mut item = |label: &str, action| {
            let item = MenuItem::new(label, true, key(action));
            items.push((item.clone(), action));
            item
        };
        let about = item(&format!("About {name}"), Action::About);
        let settings = item("Settings…", Action::Settings);
        let quit = item(&format!("Quit {name}"), Action::Quit);
        let shortcuts = item("Keyboard shortcuts", Action::Shortcuts);
        let [zoom_in, zoom_out, fit, full] =
            ZOOMS.map(|(zooming, label)| item(label, Action::Zoom(zooming)));
        let [larger, smaller] =
            THUMBNAILS.map(|(sizing, label)| item(label, Action::Thumbnails(sizing)));
        let mut checks = Vec::new();
        let mut check = |label: &str, action| {
            let item = CheckMenuItem::new(label, true, false, key(action));
            checks.push((item.clone(), action));
            item
        };
        let [grid, viewer] = View::ALL.map(|view| check(view.name(), Action::View(view)));
        let information = check(PHOTO_INFORMATION, Action::PhotoInformation);
        let [left, right, bottom] = DOCKS.map(|(side, label)| check(label, Action::Dock(side)));
        let [library, organizer, develop] =
            Mode::ALL.map(|mode| check(mode.name(), Action::Mode(mode)));

        // The first menu is the app menu. The system titles it, whatever
        // it is called here: with the name in the app bundle's
        // Info.plist, or with the program's file name when the program
        // runs outside a bundle, as `cargo run` runs it.
        let app = Submenu::with_items(
            name,
            true,
            &[
                &about,
                &PredefinedMenuItem::separator(),
                &settings,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(Some(&format!("Hide {name}"))),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &quit,
            ],
        )
        .ok()?;
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &grid,
                &viewer,
                &PredefinedMenuItem::separator(),
                &zoom_in,
                &zoom_out,
                &fit,
                &full,
                &PredefinedMenuItem::separator(),
                &larger,
                &smaller,
                &PredefinedMenuItem::separator(),
                &information,
                &PredefinedMenuItem::separator(),
                &left,
                &right,
                &bottom,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::fullscreen(Some("Enter Full Screen")),
            ],
        )
        .ok()?;
        // As the Window menu, it also lists the app's windows.
        let window = Submenu::with_items(
            "Window",
            true,
            &[
                &library,
                &organizer,
                &develop,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::maximize(None),
            ],
        )
        .ok()?;
        window.set_as_windows_menu_for_nsapp();
        // As the Help menu, it also searches the menus' items.
        let help = Submenu::with_items("Help", true, &[&shortcuts]).ok()?;
        help.set_as_help_menu_for_nsapp();
        let bar = Menu::with_items(&[&app, &view, &window, &help]).ok()?;

        let actions = items
            .iter()
            .map(|(item, action)| (item.id().clone(), *action))
            .chain(
                checks
                    .iter()
                    .map(|(item, action)| (item.id().clone(), *action)),
            )
            .collect();
        // A click comes while the system has the menu open, not in a
        // frame of the window's, so it asks for one.
        let (send, clicks) = std::sync::mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let _ = send.send(event.id);
            ctx.request_repaint();
        }));
        bar.init_for_nsapp();
        Some(Self {
            _bar: bar,
            items,
            checks,
            actions,
            clicks,
            shown: None,
        })
    }

    /// The actions asked for by the items clicked since the last call.
    pub fn clicked(&mut self) -> Vec<Action> {
        let ids: Vec<_> = self.clicks.try_iter().collect();
        // A check item checks or unchecks itself when clicked, whatever
        // the app makes of it, so what the items show is set again.
        if !ids.is_empty() {
            self.shown = None;
        }
        ids.iter()
            .filter_map(|id| self.actions.get(id).copied())
            .collect()
    }

    /// Make the items show `state`: which can be chosen, and which are
    /// checked. Only when it changed, as most frames change nothing.
    pub fn show(&mut self, state: State) {
        if self.shown == Some(state) {
            return;
        }
        for (item, action) in &self.items {
            item.set_enabled(state.enabled(*action));
        }
        for (item, action) in &self.checks {
            item.set_enabled(state.enabled(*action));
            item.set_checked(state.checked(*action));
        }
        self.shown = Some(state);
    }
}

/// The stand-in off macOS, which has no menu bar of the app's: it is
/// never made, so the app asks it nothing.
#[cfg(not(target_os = "macos"))]
pub struct MacMenu;

#[cfg(not(target_os = "macos"))]
impl MacMenu {
    /// `None`: there is no menu bar to make.
    pub fn install(_cc: &eframe::CreationContext<'_>) -> Option<Self> {
        None
    }

    /// Nothing is ever clicked.
    pub fn clicked(&mut self) -> Vec<Action> {
        Vec::new()
    }

    /// There is nothing to show it on.
    pub fn show(&mut self, _state: State) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Step;
    use crate::zoom::Zoom;

    /// Library's grid, its thumbnails at the size they start at, every
    /// dock shown, no dialog open.
    fn library() -> State {
        State {
            mode: Mode::Library,
            view: View::Grid,
            parts: Mode::Library.layout(),
            docks: Docks::default(),
            zooms: Zooms::default(),
            thumbnails: Thumbnails::of(Some(grid::DEFAULT_SIZE)),
            info_overlay: false,
            dialog: false,
            shortcuts: false,
        }
    }

    /// The zoom of a photo 2400 by 1600 pixels, its 100 % known, in a
    /// room of 800 by 600 points on a screen of a pixel a point, zoomed to
    /// `zoom`: its framing and its stage.
    fn shown(zoom: Zoom) -> (Framing, Stage) {
        let largest = previews::Size {
            width: 2400,
            height: 1600,
        };
        let room = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let framing = Framing {
            zoom,
            ..Framing::default()
        };
        (framing, Stage::new(largest, room, 1.0))
    }

    /// The viewer showing the photo of [`shown`], zoomed to `zoom`.
    fn viewer(zoom: Zoom) -> State {
        State {
            view: View::Viewer,
            parts: Mode::Library.layout(),
            zooms: Zooms::of(Some(shown(zoom))),
            thumbnails: Thumbnails::of(None),
            ..library()
        }
    }

    /// Whether each of the zoom's items can be chosen, in the View menu's
    /// order.
    fn zoom_items(state: State) -> [bool; 4] {
        ZOOMS.map(|(zooming, _)| state.enabled(Action::Zoom(zooming)))
    }

    /// Where nothing can be zoomed, in the grid or before the photo's
    /// 100 % is known, none of the zoom's items can be chosen.
    #[test]
    fn the_zoom_s_items_wait_for_a_zoom() {
        let nothing = State {
            zooms: Zooms::of(None),
            ..library()
        };
        assert_eq!(zoom_items(nothing), [false; 4]);
        assert_eq!(zoom_items(library()), [false; 4]);
    }

    /// Zooming out cannot be chosen while the photo is fitted, nor zooming
    /// in at 800 %; fitting and 100 % can be whenever something can be
    /// zoomed.
    #[test]
    fn the_zoom_s_items_follow_what_would_change() {
        assert_eq!(zoom_items(viewer(Zoom::Fit)), [true, false, true, true]);
        let most = viewer(Zoom::Scale(crate::zoom::MAX_SCALE));
        assert_eq!(zoom_items(most), [false, true, true, true]);
        assert_eq!(zoom_items(viewer(Zoom::Scale(1.0))), [true; 4]);
    }

    /// Each of the zoom's items can be chosen exactly when the viewer's
    /// rule, which the toolbar's controls follow too, says its change can
    /// be asked for, whatever the zoom; no item asks for the switch
    /// between fitted and 100 %.
    #[test]
    fn the_zoom_s_items_follow_the_viewer_s_rule() {
        let scales = [0.25, 0.5, 1.0, 3.0, crate::zoom::MAX_SCALE];
        let zooms = std::iter::once(Zoom::Fit).chain(scales.map(Zoom::Scale));
        for zoom in zooms {
            let state = viewer(zoom);
            for (zooming, _) in ZOOMS {
                let can = viewer::can_zoom(Some(shown(zoom)), zooming);
                let enabled = state.enabled(Action::Zoom(zooming));
                assert_eq!(enabled, can, "{zoom:?} {zooming:?}");
            }
            assert!(!state.enabled(Action::Zoom(Zooming::Toggle)), "{zoom:?}");
        }
    }

    /// Whether each of the thumbnail size's items can be chosen, in the
    /// View menu's order, in Library's grid with its thumbnails at `size`.
    fn thumbnail_items_at(size: f32) -> [bool; 2] {
        let state = State {
            thumbnails: Thumbnails::of(Some(size)),
            ..library()
        };
        THUMBNAILS.map(|(sizing, _)| state.enabled(Action::Thumbnails(sizing)))
    }

    /// In Library's grid the item for larger thumbnails can be chosen
    /// until they are the largest, and the one for smaller until they are
    /// the smallest; in the viewer neither can, and neither is ever
    /// checked, as neither shows or hides anything.
    #[test]
    fn the_thumbnail_size_s_items_follow_the_size() {
        assert_eq!(thumbnail_items_at(grid::DEFAULT_SIZE), [true, true]);
        assert_eq!(thumbnail_items_at(grid::LARGEST_SIZE), [false, true]);
        assert_eq!(thumbnail_items_at(grid::SMALLEST_SIZE), [true, false]);
        let viewer = viewer(Zoom::Fit);
        for (sizing, _) in THUMBNAILS {
            let action = Action::Thumbnails(sizing);
            assert!(!viewer.enabled(action), "{sizing:?}");
            assert!(!library().checked(action), "{sizing:?}");
        }
    }

    #[test]
    fn the_mode_and_the_view_shown_are_checked() {
        let state = library();
        assert!(state.checked(Action::Mode(Mode::Library)));
        assert!(!state.checked(Action::Mode(Mode::Organizer)));
        assert!(state.checked(Action::View(View::Grid)));
        assert!(!state.checked(Action::View(View::Viewer)));
        assert!(!state.checked(Action::Settings));
        assert!(!state.checked(Action::Photo(Step::Next)));
    }

    /// A dock's item is checked while the dock is shown, and can be
    /// chosen only where the window has the dock: the filmstrip is the
    /// viewer's.
    #[test]
    fn a_dock_s_item_follows_the_dock() {
        let mut state = library();
        state.parts.bottom_dock = false;
        state.docks.right = false;
        assert!(state.checked(Action::Dock(Side::Left)));
        assert!(!state.checked(Action::Dock(Side::Right)));
        assert!(state.enabled(Action::Dock(Side::Right)));
        assert!(!state.checked(Action::Dock(Side::Bottom)));
        assert!(!state.enabled(Action::Dock(Side::Bottom)));
    }

    /// The item for the photo's information can be chosen only in
    /// Library's viewer, which alone shows it, and is checked there while
    /// it is shown; in the grid it is neither, whether it is on or not.
    #[test]
    fn the_photo_information_s_item_is_the_viewer_s() {
        let item = |state: State| {
            let action = Action::PhotoInformation;
            (state.enabled(action), state.checked(action))
        };
        let on = |state| State {
            info_overlay: true,
            ..state
        };
        assert_eq!(item(viewer(Zoom::Fit)), (true, false));
        assert_eq!(item(on(viewer(Zoom::Fit))), (true, true));
        assert_eq!(item(library()), (false, false));
        assert_eq!(item(on(library())), (false, false));
    }

    #[test]
    fn a_mode_not_built_yet_cannot_be_chosen() {
        let state = library();
        assert!(state.enabled(Action::Mode(Mode::Library)));
        assert!(!state.enabled(Action::Mode(Mode::Organizer)));
        assert!(!state.enabled(Action::Mode(Mode::Develop)));
    }

    /// While a dialog is open, no item can be chosen, but the reference's
    /// own, which closes it when it is the dialog open.
    #[test]
    fn the_items_stand_down_while_a_dialog_is_open() {
        let actions = [
            Action::About,
            Action::Settings,
            Action::Shortcuts,
            Action::Quit,
            Action::Mode(Mode::Library),
            Action::View(View::Viewer),
            Action::Photo(Step::Next),
            Action::Dock(Side::Left),
            Action::Zoom(Zooming::In),
            Action::Zoom(Zooming::Out),
            Action::Zoom(Zooming::Fit),
            Action::Zoom(Zooming::Full),
            Action::PhotoInformation,
        ];
        let zoomed = viewer(Zoom::Scale(1.0));
        let settings = State {
            dialog: true,
            ..zoomed
        };
        for action in actions {
            assert!(zoomed.enabled(action), "{action:?}");
            assert!(!settings.enabled(action), "{action:?}");
        }
        // The thumbnail size's items are the grid's.
        let sized = library();
        let behind = State {
            dialog: true,
            ..sized
        };
        for (sizing, _) in THUMBNAILS {
            let action = Action::Thumbnails(sizing);
            assert!(sized.enabled(action), "{action:?}");
            assert!(!behind.enabled(action), "{action:?}");
        }
        let reference = State {
            shortcuts: true,
            ..settings
        };
        assert!(reference.enabled(Action::Shortcuts));
        assert!(!reference.enabled(Action::Settings));
    }

    /// An item carries its action's key combination only when the
    /// combination holds Cmd and the action is no toggle, spelled for the
    /// menu library.
    #[test]
    fn only_the_cmd_combinations_are_key_equivalents() {
        let spelled = |action| key_equivalent(action);
        assert_eq!(spelled(Action::Settings).as_deref(), Some("Cmd+Comma"));
        assert_eq!(spelled(Action::Shortcuts), None, "a toggle");
        assert_eq!(spelled(Action::Quit).as_deref(), Some("Cmd+Q"));
        let library = Action::Mode(Mode::Library);
        assert_eq!(spelled(library).as_deref(), Some("Cmd+Alt+1"));
        assert_eq!(spelled(Action::Mode(Mode::Organizer)), None, "not built");
        assert_eq!(spelled(Action::View(View::Grid)), None, "G");
        assert_eq!(spelled(Action::Dock(Side::Left)), None, "F7");
        assert_eq!(spelled(Action::Photo(Step::Next)), None, "Right");
        assert_eq!(spelled(Action::About), None, "no key combination");
        let zoom = |zooming| spelled(Action::Zoom(zooming));
        assert_eq!(zoom(Zooming::In).as_deref(), Some("Cmd+Equal"));
        assert_eq!(zoom(Zooming::Out).as_deref(), Some("Cmd+Minus"));
        assert_eq!(zoom(Zooming::Fit).as_deref(), Some("Cmd+0"));
        assert_eq!(zoom(Zooming::Full).as_deref(), Some("Cmd+Alt+0"));
        assert_eq!(zoom(Zooming::Toggle), None, "a toggle");
        assert_eq!(spelled(Action::PhotoInformation), None, "a toggle");
        for sizing in [Sizing::Larger, Sizing::Smaller] {
            let thumbnails = spelled(Action::Thumbnails(sizing));
            assert_eq!(thumbnails, None, "{sizing:?}: no Cmd, the grid's");
        }
    }

    /// The zoom's items carry the key combinations the viewer's keys and the
    /// toolbar's controls name, which hold Cmd.
    #[test]
    fn the_zoom_s_items_carry_the_viewer_s_combinations() {
        for (zooming, _) in ZOOMS {
            let combination =
                keymap::key_combination_for(Surface::Viewer, Action::Zoom(zooming)).unwrap();
            assert!(combination.modifiers.command, "{zooming:?}");
            assert!(
                key_equivalent(Action::Zoom(zooming)).is_some(),
                "{zooming:?}"
            );
        }
    }

    /// The menu library reads every key equivalent as the key combination it
    /// is.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_menu_library_reads_the_key_equivalents() {
        use muda::accelerator::{Accelerator, Code, Modifiers};
        let read = |action| key_equivalent(action).and_then(|keys| keys.parse().ok());
        let cmd = Modifiers::META;
        let expected = [
            (Action::Settings, Accelerator::new(cmd, Code::Comma)),
            (Action::Quit, Accelerator::new(cmd, Code::KeyQ)),
            (
                Action::Mode(Mode::Library),
                Accelerator::new(cmd | Modifiers::ALT, Code::Digit1),
            ),
            (
                Action::Zoom(Zooming::In),
                Accelerator::new(cmd, Code::Equal),
            ),
            (
                Action::Zoom(Zooming::Out),
                Accelerator::new(cmd, Code::Minus),
            ),
            (
                Action::Zoom(Zooming::Fit),
                Accelerator::new(cmd, Code::Digit0),
            ),
            (
                Action::Zoom(Zooming::Full),
                Accelerator::new(cmd | Modifiers::ALT, Code::Digit0),
            ),
        ];
        for (action, accelerator) in expected {
            assert_eq!(read(action), Some(accelerator), "{action:?}");
        }
    }
}
