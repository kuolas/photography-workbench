// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's modes: the jobs a photographer does with a folder of photos,
//! each with its own layout of the window, chosen in the title bar's mode
//! switcher (ADR010).

/// The parts of the window a mode has around its centre, besides the
/// title bar, which every mode has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// The action bar along the right edge, which picks what the right
    /// dock shows.
    pub tool_rail: bool,
    /// The dock along the left side.
    pub left_dock: bool,
    /// The dock along the right side.
    pub right_dock: bool,
}

/// One of the app's modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Browse a folder's photos and cull them.
    #[default]
    Library,
    /// Bring photos in from a memory card and rename them by their metadata.
    Organizer,
    /// Develop a RAW file into a picture.
    Develop,
}

impl Mode {
    /// Every mode, in the mode switcher's order.
    pub const ALL: [Self; 3] = [Self::Library, Self::Organizer, Self::Develop];

    /// The name the mode switcher shows, which is also the saved value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Organizer => "Organizer",
            Self::Develop => "Develop",
        }
    }

    /// The Lucide icon shown before the mode's name, and the text shown in
    /// its place if the name does not resolve.
    pub fn icon(self) -> (&'static str, &'static str) {
        match self {
            Self::Library => ("images", "▦"),
            Self::Organizer => ("import", "⇩"),
            Self::Develop => ("sliders-horizontal", "☰"),
        }
    }

    /// Whether the mode is built yet. One that is not is still shown in
    /// the mode switcher, disabled, so the switcher keeps its shape as the
    /// modes arrive.
    pub fn is_available(self) -> bool {
        match self {
            Self::Library => true,
            Self::Organizer | Self::Develop => false,
        }
    }

    /// The parts of the window the mode has. A mode not built yet has
    /// none: it cannot be entered.
    pub fn layout(self) -> Layout {
        match self {
            Self::Library => Layout {
                tool_rail: true,
                left_dock: true,
                right_dock: true,
            },
            Self::Organizer | Self::Develop => Layout::default(),
        }
    }

    /// The mode saved as `name`, its [`Self::name`]. `None` when `name` is
    /// no mode, or a mode that is not available, so the app starts in the
    /// default one.
    pub fn from_saved(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == name && mode.is_available())
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;

    #[test]
    fn a_saved_mode_restores_only_when_it_is_available() {
        assert_eq!(Mode::from_saved("Library"), Some(Mode::Library));
        assert_eq!(Mode::from_saved("Organizer"), None);
        assert_eq!(Mode::from_saved("Develop"), None);
        assert_eq!(Mode::from_saved("library"), None);
        assert_eq!(Mode::from_saved(""), None);
    }

    #[test]
    fn every_mode_icon_resolves() {
        for mode in Mode::ALL {
            let (name, _) = mode.icon();
            assert!(desktop_ui::icons::resolves(name), "{mode:?}: {name}");
        }
    }
}
