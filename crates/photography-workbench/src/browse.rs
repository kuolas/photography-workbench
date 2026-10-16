// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's side of browsing the folders: [`Browser`], which keeps the
//! folder tree fed from disk as the window runs, saves what is expanded
//! and selected in it between runs, and reads the selected folder's
//! [`Photos`].
//!
//! The tree itself reads nothing (`catalog::folders`). Each frame, before
//! anything is drawn, [`Browser::update`] hands it what the disk has
//! read since the last frame, and starts reading what it asks for next.
//! The roots are watched on a thread of their own, which reads them every
//! 2 seconds and wakes the window only when they change, so an idle
//! window draws nothing.
//!
//! A folder's photos are read each time it is selected, and again when
//! the window comes back to the front, but never by two reads at once:
//! wanted while a read of them is under way, they are that read's
//! answer. A read under way when its folder's card is taken out is
//! forgotten, so the card put back is read afresh, even while that read
//! hangs, and its answer is dropped. Each read is numbered, and only the answer of the one wanted
//! last is taken, so a slow read of a folder left behind never replaces
//! the photos of the one chosen since. Reading the folder shown again
//! keeps its photos, and the current one, on screen until the answer
//! comes.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use catalog::folders::FolderTree;
use catalog::photos::PhotoList;

use crate::disk::{Disk, Done, Job};

/// The [`eframe::Storage`] key the selected folder is saved under, as its
/// path.
const FOLDER_KEY: &str = "folder";

/// The [`eframe::Storage`] key the expanded folders are saved under, as
/// their paths, one per line.
const EXPANDED_KEY: &str = "expanded_folders";

/// The selected folder's photos, as far as they have been read.
#[derive(Debug)]
pub enum Photos {
    /// No folder is selected, or the selected one is under no root, on a
    /// card or drive taken out.
    None,
    /// The photos of a folder just selected are being read.
    Reading,
    /// The folder's photos, and the current one.
    Read(PhotoList),
    /// The folder could not be read, for this reason.
    Failed(io::ErrorKind),
}

/// The folder tree, fed from disk as the window runs: its roots as the
/// disk watches them, and the folders it asks for listed, each by one
/// job at a time; and the selected folder's photos.
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
    /// The selected folder's photos, as far as they have been read.
    photos: Photos,
    /// The folder [`photos`](Self::photos) are of, or are being read
    /// from; `None` with [`Photos::None`].
    photos_of: Option<PathBuf>,
    /// How many reads of photos have been started, each numbered by the
    /// count when it started.
    reads: u64,
    /// The reads of photos under way, by the folder each reads: one at
    /// most for each folder. A read of a folder on a card taken out is
    /// forgotten, though it may still be under way, so the card put back
    /// is read afresh rather than wait for it.
    reading: HashMap<PathBuf, u64>,
    /// The number of the read whose answer is taken: the one wanted last,
    /// which started before others when its folder was chosen again while
    /// it was under way; `None` while no photos are wanted.
    wanted: Option<u64>,
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
            photos: Photos::None,
            photos_of: None,
            reads: 0,
            reading: HashMap::new(),
            wanted: None,
        }
    }
}

impl Browser {
    /// The tree saved in `storage`: its expanded folders expanded and its
    /// selected folder selected, shown once the roots and the listings
    /// they need have been read, and its photos read once its root is
    /// among them. With nothing saved, nothing is expanded or selected.
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

    /// Bring the tree and the photos up to date, once per frame, before
    /// anything is drawn: take what the disk has read since the last
    /// frame, start watching the roots on the first frame, refresh the
    /// tree and read the photos again when the window comes back to the
    /// front, read the selected folder's photos when its root comes, or
    /// drop them when it goes, and start listing the folders the tree
    /// asks for.
    pub fn update(&mut self, ctx: &egui::Context) {
        let finished: Vec<Done> = self.disk.finished().collect();
        for done in finished {
            self.take(done);
        }

        // The watcher runs for as long as the app does, so it is started
        // once.
        if !self.watching {
            self.watching = true;
            self.disk.watch_roots(ctx);
        }

        // A photographer who copied or deleted folders or photos in the
        // system's file manager sees the change on coming back. No key
        // does this: F5 means something else in the photography tools
        // whose keys the app follows (ADR015).
        let focused = ctx.input(|i| i.focused);
        let back = self.focused == Some(false) && focused;
        if back {
            self.tree.refresh();
        }
        self.focused = Some(focused);

        // The photos follow the selected folder's root: read when it comes,
        // as on a later frame than the restored folder at start-up, and
        // dropped when it goes.
        let selected = self.reachable_selection();
        if selected != self.photos_of.as_deref() || (back && selected.is_some()) {
            self.read_photos(ctx);
        }

        // One job per folder at a time. A refresh while a folder is being
        // listed makes the answer under way stale when it comes, and the
        // tree asks for the folder again then.
        for request in self.tree.to_list() {
            if self.listing.insert(request.path().to_path_buf()) {
                self.disk.start(ctx, Job::Subfolders(request));
            }
        }
    }

    /// Take `done`, an answer from the disk.
    fn take(&mut self, done: Done) {
        match done {
            Done::Roots(roots) => {
                self.tree.set_roots(roots);
                self.forget_reads_under_no_root();
            }
            Done::Subfolders(request, result) => {
                self.listing.remove(request.path());
                self.tree.listed(&request, result);
            }
            Done::Photos {
                folder,
                read,
                result,
            } => {
                // The read is over, whether its answer is taken or not.
                if self.reading.get(&folder) == Some(&read) {
                    self.reading.remove(&folder);
                }
                if self.wanted == Some(read) && self.photos_of.as_deref() == Some(folder.as_path())
                {
                    self.photos = match result {
                        Ok(list) => Photos::Read(keep_current(&self.photos, list)),
                        Err(error) => Photos::Failed(error.kind()),
                    };
                }
            }
        }
    }

    /// Forget the reads of photos under way of folders under none of the
    /// tree's roots, on a card or drive taken out, so their answers are
    /// dropped when they come and a folder is read afresh when its root
    /// comes back, even while the read of the card taken out hangs. A
    /// folder whose read was the one wanted shows nothing until then.
    fn forget_reads_under_no_root(&mut self) {
        let roots = self.tree.roots();
        let mut forgot_wanted = false;
        self.reading.retain(|folder, read| {
            let reachable = roots.iter().any(|root| folder.starts_with(root.path()));
            forgot_wanted |= !reachable && self.wanted == Some(*read);
            reachable
        });
        // Read again on the next frame if the root is back by then, as
        // it can be when it went and came back between two frames.
        if forgot_wanted {
            self.photos = Photos::None;
            self.photos_of = None;
            self.wanted = None;
        }
    }

    /// The selected folder, if it is under one of the tree's roots.
    fn reachable_selection(&self) -> Option<&Path> {
        let roots = self.tree.roots();
        self.tree
            .selected()
            .filter(|folder| roots.iter().any(|root| folder.starts_with(root.path())))
    }

    /// Start reading the selected folder's photos, if it is under one of
    /// the tree's roots, or else drop the photos. A read of that folder
    /// under way is not doubled: its answer is the one taken. Any other
    /// read under way is overtaken, its answer dropped when it comes. A
    /// folder other than the one shown shows nothing until its photos
    /// come; the one shown keeps its photos on screen until then.
    fn read_photos(&mut self, ctx: &egui::Context) {
        let Some(folder) = self.reachable_selection().map(Path::to_path_buf) else {
            self.photos = Photos::None;
            self.photos_of = None;
            self.wanted = None;
            return;
        };
        if self.photos_of.as_ref() != Some(&folder) {
            self.photos = Photos::Reading;
            self.photos_of = Some(folder.clone());
        }
        // One read per folder at a time, so a share that hangs holds one
        // thread however often its folder is chosen.
        let read = match self.reading.get(&folder) {
            Some(&read) => read,
            None => {
                self.reads += 1;
                self.reading.insert(folder.clone(), self.reads);
                self.disk.start(
                    ctx,
                    Job::Photos {
                        folder,
                        read: self.reads,
                    },
                );
                self.reads
            }
        };
        self.wanted = Some(read);
    }

    /// The tree, to draw.
    pub fn tree(&self) -> &FolderTree {
        &self.tree
    }

    /// The selected folder's photos, as far as they have been read.
    pub fn photos(&self) -> &Photos {
        &self.photos
    }

    /// Make the photo at `index` of the selected folder the current one,
    /// or the last one when `index` is past the end. Does nothing while
    /// there are no photos.
    pub fn select_photo(&mut self, index: usize) {
        if let Photos::Read(list) = &mut self.photos {
            list.select(index);
        }
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

    /// Select the folder at `path`, and read its photos, again if it was
    /// selected already.
    pub fn select(&mut self, ctx: &egui::Context, path: &Path) {
        self.tree.select(path);
        self.read_photos(ctx);
    }

    /// Read from `disk` from now on, keeping the tree as it is, and watch
    /// its roots from the next frame on.
    #[cfg(test)]
    pub fn use_disk(&mut self, disk: Disk) {
        self.disk = disk;
        self.watching = false;
    }
}

/// `list`, a folder's photos read again, at the photo that was current
/// in `before`, the folder's photos as they were, if it is still there,
/// or else at its first.
fn keep_current(before: &Photos, mut list: PhotoList) -> PhotoList {
    let current = match before {
        Photos::Read(before) if before.folder() == list.folder() => before.current_photo(),
        _ => None,
    };
    let index = current.and_then(|current| {
        list.photos()
            .iter()
            .position(|photo| photo.path() == current.path())
    });
    if let Some(index) = index {
        list.select(index);
    }
    list
}

/// `path` as one line of text, if it can be: valid Unicode with no line
/// break in it.
fn as_line(path: &Path) -> Option<&str> {
    path.to_str().filter(|line| !line.contains(['\n', '\r']))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::{Arc, Mutex};

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

    /// A browser reading from an inline disk, on which `dir` is the only
    /// root, read already, with the folders `a` and `b` in it: `a` holds
    /// `DSC_1.NEF` and `DSC_3.NEF`, `b` holds `IMG_1.jpg`. The handle
    /// returned holds the roots the disk reads.
    fn browsing(dir: &Path) -> (Browser, Arc<Mutex<Vec<Root>>>) {
        for (folder, names) in [
            ("a", &["DSC_1.NEF", "DSC_3.NEF"][..]),
            ("b", &["IMG_1.jpg"]),
        ] {
            fs::create_dir(dir.join(folder)).unwrap();
            for name in names {
                fs::write(dir.join(folder).join(name), b"").unwrap();
            }
        }
        let roots = Arc::new(Mutex::new(vec![Root::new(Place::Volume, "r", dir)]));
        let mut browser = Browser::default();
        browser.use_disk(Disk::inline(Arc::clone(&roots)));
        let ctx = egui::Context::default();
        browser.update(&ctx);
        browser.update(&ctx);
        assert_eq!(browser.tree.roots().len(), 1);
        (browser, roots)
    }

    /// The names of the photos shown, and the name of the current one;
    /// `None` when no photos are shown.
    fn shown(browser: &Browser) -> Option<(Vec<String>, String)> {
        let Photos::Read(list) = browser.photos() else {
            return None;
        };
        let names = list.photos().iter().map(|p| p.name().into_owned());
        let current = list.current_photo().map(|p| p.name().into_owned());
        Some((names.collect(), current.unwrap_or_default()))
    }

    /// The answer to a read that a later one overtook is dropped, however
    /// late it comes, even once its folder is chosen again; a folder just
    /// chosen shows nothing until its own photos come.
    #[test]
    fn an_answer_to_a_read_overtaken_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let (mut browser, _) = browsing(dir.path());
        let ctx = egui::Context::default();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        browser.select(&ctx, &a);
        browser.update(&ctx);
        // The answer of the first read of `a`, coming again much later.
        let (late, then) = (browser.reads, PhotoList::read(&a).unwrap());
        let late_answer = || Done::Photos {
            folder: a.clone(),
            read: late,
            result: Ok(then.clone()),
        };
        fs::write(a.join("DSC_2.NEF"), b"").unwrap();

        browser.select(&ctx, &b);
        assert!(matches!(browser.photos(), Photos::Reading));
        browser.update(&ctx);
        browser.take(late_answer());
        assert_eq!(shown(&browser).unwrap().0, ["IMG_1.jpg"]);

        browser.select(&ctx, &a);
        browser.update(&ctx);
        browser.take(late_answer());
        let names = ["DSC_1.NEF", "DSC_2.NEF", "DSC_3.NEF"];
        assert_eq!(shown(&browser).unwrap().0, names);
    }

    /// Reading the folder shown again keeps its photos on screen until
    /// the answer comes, then shows the photos copied in meanwhile,
    /// keeping the current photo.
    #[test]
    fn a_read_again_keeps_the_current_photo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut browser, _) = browsing(dir.path());
        let ctx = egui::Context::default();
        let a = dir.path().join("a");
        browser.select(&ctx, &a);
        browser.update(&ctx);
        browser.select_photo(1);
        assert_eq!(shown(&browser).unwrap().1, "DSC_3.NEF");

        fs::write(a.join("DSC_2.NEF"), b"").unwrap();
        browser.select(&ctx, &a);
        assert_eq!(shown(&browser).unwrap().0, ["DSC_1.NEF", "DSC_3.NEF"]);
        browser.update(&ctx);
        let (names, current) = shown(&browser).unwrap();
        assert_eq!(names, ["DSC_1.NEF", "DSC_2.NEF", "DSC_3.NEF"]);
        assert_eq!(current, "DSC_3.NEF");
    }

    /// When the current photo is gone from the folder read again, the
    /// first photo is the current one.
    #[test]
    fn a_read_again_without_the_current_photo_goes_to_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let (mut browser, _) = browsing(dir.path());
        let ctx = egui::Context::default();
        let a = dir.path().join("a");
        browser.select(&ctx, &a);
        browser.update(&ctx);
        browser.select_photo(1);
        fs::remove_file(a.join("DSC_3.NEF")).unwrap();
        fs::write(a.join("DSC_4.NEF"), b"").unwrap();
        browser.select(&ctx, &a);
        browser.update(&ctx);
        let (names, current) = shown(&browser).unwrap();
        assert_eq!(names, ["DSC_1.NEF", "DSC_4.NEF"]);
        assert_eq!(current, "DSC_1.NEF");
    }

    /// The photos go when the root of their folder goes, the selection
    /// staying, and are read again when it comes back. A read under way
    /// when the root went is dropped when it comes.
    #[test]
    fn the_photos_go_with_their_root_and_come_back_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let (mut browser, roots) = browsing(dir.path());
        let ctx = egui::Context::default();
        let a = dir.path().join("a");
        browser.select(&ctx, &a);
        browser.update(&ctx);
        assert!(shown(&browser).is_some());
        let under_way = browser.reads;

        let root = roots.lock().unwrap().clone();
        browser.take(Done::Roots(Vec::new()));
        browser.update(&ctx);
        assert!(matches!(browser.photos(), Photos::None));
        assert_eq!(browser.tree.selected(), Some(a.as_path()));
        let photos = Done::Photos {
            folder: a.clone(),
            read: under_way,
            result: PhotoList::read(&a),
        };
        browser.take(photos);
        browser.update(&ctx);
        assert!(matches!(browser.photos(), Photos::None));

        browser.take(Done::Roots(root));
        browser.update(&ctx);
        assert!(matches!(browser.photos(), Photos::Reading));
        browser.update(&ctx);
        assert_eq!(shown(&browser).unwrap().1, "DSC_1.NEF");
    }

    /// A browser whose tree has the one root `/r`, nothing selected, on a
    /// disk that does none of its jobs, so a test sees which were started
    /// and answers them itself.
    fn holding() -> Browser {
        let mut browser = Browser::default();
        browser.use_disk(Disk::held());
        browser
            .tree
            .set_roots(vec![Root::new(Place::Volume, "r", "/r")]);
        browser
    }

    /// A frame of `browser`'s, with the window in front when `focused`.
    fn frame(browser: &mut Browser, ctx: &egui::Context, focused: bool) {
        let input = egui::RawInput {
            focused,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| browser.update(ui.ctx()));
        // Nothing is painted, so the fonts' texture is not wanted.
        output.textures_delta.clear();
    }

    /// The reads of photos the disk was asked for, each the folder read
    /// and the read's number, in the order they were started.
    fn photo_reads(browser: &Browser) -> Vec<(PathBuf, u64)> {
        let reads = browser.disk.held_jobs().iter().filter_map(|job| match job {
            Job::Photos { folder, read } => Some((folder.clone(), *read)),
            Job::Subfolders(_) => None,
        });
        reads.collect()
    }

    /// The answer to the read numbered `read` of the photos in `folder`:
    /// that they could not be read, for `reason`, which tells the answers
    /// apart.
    fn failed(folder: &Path, read: u64, reason: io::ErrorKind) -> Done {
        Done::Photos {
            folder: folder.to_path_buf(),
            read,
            result: Err(reason.into()),
        }
    }

    /// A folder chosen again, or read again as the window comes back to
    /// the front, while a read of its photos is under way, takes that
    /// read's answer rather than start another; once the answer has
    /// come, the next choice reads it again.
    #[test]
    fn a_folder_s_photos_are_never_read_twice_at_once() {
        let mut browser = holding();
        let ctx = egui::Context::default();
        let a = Path::new("/r/a");
        frame(&mut browser, &ctx, true);
        browser.select(&ctx, a);
        frame(&mut browser, &ctx, false);
        frame(&mut browser, &ctx, true);
        browser.select(&ctx, a);
        assert_eq!(photo_reads(&browser), [(a.to_path_buf(), 1)]);

        browser.take(failed(a, 1, io::ErrorKind::PermissionDenied));
        assert!(matches!(
            browser.photos(),
            Photos::Failed(io::ErrorKind::PermissionDenied)
        ));
        browser.select(&ctx, a);
        assert_eq!(photo_reads(&browser)[1..], [(a.to_path_buf(), 2)]);
    }

    /// A folder chosen again while its first read is under way, after
    /// another one, takes that read's answer, and the other folder's
    /// answer is dropped; that answer ends its read all the same, so the
    /// other folder chosen again is read again.
    #[test]
    fn a_folder_chosen_again_takes_the_answer_of_its_read_under_way() {
        let mut browser = holding();
        let ctx = egui::Context::default();
        let (a, b) = (Path::new("/r/a"), Path::new("/r/b"));
        browser.select(&ctx, a);
        browser.select(&ctx, b);
        browser.select(&ctx, a);
        let reads = [(a.to_path_buf(), 1), (b.to_path_buf(), 2)];
        assert_eq!(photo_reads(&browser), reads);
        assert!(matches!(browser.photos(), Photos::Reading));

        browser.take(failed(b, 2, io::ErrorKind::NotFound));
        assert!(matches!(browser.photos(), Photos::Reading));
        browser.take(failed(a, 1, io::ErrorKind::PermissionDenied));
        assert!(matches!(
            browser.photos(),
            Photos::Failed(io::ErrorKind::PermissionDenied)
        ));
        browser.select(&ctx, b);
        assert_eq!(photo_reads(&browser)[2..], [(b.to_path_buf(), 3)]);
    }

    /// A read of photos under way when its folder's root goes, as a card
    /// is taken out, is forgotten: when the root comes back the folder is
    /// read afresh, the old read's answer is dropped when it comes, and
    /// the new one's is taken.
    #[test]
    fn a_card_put_back_is_read_afresh() {
        let mut browser = holding();
        let ctx = egui::Context::default();
        let a = Path::new("/r/a");
        let root = browser.tree.roots().to_vec();
        browser.select(&ctx, a);
        browser.take(Done::Roots(Vec::new()));
        browser.update(&ctx);
        assert!(matches!(browser.photos(), Photos::None));
        browser.take(Done::Roots(root));
        browser.update(&ctx);
        let reads = [(a.to_path_buf(), 1), (a.to_path_buf(), 2)];
        assert_eq!(photo_reads(&browser), reads);

        browser.take(failed(a, 1, io::ErrorKind::Other));
        assert!(matches!(browser.photos(), Photos::Reading));
        browser.take(failed(a, 2, io::ErrorKind::PermissionDenied));
        assert!(matches!(
            browser.photos(),
            Photos::Failed(io::ErrorKind::PermissionDenied)
        ));
    }

    /// A card taken out and put back between two frames is read afresh
    /// too, though the folder's photos were never dropped meanwhile.
    #[test]
    fn a_card_put_back_before_the_next_frame_is_read_afresh() {
        let mut browser = holding();
        let ctx = egui::Context::default();
        let a = Path::new("/r/a");
        let root = browser.tree.roots().to_vec();
        browser.select(&ctx, a);
        browser.take(Done::Roots(Vec::new()));
        browser.take(Done::Roots(root));
        browser.update(&ctx);
        let reads = [(a.to_path_buf(), 1), (a.to_path_buf(), 2)];
        assert_eq!(photo_reads(&browser), reads);
        browser.take(failed(a, 1, io::ErrorKind::Other));
        assert!(matches!(browser.photos(), Photos::Reading));
    }

    /// A folder that cannot be read says why.
    #[test]
    fn a_folder_that_cannot_be_read_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (mut browser, _) = browsing(dir.path());
        let ctx = egui::Context::default();
        browser.select(&ctx, &dir.path().join("missing"));
        browser.update(&ctx);
        assert!(matches!(
            browser.photos(),
            Photos::Failed(io::ErrorKind::NotFound)
        ));
    }
}
