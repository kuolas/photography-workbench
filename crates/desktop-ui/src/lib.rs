// SPDX-License-Identifier: GPL-3.0-or-later

//! desktop-ui: the custom widgets and the shared parts of the window,
//! built on egui. It knows nothing of photos, so the app's modes, and any
//! other app, can use it as they are. See `docs/ARCHITECTURE.md`.

pub mod about;
pub mod action_bar;
pub mod chrome;
pub mod dialog;
pub mod dock;
pub mod drop_down;
pub mod fonts;
pub mod icons;
pub mod keys;
pub mod slider;
pub mod style;
pub mod theme;
pub mod toolbar;
