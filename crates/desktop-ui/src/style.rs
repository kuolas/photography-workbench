// SPDX-License-Identifier: GPL-3.0-or-later

//! The style every part of the window shares apart from its colours, which
//! come with the theme ([`crate::theme`]).
//!
//! The spacing: the room inside buttons, the gaps between widgets and the
//! height of a control. Controls are roomier than egui's own, 22 points
//! high to its 18, and sit about as close together. Margins, drop-down
//! lists and scroll bars keep egui's own.
//!
//! The text: small print at 11 points, where egui has 9, so it stays
//! legible, and headings at 16, where egui has 18, so a dock's heading
//! does not crowd it. Body text and buttons keep egui's 13.
//!
//! The pace: how long egui's animations take, from a section opening to a
//! tooltip, menu or dialog fading in and a dock sliding out. egui's own is
//! brisk; the window's is unhurried.

use egui::TextStyle;

/// How long every animation takes, in seconds; egui's own is 0.2.
const ANIMATION_TIME: f32 = 0.4;

/// Set the style in every one of egui's styles, light and dark. A change
/// of theme replaces only the visuals, so the style stays. Call it once,
/// before the first frame.
pub fn install(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let spacing = &mut style.spacing;
        // Room between a button's text and its edge.
        spacing.button_padding = egui::vec2(8.0, 3.0);
        // The gap between widgets.
        spacing.item_spacing = egui::vec2(6.0, 4.0);
        // The height of buttons, fields and other controls, so a row of
        // them lines up.
        spacing.interact_size.y = 22.0;
        let sizes = [(TextStyle::Small, 11.0), (TextStyle::Heading, 16.0)];
        // Only the size: the face is the fonts' to set.
        for (text_style, size) in sizes {
            if let Some(font) = style.text_styles.get_mut(&text_style) {
                font.size = size;
            }
        }
        style.animation_time = ANIMATION_TIME;
    });
}

#[cfg(test)]
mod tests {
    use super::install;
    use crate::theme::Theme;

    /// The style is in both of egui's styles, and a theme installed after
    /// it keeps it.
    #[test]
    fn the_style_survives_a_change_of_theme() {
        let ctx = egui::Context::default();
        install(&ctx);
        Theme::Paper.apply(&ctx);
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let style = ctx.style_of(theme);
            let spacing = &style.spacing;
            assert_eq!(spacing.button_padding, egui::vec2(8.0, 3.0), "{theme:?}");
            assert_eq!(spacing.item_spacing, egui::vec2(6.0, 4.0), "{theme:?}");
            assert_eq!(spacing.interact_size.y, 22.0, "{theme:?}");
            let small = egui::TextStyle::Small.resolve(&style);
            assert_eq!(small.size, 11.0, "{theme:?}");
            let heading = egui::TextStyle::Heading.resolve(&style);
            assert_eq!(heading.size, 16.0, "{theme:?}");
            assert_eq!(style.animation_time, 0.4, "{theme:?}");
        }
    }
}
