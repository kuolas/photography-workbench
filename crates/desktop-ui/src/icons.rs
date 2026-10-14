// SPDX-License-Identifier: GPL-3.0-or-later

//! The icons: Lucide's (<https://lucide.dev/icons>), drawn from the icon
//! font that iconflow bundles, so an icon never depends on the fonts a
//! system happens to have.
//!
//! [`install`] gives egui the font, once, before the first frame. An icon
//! is asked for by its Lucide name, such as `"settings"`: as text for a
//! label or a button ([`lucide`]), or as a character and a font for a
//! widget that paints its own ([`glyph`]). egui lays out an icon and a
//! word side by side when a button is given both:
//! `egui::Button::new((icons::lucide("images", 14.0, "▦"), "Library"))`.
//!
//! A name that doesn't resolve shows the caller's `fallback` instead, so a
//! misspelt name degrades rather than panics; [`resolves`] lets a test
//! check every name a crate uses.

use std::sync::Arc;

use egui::{FontFamily, FontId, RichText};
use iconflow::{Pack, Size, Style, try_icon};

/// The Lucide icon `name` as a character, and the font family that holds it.
fn resolve(name: &str) -> Option<(char, FontFamily)> {
    let icon = try_icon(Pack::Lucide, name, Style::Regular, Size::Regular).ok()?;
    let glyph = char::from_u32(icon.codepoint)?;
    Some((glyph, FontFamily::Name(icon.family.into())))
}

/// Give egui the icon font, alongside its own fonts. Call it once, before
/// the first frame; egui takes it up at the start of the next pass. Until
/// then egui knows no font for the icons, and a pass that paints one from
/// [`lucide`] or [`glyph`] panics.
///
/// It sets egui's font definitions to egui's defaults plus the icon font,
/// so a font added later must be added after it.
pub fn install(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    // egui's own fonts back the icon family up, so a character the icon
    // font lacks still has a glyph.
    let fallbacks: Vec<String> = defs.font_data.keys().cloned().collect();
    for font in iconflow::fonts() {
        defs.font_data.insert(
            font.family.to_owned(),
            Arc::new(egui::FontData::from_static(font.bytes)),
        );
        let family = defs
            .families
            .entry(FontFamily::Name(font.family.into()))
            .or_default();
        family.push(font.family.to_owned());
        family.extend(fallbacks.iter().cloned());
    }
    ctx.set_fonts(defs);
}

/// The Lucide icon `name` at `size` points, as text for a label or a
/// button, or `fallback` at that size if the name doesn't resolve. The
/// colour is left to the widget, so the icon follows its hover and
/// disabled states like any text.
pub fn lucide(name: &str, size: f32, fallback: &str) -> RichText {
    match resolve(name) {
        Some((glyph, family)) => RichText::new(glyph).font(FontId::new(size, family)),
        None => RichText::new(fallback).size(size),
    }
}

/// The Lucide icon `name` as a character and the font to paint it in at
/// `size` points, for a widget that paints its own icon; `fallback` in the
/// proportional font if the name doesn't resolve.
pub fn glyph(name: &str, size: f32, fallback: char) -> (char, FontId) {
    match resolve(name) {
        Some((glyph, family)) => (glyph, FontId::new(size, family)),
        None => (fallback, FontId::proportional(size)),
    }
}

/// Whether `name` is a Lucide icon.
pub fn resolves(name: &str) -> bool {
    resolve(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_resolves_to_a_glyph_of_the_icon_font() {
        let (glyph, font) = glyph("settings", 16.0, '?');
        assert_ne!(glyph, '?');
        assert_eq!(font.size, 16.0);
        assert!(
            matches!(&font.family, FontFamily::Name(name) if name.contains("Lucide")),
            "{:?} is not the icon font",
            font.family
        );
        assert_eq!(lucide("settings", 16.0, "?").text(), glyph.to_string());
        assert!(resolves("settings"));
    }

    #[test]
    fn an_unknown_name_shows_the_fallback() {
        let name = "no-such-icon";
        assert_eq!(glyph(name, 16.0, '?'), ('?', FontId::proportional(16.0)));
        assert_eq!(lucide(name, 16.0, "?").text(), "?");
        assert!(!resolves(name));
    }

    /// After [`install`] and one pass, egui has a glyph for an icon in the
    /// font [`glyph`] names, so the icon paints as itself, not as a box.
    #[test]
    fn install_gives_egui_the_icon_glyphs() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        // No renderer here to upload the font atlas to.
        output.textures_delta.clear();
        let (glyph, font) = glyph("settings", 16.0, '?');
        assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)));
    }
}
