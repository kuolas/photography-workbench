// SPDX-License-Identifier: GPL-3.0-or-later

//! The menu bar at the top of the screen on macOS (ADR010). The window
//! has no menu bar of its own on any system, but a Mac app without one at
//! the top of the screen is out of place there, so on macOS the app gives
//! the system one: the app menu with About, Settings and Quit, View with
//! the views and the docks, Window with the modes, and Help with the
//! reference of keyboard shortcuts.
//!
//! Its items ask for the app's actions, which the app does just as it
//! does them for a key or a button. Which items can be chosen and which
//! are checked is worked out from the app's [`State`] on every system, so
//! the tests check it everywhere; only [`MacMenu`], which hands the menus
//! to the system, is macOS's. Elsewhere it is a stand-in that is never
//! made.
//!
//! An item carries its action's chord as its key equivalent only when the
//! chord holds Cmd. The system can hand a key equivalent to the menu
//! before the window sees it, and the keys without Cmd belong to the
//! window, which leaves them to an open dialog, or to a text field that
//! has the keyboard. Nor does an item that shows or hides something carry
//! one: the system repeats a key equivalent while its keys are held, and
//! a toggle done again undoes itself, where the window's keys act on a
//! key's first press only.
//!
//! Quit closes the window, as closing it any other way does. The menu bar
//! winit, the windowing library, makes by default has a Quit that would
//! end the app without closing it, so the app turns that one off
//! (`main.rs`).

use desktop_ui::dock::Side;

use crate::keymap::{self, Action};
use crate::layout::Docks;
use crate::mode::{Layout, Mode};
use crate::view::View;

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
    /// Whether a dialog is open.
    pub dialog: bool,
    /// Whether the dialog open is the reference of keyboard shortcuts.
    pub shortcuts: bool,
}

impl State {
    /// Whether the item that asks for `action` can be chosen. While a
    /// dialog is open none can, as the keys stand down then, but the
    /// reference's own item, which closes it. A mode not built yet cannot
    /// be chosen, nor a dock the window does not have.
    pub fn enabled(&self, action: Action) -> bool {
        if self.dialog {
            return self.shortcuts && action == Action::Shortcuts;
        }
        match action {
            Action::Mode(mode) => mode.is_available(),
            Action::Dock(side) => self.parts.has_dock(side),
            _ => true,
        }
    }

    /// Whether the item that asks for `action` is checked: the mode and
    /// the view shown, and every dock shown.
    // Off macOS no menu shows it, but the tests check it on every system.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn checked(&self, action: Action) -> bool {
        match action {
            Action::Mode(mode) => self.mode == mode,
            Action::View(view) => self.view == view,
            Action::Dock(side) => self.parts.has_dock(side) && self.docks.shown(side),
            _ => false,
        }
    }
}

/// The key equivalent of the item that asks for `action`, as the menu
/// library spells it ("Cmd+Alt+1"): the chord that does `action`
/// everywhere, when it holds Cmd and the action is no toggle. `None` for
/// any other.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn key_equivalent(action: Action) -> Option<String> {
    let toggles = matches!(
        action,
        Action::Shortcuts | Action::SideDocks | Action::AllDocks | Action::Dock(_)
    );
    if toggles {
        return None;
    }
    let chord = keymap::chord_everywhere(action)?;
    let held = chord.modifiers;
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
    spelled.push_str(chord.key.name());
    Some(spelled)
}

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
        let mut checks = Vec::new();
        let mut check = |label: &str, action| {
            let item = CheckMenuItem::new(label, true, false, key(action));
            checks.push((item.clone(), action));
            item
        };
        let [grid, viewer] = View::ALL.map(|view| check(view.name(), Action::View(view)));
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

    /// Library's grid, every dock shown, no dialog open.
    fn library() -> State {
        State {
            mode: Mode::Library,
            view: View::Grid,
            parts: Mode::Library.layout(),
            docks: Docks::default(),
            dialog: false,
            shortcuts: false,
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
            Action::Dock(Side::Left),
        ];
        let settings = State {
            dialog: true,
            ..library()
        };
        for action in actions {
            assert!(library().enabled(action), "{action:?}");
            assert!(!settings.enabled(action), "{action:?}");
        }
        let reference = State {
            shortcuts: true,
            ..settings
        };
        assert!(reference.enabled(Action::Shortcuts));
        assert!(!reference.enabled(Action::Settings));
    }

    /// An item carries its action's chord only when the chord holds Cmd
    /// and the action is no toggle, spelled for the menu library.
    #[test]
    fn only_the_cmd_chords_are_key_equivalents() {
        let spelled = |action| key_equivalent(action);
        assert_eq!(spelled(Action::Settings).as_deref(), Some("Cmd+Comma"));
        assert_eq!(spelled(Action::Shortcuts), None, "a toggle");
        assert_eq!(spelled(Action::Quit).as_deref(), Some("Cmd+Q"));
        let library = Action::Mode(Mode::Library);
        assert_eq!(spelled(library).as_deref(), Some("Cmd+Alt+1"));
        assert_eq!(spelled(Action::Mode(Mode::Organizer)), None, "not built");
        assert_eq!(spelled(Action::View(View::Grid)), None, "G");
        assert_eq!(spelled(Action::Dock(Side::Left)), None, "F7");
        assert_eq!(spelled(Action::About), None, "no chord");
    }

    /// The menu library reads every key equivalent as the chord it is.
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
        ];
        for (action, accelerator) in expected {
            assert_eq!(read(action), Some(accelerator), "{action:?}");
        }
    }
}
