// SPDX-License-Identifier: GPL-3.0-or-later

//! The keyboard shortcuts: one table, [`BINDINGS`], of what each chord
//! does, where it does it, and how the reference describes it. The
//! handlers ask the table whether an action's chord was pressed
//! ([`pressed`]), and the reference is built from it ([`sections`]), so
//! neither can say something the table does not (ADR011).
//!
//! The keys are the ones established photography tools use for the same
//! thing, so a photographer's hands already know them, with Cmd for Ctrl
//! on macOS (ADR015).

use desktop_ui::dock::Side;
use desktop_ui::keys::{self, Chord};
use egui::{Key, Modifiers};

use crate::mode::Mode;
use crate::view::View;

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
    /// Which docks are shown.
    Docks,
    /// The modes, the dialogs, and quitting.
    General,
}

impl Group {
    /// Every group, in the reference's order.
    const ALL: [Self; 4] = [Self::Views, Self::Photos, Self::Docks, Self::General];

    /// The group's section title in the reference.
    fn title(self) -> &'static str {
        match self {
            Self::Views => "Views",
            Self::Photos => "Photos",
            Self::Docks => "Docks",
            Self::General => "General",
        }
    }
}

/// One shortcut: the chord that does an action on some surfaces, and the
/// group and description the reference shows it under.
struct Binding {
    action: Action,
    chord: Chord,
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
        chord: Chord::new(modifiers, key),
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
            self.chord.pressed_or_repeated(ctx)
        } else {
            self.chord.pressed(ctx)
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
/// frame.
pub fn pressed(ctx: &egui::Context, surface: Surface, action: Action) -> bool {
    BINDINGS.iter().any(|binding| {
        binding.action == action && binding.surfaces.contains(&surface) && binding.went_down(ctx)
    })
}

/// The actions whose chords were pressed in this frame on `surface`, in
/// the table's order, each once, however many of its chords were pressed.
pub fn pressed_actions(ctx: &egui::Context, surface: Surface) -> Vec<Action> {
    let mut actions = Vec::new();
    for binding in BINDINGS {
        if binding.surfaces.contains(&surface)
            && !actions.contains(&binding.action)
            && binding.went_down(ctx)
        {
            actions.push(binding.action);
        }
    }
    actions
}

/// The chord that does `action` on `surface`, for a control that does the
/// same to name in its tooltip: the first in the table, when more than
/// one does. `None` when no chord does it there, or the action does
/// nothing yet.
pub fn chord_for(surface: Surface, action: Action) -> Option<Chord> {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.action == action && binding.surfaces.contains(&surface) && binding.is_live()
        })
        .map(|binding| binding.chord)
}

/// The chord that does `action` wherever the keys apply, for a menu item,
/// which is there whatever the centre shows. `None` when none does, or the
/// action does nothing yet.
pub fn chord_everywhere(action: Action) -> Option<Chord> {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.action == action
                && EVERYWHERE.iter().all(|s| binding.surfaces.contains(s))
                && binding.is_live()
        })
        .map(|binding| binding.chord)
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
                .map(|binding| (binding.chord.label(), binding.description))
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_shortcuts_share_a_chord_where_both_apply() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                let shared = a.surfaces.iter().any(|s| b.surfaces.contains(s));
                assert!(
                    !(shared && a.chord == b.chord),
                    "{:?} and {:?} share {}",
                    a.action,
                    b.action,
                    a.chord.label()
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
            actions = pressed_actions(ui.ctx(), Surface::Viewer);
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
                actions = pressed_actions(ui.ctx(), surface);
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
                    actions = pressed_actions(ui.ctx(), Surface::Grid);
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
    fn a_control_s_chord_is_the_first_that_does_its_action_there() {
        let label = |surface, action| chord_for(surface, action).map(|c| c.label_for(false));
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
        let down = Chord::new(NONE, Key::ArrowDown);
        assert_eq!(chord_for(Surface::Grid, below), Some(down));
        assert_eq!(chord_for(Surface::Viewer, below), None);
    }

    /// The reference lists the keys that move between photos under
    /// Photos, the section after Views: the previous and next photo, the
    /// one above and below, and the first and last.
    #[test]
    fn the_reference_lists_the_photo_keys_after_the_views() {
        let sections = sections();
        let titles: Vec<_> = sections.iter().map(|section| section.title).collect();
        assert_eq!(titles, ["Views", "Photos", "Docks", "General"]);
        let label = |key: Key| Chord::new(NONE, key).label();
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

    /// The window's typeface draws every chord the reference spells,
    /// the arrow keys' symbols among them.
    #[test]
    fn the_typeface_draws_every_chord() {
        let ctx = egui::Context::default();
        desktop_ui::fonts::install(&ctx);
        // egui takes the fonts up at the start of a pass.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(13.0);
        for binding in BINDINGS {
            let label = binding.chord.label();
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
            let row = (binding.chord.label(), binding.description);
            let section = sections
                .iter()
                .find(|section| section.title == binding.group.title())
                .expect("a section per group");
            let listed = section.rows.contains(&row);
            assert_eq!(listed, binding.is_live(), "{:?}", binding.action);
        }
    }
}
