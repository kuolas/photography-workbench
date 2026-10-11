// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's name and copyright are written out in Cargo.toml for each
//! packaging tool, and the name again in the code. These tests keep the
//! copies in step, so a rename cannot miss one.

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
