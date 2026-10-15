// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard shortcuts: a key and the modifiers held with it ([`Chord`]),
//! how a chord is spelled on each system, and whether it was pressed.
//!
//! What a chord does is the app's business: an app keeps its own table of
//! actions and their chords, and asks a chord whether it was pressed.

use egui::{Event, Key, Modifiers};

/// A key pressed with a set of modifiers, such as Ctrl+Q. Its
/// [`Modifiers::COMMAND`] is Cmd on macOS and Ctrl elsewhere, so one chord
/// serves both, spelled the way each system spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The modifiers held with the key.
    pub modifiers: Modifiers,
    /// The key.
    pub key: Key,
}

impl Chord {
    /// `key` pressed with `modifiers`.
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Whether the chord was pressed in this frame: its key went down
    /// with exactly its modifiers held, so G is not pressed by Shift+G,
    /// but for a punctuation character the keyboard types with Shift on
    /// another key, as a German one types `/` with Shift+7: that Shift is
    /// the keyboard's, not the chord's. A key held down repeats, but only
    /// its first press counts, so a chord that shows or hides something
    /// does it once.
    pub fn pressed(self, ctx: &egui::Context) -> bool {
        ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(event, Event::Key { key, physical_key, pressed: true, repeat: false, modifiers }
                    if *key == self.key && self.held_with(*modifiers, *physical_key))
            })
        })
    }

    /// Whether `held` are the chord's modifiers, for its key typed on the
    /// keyboard's key `physical`. Shift is left out when the chord has
    /// none and its punctuation character came from another key, which
    /// takes Shift to type it. A character typed on its own key with
    /// Shift held is another character (`?` from `/`) or, where egui has
    /// no key for it (`<` from `,` on a US keyboard), comes as that key
    /// with Shift, which is another chord.
    fn held_with(self, held: Modifiers, physical: Option<Key>) -> bool {
        let shifted_elsewhere = physical.is_some_and(|physical| physical != self.key);
        if !self.modifiers.shift && is_punctuation(self.key) && shifted_elsewhere {
            held.alt == self.modifiers.alt && held.cmd_ctrl_matches(self.modifiers)
        } else {
            held.matches_exact(self.modifiers)
        }
    }

    /// The chord as this system spells it: "⌥⌘1" on macOS, "Ctrl+Alt+1"
    /// elsewhere.
    pub fn label(self) -> String {
        self.label_for(cfg!(target_os = "macos"))
    }

    /// The chord as macOS spells it, when `mac`, or as Windows and Linux
    /// do: on macOS the modifiers' symbols in the order its menus give
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
            | Key::Equals
            | Key::OpenBracket
            | Key::CloseBracket
    )
}

#[cfg(test)]
mod tests {
    use egui::{FontFamily, FontId, RawInput};

    use super::*;

    const CTRL_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);

    #[test]
    fn a_chord_is_spelled_as_windows_and_linux_spell_it() {
        let label = |modifiers, key| Chord::new(modifiers, key).label_for(false);
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
    fn a_chord_is_spelled_as_macos_spells_it() {
        let label = |modifiers, key| Chord::new(modifiers, key).label_for(true);
        assert_eq!(label(Modifiers::NONE, Key::G), "G");
        assert_eq!(label(Modifiers::NONE, Key::Escape), "Esc");
        assert_eq!(label(Modifiers::SHIFT, Key::Tab), "⇧Tab");
        assert_eq!(label(Modifiers::COMMAND, Key::Comma), "⌘,");
        assert_eq!(label(CTRL_ALT, Key::Num1), "⌥⌘1");
        let all = CTRL_ALT.plus(Modifiers::SHIFT).plus(Modifiers::CTRL);
        assert_eq!(label(all, Key::Q), "⌃⌥⇧⌘Q");
    }

    /// The window's typeface draws the modifiers' symbols that macOS's
    /// chords are spelled with.
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

    /// Whether `chord` is pressed in each of a run of frames, in each of
    /// which `key` goes down with `modifiers` held: egui counts every
    /// frame after the first as the key repeating.
    fn pressed_by(chord: Chord, key: Key, modifiers: Modifiers, frames: usize) -> Vec<bool> {
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
                let mut output = ctx.run_ui(input, |ui| pressed = chord.pressed(ui.ctx()));
                // A headless frame has no renderer to hand the font atlas to.
                output.textures_delta.clear();
                pressed
            })
            .collect()
    }

    #[test]
    fn a_chord_is_pressed_by_its_key_with_exactly_its_modifiers() {
        let once = |chord, key, modifiers| pressed_by(chord, key, modifiers, 1) == [true];
        let g = Chord::new(Modifiers::NONE, Key::G);
        assert!(once(g, Key::G, Modifiers::NONE));
        assert!(!once(g, Key::G, Modifiers::SHIFT), "Shift+G");
        assert!(!once(g, Key::E, Modifiers::NONE), "E");
        let ctrl_alt_1 = Chord::new(CTRL_ALT, Key::Num1);
        assert!(once(ctrl_alt_1, Key::Num1, CTRL_ALT));
        assert!(!once(ctrl_alt_1, Key::Num1, Modifiers::ALT), "Alt+1");
        assert!(!once(ctrl_alt_1, Key::Num1, Modifiers::COMMAND), "Ctrl+1");
    }

    /// Whether `chord` is pressed in a frame whose only event is `key`
    /// going down, typed on the keyboard's key `physical`, with
    /// `modifiers` held.
    fn typed(chord: Chord, (key, physical): (Key, Key), modifiers: Modifiers) -> bool {
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
        let mut output = ctx.run_ui(input, |ui| pressed = chord.pressed(ui.ctx()));
        output.textures_delta.clear();
        pressed
    }

    /// A punctuation character the keyboard types with Shift on another
    /// key presses its chord with Shift held; one typed on its own key
    /// with Shift held does not, nor does a letter.
    #[test]
    fn shift_counts_only_where_the_chord_s_character_needs_it() {
        let ctrl_slash = Chord::new(Modifiers::COMMAND, Key::Slash);
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
        let ctrl_comma = Chord::new(Modifiers::COMMAND, Key::Comma);
        assert!(
            !typed(ctrl_comma, (Key::Comma, Key::Comma), ctrl_shift),
            "Ctrl+Shift+,"
        );
        // A chord with Shift of its own needs it.
        let ctrl_shift_comma = Chord::new(ctrl_shift, Key::Comma);
        assert!(!typed(
            ctrl_shift_comma,
            (Key::Comma, Key::M),
            Modifiers::COMMAND
        ));
        // A letter typed on another key, as on a Dvorak keyboard.
        let ctrl_g = Chord::new(Modifiers::COMMAND, Key::G);
        assert!(!typed(ctrl_g, (Key::G, Key::I), ctrl_shift), "Ctrl+Shift+G");
    }

    #[test]
    fn a_key_held_down_presses_its_chord_once() {
        let tab = Chord::new(Modifiers::NONE, Key::Tab);
        let presses = pressed_by(tab, Key::Tab, Modifiers::NONE, 3);
        assert_eq!(presses, [true, false, false]);
    }
}
