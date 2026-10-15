// SPDX-License-Identifier: GPL-3.0-or-later

//! The icons: Lucide's (<https://lucide.dev/icons>), drawn from the icon
//! font that iconflow bundles, so an icon never depends on the fonts a
//! system happens to have.
//!
//! [`install`] gives egui the font, once, before the first frame. An icon
//! is asked for by its Lucide name, such as `"settings"`: as text for a
//! label or a button ([`lucide`]), or as a character and a font for a
//! widget that paints its own ([`glyph`]), or before a word, as one run
//! of text for a button ([`labeled`]).
//!
//! A name that doesn't resolve shows the caller's `fallback` instead, so a
//! misspelt name degrades rather than panics; [`resolves`] lets a test
//! check every name a crate uses.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, RichText, WidgetText};
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
/// so a font added later must be added after it; [`crate::fonts::install`]
/// installs the icon font along with the window's typeface.
pub fn install(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    add_to(&mut defs);
    ctx.set_fonts(defs);
}

/// Add the icon font to `defs`, backed up by every font already in it, so
/// a character the icon font lacks still has a glyph.
pub fn add_to(defs: &mut egui::FontDefinitions) {
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

/// The Lucide icon `name` followed by the word `text`, both at the size of
/// `ui`'s body text, as one run of text for a button; `fallback` in place
/// of the icon if the name doesn't resolve. The colour is left to the
/// widget, as with [`lucide`].
pub fn labeled(ui: &egui::Ui, name: &str, fallback: &str, text: &str) -> WidgetText {
    /// The gap between the icon and the word, in points.
    const GAP: f32 = 6.0;
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let format = |font_id| TextFormat {
        font_id,
        color: Color32::PLACEHOLDER,
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    match resolve(name) {
        Some((glyph, family)) => {
            job.append(&glyph.to_string(), 0.0, format(FontId::new(size, family)));
        }
        None => job.append(fallback, 0.0, format(FontId::proportional(size))),
    }
    job.append(text, GAP, format(FontId::proportional(size)));
    job.into()
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

    #[test]
    fn labeled_puts_the_icon_before_the_word_at_the_body_size() {
        egui::__run_test_ui(|ui| {
            let body = egui::TextStyle::Body.resolve(ui.style()).size;
            let text = labeled(ui, "settings", "?", "Settings");
            let (glyph, _) = glyph("settings", body, '?');
            assert_eq!(text.text(), format!("{glyph}Settings"));
            let WidgetText::LayoutJob(job) = text else {
                panic!("not a layout job");
            };
            let sizes: Vec<f32> = job.sections.iter().map(|s| s.format.font_id.size).collect();
            assert_eq!(sizes, [body, body]);
            assert_eq!(job.sections[1].leading_space, 6.0);

            assert_eq!(labeled(ui, "no-such-icon", "?", "X").text(), "?X");
        });
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
