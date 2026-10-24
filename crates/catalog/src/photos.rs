// SPDX-License-Identifier: GPL-3.0-or-later

//! A folder's photos and the current one: [`PhotoList`], read from disk
//! by [`PhotoList::read`], with the [`Kind`] of each [`Photo`] told by its
//! extension.

use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

use raw_files::Format;

use crate::entry::{self, Visible};
use crate::natural;

/// What a photo's file holds, by its extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A camera's raw file, in one of the formats raw-files reads.
    Raw(Format),
    /// A JPEG.
    Jpeg,
}

impl Kind {
    /// The kind of photo the file at `path` is, by its extension alone in
    /// any case (`NEF`, `nef`), or `None` for a file that is not a photo.
    /// The file is never read.
    pub fn of(path: &Path) -> Option<Kind> {
        let extension = path.extension()?.to_str()?;
        if let Some(format) = Format::from_extension(extension).filter(|format| format.is_raw()) {
            return Some(Kind::Raw(format));
        }
        ["jpg", "jpeg"]
            .iter()
            .any(|jpeg| extension.eq_ignore_ascii_case(jpeg))
            .then_some(Kind::Jpeg)
    }
}

/// One photo of a folder: its file, and what that holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Photo {
    path: PathBuf,
    kind: Kind,
}

impl Photo {
    /// The photo's file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What the photo's file holds.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The name of the photo's file, with any part that is not valid
    /// Unicode replaced, for showing.
    pub fn name(&self) -> Cow<'_, str> {
        self.path.file_name().unwrap_or_default().to_string_lossy()
    }

    /// The photo's format, for showing: for a RAW file its own extension
    /// in capitals, as the cameras name it, "NRW" for an `.nrw` file and
    /// "NEF" for a `.nef` one; "JPEG" for a JPEG file, whether its
    /// extension is `.jpg` or `.jpeg`.
    pub fn format(&self) -> String {
        match self.kind {
            Kind::Jpeg => "JPEG".to_owned(),
            Kind::Raw(format) => self.path.extension().map_or_else(
                || format.name().to_owned(),
                |extension| extension.to_string_lossy().to_uppercase(),
            ),
        }
    }
}

/// A folder's photos, in natural order of their names (`DSC_2` before
/// `DSC_10`), and the current one.
///
/// There is a current photo exactly when the list is not empty: a new
/// list starts at its first photo, and the current photo moves only
/// within the list.
#[derive(Clone, Debug, Default)]
pub struct PhotoList {
    folder: PathBuf,
    photos: Vec<Photo>,
    current: Option<usize>,
}

impl PhotoList {
    /// The photos in `folder`, not in its subfolders: its files whose
    /// extension is a photo's ([`Kind::of`]), symbolic links followed,
    /// hidden ones left out as the system's file manager leaves them out.
    ///
    /// An entry that cannot be read is left out; a folder that cannot be
    /// listed at all is the error.
    ///
    /// This reads the disk, which may take a while on a network share or
    /// a drive that has to wake up, so the app calls it off the UI thread.
    pub fn read(folder: &Path) -> io::Result<PhotoList> {
        let mut photos = Vec::new();
        for entry in std::fs::read_dir(folder)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            // The extension first: it costs no lookup on any system.
            let Some(kind) = Kind::of(&path) else {
                continue;
            };
            if entry::visible(&entry) == Some(Visible::File) {
                photos.push(Photo { path, kind });
            }
        }
        photos.sort_by(|a, b| natural::cmp_file_names(&a.path, &b.path));
        Ok(PhotoList {
            folder: folder.to_path_buf(),
            current: (!photos.is_empty()).then_some(0),
            photos,
        })
    }

    /// The folder the photos are in.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The photos, in natural order of their names.
    pub fn photos(&self) -> &[Photo] {
        &self.photos
    }

    /// The index of the current photo, `None` only when the list is empty.
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The current photo, `None` only when the list is empty.
    pub fn current_photo(&self) -> Option<&Photo> {
        self.photos.get(self.current?)
    }

    /// Whether the folder has no photos.
    pub fn is_empty(&self) -> bool {
        self.photos.is_empty()
    }

    /// How many photos the folder has.
    pub fn len(&self) -> usize {
        self.photos.len()
    }

    /// Makes the photo at `index` the current one, or the last photo when
    /// `index` is past the end. Does nothing to an empty list.
    pub fn select(&mut self, index: usize) {
        if let Some(last) = self.photos.len().checked_sub(1) {
            self.current = Some(index.min(last));
        }
    }

    /// Moves the current photo `by` photos, forwards when it is positive,
    /// stopping at the first and the last photo and never wrapping around.
    /// Does nothing to an empty list.
    pub fn step(&mut self, by: isize) {
        if let Some(current) = self.current {
            self.select(current.saturating_add_signed(by));
        }
    }

    /// Makes the first photo the current one. Does nothing to an empty
    /// list.
    pub fn first(&mut self) {
        self.select(0);
    }

    /// Makes the last photo the current one. Does nothing to an empty list.
    pub fn last(&mut self) {
        self.select(usize::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Empty files named `names` in `folder`.
    fn touch(folder: &Path, names: &[&str]) {
        for name in names {
            fs::write(folder.join(name), b"").unwrap();
        }
    }

    /// The names of the photos in `folder`, in the list's order.
    fn names(folder: &Path) -> Vec<String> {
        let list = PhotoList::read(folder).unwrap();
        list.photos()
            .iter()
            .map(|photo| photo.name().into_owned())
            .collect()
    }

    /// A list of `count` photos, the files made in a new folder.
    fn list_of(count: usize) -> (tempfile::TempDir, PhotoList) {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..count {
            touch(dir.path(), &[&format!("DSC_{i}.NEF")]);
        }
        let list = PhotoList::read(dir.path()).unwrap();
        (dir, list)
    }

    #[test]
    fn a_photo_s_kind_is_told_by_its_extension_in_any_case() {
        let kind = |name| Kind::of(Path::new(name));
        assert_eq!(kind("DSC_0001.NEF"), Some(Kind::Raw(Format::Nef)));
        assert_eq!(kind("DSC_0001.nef"), Some(Kind::Raw(Format::Nef)));
        assert_eq!(kind("a.ARW"), Some(Kind::Raw(Format::Arw)));
        assert_eq!(kind("b.jpeg"), Some(Kind::Jpeg));
        assert_eq!(kind("c.JPG"), Some(Kind::Jpeg));
        assert_eq!(kind("d.Jpg"), Some(Kind::Jpeg));
    }

    #[test]
    fn every_raw_extension_raw_files_reads_is_a_raw_photo() {
        for format in Format::all()
            .iter()
            .copied()
            .filter(|format| format.is_raw())
        {
            for extension in format.extensions() {
                for name in [
                    format!("x.{extension}"),
                    format!("x.{}", extension.to_uppercase()),
                ] {
                    assert_eq!(
                        Kind::of(Path::new(&name)),
                        Some(Kind::Raw(format)),
                        "{name}"
                    );
                }
            }
        }
    }

    /// A photo whose file is named `name`, of the kind its extension
    /// tells.
    fn photo(name: &str) -> Photo {
        let path = PathBuf::from(name);
        let kind = Kind::of(&path).unwrap();
        Photo { path, kind }
    }

    /// A RAW file's format is its own extension in capitals, a JPEG
    /// file's is "JPEG" whichever of its two extensions it has.
    #[test]
    fn a_photo_s_format_is_named_by_its_extension() {
        let format = |name: &str| photo(name).format();
        assert_eq!(format("DSC_0001.NEF"), "NEF");
        assert_eq!(format("DSCN0001.nrw"), "NRW");
        assert_eq!(format("DSC00001.ARW"), "ARW");
        assert_eq!(format("DSC00001.sr2"), "SR2");
        assert_eq!(format("DSC00001.srf"), "SRF");
        assert_eq!(format("DSC00001.arq"), "ARQ");
        assert_eq!(format("IMG_0001.jpg"), "JPEG");
        assert_eq!(format("IMG_0001.JPEG"), "JPEG");
        for format in Format::all()
            .iter()
            .copied()
            .filter(|format| format.is_raw())
        {
            for extension in format.extensions() {
                let upper = extension.to_uppercase();
                for name in [format!("x.{extension}"), format!("x.{upper}")] {
                    assert_eq!(photo(&name).format(), upper, "{name}");
                }
            }
        }
    }

    #[test]
    fn a_file_with_another_extension_or_none_is_not_a_photo() {
        for name in ["a.tif", "DSC_0001.xmp", "README", "nef", "a.nef.txt"] {
            assert_eq!(Kind::of(Path::new(name)), None, "{name}");
        }
    }

    #[test]
    fn a_folder_lists_its_photos_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["DSC_0001.NEF", "a.ARW", "b.jpeg", "c.JPG"]);
        touch(dir.path(), &["d.tif", "DSC_0001.xmp", "README"]);
        assert_eq!(
            names(dir.path()),
            ["a.ARW", "b.jpeg", "c.JPG", "DSC_0001.NEF"]
        );
        let list = PhotoList::read(dir.path()).unwrap();
        assert_eq!(list.folder(), dir.path());
        let kinds: Vec<_> = list.photos().iter().map(Photo::kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Raw(Format::Arw),
                Kind::Jpeg,
                Kind::Jpeg,
                Kind::Raw(Format::Nef)
            ]
        );
        assert_eq!(list.photos()[0].path(), dir.path().join("a.ARW"));
    }

    #[test]
    fn photos_are_in_natural_order() {
        let dir = tempfile::tempdir().unwrap();
        touch(
            dir.path(),
            &["DSC_10.NEF", "DSC_2.NEF", "dsc_3.jpg", "DSC_1.NEF"],
        );
        assert_eq!(
            names(dir.path()),
            ["DSC_1.NEF", "DSC_2.NEF", "dsc_3.jpg", "DSC_10.NEF"]
        );
    }

    #[test]
    fn subfolders_and_their_photos_are_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["DSC_0001.NEF"]);
        fs::create_dir(dir.path().join("x.NEF")).unwrap();
        fs::create_dir(dir.path().join("Trip")).unwrap();
        touch(&dir.path().join("Trip"), &["DSC_0002.NEF"]);
        assert_eq!(names(dir.path()), ["DSC_0001.NEF"]);
    }

    #[test]
    fn hidden_files_are_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        touch(
            dir.path(),
            &["DSC_0001.NEF", ".DSC_0003.NEF", "._DSC_0001.NEF"],
        );
        assert_eq!(names(dir.path()), ["DSC_0001.NEF"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_file_flagged_hidden_is_not_listed_on_macos() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["DSC_0001.NEF", "DSC_0002.NEF"]);
        let status = std::process::Command::new("chflags")
            .arg("hidden")
            .arg(dir.path().join("DSC_0002.NEF"))
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(names(dir.path()), ["DSC_0001.NEF"]);
    }

    #[cfg(windows)]
    #[test]
    fn a_file_flagged_hidden_is_not_listed_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["DSC_0001.NEF", "DSC_0002.NEF"]);
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(dir.path().join("DSC_0002.NEF"))
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(names(dir.path()), ["DSC_0001.NEF"]);
    }

    /// A link to a photo is listed under its own name; a link to nothing,
    /// or to a folder, is not.
    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_to_a_photo_is_listed_and_a_broken_one_is_not() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        touch(elsewhere.path(), &["DSC_0001.NEF"]);
        fs::create_dir(elsewhere.path().join("Trip")).unwrap();
        symlink(
            elsewhere.path().join("DSC_0001.NEF"),
            dir.path().join("a.NEF"),
        )
        .unwrap();
        symlink(
            elsewhere.path().join("missing.NEF"),
            dir.path().join("b.NEF"),
        )
        .unwrap();
        symlink(elsewhere.path().join("Trip"), dir.path().join("c.NEF")).unwrap();
        assert_eq!(names(dir.path()), ["a.NEF"]);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(PhotoList::read(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn an_empty_folder_has_no_photos_and_no_current_photo() {
        let (_dir, mut list) = list_of(0);
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.current(), None);
        assert_eq!(list.current_photo(), None);
        list.step(1);
        list.select(3);
        list.first();
        list.last();
        assert_eq!(list.current(), None);
        assert_eq!(PhotoList::default().current(), None);
    }

    #[test]
    fn a_new_list_starts_at_its_first_photo() {
        let (_dir, list) = list_of(3);
        assert_eq!(list.len(), 3);
        assert_eq!(list.current(), Some(0));
        assert_eq!(list.current_photo().unwrap().name(), "DSC_0.NEF");
    }

    #[test]
    fn stepping_stops_at_the_first_and_the_last_photo() {
        let (_dir, mut list) = list_of(3);
        list.step(-1);
        assert_eq!(list.current(), Some(0));
        list.step(1);
        assert_eq!(list.current(), Some(1));
        list.step(1);
        list.step(1);
        assert_eq!(list.current(), Some(2));
        list.step(-2);
        assert_eq!(list.current(), Some(0));
        list.step(isize::MAX);
        assert_eq!(list.current(), Some(2));
        list.step(isize::MAX);
        assert_eq!(list.current(), Some(2));
        list.step(isize::MIN);
        assert_eq!(list.current(), Some(0));
        list.step(isize::MIN);
        assert_eq!(list.current(), Some(0));
    }

    #[test]
    fn selecting_past_the_end_selects_the_last_photo() {
        let (_dir, mut list) = list_of(3);
        list.select(1);
        assert_eq!(list.current(), Some(1));
        list.select(3);
        assert_eq!(list.current(), Some(2));
        list.select(usize::MAX);
        assert_eq!(list.current_photo().unwrap().name(), "DSC_2.NEF");
    }

    #[test]
    fn first_and_last_go_to_the_ends() {
        let (_dir, mut list) = list_of(3);
        list.last();
        assert_eq!(list.current(), Some(2));
        list.first();
        assert_eq!(list.current(), Some(0));
    }
}
