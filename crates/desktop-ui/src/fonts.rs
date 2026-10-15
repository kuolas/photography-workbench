// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's typeface: Inter (<https://rsms.me/inter/>), drawn for
//! screens, in its regular weight for text and its semibold for headings.
//! The font files and their licence, the SIL Open Font License, are in
//! `assets/fonts/`.
//!
//! [`install`] gives egui the typeface and the icon font together, in
//! place of [`crate::icons::install`]. egui's own fonts back Inter up, so
//! a character Inter lacks still has a glyph.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, TextStyle};

use crate::icons;

/// The name egui knows Inter's regular weight by.
const REGULAR: &str = "Inter";

/// The name of the font family for semibold text: Inter's semibold weight,
/// then egui's own fonts.
pub const SEMIBOLD: &str = "Inter SemiBold";

/// Give egui the typeface and the icon font, and set headings in the
/// semibold weight. Call it once, before the first frame; egui takes the
/// fonts up at the start of the next pass.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
    ctx.all_styles_mut(|style| {
        if let Some(heading) = style.text_styles.get_mut(&TextStyle::Heading) {
            heading.family = FontFamily::Name(SEMIBOLD.into());
        }
    });
}

/// egui's fonts with the icon font, Inter first for proportional text, and
/// the [`SEMIBOLD`] family.
fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    // Before Inter, so the icon font is backed up by egui's fonts alone.
    icons::add_to(&mut defs);
    let egui_fonts = defs.families[&FontFamily::Proportional].clone();
    let faces = [
        (
            REGULAR,
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            SEMIBOLD,
            &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
    ];
    for (name, bytes) in faces {
        let data = Arc::new(FontData::from_static(bytes));
        defs.font_data.insert(name.to_owned(), data);
    }
    let proportional = defs.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, REGULAR.to_owned());
    let semibold = std::iter::once(SEMIBOLD.to_owned())
        .chain(egui_fonts)
        .collect();
    defs.families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    defs
}

#[cfg(test)]
mod tests {
    use egui::{FontFamily, FontId, TextStyle};

    use super::{REGULAR, SEMIBOLD, definitions, install};
    use crate::{icons, style};

    #[test]
    fn inter_comes_first_and_egui_s_fonts_back_it_up() {
        let defs = definitions();
        let egui_fonts = &egui::FontDefinitions::default().families[&FontFamily::Proportional];
        let proportional = &defs.families[&FontFamily::Proportional];
        assert_eq!(proportional[0], REGULAR);
        assert_eq!(&proportional[1..], &egui_fonts[..]);
        let semibold = &defs.families[&FontFamily::Name(SEMIBOLD.into())];
        assert_eq!(semibold[0], SEMIBOLD);
        assert_eq!(&semibold[1..], &egui_fonts[..]);
    }

    /// The icon font is installed along with Inter, and backed up by
    /// egui's fonts, not by Inter.
    #[test]
    fn the_icon_font_comes_along() {
        let defs = definitions();
        let (_, FontId { family, .. }) = icons::glyph("settings", 16.0, '?');
        let icon_family = &defs.families[&family];
        assert!(
            !icon_family.iter().any(|font| font.starts_with(REGULAR)),
            "{icon_family:?}"
        );
    }

    /// Headings are semibold at the style's size, whichever of the two is
    /// installed first, and a pass can lay one out.
    #[test]
    fn headings_are_semibold_whatever_the_order() {
        for fonts_first in [true, false] {
            let ctx = egui::Context::default();
            if fonts_first {
                install(&ctx);
                style::install(&ctx);
            } else {
                style::install(&ctx);
                install(&ctx);
            }
            let heading = TextStyle::Heading.resolve(&ctx.global_style());
            assert_eq!(
                heading.family,
                FontFamily::Name(SEMIBOLD.into()),
                "{fonts_first}"
            );
            assert_eq!(heading.size, 16.0, "{fonts_first}");
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.heading("Folders");
            });
            // No renderer here to upload the font atlas to.
            output.textures_delta.clear();
        }
    }
}
