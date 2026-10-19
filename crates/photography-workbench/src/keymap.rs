// SPDX-License-Identifier: GPL-3.0-or-later

//! The keyboard shortcuts: one table, [`BINDINGS`], of what each chord
//! does, where it does it, and how the reference describes it. The
//! handlers ask the table whether an action's chord was pressed
//! ([`take_pressed`]), and the reference is built from it ([`sections`]),
//! so neither can say something the table does not (ADR011). Asking takes
//! the chord's key out of the frame's input, so a key that does something
//! here does nothing else too.
//!
//! The keys are the ones established photography tools use for the same
//! thing, so a photographer's hands already know them, with Cmd for Ctrl
//! on macOS (ADR015).

use desktop_ui::dock::Side;
use desktop_ui::keys::{self, KeyCombination};
use egui::{Key, Modifiers};

use crate::mode::Mode;
use crate::view::View;
use crate::zoom::Zooming;

/// Where a shortcut applies: the view Library shows a folder's photos in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The thumbnail grid.
    Grid,
    /// The image viewer.
    Viewer,
}

/// What a shortcut, a button or a menu item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Switch to a mode, if it is built.
    Mode(Mode),
    /// Show the folder's photos in a view.
    View(View),
    /// Make another of the folder's photos the current one.
    Photo(Step),
    /// Zoom the viewer's photo, once its 100 % is known.
    Zoom(Zooming),
    /// Show or hide the side docks together.
    SideDocks,
    /// Show or hide every dock together.
    AllDocks,
    /// Show or hide the dock on a side.
    Dock(Side),
    /// Open Settings.
    Settings,
    /// Open the reference of keyboard shortcuts, or close it.
    Shortcuts,
    /// Open About, which no shortcut does.
    About,
    /// Close the window, which quits the app.
    Quit,
}

/// Which photo becomes the current one, from the current one. None goes
/// past either end of the folder, nor wraps around to the other (ADR011).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The one before it.
    Previous,
    /// The one after it.
    Next,
    /// The one above it in the grid.
    Above,
    /// The one below it in the grid, or the last photo when the row
    /// below is too short to have one there.
    Below,
    /// The folder's first photo.
    First,
    /// The folder's last photo.
    Last,
}

/// The sections of the reference, each holding the shortcuts of one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// How the centre shows the photos.
    Views,
    /// Moving between the photos.
    Photos,
    /// How large the viewer shows its photo.
    Zoom,
    /// Which docks are shown.
    Docks,
    /// The modes, the dialogs, and quitting.
    General,
}

impl Group {
    /// Every group, in the reference's order.
    const ALL: [Self; 5] = [
        Self::Views,
        Self::Photos,
        Self::Zoom,
        Self::Docks,
        Self::General,
    ];

    /// The group's section title in the reference.
    fn title(self) -> &'static str {
        match self {
            Self::Views => "Views",
            Self::Photos => "Photos",
            Self::Zoom => "Zoom",
            Self::Docks => "Docks",
            Self::General => "General",
        }
    }
}

/// One shortcut: the chord that does an action on some surfaces, and the
/// group and description the reference shows it under.
struct Binding {
    action: Action,
    key_combination: KeyCombination,
    surfaces: &'static [Surface],
    group: Group,
    description: &'static str,
    /// Whether the action is done again while the key is held, as the
    /// system repeats it: for a step through the photos, but not for a
    /// toggle, which a held key would turn on and off.
    repeats: bool,
}

/// Both views: everywhere in Library.
const EVERYWHERE: &[Surface] = &[Surface::Grid, Surface::Viewer];

/// The grid only.
const GRID: &[Surface] = &[Surface::Grid];

/// The viewer only.
const VIEWER: &[Surface] = &[Surface::Viewer];

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const CTRL: Modifiers = Modifiers::COMMAND;
const CTRL_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);

/// A row of [`BINDINGS`].
const fn bind(
    group: Group,
    (modifiers, key): (Modifiers, Key),
    action: Action,
    surfaces: &'static [Surface],
    description: &'static str,
) -> Binding {
    Binding {
        action,
        key_combination: KeyCombination::new(modifiers, key),
        surfaces,
        group,
        description,
        repeats: false,
    }
}

/// `binding`, done again while its key is held ([`Binding::repeats`]).
const fn repeating(binding: Binding) -> Binding {
    Binding {
        repeats: true,
        ..binding
    }
}

/// Every keyboard shortcut, in the reference's order within each group.
const BINDINGS: &[Binding] = &[
    bind(
        Group::Views,
        (NONE, Key::G),
        Action::View(View::Grid),
        EVERYWHERE,
        "Thumbnail grid",
    ),
    bind(
        Group::Views,
        (NONE, Key::E),
        Action::View(View::Viewer),
        EVERYWHERE,
        "Image viewer",
    ),
    bind(
        Group::Views,
        (NONE, Key::Enter),
        Action::View(View::Viewer),
        GRID,
        "Open the photo in the viewer",
    ),
    bind(
        Group::Views,
        (NONE, Key::Escape),
        Action::View(View::Grid),
        VIEWER,
        "Back to the thumbnail grid",
    ),
    repeating(bind(
        Group::Photos,
        (NONE, Key::ArrowLeft),
        Action::Photo(Step::Previous),
        EVERYWHERE,
        "Previous photo",
    )),
    repeating(bind(
        Group::Photos,
        (NONE, Key::ArrowRight),
        Action::Photo(Step::Next),
        EVERYWHERE,
        "Next photo",
    )),
    repeating(bind(
        Group::Photos,
        (NONE, Key::ArrowUp),
        Action::Photo(Step::Above),
        GRID,
        "Photo above",
    )),
    repeating(bind(
        Group::Photos,
        (NONE, Key::ArrowDown),
        Action::Photo(Step::Below),
        GRID,
        "Photo below",
    )),
    repeating(bind(
        Group::Photos,
        (NONE, Key::Home),
        Action::Photo(Step::First),
        GRID,
        "First photo",
    )),
    repeating(bind(
        Group::Photos,
        (NONE, Key::End),
        Action::Photo(Step::Last),
        GRID,
        "Last photo",
    )),
    bind(
        Group::Zoom,
        (NONE, Key::Z),
        Action::Zoom(Zooming::Toggle),
        VIEWER,
        "Switch between fitted and 100 %",
    ),
    bind(
        Group::Zoom,
        (NONE, Key::Space),
        Action::Zoom(Zooming::Toggle),
        VIEWER,
        "Switch between fitted and 100 %",
    ),
    bind(
        Group::Zoom,
        (CTRL, Key::Equals),
        Action::Zoom(Zooming::In),
        VIEWER,
        "Zoom in",
    ),
    // Where the keyboard types `+` with Shift on the `=` key, as a US one
    // does, or on a key of its own, as a German one does, or on the
    // keypad.
    bind(
        Group::Zoom,
        (CTRL, Key::Plus),
        Action::Zoom(Zooming::In),
        VIEWER,
        "Zoom in",
    ),
    bind(
        Group::Zoom,
        (CTRL, Key::Minus),
        Action::Zoom(Zooming::Out),
        VIEWER,
        "Zoom out",
    ),
    bind(
        Group::Zoom,
        (CTRL, Key::Num0),
        Action::Zoom(Zooming::Fit),
        VIEWER,
        "Zoom to fit",
    ),
    bind(
        Group::Zoom,
        (CTRL_ALT, Key::Num0),
        Action::Zoom(Zooming::Full),
        VIEWER,
        "Zoom to 100 %",
    ),
    bind(
        Group::Docks,
        (NONE, Key::Tab),
        Action::SideDocks,
        EVERYWHERE,
        "Show or hide the side docks",
    ),
    bind(
        Group::Docks,
        (SHIFT, Key::Tab),
        Action::AllDocks,
        EVERYWHERE,
        "Show or hide all the docks",
    ),
    bind(
        Group::Docks,
        (NONE, Key::F6),
        Action::Dock(Side::Bottom),
        VIEWER,
        "Show or hide the filmstrip",
    ),
    bind(
        Group::Docks,
        (NONE, Key::F7),
        Action::Dock(Side::Left),
        EVERYWHERE,
        "Show or hide the left dock",
    ),
    bind(
        Group::Docks,
        (NONE, Key::F8),
        Action::Dock(Side::Right),
        EVERYWHERE,
        "Show or hide the right dock",
    ),
    bind(
        Group::General,
        (CTRL_ALT, Key::Num1),
        Action::Mode(Mode::Library),
        EVERYWHERE,
        "Library",
    ),
    bind(
        Group::General,
        (CTRL_ALT, Key::Num2),
        Action::Mode(Mode::Organizer),
        EVERYWHERE,
        "Organizer",
    ),
    bind(
        Group::General,
        (CTRL_ALT, Key::Num3),
        Action::Mode(Mode::Develop),
        EVERYWHERE,
        "Develop",
    ),
    bind(
        Group::General,
        (CTRL, Key::Comma),
        Action::Settings,
        EVERYWHERE,
        "Settings",
    ),
    bind(
        Group::General,
        (CTRL, Key::Slash),
        Action::Shortcuts,
        EVERYWHERE,
        "Keyboard shortcuts",
    ),
    bind(
        Group::General,
        (CTRL, Key::Q),
        Action::Quit,
        EVERYWHERE,
        "Quit",
    ),
];

impl Binding {
    /// Whether the binding's chord was pressed in this frame, or, for one
    /// that repeats, its key held down repeated.
    fn went_down(&self, ctx: &egui::Context) -> bool {
        if self.repeats {
            self.key_combination.pressed_or_repeated(ctx)
        } else {
            self.key_combination.pressed(ctx)
        }
    }

    /// Whether the shortcut does anything yet: one that switches to a
    /// mode not built yet does not, and the reference leaves it out.
    fn is_live(&self) -> bool {
        match self.action {
            Action::Mode(mode) => mode.is_available(),
            _ => true,
        }
    }
}

/// Whether a chord that does `action` on `surface` was pressed in this
/// frame. The chords that do it there are taken out of the frame's input,
/// pressed or held down, so no widget drawn after sees their keys too
/// ([`KeyCombination::take`]).
pub fn take_pressed(ctx: &egui::Context, surface: Surface, action: Action) -> bool {
    let bindings = || {
        BINDINGS
            .iter()
            .filter(|binding| binding.action == action && binding.surfaces.contains(&surface))
    };
    let pressed = bindings().any(|binding| binding.went_down(ctx));
    bindings().for_each(|binding| binding.key_combination.take(ctx));
    pressed
}

/// The actions whose chords were pressed in this frame on `surface`, in
/// the table's order, each once, however many of its chords were pressed.
/// Every chord on `surface` is taken out of the frame's input, pressed or
/// held down, so no widget drawn after sees its key too
/// ([`KeyCombination::take`]).
pub fn take_pressed_actions(ctx: &egui::Context, surface: Surface) -> Vec<Action> {
    let bindings = || {
        BINDINGS
            .iter()
            .filter(|binding| binding.surfaces.contains(&surface))
    };
    let mut actions = Vec::new();
    for binding in bindings() {
        if !actions.contains(&binding.action) && binding.went_down(ctx) {
            actions.push(binding.action);
        }
    }
    bindings().for_each(|binding| binding.key_combination.take(ctx));
    actions
}

/// The chord that does `action` on `surface`, for a control that does the
/// same to name in its tooltip: the first in the table, when more than
/// one does. `None` when no chord does it there, or the action does
/// nothing yet.
pub fn key_combination_for(surface: Surface, action: Action) -> Option<KeyCombination> {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.action == action && binding.surfaces.contains(&surface) && binding.is_live()
        })
        .map(|binding| binding.key_combination)
}

/// The chord that does `action` wherever the keys apply, for a menu item,
/// which is there whatever the centre shows. `None` when none does, or the
/// action does nothing yet.
pub fn key_combination_everywhere(action: Action) -> Option<KeyCombination> {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.action == action
                && EVERYWHERE.iter().all(|s| binding.surfaces.contains(s))
                && binding.is_live()
        })
        .map(|binding| binding.key_combination)
}

/// The reference's sections: a section per group, each listing the
/// shortcuts that do something, with their chords spelled as this
/// system spells them.
pub fn sections() -> Vec<keys::Section<'static>> {
    Group::ALL
        .into_iter()
        .map(|group| keys::Section {
            title: group.title(),
            rows: BINDINGS
                .iter()
                .filter(|binding| binding.group == group && binding.is_live())
                .map(|binding| (binding.key_combination.label(), binding.description))
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_shortcuts_share_a_combination_where_both_apply() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                let shared = a.surfaces.iter().any(|s| b.surfaces.contains(s));
                assert!(
                    !(shared && a.key_combination == b.key_combination),
                    "{:?} and {:?} share {}",
                    a.action,
                    b.action,
                    a.key_combination.label()
                );
            }
        }
    }

    #[test]
    fn every_shortcut_applies_somewhere_and_says_what_it_does() {
        for binding in BINDINGS {
            assert!(!binding.surfaces.is_empty(), "{:?}", binding.action);
            assert!(!binding.description.is_empty(), "{:?}", binding.action);
        }
    }

    /// An action pressed by two of its chords in one frame is done once,
    /// as a toggle done twice would undo itself.
    #[test]
    fn an_action_pressed_twice_in_a_frame_is_done_once() {
        let press = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: NONE,
        };
        let input = egui::RawInput {
            events: vec![press(Key::G), press(Key::Escape)],
            ..egui::RawInput::default()
        };
        let mut actions = Vec::new();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(input, |ui| {
            actions = take_pressed_actions(ui.ctx(), Surface::Viewer);
        });
        output.textures_delta.clear();
        assert_eq!(actions, [Action::View(View::Grid)]);
    }

    /// Enter opens the viewer from the grid, and does nothing in the
    /// viewer itself.
    #[test]
    fn enter_opens_the_viewer_from_the_grid_only() {
        let actions = |surface| {
            // A context of its own, so Enter goes down afresh, not as the
            // system repeating it.
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: NONE,
                }],
                ..egui::RawInput::default()
            };
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(input, |ui| {
                actions = take_pressed_actions(ui.ctx(), surface);
            });
            output.textures_delta.clear();
            actions
        };
        assert_eq!(actions(Surface::Grid), [Action::View(View::Viewer)]);
        assert_eq!(actions(Surface::Viewer), []);
    }

    /// A key held down does again, as the system repeats it, an action
    /// that steps through the photos, and not one that shows or hides
    /// something, which would flicker on and off.
    #[test]
    fn a_held_key_steps_through_the_photos_but_toggles_once() {
        let press = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: NONE,
        };
        let ctx = egui::Context::default();
        let frames: Vec<Vec<Action>> = (0..3)
            .map(|_| {
                // egui counts a key that goes down again while it is
                // down as repeating.
                let input = egui::RawInput {
                    events: vec![press(Key::ArrowRight), press(Key::Tab)],
                    ..egui::RawInput::default()
                };
                let mut actions = Vec::new();
                let mut output = ctx.run_ui(input, |ui| {
                    actions = take_pressed_actions(ui.ctx(), Surface::Grid);
                });
                output.textures_delta.clear();
                actions
            })
            .collect();
        let next = Action::Photo(Step::Next);
        assert_eq!(
            frames,
            [vec![next, Action::SideDocks], vec![next], vec![next]]
        );
    }

    /// The keys still in the frame's input, in their order.
    fn keys_left(ctx: &egui::Context) -> Vec<Key> {
        ctx.input(|input| {
            let keys = input.events.iter().filter_map(|event| match event {
                egui::Event::Key { key, .. } => Some(*key),
                _ => None,
            });
            keys.collect()
        })
    }

    /// The keys of every shortcut on a surface are taken out of the
    /// frame's input, pressed or held down, even where a held key does
    /// nothing, as a toggle's does not; a key with no shortcut there is
    /// left, as Enter is in the viewer.
    #[test]
    fn the_keys_of_the_shortcuts_here_are_taken_and_no_others() {
        let press = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: NONE,
        };
        let ctx = egui::Context::default();
        let frames: Vec<(Vec<Action>, Vec<Key>)> = (0..2)
            .map(|_| {
                // egui counts a key that goes down again while it is
                // down as repeating.
                let input = egui::RawInput {
                    events: vec![press(Key::Tab), press(Key::Enter)],
                    ..egui::RawInput::default()
                };
                let mut frame = Default::default();
                let mut output = ctx.run_ui(input, |ui| {
                    let actions = take_pressed_actions(ui.ctx(), Surface::Viewer);
                    frame = (actions, keys_left(ui.ctx()));
                });
                output.textures_delta.clear();
                frame
            })
            .collect();
        assert_eq!(
            frames,
            [
                (vec![Action::SideDocks], vec![Key::Enter]),
                (vec![], vec![Key::Enter])
            ]
        );
    }

    /// Asking whether one action's chord was pressed takes that chord
    /// alone out of the frame's input.
    #[test]
    fn asking_for_one_action_takes_its_combination_alone() {
        let press = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let input = egui::RawInput {
            events: vec![press(Key::Slash, CTRL), press(Key::Tab, NONE)],
            ..egui::RawInput::default()
        };
        let ctx = egui::Context::default();
        let (mut pressed, mut left) = (false, Vec::new());
        let mut output = ctx.run_ui(input, |ui| {
            pressed = take_pressed(ui.ctx(), Surface::Grid, Action::Shortcuts);
            left = keys_left(ui.ctx());
        });
        output.textures_delta.clear();
        assert!(pressed);
        assert_eq!(left, [Key::Tab]);
    }

    /// Only the keys that move between photos repeat.
    #[test]
    fn only_the_photo_keys_repeat() {
        for binding in BINDINGS {
            let photo = matches!(binding.action, Action::Photo(_));
            assert_eq!(binding.repeats, photo, "{:?}", binding.action);
        }
    }

    /// A control is named the chord that does what it does where it is,
    /// and none where no chord does, or where it does nothing yet.
    #[test]
    fn a_control_s_combination_is_the_first_that_does_its_action_there() {
        let label =
            |surface, action| key_combination_for(surface, action).map(|c| c.label_for(false));
        let grid = Action::View(View::Grid);
        assert_eq!(label(Surface::Viewer, grid).as_deref(), Some("G"));
        assert_eq!(label(Surface::Grid, grid).as_deref(), Some("G"));
        // Enter opens the viewer from the grid too, after E in the table.
        let viewer = Action::View(View::Viewer);
        assert_eq!(label(Surface::Grid, viewer).as_deref(), Some("E"));
        let filmstrip = Action::Dock(Side::Bottom);
        assert_eq!(label(Surface::Viewer, filmstrip).as_deref(), Some("F6"));
        assert_eq!(label(Surface::Grid, filmstrip), None);
        assert_eq!(label(Surface::Grid, Action::Mode(Mode::Organizer)), None);
        let below = Action::Photo(Step::Below);
        let down = KeyCombination::new(NONE, Key::ArrowDown);
        assert_eq!(key_combination_for(Surface::Grid, below), Some(down));
        assert_eq!(key_combination_for(Surface::Viewer, below), None);
    }

    /// The reference lists the keys that move between photos under
    /// Photos, the section after Views: the previous and next photo, the
    /// one above and below, and the first and last.
    #[test]
    fn the_reference_lists_the_photo_keys_after_the_views() {
        let sections = sections();
        let titles: Vec<_> = sections.iter().map(|section| section.title).collect();
        assert_eq!(titles, ["Views", "Photos", "Zoom", "Docks", "General"]);
        let label = |key: Key| KeyCombination::new(NONE, key).label();
        let rows = [
            (label(Key::ArrowLeft), "Previous photo"),
            (label(Key::ArrowRight), "Next photo"),
            (label(Key::ArrowUp), "Photo above"),
            (label(Key::ArrowDown), "Photo below"),
            (label(Key::Home), "First photo"),
            (label(Key::End), "Last photo"),
        ];
        assert_eq!(sections[1].rows, rows);
    }

    /// The reference lists the viewer's zoom keys under Zoom, the section
    /// after Photos: Z and Space between fitted and 100 %, Ctrl+= and
    /// Ctrl++ in, Ctrl+- out, Ctrl+0 to fit and Ctrl+Alt+0 to 100 %.
    #[test]
    fn the_reference_lists_the_zoom_keys_after_the_photos() {
        let sections = sections();
        assert_eq!(sections[2].title, "Zoom");
        let label = |modifiers, key| KeyCombination::new(modifiers, key).label();
        let toggle = "Switch between fitted and 100 %";
        let rows = [
            (label(NONE, Key::Z), toggle),
            (label(NONE, Key::Space), toggle),
            (label(CTRL, Key::Equals), "Zoom in"),
            (label(CTRL, Key::Plus), "Zoom in"),
            (label(CTRL, Key::Minus), "Zoom out"),
            (label(CTRL, Key::Num0), "Zoom to fit"),
            (label(CTRL_ALT, Key::Num0), "Zoom to 100 %"),
        ];
        assert_eq!(sections[2].rows, rows);
    }

    /// The actions pressed on `surface` in a frame whose input is
    /// `events`, in a context of its own, so every key goes down afresh.
    fn pressed_in(surface: Surface, events: Vec<egui::Event>) -> Vec<Action> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events,
            ..egui::RawInput::default()
        };
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            actions = take_pressed_actions(ui.ctx(), surface);
        });
        output.textures_delta.clear();
        actions
    }

    /// `key` going down with `modifiers` held, typed on the keyboard's
    /// key `physical`.
    fn typed(key: Key, physical: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(physical),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// In the viewer, Z and Space each switch between fitted and 100 %,
    /// Ctrl+= zooms in, and so does Ctrl++, typed with Shift on the `=`
    /// key as a US keyboard types it, or on a key of its own; Ctrl+-
    /// zooms out, Ctrl+0 fits and Ctrl+Alt+0 goes to 100 %.
    #[test]
    fn the_zoom_keys_zoom_in_the_viewer() {
        let zoom = |zooming| vec![Action::Zoom(zooming)];
        let cases = [
            (typed(Key::Z, Key::Z, NONE), zoom(Zooming::Toggle)),
            (typed(Key::Space, Key::Space, NONE), zoom(Zooming::Toggle)),
            (typed(Key::Equals, Key::Equals, CTRL), zoom(Zooming::In)),
            (
                typed(Key::Plus, Key::Equals, CTRL.plus(SHIFT)),
                zoom(Zooming::In),
            ),
            (typed(Key::Plus, Key::CloseBracket, CTRL), zoom(Zooming::In)),
            (typed(Key::Minus, Key::Minus, CTRL), zoom(Zooming::Out)),
            (typed(Key::Num0, Key::Num0, CTRL), zoom(Zooming::Fit)),
            (typed(Key::Num0, Key::Num0, CTRL_ALT), zoom(Zooming::Full)),
        ];
        for (event, actions) in cases {
            assert_eq!(
                pressed_in(Surface::Viewer, vec![event.clone()]),
                actions,
                "{event:?}"
            );
        }
        // Shift+= types `+` with no Ctrl, which zooms nothing.
        let plus = typed(Key::Plus, Key::Equals, SHIFT);
        assert_eq!(pressed_in(Surface::Viewer, vec![plus]), []);
    }

    /// The zoom keys do nothing in the grid, and are left in its input.
    #[test]
    fn the_zoom_keys_do_nothing_in_the_grid() {
        let zoom_keys = BINDINGS
            .iter()
            .filter(|binding| matches!(binding.action, Action::Zoom(_)));
        for binding in zoom_keys {
            let KeyCombination { modifiers, key } = binding.key_combination;
            assert_eq!(
                binding.surfaces,
                VIEWER,
                "{}",
                binding.key_combination.label()
            );
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                events: vec![typed(key, key, modifiers)],
                ..egui::RawInput::default()
            };
            let (mut actions, mut left) = (Vec::new(), Vec::new());
            let mut output = ctx.run_ui(input, |ui| {
                actions = take_pressed_actions(ui.ctx(), Surface::Grid);
                left = keys_left(ui.ctx());
            });
            output.textures_delta.clear();
            assert_eq!(actions, [], "{}", binding.key_combination.label());
            assert_eq!(left, [key], "{}", binding.key_combination.label());
        }
    }

    /// Space or Z held down, the system repeating it, switches between
    /// fitted and 100 % once, as a toggle does, and its repeats are taken
    /// out of the input all the same.
    #[test]
    fn a_held_zoom_toggle_switches_once() {
        for key in [Key::Space, Key::Z] {
            let ctx = egui::Context::default();
            let frames: Vec<(Vec<Action>, Vec<Key>)> = (0..3)
                .map(|_| {
                    // egui counts a key that goes down again while it is
                    // down as repeating.
                    let input = egui::RawInput {
                        events: vec![typed(key, key, NONE)],
                        ..egui::RawInput::default()
                    };
                    let mut frame = Default::default();
                    let mut output = ctx.run_ui(input, |ui| {
                        let actions = take_pressed_actions(ui.ctx(), Surface::Viewer);
                        frame = (actions, keys_left(ui.ctx()));
                    });
                    output.textures_delta.clear();
                    frame
                })
                .collect();
            let toggle = Action::Zoom(Zooming::Toggle);
            let once = [(vec![toggle], vec![]), (vec![], vec![]), (vec![], vec![])];
            assert_eq!(frames, once, "{key:?}");
        }
    }

    /// The toolbar's zoom controls are named the chords that do what they
    /// do in the viewer, and none in the grid.
    #[test]
    fn the_zoom_controls_combinations_are_the_viewer_s() {
        let label = |surface, zooming| {
            key_combination_for(surface, Action::Zoom(zooming)).map(|c| c.label_for(false))
        };
        assert_eq!(
            label(Surface::Viewer, Zooming::Toggle).as_deref(),
            Some("Z")
        );
        assert_eq!(
            label(Surface::Viewer, Zooming::In).as_deref(),
            Some("Ctrl+=")
        );
        assert_eq!(
            label(Surface::Viewer, Zooming::Out).as_deref(), // egui spells the minus as the minus sign, not the hyphen.
            Some("Ctrl+−")
        );
        assert_eq!(
            label(Surface::Viewer, Zooming::Fit).as_deref(),
            Some("Ctrl+0")
        );
        let full = label(Surface::Viewer, Zooming::Full);
        assert_eq!(full.as_deref(), Some("Ctrl+Alt+0"));
        assert_eq!(label(Surface::Grid, Zooming::In), None);
        assert_eq!(key_combination_everywhere(Action::Zoom(Zooming::In)), None);
    }

    /// The window's typeface draws every chord the reference spells,
    /// the arrow keys' symbols among them.
    #[test]
    fn the_typeface_draws_every_combination() {
        let ctx = egui::Context::default();
        desktop_ui::fonts::install(&ctx);
        // egui takes the fonts up at the start of a pass.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(13.0);
        for binding in BINDINGS {
            let label = binding.key_combination.label();
            let drawn = ctx.fonts_mut(|fonts| fonts.has_glyphs(&font, &label));
            assert!(drawn, "{label} for {:?}", binding.action);
        }
    }

    /// The reference lists every shortcut that does something, under its
    /// group, and leaves out a mode not built yet.
    #[test]
    fn the_reference_lists_every_shortcut_that_does_something() {
        let sections = sections();
        for binding in BINDINGS {
            let row = (binding.key_combination.label(), binding.description);
            let section = sections
                .iter()
                .find(|section| section.title == binding.group.title())
                .expect("a section per group");
            let listed = section.rows.contains(&row);
            assert_eq!(listed, binding.is_live(), "{:?}", binding.action);
        }
    }
}
