// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's colour themes.
//!
//! What the user picks in View ▸ Theme, and the app saves between runs, is an
//! `Option<Theme>`: `None` follows the OS light/dark preference. [`resolve`]
//! turns it into the [`Theme`] to install. Only a [`Theme`] can be installed
//! ([`apply`](Theme::apply)), so "System", which names no colours, can't be
//! painted by mistake.
//!
//! None of this is egui's own [`egui::Theme`], which only says dark or light:
//! which of egui's two style slots is active. This module always writes
//! egui's type out in full, so the two never blur.
//!
//! [`Theme::apply`] maps a theme's [`Palette`] onto egui's [`egui::Visuals`];
//! Dark has no palette and installs egui's stock dark visuals as they are.
//! Custom painters, when they arrive, read their colours from new [`Palette`]
//! slots rather than hard-coding them, so every colour the app paints changes
//! with the theme. Adding a theme is one [`Palette`] constructor, a [`Theme`]
//! variant with its match arms, and an entry in [`Theme::ALL`].
//!
//! Themes set colours only; widget shapes, spacing and animation are egui's
//! defaults.

use egui::Color32;

/// `0xRRGGBB` → opaque [`Color32`]. Keeps the palette tables readable.
const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A concrete theme: colours that [`Self::apply`] installs on egui.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// A warm "paper" theme for well-lit rooms.
    Paper,
    /// Mid-grey chrome with egui's dark colours for windows, fields and
    /// selection.
    Graphite,
    /// A near-black theme with vivid yellow links, for judging photos
    /// against neutral dark surroundings.
    Nexus,
    /// egui's stock dark visuals (flat charcoal): installing it installs
    /// `egui::Visuals::dark()` unchanged.
    Dark,
}

impl Theme {
    /// Every theme, in menu order: lightest to darkest. The View ▸ Theme menu
    /// lists "System", then these.
    pub const ALL: [Self; 4] = [Self::Paper, Self::Graphite, Self::Nexus, Self::Dark];

    /// Display name. It is also the value the app saves for the choice, so
    /// renaming a theme resets users who chose it to System.
    pub fn name(self) -> &'static str {
        match self {
            Self::Paper => "Paper",
            Self::Graphite => "Graphite",
            Self::Nexus => "Nexus",
            Self::Dark => "Dark",
        }
    }

    /// This theme's colours, `None` for Dark, which has none of its own: it
    /// is egui's stock dark visuals.
    fn palette(self) -> Option<Palette> {
        match self {
            Self::Paper => Some(Palette::paper()),
            Self::Graphite => Some(Palette::graphite()),
            Self::Nexus => Some(Palette::nexus()),
            Self::Dark => None,
        }
    }

    /// Whether this theme is built on egui's dark visuals rather than its
    /// light ones.
    pub fn is_dark(self) -> bool {
        self.palette().is_none_or(|p| p.dark_mode)
    }

    /// The egui `Visuals` for this theme: the palette mapped onto the fills,
    /// text and selection that most define the look, over the base dark or
    /// light visuals. Dark's are egui's own, unchanged.
    fn visuals(self) -> egui::Visuals {
        let Some(p) = self.palette() else {
            return egui::Visuals::dark();
        };
        let mut v = if p.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        // Resting text and icons only: labels, and widgets neither hovered,
        // pressed nor open. The other states keep the base visuals' text colours, which
        // are egui's feedback for them; `override_text_color` would paint
        // every state alike.
        v.widgets.noninteractive.fg_stroke.color = p.text;
        v.widgets.inactive.fg_stroke.color = p.text;
        v.panel_fill = p.panel;
        v.window_fill = p.window; // dialogs, menus, combo popups
        v.extreme_bg_color = p.field; // TextEdit / DragValue background
        v.selection.bg_fill = p.selection;
        v.hyperlink_color = p.link;
        // Separators, frame borders and indent lines.
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.separator);
        v
    }

    /// Build this theme's egui `Visuals` and install them on `ctx`.
    ///
    /// egui's theme preference is left alone: it belongs to the user's
    /// choice, not the theme (see [`egui_preference`]).
    pub fn apply(self, ctx: &egui::Context) {
        let v = self.visuals();

        // egui keeps *two* style slots (dark and light) and picks one per frame
        // from its own theme preference. Under `System` that preference
        // follows the OS, which the windowing system reports after the app is
        // constructed and again whenever it changes, so egui may switch slots
        // at any frame. Writing the visuals into *both* slots makes the slot
        // egui picks irrelevant: the theme stays installed whatever the OS
        // reports.
        ctx.set_visuals_of(egui::Theme::Dark, v.clone());
        ctx.set_visuals_of(egui::Theme::Light, v);
    }
}

/// The theme to install for the user's `choice`, given the OS preference.
///
/// `None`, "System", is Paper when the OS is light and Graphite when it is
/// dark or the platform doesn't report a preference.
pub fn resolve(choice: Option<Theme>, ctx: &egui::Context) -> Theme {
    choice.unwrap_or_else(|| match ctx.system_theme() {
        Some(egui::Theme::Light) => Theme::Paper,
        _ => Theme::Graphite, // OS dark, or preference unknown
    })
}

/// The egui theme preference (`egui::Options::theme_preference`) that goes
/// with the user's `choice`. egui sends it on to the window, so it sets the
/// window's own light or dark appearance, such as its title bar.
///
/// A theme pins the preference to its light or dark, so the window matches
/// the theme. `None`, "System", leaves it at `ThemePreference::System`: once a
/// window's appearance is set, the windowing system stops reporting the OS
/// appearance changing (on macOS and Windows), and System would no longer
/// follow the OS.
pub fn egui_preference(choice: Option<Theme>) -> egui::ThemePreference {
    match choice {
        None => egui::ThemePreference::System,
        Some(theme) if theme.is_dark() => egui::ThemePreference::Dark,
        Some(_) => egui::ThemePreference::Light,
    }
}

/// Semantic colour set for one theme. All app colours flow from here; no widget
/// should hard-code a `Color32` outside a `Palette` constructor.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Whether egui's base visuals are dark (drives `Visuals::dark/light`).
    pub dark_mode: bool,

    // --- backgrounds ---
    /// Panels: the menu bar and the main view (egui `panel_fill`).
    pub panel: Color32,
    /// Modals, windows and popups such as menus and dropdowns (egui
    /// `window_fill`).
    pub window: Color32,
    /// Text-input background (egui `extreme_bg_color`).
    pub field: Color32,
    /// Separator, frame-border and indent-line stroke (egui
    /// `widgets.noninteractive.bg_stroke`).
    pub separator: Color32,

    // --- foreground ---
    /// Resting text and icons: labels, and widgets that are neither hovered,
    /// pressed nor open (egui `widgets.noninteractive.fg_stroke` and
    /// `widgets.inactive.fg_stroke`). Hovered, pressed and open widgets keep
    /// the base visuals' colours.
    pub text: Color32,

    // --- links and selection ---
    /// Hyperlink colour, chosen to stay legible on [`Self::panel`]: at least
    /// 3:1.
    pub link: Color32,
    /// Text-selection fill (egui `selection.bg_fill`).
    pub selection: Color32,
}

impl Palette {
    /// A warm "paper" light theme — soft off-whites rather than flat pure white
    /// (egui has no gradients or shadows, so pure #FFF reads harsh); dark warm
    /// text. Background tiers step slightly so surfaces separate without
    /// borders.
    fn paper() -> Self {
        Self {
            dark_mode: false,
            panel: hex(0xF3F1E9),     // main content paper
            window: hex(0xFBF9F3),    // dialogs / menus, brighter paper
            field: hex(0xFFFDF7),     // inputs, brightest (soft, not pure white)
            separator: hex(0xC9C3B4), // warm light rule (≈ egui light default)
            text: hex(0x2A2822),      // warm near-black
            link: hex(0x0A6CC4),      // slightly deep blue for contrast on paper
            selection: hex(0xBFDBF5),
        }
    }

    /// Mid-grey chrome. The window, field, selection and separator colours
    /// equal egui's dark defaults.
    fn graphite() -> Self {
        Self {
            dark_mode: true,
            panel: Color32::DARK_GRAY,
            window: Color32::from_gray(27),
            field: Color32::from_gray(10),
            separator: Color32::from_gray(60), // egui dark default
            text: Color32::WHITE,
            // Its blue accent (#0078D7) lightened to stay legible on the
            // mid-grey panel.
            link: hex(0x7BBAEA),
            selection: Color32::from_rgb(0, 92, 128),
        }
    }

    /// Near-black wells, mid-grey chrome and raised #404040 controls (egui's
    /// own dark widget fills already match these, so they are left as they
    /// are). The yellow is kept for links rather than selection
    /// fills, because selected text would be unreadable on yellow.
    fn nexus() -> Self {
        Self {
            dark_mode: true,
            panel: hex(0x333333),     // chrome bars and docks
            window: hex(0x242424),    // menus and dialogs, a step below the chrome
            field: hex(0x1A1A1A),     // text inputs sunk to the content-well tone
            separator: hex(0x1A1A1A), // dock edges, the content-well tone
            text: Color32::WHITE,
            link: hex(0xFFF800),      // vivid yellow
            selection: hex(0x404040), // selected rows: a raised grey, not the yellow — white text stays legible on it
        }
    }
}
