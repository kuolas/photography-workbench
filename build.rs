// SPDX-License-Identifier: GPL-3.0-or-later

//! Embeds the icon and version information into the Windows executable, so
//! Explorer and the taskbar show them. Other platforms need nothing here: the
//! window icon is set at run time (`src/app.rs`), and macOS and Linux bundles
//! carry their own icon files.

fn main() -> std::io::Result<()> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=assets/icon.ico");
    // The version-information strings come from the
    // `[package.metadata.winresource]` section of Cargo.toml.
    println!("cargo::rerun-if-changed=Cargo.toml");

    // The target, not the host: `#[cfg(windows)]` here would test the machine
    // running the build script.
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        // winresource links the resource into every executable this package
        // builds, test executables included; only the app's is shipped.
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile()?;
    }
    Ok(())
}
