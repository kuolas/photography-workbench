// SPDX-License-Identifier: GPL-3.0-or-later

//! The style every part of the window shares apart from its colours, which
//! come with the theme ([`crate::theme`]).
//!
//! The spacing: the room inside buttons, the gaps between widgets, the
//! height of a control, and the margins of menus and windows. egui's own
//! suits a compact tool: a button's text sits a point from its edge, and
//! controls are 18 points high. Parts that are drawn at a size of their
//! own, such as the window controls and the title bar's icon buttons, keep
//! it.
//!
//! The scroll bars float over what they scroll, so one appearing never
//! moves the content, and stay visible at rest whenever there is something
//! to scroll, so the reader sees there is more without moving the pointer.
//!
//! The pace: how long egui's animations take, from a section opening to a
//! tooltip, menu or dialog fading in and a scroll bar appearing. egui's own
//! is brisk; the window's is unhurried.

/// How long every animation takes, in seconds; egui's own is 0.2.
const ANIMATION_TIME: f32 = 0.4;

/// Set the style in every one of egui's styles, light and dark. A change
/// of theme replaces only the visuals, so the style stays. Call it once,
/// before the first frame.
pub fn install(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let spacing = &mut style.spacing;
        // Room between a button's text and its edge.
        spacing.button_padding = egui::vec2(10.0, 6.0);
        // The gap between widgets: egui's across, more down.
        spacing.item_spacing = egui::vec2(8.0, 6.0);
        // The height of buttons, fields and other controls, so a row of
        // them lines up.
        spacing.interact_size.y = 26.0;
        spacing.menu_margin = egui::Margin::same(8);
        spacing.window_margin = egui::Margin::same(12);
        // egui's floating bars vanish at rest; these stay faintly visible.
        spacing.scroll = egui::style::ScrollStyle::floating();
        spacing.scroll.dormant_background_opacity = 0.3;
        spacing.scroll.dormant_handle_opacity = 0.7;
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
            let spacing = ctx.style_of(theme).spacing.clone();
            assert_eq!(spacing.button_padding, egui::vec2(10.0, 6.0), "{theme:?}");
            assert_eq!(spacing.item_spacing, egui::vec2(8.0, 6.0), "{theme:?}");
            assert_eq!(spacing.interact_size.y, 26.0, "{theme:?}");
            assert_eq!(spacing.menu_margin, egui::Margin::same(8), "{theme:?}");
            assert_eq!(spacing.window_margin, egui::Margin::same(12), "{theme:?}");
            assert!(spacing.scroll.floating, "{theme:?}");
            assert_eq!(spacing.scroll.dormant_background_opacity, 0.3, "{theme:?}");
            assert_eq!(spacing.scroll.dormant_handle_opacity, 0.7, "{theme:?}");
            assert_eq!(ctx.style_of(theme).animation_time, 0.4, "{theme:?}");
        }
    }
}
