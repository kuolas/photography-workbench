// SPDX-License-Identifier: GPL-3.0-or-later

// Release builds on Windows are GUI programs: no console window behind the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use photography_workbench::PhotographyWorkbenchApp;

fn main() -> eframe::Result {
    // Logs go to stderr; set `RUST_LOG=debug` to see them.
    env_logger::init();

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(PhotographyWorkbenchApp::MIN_WINDOW)
        .with_min_inner_size(PhotographyWorkbenchApp::MIN_WINDOW)
        .with_icon(PhotographyWorkbenchApp::icon())
        // Wayland ignores the window icon: the desktop finds it through
        // the .desktop entry whose name matches the app id, and packages
        // name theirs after the binary. eframe also names the directory
        // it persists state in after the app id.
        .with_app_id(env!("CARGO_BIN_NAME"));
    // The window draws its own title bar on every system (ADR010). On
    // Windows and Linux the system's frame is off, so the app draws the
    // window controls and resizes the window at its edges. On macOS the
    // frame stays, its title bar hidden and the content extended under it,
    // so the system's window buttons sit on the app's title bar.
    #[cfg(not(target_os = "macos"))]
    let viewport = viewport.with_decorations(false);
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_titlebar_shown(false)
        .with_title_shown(false)
        .with_fullsize_content_view(true);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        PhotographyWorkbenchApp::NAME,
        options,
        Box::new(|cc| Ok(Box::new(PhotographyWorkbenchApp::new(cc)))),
    )
}
