// SPDX-License-Identifier: GPL-3.0-or-later

//! desktop-ui: the custom widgets and the shared parts of the window,
//! built on egui. It knows nothing of photos, so the app's modes, and any
//! other app, can use it as they are. See `docs/ARCHITECTURE.md`.

pub mod chrome;
pub mod icons;
pub mod theme;
