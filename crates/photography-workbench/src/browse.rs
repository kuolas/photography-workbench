// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's side of browsing the folders: [`Browser`], which keeps the
//! folder tree fed from disk as the window runs, and saves what is
//! expanded and selected in it between runs.
//!
//! The tree itself reads nothing (`catalog::folders`). Each frame, before
//! anything is drawn, [`Browser::update`] hands it what the disk has
//! read since the last frame, and starts reading what it asks for next.
//! The roots are watched on a thread of their own, which reads them every
//! 2 seconds and wakes the window only when they change, so an idle
//! window draws nothing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use catalog::folders::FolderTree;

use crate::disk::{Disk, Done, Job};

/// The [`eframe::Storage`] key the selected folder is saved under, as its
/// path.
const FOLDER_KEY: &str = "folder";

/// The [`eframe::Storage`] key the expanded folders are saved under, as
/// their paths, one per line.
const EXPANDED_KEY: &str = "expanded_folders";

/// The folder tree, fed from disk as the window runs: its roots as the
/// disk watches them, and the folders it asks for listed, each by one
/// job at a time.
pub struct Browser {
    /// The tree as the user left it.
    tree: FolderTree,
    /// Where the roots and the listings are read.
    disk: Disk,
    /// The folders being listed: a job for each is under way.
    listing: HashSet<PathBuf>,
    /// Whether the disk watches the roots, as it does from the first
    /// frame on.
    watching: bool,
    /// Whether the window had the keyboard focus in the last frame;
    /// `None` before the first.
    focused: Option<bool>,
}

impl Default for Browser {
    /// A tree with nothing expanded or selected, on the real disk, its
    /// roots watched from the first frame on.
    fn default() -> Self {
        Self {
            tree: FolderTree::default(),
            disk: Disk::new(),
            listing: HashSet::new(),
            watching: false,
            focused: None,
        }
    }
}

impl Browser {
    /// The tree saved in `storage`: its expanded folders expanded and its
    /// selected folder selected, shown once the roots and the listings
    /// they need have been read. With nothing saved, nothing is expanded
    /// or selected.
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let saved = |key| storage.and_then(|storage| storage.get_string(key));
        let mut browser = Self::default();
        for line in saved(EXPANDED_KEY).unwrap_or_default().split('\n') {
            if !line.is_empty() {
                browser.tree.expand(Path::new(line));
            }
        }
        if let Some(folder) = saved(FOLDER_KEY).filter(|folder| !folder.is_empty()) {
            browser.tree.select(Path::new(&folder));
        }
        browser
    }

    /// Save the selected folder and the expanded ones in `storage`, as
    /// [`FolderTree::expanded`] names them. A path that is not valid
    /// Unicode, or holds a line break, cannot be stored as a line of
    /// text, so it is not saved: a folder by such a path is not selected
    /// or expanded on the next run.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let folder = self.tree.selected().and_then(as_line).unwrap_or_default();
        storage.set_string(FOLDER_KEY, folder.to_owned());
        let expanded: Vec<&str> = self
            .tree
            .expanded()
            .into_iter()
            .filter_map(as_line)
            .collect();
        storage.set_string(EXPANDED_KEY, expanded.join("\n"));
    }

    /// Bring the tree up to date, once per frame, before anything is
    /// drawn: take what the disk has read since the last frame, start
    /// watching the roots on the first frame, refresh the tree when the
    /// window comes back to the front, and start listing the folders the
    /// tree asks for.
    pub fn update(&mut self, ctx: &egui::Context) {
        for done in self.disk.finished() {
            match done {
                Done::Roots(roots) => self.tree.set_roots(roots),
                Done::Subfolders(request, result) => {
                    self.listing.remove(request.path());
                    self.tree.listed(&request, result);
                }
            }
        }

        // The watcher runs for as long as the app does, so it is started
        // once.
        if !self.watching {
            self.watching = true;
            self.disk.watch_roots(ctx);
        }

        // A photographer who copied or deleted folders in the system's
        // file manager sees the change on coming back. No key does this:
        // F5 means something else in the photography tools whose keys
        // the app follows (ADR015).
        let focused = ctx.input(|i| i.focused);
        if self.focused == Some(false) && focused {
            self.tree.refresh();
        }
        self.focused = Some(focused);

        // One job per folder at a time. A refresh while a folder is being
        // listed makes the answer under way stale when it comes, and the
        // tree asks for the folder again then.
        for request in self.tree.to_list() {
            if self.listing.insert(request.path().to_path_buf()) {
                self.disk.start(ctx, Job::Subfolders(request));
            }
        }
    }

    /// The tree, to draw.
    pub fn tree(&self) -> &FolderTree {
        &self.tree
    }

    /// Expand the folder at `path` if it is collapsed, and collapse it if
    /// it is expanded.
    pub fn toggle(&mut self, path: &Path) {
        if self.tree.is_expanded(path) {
            self.tree.collapse(path);
        } else {
            self.tree.expand(path);
        }
    }

    /// Select the folder at `path`.
    pub fn select(&mut self, path: &Path) {
        self.tree.select(path);
    }

    /// Read from `disk` from now on, keeping the tree as it is, and watch
    /// its roots from the next frame on.
    #[cfg(test)]
    pub fn use_disk(&mut self, disk: Disk) {
        self.disk = disk;
        self.watching = false;
    }
}

/// `path` as one line of text, if it can be: valid Unicode with no line
/// break in it.
fn as_line(path: &Path) -> Option<&str> {
    path.to_str().filter(|line| !line.contains(['\n', '\r']))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use catalog::folders::{Place, Root};
    use eframe::Storage as _;

    use super::*;

    /// Settings kept in memory, in place of eframe's file.
    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    /// A browser whose tree has the one root `/r`, with `expanded`
    /// expanded and `selected` selected, none of them listed.
    fn browser(expanded: &[&str], selected: &str) -> Browser {
        let mut browser = Browser::default();
        browser
            .tree
            .set_roots(vec![Root::new(Place::Volume, "r", "/r")]);
        for path in expanded {
            browser.tree.expand(Path::new(path));
        }
        browser.tree.select(Path::new(selected));
        browser
    }

    #[test]
    fn what_is_expanded_and_selected_is_saved_and_restored() {
        let mut storage = MemoryStorage::default();
        browser(&["/r", "/r/a b", "/r/a b/c"], "/r/a b").save(&mut storage);
        assert_eq!(
            storage.get_string(EXPANDED_KEY).as_deref(),
            Some("/r\n/r/a b\n/r/a b/c")
        );
        assert_eq!(storage.get_string(FOLDER_KEY).as_deref(), Some("/r/a b"));
        let restored = Browser::restore(Some(&storage));
        assert_eq!(restored.tree.selected(), Some(Path::new("/r/a b")));
        for path in ["/r", "/r/a b", "/r/a b/c"] {
            assert!(restored.tree.is_expanded(Path::new(path)), "{path}");
        }
    }

    /// A line break in a path would split it in two lines, and restore
    /// two folders that are not there.
    #[test]
    fn a_path_holding_a_line_break_is_not_saved() {
        let mut storage = MemoryStorage::default();
        browser(&["/r", "/r/a\nb", "/r/c\rd", "/r/e"], "/r/a\nb").save(&mut storage);
        assert_eq!(
            storage.get_string(EXPANDED_KEY).as_deref(),
            Some("/r\n/r/e")
        );
        assert_eq!(storage.get_string(FOLDER_KEY).as_deref(), Some(""));
        let restored = Browser::restore(Some(&storage));
        assert_eq!(restored.tree.selected(), None);
        assert!(!restored.tree.is_expanded(Path::new("/r/a")));
    }

    /// A path that is not valid Unicode cannot be stored as text.
    #[cfg(unix)]
    #[test]
    fn a_path_that_is_not_unicode_is_not_saved() {
        use std::os::unix::ffi::OsStrExt as _;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/r/\xff"));
        let mut storage = MemoryStorage::default();
        let mut browser = browser(&["/r"], "/r");
        browser.tree.expand(path);
        browser.tree.select(path);
        browser.save(&mut storage);
        assert_eq!(storage.get_string(EXPANDED_KEY).as_deref(), Some("/r"));
        assert_eq!(storage.get_string(FOLDER_KEY).as_deref(), Some(""));
    }

    #[test]
    fn nothing_saved_restores_nothing_expanded_or_selected() {
        let restored = Browser::restore(None);
        assert_eq!(restored.tree.selected(), None);
        let mut storage = MemoryStorage::default();
        storage.set_string(EXPANDED_KEY, String::new());
        storage.set_string(FOLDER_KEY, String::new());
        let restored = Browser::restore(Some(&storage));
        assert_eq!(restored.tree.selected(), None);
        assert!(!restored.tree.is_expanded(Path::new("")));
    }
}
