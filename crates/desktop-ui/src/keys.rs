// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard shortcuts: a key and the modifiers held with it
//! ([`KeyCombination`]), how a key combination is spelled on each system,
//! whether it was pressed, and taking it out of the frame's input, so
//! nothing else acts on its key; and the reference that lists an app's
//! shortcuts ([`reference()`]).
//!
//! What a key combination does is the app's business: an app keeps its own
//! table of actions and their combinations, asks a combination whether it
//! was pressed, and builds its reference from the same table, so the two
//! cannot disagree.

use egui::{Event, FontFamily, FontId, Grid, Key, Label, Modifiers, RichText};

use crate::fonts;

/// The gap between a reference's key combinations and their descriptions.
const COLUMN_GAP: f32 = 18.0;

/// The narrowest a reference's descriptions are, however little room the key
/// combinations leave them.
const MIN_DESCRIPTION_WIDTH: f32 = 160.0;

/// A key pressed with a set of modifiers, such as Ctrl+Q. Its
/// [`Modifiers::COMMAND`] is Cmd on macOS and Ctrl elsewhere, so one key
/// combination serves both, spelled the way each system spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyCombination {
    /// The modifiers held with the key.
    pub modifiers: Modifiers,
    /// The key.
    pub key: Key,
}

impl KeyCombination {
    /// `key` pressed with `modifiers`.
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Whether the key combination was pressed in this frame: its key went
    /// down with exactly its modifiers held, so G is not pressed by Shift+G,
    /// but for a punctuation character the keyboard types with Shift on
    /// another key, as a German one types `/` with Shift+7: that Shift is
    /// the keyboard's, not the combination's. A key held down repeats, but
    /// only its first press counts, so a combination that shows or hides
    /// something does it once.
    pub fn pressed(self, ctx: &egui::Context) -> bool {
        self.went_down(ctx, false)
    }

    /// Whether the key combination was pressed in this frame, as for
    /// [`pressed`](Self::pressed), or its key, held down since, repeated:
    /// for a combination that moves through something, as to the next photo,
    /// which keeps moving while the key is held. A held key repeats as the
    /// system repeats it in a text field, after the system's delay and at
    /// its pace, so a press moves once and a short hold no more.
    pub fn pressed_or_repeated(self, ctx: &egui::Context) -> bool {
        self.went_down(ctx, true)
    }

    /// Take the key combination out of this frame's input, so nothing that
    /// reads the input after it, such as a widget drawn later in the frame,
    /// sees its key go down too: each time the key went down with the
    /// combination's modifiers held, as [`pressed`](Self::pressed) counts
    /// it, or, held down, repeated. egui takes Enter and Space for a click
    /// on the widget that has the keyboard focus, so a combination that does
    /// something is taken before any widget is drawn, or one press would do
    /// two things. Its repeats go too, even for a combination that does
    /// nothing with them, as the held key is still the combination's. The
    /// key coming back up is left, as no widget does anything with that.
    /// egui's own `consume_key` would take a key held with more modifiers
    /// than the combination's, Shift+G for G, which is another combination
    /// or none.
    pub fn take(self, ctx: &egui::Context) {
        ctx.input_mut(|input| {
            input.events.retain(|event| !self.goes_down_in(event, true));
        });
    }

    /// Whether its key went down in this frame with its modifiers held,
    /// counting a held key's repeats when `repeats`.
    fn went_down(self, ctx: &egui::Context, repeats: bool) -> bool {
        ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| self.goes_down_in(event, repeats))
        })
    }

    /// Whether `event` is its key going down with its modifiers held, or,
    /// when `repeats`, repeating.
    fn goes_down_in(self, event: &Event, repeats: bool) -> bool {
        matches!(event, Event::Key { key, physical_key, pressed: true, repeat, modifiers }
            if *key == self.key
                && (repeats || !*repeat)
                && self.held_with(*modifiers, *physical_key))
    }

    /// Whether `held` are its modifiers, for its key typed on the keyboard's
    /// key `physical`. Shift is left out when the combination has none and
    /// its punctuation character came from another key, which takes Shift
    /// to type it. A character typed on its own key with Shift held is
    /// another character (`?` from `/`) or, where egui has no key for it
    /// (`<` from `,` on a US keyboard), comes as that key with Shift, which
    /// is another combination.
    fn held_with(self, held: Modifiers, physical: Option<Key>) -> bool {
        let shifted_elsewhere = physical.is_some_and(|physical| physical != self.key);
        if !self.modifiers.shift && is_punctuation(self.key) && shifted_elsewhere {
            held.alt == self.modifiers.alt && held.cmd_ctrl_matches(self.modifiers)
        } else {
            held.matches_exact(self.modifiers)
        }
    }

    /// The key combination as this system spells it: "⌥⌘1" on macOS,
    /// "Ctrl+Alt+1" elsewhere.
    pub fn label(self) -> String {
        self.label_for(cfg!(target_os = "macos"))
    }

    /// The key combination as macOS spells it, when `mac`, or as Windows and
    /// Linux do: on macOS the modifiers' symbols in the order its menus give
    /// them, ⌃⌥⇧⌘, run into the key; elsewhere their names in the order
    /// Ctrl, Alt, Shift, each followed by a plus.
    pub fn label_for(self, mac: bool) -> String {
        let Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd,
            command,
        } = self.modifiers;
        let held: &[(bool, &str)] = if mac {
            &[
                (ctrl, "⌃"),
                (alt, "⌥"),
                (shift, "⇧"),
                (command || mac_cmd, "⌘"),
            ]
        } else {
            &[(ctrl || command, "Ctrl+"), (alt, "Alt+"), (shift, "Shift+")]
        };
        let mut label: String = held
            .iter()
            .filter(|(down, _)| *down)
            .map(|(_, name)| *name)
            .collect();
        label.push_str(match self.key {
            Key::Escape => "Esc",
            key => key.symbol_or_name(),
        });
        label
    }
}

/// Whether `key` types a punctuation character.
fn is_punctuation(key: Key) -> bool {
    matches!(
        key,
        Key::Comma
            | Key::Period
            | Key::Slash
            | Key::Backslash
            | Key::Semicolon
            | Key::Quote
            | Key::Backtick
            | Key::Minus
            | Key::Plus
            | Key::Equals
            | Key::OpenBracket
            | Key::CloseBracket
    )
}

/// What the tooltip of a control with a key combination says: its `label`,
/// then, when it has one, the `key_combination` that does what a click on it
/// does, in brackets: "Thumbnail grid (G)".
pub fn tooltip(label: &str, key_combination: Option<KeyCombination>) -> String {
    match key_combination {
        Some(key_combination) => format!("{label} ({})", key_combination.label()),
        None => label.to_owned(),
    }
}

/// One section of a shortcut reference: its title, and its rows, each a key
/// combination's label and what the combination does.
pub struct Section<'a> {
    /// The section's title.
    pub title: &'a str,
    /// The section's rows.
    pub rows: Vec<(String, &'a str)>,
}

/// A reference of keyboard shortcuts: each section's title over its rows,
/// the key combination at the left and what it does at the right, with a
/// line between sections. A section with no rows is left out.
///
/// The key combinations' column is as wide as the widest combination in any
/// section, so the descriptions line up from one section to the next. A
/// description longer than the room left beside the combinations wraps.
pub fn reference(ui: &mut egui::Ui, sections: &[Section<'_>]) {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let combinations_width = sections
        .iter()
        .flat_map(|section| &section.rows)
        .map(|(combination, _)| {
            ui.fonts_mut(|fonts| {
                let layout =
                    fonts.layout_no_wrap(combination.clone(), body.clone(), Default::default());
                layout.size().x
            })
        })
        .fold(0.0, f32::max);
    let descriptions_width =
        (ui.available_width() - combinations_width - COLUMN_GAP).max(MIN_DESCRIPTION_WIDTH);
    let title_font = FontId::new(body.size, FontFamily::Name(fonts::SEMIBOLD.into()));
    let shown = sections.iter().filter(|section| !section.rows.is_empty());
    for (index, section) in shown.enumerate() {
        if index > 0 {
            ui.add_space(4.0);
            ui.separator();
        }
        ui.add_space(4.0);
        let title = RichText::new(section.title).font(title_font.clone());
        ui.add(Label::new(title).selectable(false));
        ui.add_space(2.0);
        Grid::new(("shortcuts", section.title))
            .num_columns(2)
            .min_col_width(combinations_width)
            .max_col_width(descriptions_width)
            .spacing([COLUMN_GAP, 4.0])
            .show(ui, |ui| {
                for (combination, description) in &section.rows {
                    ui.add(Label::new(combination.as_str()).selectable(false));
                    ui.add(Label::new(*description).wrap().selectable(false));
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use egui::RawInput;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    const CTRL_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);

    #[test]
    fn a_combination_is_spelled_as_windows_and_linux_spell_it() {
        let label = |modifiers, key| KeyCombination::new(modifiers, key).label_for(false);
        assert_eq!(label(Modifiers::NONE, Key::G), "G");
        assert_eq!(label(Modifiers::NONE, Key::F7), "F7");
        assert_eq!(label(Modifiers::NONE, Key::Escape), "Esc");
        assert_eq!(label(Modifiers::SHIFT, Key::Tab), "Shift+Tab");
        assert_eq!(label(Modifiers::COMMAND, Key::Comma), "Ctrl+,");
        assert_eq!(label(Modifiers::COMMAND, Key::Slash), "Ctrl+/");
        assert_eq!(label(CTRL_ALT, Key::Num1), "Ctrl+Alt+1");
        let all = CTRL_ALT.plus(Modifiers::SHIFT);
        assert_eq!(label(all, Key::Q), "Ctrl+Alt+Shift+Q");
    }

    #[test]
    fn a_combination_is_spelled_as_macos_spells_it() {
        let label = |modifiers, key| KeyCombination::new(modifiers, key).label_for(true);
        assert_eq!(label(Modifiers::NONE, Key::G), "G");
        assert_eq!(label(Modifiers::NONE, Key::Escape), "Esc");
        assert_eq!(label(Modifiers::SHIFT, Key::Tab), "⇧Tab");
        assert_eq!(label(Modifiers::COMMAND, Key::Comma), "⌘,");
        assert_eq!(label(CTRL_ALT, Key::Num1), "⌥⌘1");
        let all = CTRL_ALT.plus(Modifiers::SHIFT).plus(Modifiers::CTRL);
        assert_eq!(label(all, Key::Q), "⌃⌥⇧⌘Q");
    }

    /// The window's typeface draws the modifiers' symbols that macOS's key
    /// combinations are spelled with.
    #[test]
    fn the_typeface_draws_the_modifier_symbols() {
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        // egui takes the fonts up at the start of a pass.
        let mut output = ctx.run_ui(RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = FontId::new(13.0, FontFamily::Proportional);
        let drawn = ctx.fonts_mut(|fonts| fonts.has_glyphs(&font, "⌃⌥⇧⌘"));
        assert!(drawn, "a modifier symbol has no glyph");
    }

    /// Whether `combination` is pressed in each of a run of frames, in each
    /// of which `key` goes down with `modifiers` held: egui counts every
    /// frame after the first as the key repeating.
    fn pressed_by(
        combination: KeyCombination,
        key: Key,
        modifiers: Modifiers,
        frames: usize,
    ) -> Vec<bool> {
        went_down_by(combination, key, modifiers, frames, KeyCombination::pressed)
    }

    /// Whether `went_down` says `combination` went down in each of a run of
    /// frames, as for [`pressed_by`].
    fn went_down_by(
        combination: KeyCombination,
        key: Key,
        modifiers: Modifiers,
        frames: usize,
        went_down: fn(KeyCombination, &egui::Context) -> bool,
    ) -> Vec<bool> {
        let ctx = egui::Context::default();
        let press = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        (0..frames)
            .map(|_| {
                let input = RawInput {
                    events: vec![press.clone()],
                    ..RawInput::default()
                };
                let mut pressed = false;
                let mut output = ctx.run_ui(input, |ui| pressed = went_down(combination, ui.ctx()));
                // A headless frame has no renderer to hand the font atlas to.
                output.textures_delta.clear();
                pressed
            })
            .collect()
    }

    #[test]
    fn a_combination_is_pressed_by_its_key_with_exactly_its_modifiers() {
        let once =
            |combination, key, modifiers| pressed_by(combination, key, modifiers, 1) == [true];
        let g = KeyCombination::new(Modifiers::NONE, Key::G);
        assert!(once(g, Key::G, Modifiers::NONE));
        assert!(!once(g, Key::G, Modifiers::SHIFT), "Shift+G");
        assert!(!once(g, Key::E, Modifiers::NONE), "E");
        let ctrl_alt_1 = KeyCombination::new(CTRL_ALT, Key::Num1);
        assert!(once(ctrl_alt_1, Key::Num1, CTRL_ALT));
        assert!(!once(ctrl_alt_1, Key::Num1, Modifiers::ALT), "Alt+1");
        assert!(!once(ctrl_alt_1, Key::Num1, Modifiers::COMMAND), "Ctrl+1");
    }

    /// Whether `combination` is pressed in a frame whose only event is `key`
    /// going down, typed on the keyboard's key `physical`, with
    /// `modifiers` held.
    fn typed(
        combination: KeyCombination,
        (key, physical): (Key, Key),
        modifiers: Modifiers,
    ) -> bool {
        let ctx = egui::Context::default();
        let input = RawInput {
            events: vec![Event::Key {
                key,
                physical_key: Some(physical),
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..RawInput::default()
        };
        let mut pressed = false;
        let mut output = ctx.run_ui(input, |ui| pressed = combination.pressed(ui.ctx()));
        output.textures_delta.clear();
        pressed
    }

    /// A punctuation character the keyboard types with Shift on another key
    /// presses its key combination with Shift held; one typed on its own key
    /// with Shift held does not, nor does a letter.
    #[test]
    fn shift_counts_only_where_the_combination_s_character_needs_it() {
        let ctrl_slash = KeyCombination::new(Modifiers::COMMAND, Key::Slash);
        let ctrl_shift = Modifiers::COMMAND.plus(Modifiers::SHIFT);
        // US: `/` on its own key.
        assert!(typed(
            ctrl_slash,
            (Key::Slash, Key::Slash),
            Modifiers::COMMAND
        ));
        // German: `/` is Shift+7.
        assert!(
            typed(ctrl_slash, (Key::Slash, Key::Num7), ctrl_shift),
            "Ctrl+Shift+7"
        );
        let ctrl_alt_shift = CTRL_ALT.plus(Modifiers::SHIFT);
        assert!(
            !typed(ctrl_slash, (Key::Slash, Key::Num7), ctrl_alt_shift),
            "with Alt"
        );
        assert!(
            !typed(ctrl_slash, (Key::Slash, Key::Num7), Modifiers::SHIFT),
            "no Ctrl"
        );
        // US: Shift+, types `<`, which comes as the comma's own key.
        let ctrl_comma = KeyCombination::new(Modifiers::COMMAND, Key::Comma);
        assert!(
            !typed(ctrl_comma, (Key::Comma, Key::Comma), ctrl_shift),
            "Ctrl+Shift+,"
        );
        // A key combination with Shift of its own needs it.
        let ctrl_shift_comma = KeyCombination::new(ctrl_shift, Key::Comma);
        assert!(!typed(
            ctrl_shift_comma,
            (Key::Comma, Key::M),
            Modifiers::COMMAND
        ));
        // US: `+` is Shift+=, typed on the `=` key; German: it has a key
        // of its own, which egui names by where it is on a US one.
        let ctrl_plus = KeyCombination::new(Modifiers::COMMAND, Key::Plus);
        assert!(
            typed(ctrl_plus, (Key::Plus, Key::Equals), ctrl_shift),
            "Ctrl+Shift+="
        );
        assert!(typed(
            ctrl_plus,
            (Key::Plus, Key::CloseBracket),
            Modifiers::COMMAND
        ));
        // The `=` key alone types `=`, which is another key combination.
        let ctrl_equals = KeyCombination::new(Modifiers::COMMAND, Key::Equals);
        assert!(!typed(
            ctrl_plus,
            (Key::Equals, Key::Equals),
            Modifiers::COMMAND
        ));
        assert!(typed(
            ctrl_equals,
            (Key::Equals, Key::Equals),
            Modifiers::COMMAND
        ));
        // A letter typed on another key, as on a Dvorak keyboard.
        let ctrl_g = KeyCombination::new(Modifiers::COMMAND, Key::G);
        assert!(!typed(ctrl_g, (Key::G, Key::I), ctrl_shift), "Ctrl+Shift+G");
    }

    #[test]
    fn a_key_held_down_presses_its_combination_once() {
        let tab = KeyCombination::new(Modifiers::NONE, Key::Tab);
        let presses = pressed_by(tab, Key::Tab, Modifiers::NONE, 3);
        assert_eq!(presses, [true, false, false]);
    }

    /// A key held down repeats a key combination that asks for its repeats,
    /// with exactly its modifiers held, as when it was first pressed.
    #[test]
    fn a_key_held_down_repeats_a_combination_that_counts_its_repeats() {
        let right = KeyCombination::new(Modifiers::NONE, Key::ArrowRight);
        let held = |key, modifiers| {
            went_down_by(
                right,
                key,
                modifiers,
                3,
                KeyCombination::pressed_or_repeated,
            )
        };
        assert_eq!(held(Key::ArrowRight, Modifiers::NONE), [true, true, true]);
        assert_eq!(
            held(Key::ArrowRight, Modifiers::SHIFT),
            [false, false, false]
        );
        assert_eq!(held(Key::ArrowLeft, Modifiers::NONE), [false, false, false]);
    }

    /// `key` typed on the keyboard's key `physical` with `modifiers`
    /// held, going down when `pressed`, else coming back up.
    fn key_event(key: Key, physical: Option<Key>, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: physical,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    /// What is left of a frame's input of `events` once `combination` is
    /// taken out of it, each key event as its key, the keyboard's key,
    /// whether it went down, and the modifiers held. egui counts a key that
    /// goes down again while it is down as repeating.
    fn left_by_taking(
        combination: KeyCombination,
        events: Vec<Event>,
    ) -> Vec<(Key, Option<Key>, bool, Modifiers)> {
        let ctx = egui::Context::default();
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut left = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            combination.take(ui.ctx());
            left = ui.input(|input| input.events.clone());
        });
        output.textures_delta.clear();
        left.into_iter()
            .filter_map(|event| match event {
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    modifiers,
                    ..
                } => Some((key, physical_key, pressed, modifiers)),
                _ => None,
            })
            .collect()
    }

    /// Taking a key combination takes its key going down, and repeating, out
    /// of the frame's input, and leaves every other key, the same key with
    /// other modifiers held, and its key coming back up.
    #[test]
    fn taking_a_combination_takes_only_its_key_going_down() {
        let enter = KeyCombination::new(Modifiers::NONE, Key::Enter);
        let event = |key, pressed, modifiers| key_event(key, None, pressed, modifiers);
        let events = vec![
            event(Key::Enter, true, Modifiers::NONE),
            event(Key::Enter, true, Modifiers::NONE),
            event(Key::Enter, true, Modifiers::SHIFT),
            event(Key::Enter, true, Modifiers::COMMAND),
            event(Key::Space, true, Modifiers::NONE),
            event(Key::Enter, false, Modifiers::NONE),
        ];
        let left = [
            (Key::Enter, None, true, Modifiers::SHIFT),
            (Key::Enter, None, true, Modifiers::COMMAND),
            (Key::Space, None, true, Modifiers::NONE),
            (Key::Enter, None, false, Modifiers::NONE),
        ];
        assert_eq!(left_by_taking(enter, events), left);
    }

    /// Taking a key combination takes a punctuation character typed with
    /// Shift on another key, which presses the combination, as well as one
    /// typed on its own key, and leaves one typed with modifiers that do
    /// not.
    #[test]
    fn taking_a_combination_takes_it_however_the_keyboard_types_it() {
        let ctrl_slash = KeyCombination::new(Modifiers::COMMAND, Key::Slash);
        let slash = |physical, modifiers| key_event(Key::Slash, Some(physical), true, modifiers);
        let ctrl_alt_shift = CTRL_ALT.plus(Modifiers::SHIFT);
        let events = vec![
            slash(Key::Slash, Modifiers::COMMAND),
            // German: `/` is Shift+7.
            slash(Key::Num7, Modifiers::COMMAND.plus(Modifiers::SHIFT)),
            slash(Key::Num7, ctrl_alt_shift),
        ];
        let left = [(Key::Slash, Some(Key::Num7), true, ctrl_alt_shift)];
        assert_eq!(left_by_taking(ctrl_slash, events), left);
    }

    #[test]
    fn a_tooltip_names_the_combination_in_brackets() {
        let g = KeyCombination::new(Modifiers::NONE, Key::G);
        assert_eq!(tooltip("Thumbnail grid", Some(g)), "Thumbnail grid (G)");
        assert_eq!(tooltip("About", None), "About");
    }

    /// A reference of `sections` in a window `width` points wide, drawn
    /// once the fonts are in: the first frame installs them and draws
    /// nothing.
    fn shown(width: f32, sections: fn() -> Vec<Section<'static>>) -> Harness<'static> {
        let mut fonts_installed = false;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, 400.0))
            .build_ui(move |ui| {
                if fonts_installed {
                    reference(ui, &sections());
                } else {
                    fonts::install(ui.ctx());
                    fonts_installed = true;
                }
            });
        harness.run();
        harness
    }

    fn two_sections() -> Vec<Section<'static>> {
        vec![
            Section {
                title: "Views",
                rows: vec![("G".to_owned(), "Thumbnail grid")],
            },
            Section {
                title: "Window",
                rows: vec![("Ctrl+Alt+Shift+Q".to_owned(), "Quit")],
            },
            Section {
                title: "Nothing",
                rows: vec![],
            },
        ]
    }

    #[test]
    fn the_descriptions_line_up_from_section_to_section() {
        let harness = shown(500.0, two_sections);
        let grid = harness.get_by_label("Thumbnail grid").rect();
        let quit = harness.get_by_label("Quit").rect();
        assert_eq!(grid.left(), quit.left(), "{grid:?} {quit:?}");
        let combination = harness.get_by_label("Ctrl+Alt+Shift+Q").rect();
        assert!(
            combination.right() < quit.left(),
            "{combination:?} {quit:?}"
        );
    }

    #[test]
    fn a_section_with_no_rows_is_left_out() {
        let harness = shown(500.0, two_sections);
        assert!(harness.query_by_label("Views").is_some());
        assert!(harness.query_by_label("Nothing").is_none());
    }

    #[test]
    fn a_long_description_wraps_in_the_room_beside_the_combinations() {
        fn long() -> Vec<Section<'static>> {
            let description = "Show or hide every dock the mode has, the side docks and \
                the filmstrip together, as one";
            vec![Section {
                title: "Docks",
                rows: vec![("Shift+Tab".to_owned(), description)],
            }]
        }
        let harness = shown(300.0, long);
        let row = harness.get_by_label_contains("Show or hide").rect();
        assert!(row.right() <= 300.0, "{row:?}");
        let combination = harness.get_by_label("Shift+Tab").rect();
        assert!(row.height() > 2.0 * combination.height(), "{row:?}");
    }
}
