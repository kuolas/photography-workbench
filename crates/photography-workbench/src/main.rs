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
    let mut options = eframe::NativeOptions {
        viewport,
        // The app gives macOS a menu bar of its own (`menu.rs`), whose
        // Quit closes the window. The one the system gives by default
        // would end the app without closing it.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS as _;
            builder.with_default_menu(false);
        })),
        ..Default::default()
    };
    // Textures as large as the graphics card supports, rather than egui's
    // 8192 pixels a side, so a photo's largest preview fits in one
    // (`graphics.rs`). The rest of eframe's wgpu options stay its own.
    options.wgpu_options.wgpu_setup = photography_workbench::wgpu_setup();
    eframe::run_native(
        PhotographyWorkbenchApp::NAME,
        options,
        Box::new(|cc| Ok(Box::new(PhotographyWorkbenchApp::new(cc)))),
    )
}
