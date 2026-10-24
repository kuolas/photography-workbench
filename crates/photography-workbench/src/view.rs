// SPDX-License-Identifier: GPL-3.0-or-later

//! The ways Library shows a folder's photos in the window's centre: many
//! at a time in a grid of thumbnails, or one at a time in the viewer.
//! Which one is chosen on the toolbar under the centre.

use crate::mode::Mode;

/// How Library shows a folder's photos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Many photos at a time, as thumbnails in a grid.
    #[default]
    Grid,
    /// One photo at a time, as large as the centre allows.
    Viewer,
}

/// Whether the centre shows Library's viewer in `mode` and `view`, which
/// alone has the zoom controls and shows the photo's information over its
/// photo.
pub fn shows_viewer(mode: Mode, view: View) -> bool {
    mode == Mode::Library && view == View::Viewer
}

impl View {
    /// Every view, in the toolbar's order.
    pub const ALL: [Self; 2] = [Self::Grid, Self::Viewer];

    /// The view's name: its button's tooltip, and the saved value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Grid => "Thumbnail grid",
            Self::Viewer => "Image viewer",
        }
    }

    /// The Lucide icon on the view's button, and the text shown in its
    /// place if the name does not resolve.
    pub fn icon(self) -> (&'static str, char) {
        match self {
            Self::Grid => ("layout-grid", '▦'),
            Self::Viewer => ("image", '🖼'),
        }
    }

    /// The view saved as `name`, its [`Self::name`]; `None` when `name` is
    /// no view.
    pub fn from_saved(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::View;

    #[test]
    fn a_saved_view_restores_by_its_name() {
        for view in View::ALL {
            assert_eq!(View::from_saved(view.name()), Some(view));
        }
        assert_eq!(View::from_saved("grid"), None);
        assert_eq!(View::from_saved(""), None);
    }

    #[test]
    fn every_view_icon_resolves() {
        for view in View::ALL {
            let (name, _) = view.icon();
            assert!(desktop_ui::icons::resolves(name), "{view:?}: {name}");
        }
    }
}
