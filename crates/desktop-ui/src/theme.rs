// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's colour themes.
//!
//! What the user picks, and the app saves between runs, is an
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
//! Dark keeps the colours of egui's stock dark visuals as they are, and its
//! palette repeats them. Whatever the app paints itself, such as the title
//! bar, the docks and the thumbnails, reads its colour from a [`Palette`]
//! slot rather than hard-coding it, so every colour the app paints changes
//! with the theme. Adding a theme is one [`Palette`] constructor, a [`Theme`]
//! variant with its match arms, and an entry in [`Theme::ALL`].
//!
//! egui keeps the shape of its widgets with their colours, in the visuals,
//! so every theme also gives buttons and fields the same rounded corners,
//! and popups and dialogs the same shadows. The spacing is not part of a
//! theme: see [`crate::style`].

use egui::{Color32, CornerRadius, Shadow};

use crate::drop_down;

/// The corner radius of buttons, fields and the other controls, in every
/// theme, in points; egui's own is 2 to 3.
const CONTROL_CORNER_RADIUS: u8 = 4;

/// The shadow under a popup, such as a menu or a drop-down list, and
/// under a dialog: straight down and soft, where egui's falls to the
/// lower right. Its opacity is [`SHADOW_ALPHA`]'s.
const POPUP_SHADOW: Shadow = shadow(4, 16);

/// The shadow under a window, as [`POPUP_SHADOW`] but deeper.
const WINDOW_SHADOW: Shadow = shadow(8, 30);

/// The opacity of [`POPUP_SHADOW`] and [`WINDOW_SHADOW`] on a dark theme
/// and on a light one, out of 255: a dark theme needs a darker shadow to
/// show at all.
const SHADOW_ALPHA: [(u8, u8); 2] = [(140, 160), (50, 60)];

/// A black shadow `drop` points straight down, blurred over `blur` points;
/// its colour is set per theme.
const fn shadow(drop: i8, blur: u8) -> Shadow {
    Shadow {
        offset: [0, drop],
        blur,
        spread: 0,
        color: Color32::BLACK,
    }
}

/// `0xRRGGBB` → opaque [`Color32`]. Keeps the palette tables readable.
const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A concrete theme: colours that [`Self::apply`] installs on egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// A warm "paper" theme for well-lit rooms.
    Paper,
    /// Mid-grey chrome with egui's dark colours for windows, fields and
    /// selection.
    Graphite,
    /// A near-black theme with vivid yellow links, for judging photos
    /// against neutral dark surroundings.
    Nexus,
    /// egui's stock dark colours (flat charcoal): those of
    /// `egui::Visuals::dark()`, unchanged.
    Dark,
}

impl Theme {
    /// Every theme, in the order a theme picker lists them after "System":
    /// lightest to darkest.
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

    /// This theme's colours.
    pub fn palette(self) -> Palette {
        match self {
            Self::Paper => Palette::paper(),
            Self::Graphite => Palette::graphite(),
            Self::Nexus => Palette::nexus(),
            Self::Dark => Palette::dark(),
        }
    }

    /// Whether this theme is built on egui's dark visuals rather than its
    /// light ones.
    pub fn is_dark(self) -> bool {
        self.palette().dark_mode
    }

    /// The egui `Visuals` for this theme: its colours, with the rounded
    /// corners and the shadows every theme shares. Labels, separators and
    /// frames, which are not controls, keep egui's corners.
    fn visuals(self) -> egui::Visuals {
        let mut v = self.colours();
        let w = &mut v.widgets;
        for state in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            state.corner_radius = CornerRadius::same(CONTROL_CORNER_RADIUS);
        }
        let (popup, window) = SHADOW_ALPHA[usize::from(!v.dark_mode)];
        v.popup_shadow = Shadow {
            color: Color32::from_black_alpha(popup),
            ..POPUP_SHADOW
        };
        v.window_shadow = Shadow {
            color: Color32::from_black_alpha(window),
            ..WINDOW_SHADOW
        };
        v
    }

    /// The egui `Visuals` for this theme's colours: the palette mapped onto
    /// the fills, text and selection that most define the look, over the
    /// base dark or light visuals. Dark's are egui's own, unchanged.
    fn colours(self) -> egui::Visuals {
        if self == Self::Dark {
            return egui::Visuals::dark();
        }
        let p = self.palette();
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

/// The name of the user's `choice`: the theme's [`Theme::name`], or
/// "System" for `None`.
pub fn choice_name(choice: Option<Theme>) -> &'static str {
    choice.map_or("System", Theme::name)
}

/// A drop-down list of the theme choices, System first and then every
/// theme, lightest to darkest. Picking one sets `choice`.
pub fn picker(ui: &mut egui::Ui, id_salt: &str, choice: &mut Option<Theme>) -> egui::Response {
    let choices = std::iter::once(None).chain(Theme::ALL.map(Some));
    drop_down::show(ui, id_salt, choice, choices, choice_name)
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
    /// The title bar.
    pub title_bar: Color32,
    /// A strip of chrome set into a panel: one shade below
    /// [`Self::panel`] in every theme, so the two read as one family.
    pub strip: Color32,
    /// Modals, windows and popups such as menus and dropdowns (egui
    /// `window_fill`).
    pub window: Color32,
    /// Text-input background (egui `extreme_bg_color`).
    pub field: Color32,
    /// A thumbnail's tile, under and around the picture.
    pub chip: Color32,
    /// Behind the photo in the viewer.
    pub viewer_backdrop: Color32,
    /// Separator, frame-border and indent-line stroke (egui
    /// `widgets.noninteractive.bg_stroke`).
    pub separator: Color32,

    // --- foreground ---
    /// Resting text and icons: labels, and widgets that are neither hovered,
    /// pressed nor open (egui `widgets.noninteractive.fg_stroke` and
    /// `widgets.inactive.fg_stroke`). Hovered, pressed and open widgets keep
    /// the base visuals' colours.
    pub text: Color32,
    /// An icon drawn by the app that is on, or under the pointer.
    pub icon_active: Color32,
    /// An icon drawn by the app that is off.
    pub icon_inactive: Color32,
    /// The window controls' glyphs: close, maximize and minimize.
    pub window_ctrl: Color32,
    /// A see-through wash over a control under the pointer: white on a dark
    /// theme, black on a light one.
    pub hover_overlay: Color32,
    /// Behind the icon of a bar's button that is on: opaque, twice as far
    /// from [`Self::panel`] as [`Self::hover_overlay`] takes it, so a
    /// button that is on stands out from one under the pointer.
    pub button_on: Color32,

    // --- links, selection and accents ---
    /// Hyperlink colour, chosen to stay legible on [`Self::panel`]: at least
    /// 3:1.
    pub link: Color32,
    /// Text-selection fill (egui `selection.bg_fill`).
    pub selection: Color32,
    /// The outline of the current photo, in the grid and the filmstrip.
    pub accent: Color32,
    /// A control that destroys or closes, under the pointer: the close
    /// button.
    pub danger: Color32,
    /// Text and icons on [`Self::danger`]: the close button's cross while
    /// the pointer is over it.
    pub on_danger: Color32,
}

impl Palette {
    /// A warm "paper" light theme — soft off-whites rather than flat pure white
    /// (egui has no gradients or shadows, so pure #FFF reads harsh); dark warm
    /// text. Background tiers step slightly so surfaces separate without
    /// borders.
    fn paper() -> Self {
        Self {
            dark_mode: false,
            panel: hex(0xF3F1E9),           // main content paper
            title_bar: hex(0xE7E3D8),       // a step below the paper
            strip: hex(0xE2DDD0),           // a step below the title bar
            window: hex(0xFBF9F3),          // dialogs / menus, brighter paper
            field: hex(0xFFFDF7),           // inputs, brightest (soft, not pure white)
            chip: hex(0xE4DFD2),            // tiles sit just below the paper
            viewer_backdrop: hex(0xCFC9BA), // medium warm grey behind the photo
            separator: hex(0xC9C3B4),       // warm light rule (≈ egui light default)
            text: hex(0x2A2822),            // warm near-black
            icon_active: hex(0x2A2822),     // as the text
            icon_inactive: hex(0x9A9488),   // warm grey
            window_ctrl: hex(0x4A463C),
            // Light chrome darkens, rather than lightens, under the pointer.
            hover_overlay: Color32::from_black_alpha(20),
            button_on: hex(0xCECDC6),
            link: hex(0x0A6CC4), // slightly deep blue for contrast on paper
            selection: hex(0xBFDBF5),
            accent: hex(0x0A6CC4), // the link blue
            danger: hex(0xB3261E),
            on_danger: Color32::WHITE,
        }
    }

    /// Mid-grey chrome. The window, field, selection and separator colours
    /// equal egui's dark defaults.
    fn graphite() -> Self {
        Self {
            dark_mode: true,
            panel: Color32::DARK_GRAY,
            title_bar: Color32::from_gray(48),
            strip: Color32::from_gray(70),
            window: Color32::from_gray(27),
            field: Color32::from_gray(10),
            chip: Color32::from_gray(55),
            viewer_backdrop: Color32::from_gray(28),
            separator: Color32::from_gray(60), // egui dark default
            text: Color32::WHITE,
            icon_active: Color32::from_gray(235),
            icon_inactive: Color32::from_gray(150),
            window_ctrl: Color32::from_gray(210),
            hover_overlay: Color32::from_white_alpha(28),
            button_on: Color32::from_gray(129),
            // Its blue accent (#0078D7) lightened to stay legible on the
            // mid-grey panel.
            link: hex(0x7BBAEA),
            selection: Color32::from_rgb(0, 92, 128),
            accent: hex(0x0078D7),
            danger: Color32::from_rgb(196, 43, 28),
            on_danger: Color32::WHITE,
        }
    }

    /// Near-black wells, mid-grey chrome and raised #404040 controls (egui's
    /// own dark widget fills already match these, so they are left as they
    /// are). The yellow is kept for links and the current photo's outline
    /// rather than selection fills, because selected text would be
    /// unreadable on yellow.
    fn nexus() -> Self {
        Self {
            dark_mode: true,
            panel: hex(0x333333),           // chrome bars and docks
            title_bar: hex(0x262626),       // a step below the chrome
            strip: hex(0x292929),           // a shade below the chrome
            window: hex(0x242424),          // menus and dialogs, a step below the chrome
            field: hex(0x1A1A1A),           // text inputs sunk to the content-well tone
            chip: hex(0x404040),            // the raised control grey
            viewer_backdrop: hex(0x1A1A1A), // the content-well tone
            separator: hex(0x1A1A1A),       // dock edges, the content-well tone
            text: Color32::WHITE,
            icon_active: hex(0xEBEBEB),
            icon_inactive: hex(0x9A9A9A),
            window_ctrl: hex(0xD2D2D2),
            hover_overlay: Color32::from_white_alpha(24),
            button_on: hex(0x585858),
            link: hex(0xFFF800),      // vivid yellow
            selection: hex(0x404040), // selected rows: a raised grey, not the yellow — white text stays legible on it
            accent: hex(0xFFF800),    // the link yellow
            danger: hex(0xC42B1C),
            on_danger: Color32::WHITE,
        }
    }

    /// egui's stock dark visuals: the colours [`Theme::visuals`] would map
    /// equal the ones `egui::Visuals::dark()` already has, and `text` is its
    /// label colour. The rest step around its flat charcoal so the window's
    /// parts still read apart.
    fn dark() -> Self {
        Self {
            dark_mode: true,
            panel: Color32::from_gray(27),
            title_bar: Color32::from_gray(30),
            strip: Color32::from_gray(22),
            window: Color32::from_gray(27),
            field: Color32::from_gray(10),
            chip: Color32::from_gray(40),
            viewer_backdrop: Color32::from_gray(10),
            separator: Color32::from_gray(60),
            text: Color32::from_gray(140),
            icon_active: Color32::from_gray(220),
            icon_inactive: Color32::from_gray(120),
            window_ctrl: Color32::from_gray(180),
            hover_overlay: Color32::from_white_alpha(24),
            button_on: Color32::from_gray(68),
            link: Color32::from_rgb(90, 170, 255),
            selection: Color32::from_rgb(0, 92, 128),
            accent: Color32::from_rgb(90, 170, 255), // the link blue
            danger: Color32::from_rgb(220, 90, 90),
            on_danger: Color32::WHITE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip set into a panel is the panel's own colour a shade
    /// darker, never a separate surface: the mean of the three channels of
    /// `strip` is between 55 % and 96 % of `panel`'s, in every theme.
    #[test]
    fn the_strip_is_a_shade_below_the_panel_in_every_theme() {
        fn mean(c: Color32) -> f32 {
            (f32::from(c.r()) + f32::from(c.g()) + f32::from(c.b())) / 3.0
        }
        for theme in Theme::ALL {
            let p = theme.palette();
            let ratio = mean(p.strip) / mean(p.panel);
            assert!(
                (0.55..=0.96).contains(&ratio),
                "{}: strip / panel = {ratio:.2}, outside 0.55..=0.96",
                theme.name()
            );
        }
    }

    /// A bar's button that is on stands out from one under the pointer: its
    /// fill is opaque, and at least half again as far from the panel's
    /// colour as one hover wash takes it, in every theme.
    #[test]
    fn a_button_that_is_on_stands_out_from_one_under_the_pointer() {
        fn mean(c: Color32) -> f32 {
            (f32::from(c.r()) + f32::from(c.g()) + f32::from(c.b())) / 3.0
        }
        for theme in Theme::ALL {
            let p = theme.palette();
            assert!(p.button_on.is_opaque(), "{}", theme.name());
            let hovered = (mean(p.panel.blend(p.hover_overlay)) - mean(p.panel)).abs();
            let on = (mean(p.button_on) - mean(p.panel)).abs();
            assert!(
                on >= 1.5 * hovered,
                "{}: on {on:.1} from the panel, under the pointer {hovered:.1}",
                theme.name()
            );
        }
    }

    /// Dark keeps egui's stock colours unmapped, so its palette must hold
    /// them, or what the app paints itself would not match what
    /// egui paints.
    #[test]
    fn the_dark_palette_holds_the_stock_dark_colours() {
        let p = Theme::Dark.palette();
        let v = egui::Visuals::dark();
        assert!(p.dark_mode);
        assert_eq!(p.panel, v.panel_fill);
        assert_eq!(p.window, v.window_fill);
        assert_eq!(p.field, v.extreme_bg_color);
        assert_eq!(p.separator, v.widgets.noninteractive.bg_stroke.color);
        assert_eq!(p.text, v.widgets.noninteractive.fg_stroke.color);
        assert_eq!(p.link, v.hyperlink_color);
        assert_eq!(p.selection, v.selection.bg_fill);
    }

    #[test]
    fn the_picker_offers_every_choice_and_sets_the_one_picked() {
        use egui::accesskit::Role;
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable as _;

        let mut harness = Harness::builder().build_ui_state(
            |ui, choice: &mut Option<Theme>| {
                picker(ui, "theme", choice);
            },
            None,
        );
        assert!(harness.query_by_label("Graphite").is_none());
        harness.get_by_role(Role::ComboBox).click();
        harness.run();
        harness.get_by_label("System");
        for theme in Theme::ALL {
            harness.get_by_label(theme.name());
        }
        harness.get_by_label("Graphite").click();
        harness.run();
        assert_eq!(*harness.state(), Some(Theme::Graphite));
    }

    /// Every theme rounds its controls alike, in every state they can be
    /// in, and leaves egui's corners on what is not a control.
    #[test]
    fn every_theme_rounds_its_controls() {
        let round = CornerRadius::same(CONTROL_CORNER_RADIUS);
        let stock = egui::Visuals::dark().widgets.noninteractive.corner_radius;
        for theme in Theme::ALL {
            let w = theme.visuals().widgets;
            for (state, visuals) in [
                ("inactive", w.inactive),
                ("hovered", w.hovered),
                ("active", w.active),
                ("open", w.open),
            ] {
                assert_eq!(visuals.corner_radius, round, "{}: {state}", theme.name());
            }
            assert_eq!(w.noninteractive.corner_radius, stock, "{}", theme.name());
        }
    }

    /// Popups and windows cast their shadow straight down, darker on a
    /// dark theme than on a light one.
    #[test]
    fn every_theme_drops_its_shadows_straight_down() {
        for theme in Theme::ALL {
            let v = theme.visuals();
            for shadow in [v.popup_shadow, v.window_shadow] {
                assert_eq!(shadow.offset[0], 0, "{}", theme.name());
            }
            assert_eq!(v.popup_shadow.offset[1], 4, "{}", theme.name());
            assert_eq!(v.window_shadow.blur, 30, "{}", theme.name());
            let alpha = v.popup_shadow.color.a();
            assert_eq!(
                alpha,
                if theme.is_dark() { 140 } else { 50 },
                "{}",
                theme.name()
            );
        }
    }
}
