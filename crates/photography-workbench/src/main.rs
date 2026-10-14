// SPDX-License-Identifier: GPL-3.0-or-later

// Release builds on Windows are GUI programs: no console window behind the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use photography_workbench::PhotographyWorkbenchApp;

fn main() -> eframe::Result {
    // Logs go to stderr; set `RUST_LOG=debug` to see them.
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(PhotographyWorkbenchApp::icon())
            // Wayland ignores the window icon: the desktop finds it through
            // the .desktop entry whose name matches the app id, and packages
            // name theirs after the binary. eframe also names the directory
            // it persists state in after the app id.
            .with_app_id(env!("CARGO_BIN_NAME")),
        ..Default::default()
    };
    eframe::run_native(
        PhotographyWorkbenchApp::NAME,
        options,
        Box::new(|cc| Ok(Box::new(PhotographyWorkbenchApp::new(cc)))),
    )
}
