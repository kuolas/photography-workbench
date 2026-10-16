// SPDX-License-Identifier: GPL-3.0-or-later

//! photography-workbench: a desktop app for culling photos.
//!
//! The binary in `main.rs` only opens the window; everything else lives in
//! this library, so tests can drive it without a display. See
//! `docs/ARCHITECTURE.md`.

mod app;
mod browse;
mod dialogs;
mod disk;
mod filmstrip;
mod folder_tree;
mod follow;
mod graphics;
mod grid;
mod keymap;
mod layout;
mod menu;
mod mode;
mod textures;
mod view;

pub use app::PhotographyWorkbenchApp;
pub use graphics::wgpu_setup;
