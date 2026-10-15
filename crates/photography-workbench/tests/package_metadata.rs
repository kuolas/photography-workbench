// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's name, tagline and copyright are written out in Cargo.toml for
//! each packaging tool, and again in the code. These tests keep the
//! copies in step, so a rename cannot miss one, and keep every crate's
//! version where the workspace sets it.

use std::path::Path;

use photography_workbench::PhotographyWorkbenchApp;

fn manifest() -> toml::Table {
    include_str!("../Cargo.toml")
        .parse()
        .expect("Cargo.toml is valid TOML")
}

fn string<'a>(table: &'a toml::Table, path: &[&str]) -> &'a str {
    let (last, parents) = path.split_last().expect("a non-empty path");
    let mut table = table;
    for key in parents {
        table = table[*key]
            .as_table()
            .unwrap_or_else(|| panic!("Cargo.toml: {key} is a table"));
    }
    table[*last]
        .as_str()
        .unwrap_or_else(|| panic!("Cargo.toml: {} is a string", path.join(".")))
}

#[test]
fn every_display_name_is_the_window_title() {
    let manifest = manifest();
    for path in [
        &["package", "metadata", "bundle", "name"][..],
        &["package", "metadata", "winresource", "ProductName"],
        &["package", "metadata", "winresource", "FileDescription"],
    ] {
        assert_eq!(
            string(&manifest, path),
            PhotographyWorkbenchApp::NAME,
            "{}",
            path.join(".")
        );
    }
}

/// About shows the same tagline and copyright the packages carry.
#[test]
fn about_matches_the_bundle() {
    let manifest = manifest();
    let bundle = |key| string(&manifest, &["package", "metadata", "bundle", key]);
    assert_eq!(
        bundle("short_description"),
        PhotographyWorkbenchApp::TAGLINE
    );
    assert_eq!(bundle("copyright"), PhotographyWorkbenchApp::COPYRIGHT);
}

#[test]
fn windows_copyright_matches_the_bundle() {
    let manifest = manifest();
    assert_eq!(
        string(
            &manifest,
            &["package", "metadata", "winresource", "LegalCopyright"]
        ),
        string(&manifest, &["package", "metadata", "bundle", "copyright"]),
    );
}

/// The workspace sets the one version every crate has (ADR012); a version
/// written in a crate's own manifest would let that crate's drift from it.
#[test]
fn every_crate_takes_its_version_from_the_workspace() {
    // This crate sits in crates/, next to every other crate.
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the app crate is in crates/");
    let mut checked = 0;
    for entry in std::fs::read_dir(crates).expect("crates/ is readable") {
        let dir = entry.expect("crates/ is readable").path();
        if !dir.is_dir() {
            continue;
        }
        let path = dir.join("Cargo.toml");
        let manifest: toml::Table = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .parse()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let inherited = manifest["package"]
            .get("version")
            .and_then(|version| version.get("workspace"))
            .and_then(toml::Value::as_bool);
        assert_eq!(
            inherited,
            Some(true),
            "{}: package.version is `version.workspace = true`",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 0, "no crate found in {}", crates.display());
}

#[test]
fn windows_file_name_is_the_binary() {
    let manifest = manifest();
    assert_eq!(
        string(
            &manifest,
            &["package", "metadata", "winresource", "OriginalFilename"]
        ),
        format!("{}.exe", env!("CARGO_PKG_NAME")),
    );
}
