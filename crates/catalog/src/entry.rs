// SPDX-License-Identifier: GPL-3.0-or-later

//! Which entries of a folder the catalog shows, and what each one is: the
//! one rule every listing of a folder follows.
//!
//! An entry is shown when the system's file manager would show it, and
//! when it is a file or a folder once a symbolic link is followed.
//! Hidden means a name that begins with a dot on every system (which also
//! hides the `._DSC_0001.NEF` files macOS writes beside photos on memory
//! cards formatted for other systems: they carry a photo's extension but
//! hold no photo), and on macOS and Windows the system's own hidden flag
//! too.
//!
//! The rule costs no more than the system needs: on Linux and the other
//! Unix systems the listing usually says what an entry is, so mostly only
//! a symbolic link is looked up; on macOS the hidden flag is looked up for
//! every entry; and on Windows the listing carries the flag.

use std::fs::{DirEntry, FileType};

/// What an entry the catalog shows is, once a symbolic link is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Visible {
    /// A file.
    File,
    /// A folder.
    Folder,
}

/// Whether the catalog shows `entry`, and as what: `None` for an entry
/// that is hidden, that is neither a file nor a folder, or that cannot be
/// read, a symbolic link whose target is missing among them.
pub(crate) fn visible(entry: &DirEntry) -> Option<Visible> {
    if entry.file_name().as_encoded_bytes().starts_with(b".") {
        return None;
    }
    let mut file_type = shown_type(entry)?;
    if file_type.is_symlink() {
        file_type = std::fs::metadata(entry.path()).ok()?.file_type();
    }
    if file_type.is_file() {
        Some(Visible::File)
    } else if file_type.is_dir() {
        Some(Visible::Folder)
    } else {
        None
    }
}

/// The entry's own type, a symbolic link not followed, or `None` when the
/// system's hidden flag is set on it (`UF_HIDDEN`, which Finder hides) or
/// it cannot be read.
#[cfg(target_os = "macos")]
fn shown_type(entry: &DirEntry) -> Option<FileType> {
    use std::os::macos::fs::MetadataExt;
    /// The `UF_HIDDEN` flag of `chflags hidden`.
    const UF_HIDDEN: u32 = 0x8000;
    // An entry's metadata is its own, a symbolic link's not its target's.
    let metadata = entry.metadata().ok()?;
    (metadata.st_flags() & UF_HIDDEN == 0).then(|| metadata.file_type())
}

/// The entry's own type, a symbolic link not followed, or `None` when the
/// system's hidden attribute is set on it (`FILE_ATTRIBUTE_HIDDEN`, which
/// Explorer hides) or it cannot be read.
#[cfg(windows)]
fn shown_type(entry: &DirEntry) -> Option<FileType> {
    use std::os::windows::fs::MetadataExt;
    /// The `FILE_ATTRIBUTE_HIDDEN` attribute of `attrib +h`.
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    // The listing carries an entry's metadata, a symbolic link's own and
    // not its target's, so reading it costs nothing.
    let metadata = entry.metadata().ok()?;
    (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN == 0).then(|| metadata.file_type())
}

/// The entry's own type, a symbolic link not followed, or `None` when it
/// cannot be read. These systems have no hidden flag but the dot, and the
/// listing says what an entry is, mostly without a lookup.
#[cfg(not(any(target_os = "macos", windows)))]
fn shown_type(entry: &DirEntry) -> Option<FileType> {
    entry.file_type().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    /// What [`visible`] says of each entry of `folder`, by name.
    fn seen(folder: &Path) -> BTreeMap<String, Option<Visible>> {
        fs::read_dir(folder)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    visible(&entry),
                )
            })
            .collect()
    }

    #[test]
    fn files_and_folders_are_shown_as_what_they_are() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("photo.nef"), b"").unwrap();
        fs::create_dir(dir.path().join("Trip")).unwrap();
        let seen = seen(dir.path());
        assert_eq!(seen["photo.nef"], Some(Visible::File));
        assert_eq!(seen["Trip"], Some(Visible::Folder));
    }

    #[test]
    fn a_name_beginning_with_a_dot_is_hidden() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".hidden"), b"").unwrap();
        fs::write(dir.path().join("._DSC_0001.NEF"), b"").unwrap();
        fs::create_dir(dir.path().join(".Trashes")).unwrap();
        let seen = seen(dir.path());
        assert_eq!(seen[".hidden"], None);
        assert_eq!(seen["._DSC_0001.NEF"], None);
        assert_eq!(seen[".Trashes"], None);
    }

    /// Symbolic links are followed: to what they point at, or skipped when
    /// that is missing.
    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_shown_as_its_target() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("photo.nef"), b"").unwrap();
        fs::create_dir(dir.path().join("Trip")).unwrap();
        symlink(dir.path().join("photo.nef"), dir.path().join("to-photo")).unwrap();
        symlink(dir.path().join("Trip"), dir.path().join("to-trip")).unwrap();
        symlink(dir.path().join("missing"), dir.path().join("broken")).unwrap();
        let seen = seen(dir.path());
        assert_eq!(seen["to-photo"], Some(Visible::File));
        assert_eq!(seen["to-trip"], Some(Visible::Folder));
        assert_eq!(seen["broken"], None);
    }

    /// A socket, neither a file nor a folder, is not shown.
    #[cfg(unix)]
    #[test]
    fn anything_but_a_file_or_a_folder_is_not_shown() {
        let dir = tempfile::tempdir().unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(dir.path().join("socket")).unwrap();
        assert_eq!(seen(dir.path())["socket"], None);
    }
}
