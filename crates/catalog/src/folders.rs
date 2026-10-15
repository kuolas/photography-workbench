// SPDX-License-Identifier: GPL-3.0-or-later

//! The folder tree: its [`Root`]s, the home folder and the mounted volumes
//! read by [`roots`]; a folder's [`Folder`]s read by [`subfolders`]; and
//! [`FolderTree`], which folders are expanded and which one is selected.
//!
//! Reading a folder can take a while, on a network share or a drive that
//! has to wake up, so the tree reads nothing itself. The caller asks it
//! which folders to list ([`FolderTree::to_list`]), lists them with
//! [`subfolders`] wherever it likes, and hands each result back with its
//! [`ListRequest`] ([`FolderTree::listed`]), which tells the tree whether
//! it was refreshed in between. Until then the tree shows what it has.
//!
//! The tree remembers what was expanded and selected: the caller saves
//! [`FolderTree::expanded`] and [`FolderTree::selected`], and restores
//! them with [`FolderTree::expand`] and [`FolderTree::select`].

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::entry::{self, Visible};
use crate::natural;

/// What a root of the tree is, which its icon shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The user's home folder.
    Home,
    /// A mounted volume: a memory card, an external drive, a network
    /// share, or on Windows any drive.
    Volume,
}

/// A root of the tree: the home folder or a mounted volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    place: Place,
    name: String,
    path: PathBuf,
}

impl Root {
    /// A root at `path`, shown as `name`.
    pub fn new(place: Place, name: impl Into<String>, path: impl Into<PathBuf>) -> Root {
        Root {
            place,
            name: name.into(),
            path: path.into(),
        }
    }

    /// What the root is.
    pub fn place(&self) -> Place {
        self.place
    }

    /// The name the root is shown as.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The root's folder.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The roots of the tree: the home folder first, named by its last part,
/// then the mounted volumes in natural order of their names.
///
/// What counts as a volume depends on the system:
///
/// - On macOS, the folders in `/Volumes`, apart from symbolic links (the
///   startup disk is there as a link to `/`, which the home folder
///   already stands for) and hidden ones.
/// - On Linux, the mount points in `/media/<user>`, `/run/media/<user>`
///   and `/mnt`, where the desktop and the user mount drives: folders on
///   another device than the folder they are in, so an empty folder left
///   behind by an unmounted drive is not one. The user is `USER`, or the
///   home folder's last part when `USER` is not set, and the first two
///   are skipped when neither is known. A place that is another's folder
///   by another name, as `/media` is a link to `/run/media` on some
///   systems, is read once.
/// - On Windows, the drives `C:` to `Z:` that are there, in that order.
/// - Elsewhere, none.
///
/// This reads the disk, which may take a while (a network drive that is
/// not connected can stall a drive's check on Windows), so the app calls
/// it off the UI thread.
pub fn roots() -> Vec<Root> {
    // An empty home would be a root with no folder.
    let home = std::env::home_dir().filter(|home| !home.as_os_str().is_empty());
    home.map(|path| Root::new(Place::Home, name_of(&path).into_owned(), path))
        .into_iter()
        .chain(volumes())
        .collect()
}

/// The mounted volumes in `/Volumes`, in natural order of their names.
#[cfg(target_os = "macos")]
fn volumes() -> Vec<Root> {
    volumes_in(Path::new("/Volumes"))
}

/// The mounted volumes where the desktop and the user mount drives, in
/// natural order of their names.
#[cfg(target_os = "linux")]
fn volumes() -> Vec<Root> {
    let places = places(std::env::var_os("USER"), std::env::home_dir());
    let mut volumes: Vec<Root> = distinct(places)
        .iter()
        .flat_map(|place| mount_points_in(place))
        .collect();
    sort_by_name(&mut volumes);
    volumes
}

/// The folders on Linux where the desktop and the user mount drives:
/// `/media/<user>` and `/run/media/<user>`, then `/mnt`. The user is
/// `user`, the value of `USER`, or the last part of `home`, the home
/// folder, when `user` is not set or empty; with neither, only `/mnt`.
/// Built for the tests on every system, as it reads nothing.
#[cfg(any(target_os = "linux", test))]
fn places(user: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Vec<PathBuf> {
    let user = user
        .filter(|user| !user.is_empty())
        .or_else(|| Some(home?.file_name()?.to_os_string()));
    let mut places = Vec::new();
    if let Some(user) = user {
        places.push(Path::new("/media").join(&user));
        places.push(Path::new("/run/media").join(&user));
    }
    places.push(PathBuf::from("/mnt"));
    places
}

/// `places` in their order, each folder once: a place whose resolved
/// path is that of one before it is left out, so a folder reached by a
/// link is not read twice, and so is a place that cannot be resolved, as
/// it cannot be listed either. Built for the tests on every Unix system,
/// where a symbolic link can be made to check it on.
#[cfg(any(target_os = "linux", all(unix, test)))]
fn distinct(places: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut resolved = BTreeSet::new();
    places
        .into_iter()
        .filter(|place| std::fs::canonicalize(place).is_ok_and(|path| resolved.insert(path)))
        .collect()
}

/// The drives `C:` to `Z:` whose root is a folder, in letter order. `A:`
/// and `B:` are left out: they were floppy drives, and checking one that
/// is still configured can wait on it.
#[cfg(windows)]
fn volumes() -> Vec<Root> {
    (b'C'..=b'Z')
        .filter_map(|letter| {
            let name = format!("{}:", char::from(letter));
            let path = PathBuf::from(format!("{name}\\"));
            path.is_dir().then(|| Root::new(Place::Volume, name, path))
        })
        .collect()
}

/// No volumes: the home folder is the only root on these systems.
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn volumes() -> Vec<Root> {
    Vec::new()
}

/// The volumes in `dir` by macOS's rule, in natural order of their names:
/// its folders, not symbolic links and not hidden ones. None when `dir`
/// cannot be listed. Built for the tests on every system, so the rule is
/// checked on each.
#[cfg(any(target_os = "macos", test))]
fn volumes_in(dir: &Path) -> Vec<Root> {
    let mut volumes = volumes_where(dir, |_| true);
    sort_by_name(&mut volumes);
    volumes
}

/// The mount points in `dir`, in the order the system lists them: its
/// folders, not symbolic links and not hidden ones, on another device
/// than `dir`. None when `dir` cannot be read. Built for the tests on
/// every Unix system, where a mount point can be found to check it on.
#[cfg(any(target_os = "linux", all(unix, test)))]
fn mount_points_in(dir: &Path) -> Vec<Root> {
    use std::os::unix::fs::MetadataExt;
    let Ok(device) = std::fs::metadata(dir).map(|metadata| metadata.dev()) else {
        return Vec::new();
    };
    // Not a link, so the entry's own metadata is the folder's.
    volumes_where(dir, |entry| {
        entry
            .metadata()
            .is_ok_and(|metadata| metadata.dev() != device)
    })
}

/// The volumes in `dir`, in the order the system lists them: its folders,
/// not symbolic links and not hidden ones, that `keep` keeps, each named
/// by its folder's name. None when `dir` cannot be listed.
#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn volumes_where(dir: &Path, mut keep: impl FnMut(&std::fs::DirEntry) -> bool) -> Vec<Root> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let shown = entry::shown(&entry)?;
            (shown.visible == Visible::Folder && !shown.link && keep(&entry)).then(|| {
                Root::new(
                    Place::Volume,
                    entry.file_name().to_string_lossy(),
                    entry.path(),
                )
            })
        })
        .collect()
}

/// Puts `roots` in natural order of their names, and roots of the same
/// name in order of their paths.
#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn sort_by_name(roots: &mut [Root]) {
    roots.sort_by(|a, b| {
        natural::cmp(a.name.as_ref(), b.name.as_ref()).then_with(|| a.path.cmp(&b.path))
    });
}

/// A folder of the tree, below a root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    path: PathBuf,
}

impl Folder {
    /// The folder at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Folder {
        Folder { path: path.into() }
    }

    /// The folder's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The folder's name, with any part that is not valid Unicode
    /// replaced, for showing.
    pub fn name(&self) -> Cow<'_, str> {
        name_of(&self.path)
    }
}

/// The folders in `dir`, not its files, in natural order of their names
/// (`Trip 2` before `Trip 10`): symbolic links followed, hidden ones left
/// out as the system's file manager leaves them out.
///
/// An entry that cannot be read is left out; a folder that cannot be
/// listed at all is the error.
///
/// This reads the disk, which may take a while on a network share or a
/// drive that has to wake up, so the app calls it off the UI thread.
pub fn subfolders(dir: &Path) -> io::Result<Vec<Folder>> {
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        // A file is never a folder, and saying so first spares a lookup
        // for each of a folder's photos (the hidden flag, on macOS). The
        // listing gives an entry's own type on macOS, Linux and Windows,
        // so this costs no lookup of its own, unless the file system
        // leaves the type out of its listing. A link is not a file here,
        // and goes through the rule below, as a folder does.
        if entry.file_type().is_ok_and(|own_type| own_type.is_file()) {
            continue;
        }
        if entry::visible(&entry) == Some(Visible::Folder) {
            folders.push(Folder::new(entry.path()));
        }
    }
    folders.sort_by(|a, b| natural::cmp_file_names(&a.path, &b.path));
    Ok(folders)
}

/// The last part of `path`, or the whole of it when it has none (`/`),
/// with any part that is not valid Unicode replaced, for showing.
fn name_of(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
}

/// What a row of the tree knows of its folder's subfolders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subfolders {
    /// The folder has not been listed yet, so it may have some.
    Unknown,
    /// The folder was listed and has none.
    Empty,
    /// The folder was listed and has some.
    Present,
    /// The folder could not be listed, for this reason.
    Unreadable(io::ErrorKind),
}

/// A row of the tree as it is drawn: a root or a folder below one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    /// The folder's path.
    pub path: &'a Path,
    /// The name it is shown as: a root's own name, or the folder's.
    pub name: Cow<'a, str>,
    /// How deep it is: 0 for a root, 1 for a folder in a root, and so on.
    pub depth: usize,
    /// What a root is; `None` for a folder below one.
    pub place: Option<Place>,
    /// Whether it is expanded. An expanded folder shows its subfolders
    /// below it once it has been listed.
    pub expanded: bool,
    /// What is known of its subfolders. A listing that
    /// [`FolderTree::refresh`] made stale still reports what it last knew.
    pub subfolders: Subfolders,
}

/// A folder the tree asks its caller to list, as [`FolderTree::to_list`]
/// names it, handed back to [`FolderTree::listed`] with the listing.
///
/// It carries when the tree asked, so a listing asked for before a
/// [`refresh`](FolderTree::refresh) and handed back after it is not taken
/// for a fresh one. Two requests are equal when they name the same folder
/// and were made between the same two refreshes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ListRequest {
    path: PathBuf,
    refreshes: u64,
}

impl ListRequest {
    /// The folder to list.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A folder's listing: its subfolders, or why it could not be listed, and
/// how many refreshes the tree had had when it asked for it. A listing
/// asked for before the latest refresh is stale: still shown, but to be
/// listed again.
#[derive(Clone, Debug)]
struct Listing {
    subfolders: Result<Vec<Folder>, io::ErrorKind>,
    refreshes: u64,
}

/// The folder tree as the user left it: its roots, which folders are
/// expanded, which one is selected, and the listings of the folders read
/// so far. It reads nothing from disk; see [the module](self) for how it
/// gets its listings.
///
/// Folders are told apart by their paths, compared part by part as
/// given, never resolved: a folder reached by two paths is two folders.
/// A folder can be expanded or selected before it is shown, which is how
/// a saved tree is restored.
#[derive(Clone, Debug, Default)]
pub struct FolderTree {
    roots: Vec<Root>,
    expanded: BTreeSet<PathBuf>,
    selected: Option<PathBuf>,
    listings: BTreeMap<PathBuf, Listing>,
    /// How many times the tree has been refreshed: a listing asked for
    /// with fewer is stale.
    refreshes: u64,
}

impl FolderTree {
    /// A tree of `roots`, nothing expanded, selected or listed.
    pub fn new(roots: Vec<Root>) -> FolderTree {
        FolderTree {
            roots,
            ..FolderTree::default()
        }
    }

    /// The roots, in the order they are shown.
    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    /// Replaces the roots, as volumes are mounted and unmounted. The
    /// listings of a root that stays, and of the folders below it, are
    /// kept; those below no new root are dropped, so a volume that comes
    /// back, or another one mounted at the same path, is listed again.
    /// What is expanded and the selection are kept, so a volume that
    /// comes back shows expanded as it was left once it is listed again.
    ///
    /// When a root goes, the tree is also refreshed: a listing still
    /// being read from the volume that went may be handed back after
    /// another volume is mounted at its path, and must not be taken for
    /// that one's.
    pub fn set_roots(&mut self, roots: Vec<Root>) {
        let gone = self
            .roots
            .iter()
            .any(|old| !roots.iter().any(|new| new.path == old.path));
        self.listings.retain(|path, _| under_any(&roots, path));
        self.roots = roots;
        if gone {
            self.refresh();
        }
    }

    /// Expands the folder at `path`, so its subfolders show below it once
    /// it has been listed.
    pub fn expand(&mut self, path: &Path) {
        self.expanded.insert(path.to_path_buf());
    }

    /// Collapses the folder at `path`. Its listing is kept, so expanding
    /// it again shows its subfolders at once, and so is which of them are
    /// expanded, as in the system's file manager.
    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
    }

    /// Whether the folder at `path` is expanded, shown or not.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Selects the folder at `path`, the one whose photos are shown.
    pub fn select(&mut self, path: &Path) {
        self.selected = Some(path.to_path_buf());
    }

    /// The selected folder, if any.
    pub fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    /// The folders the caller is to list now, each to be handed back to
    /// [`listed`](Self::listed) with its listing: every expanded folder
    /// the tree can reach, by the rule of [`expanded`](Self::expanded),
    /// that has not been listed or whose listing is stale, each once, in
    /// order of their paths, so a folder comes before those below it.
    ///
    /// So a tree restored many levels deep is asked for at once, not a
    /// level at a time as each listing arrives; its rows still show only
    /// the folders listed so far. A folder expanded below a collapsed
    /// one, or missing from its parent's fresh listing, is not asked for.
    ///
    /// A folder is asked for until its listing arrives, so the caller
    /// keeps track of the requests it is carrying out. After a
    /// [`refresh`](Self::refresh) a folder being listed is asked for in a
    /// new request, as the listing under way may be older than the
    /// change the refresh is for.
    pub fn to_list(&self) -> Vec<ListRequest> {
        self.expanded()
            .into_iter()
            .filter(|path| {
                self.listings
                    .get(*path)
                    .is_none_or(|listing| self.is_stale(listing))
            })
            .map(|path| ListRequest {
                path: path.to_path_buf(),
                refreshes: self.refreshes,
            })
            .collect()
    }

    /// Takes the listing `request` asked for: the folder's subfolders, as
    /// [`subfolders`] reads them, or the error that kept it from being
    /// listed.
    ///
    /// A listing asked for since the latest [`refresh`](Self::refresh) is
    /// fresh until the next one. One asked for before it is kept as
    /// stale, so the rows show it while [`to_list`](Self::to_list) asks
    /// for the folder again, unless a listing asked for later has arrived
    /// already, which it does not replace. A listing of a folder below no
    /// root, whose root has gone since it was asked for, is dropped.
    pub fn listed(&mut self, request: &ListRequest, result: io::Result<Vec<Folder>>) {
        if !under_any(&self.roots, &request.path) {
            return;
        }
        let later = self
            .listings
            .get(&request.path)
            .is_some_and(|listing| listing.refreshes > request.refreshes);
        if later {
            return;
        }
        let listing = Listing {
            subfolders: result.map_err(|error| error.kind()),
            refreshes: request.refreshes,
        };
        self.listings.insert(request.path.clone(), listing);
    }

    /// Makes every listing stale, so [`to_list`](Self::to_list) asks for
    /// the expanded folders again, those being listed among them. The
    /// rows stay as they are until the fresh listings arrive, so the tree
    /// does not flicker.
    pub fn refresh(&mut self) {
        self.refreshes += 1;
    }

    /// The tree as it is drawn, depth first: a row for each root and,
    /// below each expanded folder that has been listed, a row for each
    /// of its subfolders.
    pub fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        self.walk(&mut |path, depth, root| {
            rows.push(Row {
                path,
                name: root.map_or_else(|| name_of(path), |root| Cow::Borrowed(root.name.as_str())),
                depth,
                place: root.map(Root::place),
                expanded: self.expanded.contains(path),
                subfolders: self.subfolders(path),
            });
        });
        rows
    }

    /// The expanded folders worth saving, in order of their paths: those
    /// below a root (or a root itself) with every folder between them and
    /// the root expanded too, and found in their parent's listing where
    /// that is fresh and readable.
    ///
    /// So a folder deleted from disk, or one below a collapsed folder, is
    /// forgotten when the app saves, though [`collapse`](Self::collapse)
    /// keeps it while the app runs. A folder restored at start-up whose
    /// parent has not been listed yet, or could not be, is kept: nothing
    /// says it is gone.
    pub fn expanded(&self) -> Vec<&Path> {
        self.expanded
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| self.roots.iter().any(|root| self.reaches(&root.path, path)))
            .collect()
    }

    /// Whether `listing` was asked for before the latest refresh.
    fn is_stale(&self, listing: &Listing) -> bool {
        listing.refreshes < self.refreshes
    }

    /// Whether `path` is `root` or below it, with every folder from `root`
    /// down to its parent expanded, and in each of those folders' listings
    /// that is fresh and readable.
    fn reaches(&self, root: &Path, path: &Path) -> bool {
        let Ok(below) = path.strip_prefix(root) else {
            return false;
        };
        let mut folder = root.to_path_buf();
        for part in below.components() {
            if !self.expanded.contains(&folder) {
                return false;
            }
            let child = folder.join(part);
            if let Some(listing) = self.listings.get(&folder)
                && !self.is_stale(listing)
                && let Ok(subfolders) = &listing.subfolders
                && !subfolders.iter().any(|subfolder| subfolder.path == child)
            {
                return false;
            }
            folder = child;
        }
        true
    }

    /// What is known of the subfolders of the folder at `path`.
    fn subfolders(&self, path: &Path) -> Subfolders {
        match self.listings.get(path).map(|listing| &listing.subfolders) {
            None => Subfolders::Unknown,
            Some(Ok(subfolders)) if subfolders.is_empty() => Subfolders::Empty,
            Some(Ok(_)) => Subfolders::Present,
            Some(Err(kind)) => Subfolders::Unreadable(*kind),
        }
    }

    /// Calls `visit` on each folder the tree shows, in the order of the
    /// rows, with its depth and, for a root, the root.
    fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Path, usize, Option<&'a Root>)) {
        for root in &self.roots {
            visit(&root.path, 0, Some(root));
            self.walk_below(&root.path, 1, visit);
        }
    }

    /// Calls `visit` on each folder shown below `dir`, which is at
    /// `depth - 1`: its subfolders, and theirs, when it is expanded and
    /// listed.
    fn walk_below<'a>(
        &'a self,
        dir: &Path,
        depth: usize,
        visit: &mut impl FnMut(&'a Path, usize, Option<&'a Root>),
    ) {
        if !self.expanded.contains(dir) {
            return;
        }
        let Some(Listing {
            subfolders: Ok(subfolders),
            ..
        }) = self.listings.get(dir)
        else {
            return;
        };
        for subfolder in subfolders {
            visit(&subfolder.path, depth, None);
            self.walk_below(&subfolder.path, depth + 1, visit);
        }
    }
}

/// Whether `path` is one of `roots` or below one, part by part.
fn under_any(roots: &[Root], path: &Path) -> bool {
    roots.iter().any(|root| path.starts_with(&root.path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind;

    // The I/O: the roots and a folder's subfolders, over temporary
    // folders.

    /// The names of `roots`, in their order.
    fn root_names(roots: &[Root]) -> Vec<&str> {
        roots.iter().map(Root::name).collect()
    }

    /// The names of the subfolders of `dir`, in their order.
    fn subfolder_names(dir: &Path) -> Vec<String> {
        subfolders(dir)
            .unwrap()
            .iter()
            .map(|folder| folder.name().into_owned())
            .collect()
    }

    /// Folders named `names` in `dir`.
    fn make_folders(dir: &Path, names: &[&str]) {
        for name in names {
            fs::create_dir(dir.join(name)).unwrap();
        }
    }

    /// Sets the system's hidden flag on `path`, with `chflags hidden` on
    /// macOS and `attrib +h` on Windows.
    #[cfg(any(target_os = "macos", windows))]
    fn flag_hidden(path: &Path) {
        let (program, flag) = if cfg!(windows) {
            ("attrib", "+h")
        } else {
            ("chflags", "hidden")
        };
        let status = std::process::Command::new(program)
            .arg(flag)
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn a_folder_s_subfolders_are_its_folders_in_natural_order() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Trip 10", "trip 3", "Trip 2", "1"]);
        fs::write(dir.path().join("DSC_0001.NEF"), b"").unwrap();
        fs::write(dir.path().join("Trip 4"), b"").unwrap();
        assert_eq!(
            subfolder_names(dir.path()),
            ["1", "Trip 2", "trip 3", "Trip 10"]
        );
        let folders = subfolders(dir.path()).unwrap();
        assert_eq!(folders[0].path(), dir.path().join("1"));
        assert_eq!(folders[0], Folder::new(dir.path().join("1")));
    }

    #[test]
    fn hidden_folders_are_not_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Trip", ".git", ".Trashes"]);
        assert_eq!(subfolder_names(dir.path()), ["Trip"]);
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn a_folder_flagged_hidden_is_not_a_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Trip", "Secret"]);
        flag_hidden(&dir.path().join("Secret"));
        assert_eq!(subfolder_names(dir.path()), ["Trip"]);
    }

    /// A link to a folder is a subfolder under its own name; a link to a
    /// file or to nothing is not.
    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_to_a_folder_is_a_subfolder_and_a_broken_one_is_not() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        make_folders(elsewhere.path(), &["Trip"]);
        fs::write(elsewhere.path().join("DSC_0001.NEF"), b"").unwrap();
        symlink(elsewhere.path().join("Trip"), dir.path().join("a")).unwrap();
        symlink(elsewhere.path().join("missing"), dir.path().join("b")).unwrap();
        symlink(elsewhere.path().join("DSC_0001.NEF"), dir.path().join("c")).unwrap();
        assert_eq!(subfolder_names(dir.path()), ["a"]);
        assert_eq!(
            subfolders(dir.path()).unwrap()[0].path(),
            dir.path().join("a")
        );
    }

    #[test]
    fn listing_a_folder_that_does_not_exist_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(subfolders(&dir.path().join("missing")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn listing_a_folder_that_cannot_be_read_is_an_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        make_folders(dir.path(), &["locked"]);
        make_folders(&locked, &["Trip"]);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads any folder, so the error cannot be made.
        let readable = fs::read_dir(&locked).is_ok();
        let result = subfolders(&locked);
        // Restored before the temporary folder is removed.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
        }
    }

    #[test]
    fn a_folder_s_name_is_its_last_part_or_the_whole_path() {
        assert_eq!(Folder::new("/r/Trip 2").name(), "Trip 2");
        assert_eq!(Folder::new("/").name(), "/");
    }

    #[test]
    fn volumes_are_the_folders_in_natural_order() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Card 10", "card 2", "Backup"]);
        fs::write(dir.path().join("note.txt"), b"").unwrap();
        let volumes = volumes_in(dir.path());
        assert_eq!(root_names(&volumes), ["Backup", "card 2", "Card 10"]);
        assert_eq!(volumes[0].place(), Place::Volume);
        assert_eq!(volumes[0].path(), dir.path().join("Backup"));
    }

    #[test]
    fn hidden_folders_are_not_volumes() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Card", ".Trashes"]);
        assert_eq!(root_names(&volumes_in(dir.path())), ["Card"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_folder_flagged_hidden_is_not_a_volume() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Card", "Recovery"]);
        flag_hidden(&dir.path().join("Recovery"));
        assert_eq!(root_names(&volumes_in(dir.path())), ["Card"]);
    }

    /// The startup disk is in `/Volumes` as a link to `/`.
    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_to_a_folder_is_not_a_volume() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Card"]);
        std::os::unix::fs::symlink("/", dir.path().join("Macintosh HD")).unwrap();
        assert_eq!(root_names(&volumes_in(dir.path())), ["Card"]);
    }

    #[test]
    fn a_missing_folder_of_volumes_has_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(volumes_in(&dir.path().join("missing")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_on_the_same_device_is_not_a_mount_point() {
        let dir = tempfile::tempdir().unwrap();
        make_folders(dir.path(), &["Card"]);
        assert!(mount_points_in(dir.path()).is_empty());
        assert!(mount_points_in(&dir.path().join("missing")).is_empty());
    }

    /// A file system every system of the kind mounts: `/proc` on Linux,
    /// a volume of the startup disk's container on macOS.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn a_folder_on_another_device_is_a_mount_point() {
        let (dir, name) = if cfg!(target_os = "linux") {
            ("/", "proc")
        } else {
            ("/System/Volumes", "Preboot")
        };
        let mount_points = mount_points_in(Path::new(dir));
        assert!(root_names(&mount_points).contains(&name));
        assert!(
            mount_points
                .iter()
                .all(|root| root.place() == Place::Volume)
        );
    }

    #[test]
    fn the_places_of_drives_are_the_user_s_and_mnt() {
        let of = |user: Option<&str>, home: Option<&str>| {
            places(user.map(Into::into), home.map(PathBuf::from))
        };
        let of_user = |user: &str| {
            paths(&[
                &format!("/media/{user}"),
                &format!("/run/media/{user}"),
                "/mnt",
            ])
        };
        assert_eq!(of(Some("ana"), Some("/home/bo")), of_user("ana"));
        // Without `USER`, the home folder names the user.
        assert_eq!(of(None, Some("/home/bo")), of_user("bo"));
        assert_eq!(of(Some(""), Some("/home/bo")), of_user("bo"));
        assert_eq!(of(None, Some("/")), paths(&["/mnt"]));
        assert_eq!(of(None, None), paths(&["/mnt"]));
    }

    /// `/media` is a link to `/run/media` on some systems.
    #[cfg(unix)]
    #[test]
    fn a_place_reached_by_a_link_to_another_or_not_there_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("run/media/ana")).unwrap();
        make_folders(dir.path(), &["mnt"]);
        std::os::unix::fs::symlink(dir.path().join("run/media"), dir.path().join("media")).unwrap();
        let places = ["media/ana", "run/media/ana", "missing", "mnt"]
            .map(|place| dir.path().join(place))
            .to_vec();
        assert_eq!(
            distinct(places),
            [dir.path().join("media/ana"), dir.path().join("mnt")]
        );
    }

    #[test]
    fn the_home_folder_is_the_first_root() {
        let roots = roots();
        if let Some(home) = std::env::home_dir().filter(|home| !home.as_os_str().is_empty()) {
            assert_eq!(roots[0].place(), Place::Home);
            assert_eq!(roots[0].path(), home);
            assert_eq!(roots[0].name(), name_of(&home));
        }
        assert!(
            roots
                .iter()
                .skip(1)
                .all(|root| root.place() == Place::Volume)
        );
    }

    // The tree: pure state, fed made-up listings, with no disk.

    /// A tree of the roots at `paths`, each a volume named by its path.
    fn tree(paths: &[&str]) -> FolderTree {
        FolderTree::new(
            paths
                .iter()
                .map(|path| Root::new(Place::Volume, *path, *path))
                .collect(),
        )
    }

    /// A listing of the folders at `paths`.
    fn folders(paths: &[&str]) -> io::Result<Vec<Folder>> {
        Ok(paths.iter().map(|path| Folder::new(*path)).collect())
    }

    /// A listing that failed.
    fn denied() -> io::Result<Vec<Folder>> {
        Err(io::Error::from(ErrorKind::PermissionDenied))
    }

    /// The rows as an outline: each name indented two spaces a level.
    fn outline(tree: &FolderTree) -> Vec<String> {
        tree.rows()
            .iter()
            .map(|row| format!("{}{}", "  ".repeat(row.depth), row.name))
            .collect()
    }

    /// Paths, for comparing with what the tree hands out.
    fn paths(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    /// The folders `tree` asks to list, in its order.
    fn asked(tree: &FolderTree) -> Vec<PathBuf> {
        tree.to_list()
            .into_iter()
            .map(|request| request.path)
            .collect()
    }

    /// A request for `dir` made now, whether `tree` asks for it or not.
    fn request(tree: &FolderTree, dir: &str) -> ListRequest {
        ListRequest {
            path: PathBuf::from(dir),
            refreshes: tree.refreshes,
        }
    }

    /// Hands `tree` `result` as the listing of `dir`, asked for now.
    fn list(tree: &mut FolderTree, dir: &str, result: io::Result<Vec<Folder>>) {
        let request = request(tree, dir);
        tree.listed(&request, result);
    }

    /// The row for the folder at `path`.
    fn row<'a>(tree: &'a FolderTree, path: &str) -> Row<'a> {
        tree.rows()
            .into_iter()
            .find(|row| row.path == Path::new(path))
            .unwrap()
    }

    /// A tree with `/r` expanded and listed as holding `a` and `b`, and
    /// `/r/a` expanded and listed as holding `x`.
    fn listed_tree() -> FolderTree {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        list(&mut tree, "/r", folders(&["/r/a", "/r/b"]));
        tree.expand(Path::new("/r/a"));
        list(&mut tree, "/r/a", folders(&["/r/a/x"]));
        tree
    }

    #[test]
    fn a_new_tree_shows_its_roots_collapsed_and_unlisted() {
        let tree = FolderTree::new(vec![
            Root::new(Place::Home, "alvaro", "/home/alvaro"),
            Root::new(Place::Volume, "Card", "/media/alvaro/Card"),
        ]);
        let rows = tree.rows();
        assert_eq!(
            rows[0],
            Row {
                path: Path::new("/home/alvaro"),
                name: Cow::Borrowed("alvaro"),
                depth: 0,
                place: Some(Place::Home),
                expanded: false,
                subfolders: Subfolders::Unknown,
            }
        );
        assert_eq!(rows[1].name, "Card");
        assert_eq!(rows[1].place, Some(Place::Volume));
        assert_eq!(rows.len(), 2);
        assert_eq!(tree.roots().len(), 2);
        assert!(asked(&tree).is_empty());
        assert_eq!(tree.selected(), None);
        assert!(tree.expanded().is_empty());
    }

    #[test]
    fn an_expanded_root_is_listed_and_shows_its_subfolders() {
        let mut tree = tree(&["/r", "/s"]);
        tree.expand(Path::new("/r"));
        assert!(tree.is_expanded(Path::new("/r")));
        assert_eq!(asked(&tree), paths(&["/r"]));
        assert_eq!(outline(&tree), ["/r", "/s"]);
        list(&mut tree, "/r", folders(&["/r/a", "/r/b"]));
        assert!(asked(&tree).is_empty());
        assert_eq!(outline(&tree), ["/r", "  a", "  b", "/s"]);
        let a = row(&tree, "/r/a");
        assert_eq!((a.depth, a.place, a.expanded), (1, None, false));
        assert_eq!(a.subfolders, Subfolders::Unknown);
    }

    #[test]
    fn expanding_a_subfolder_asks_for_it_and_shows_its_own() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        list(&mut tree, "/r", folders(&["/r/a", "/r/b"]));
        tree.expand(Path::new("/r/a"));
        assert_eq!(asked(&tree), paths(&["/r/a"]));
        list(&mut tree, "/r/a", folders(&["/r/a/x"]));
        assert!(asked(&tree).is_empty());
        assert_eq!(outline(&tree), ["/r", "  a", "    x", "  b"]);
        assert_eq!(row(&tree, "/r/a/x").depth, 2);
    }

    #[test]
    fn a_folder_expanded_below_a_collapsed_one_is_neither_shown_nor_listed() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r/a"));
        assert!(asked(&tree).is_empty());
        assert_eq!(outline(&tree), ["/r"]);
        tree.expand(Path::new("/r"));
        assert_eq!(asked(&tree), paths(&["/r", "/r/a"]));
        assert_eq!(outline(&tree), ["/r"]);
        list(&mut tree, "/r", folders(&["/r/a"]));
        assert_eq!(asked(&tree), paths(&["/r/a"]));
        assert!(row(&tree, "/r/a").expanded);
    }

    /// A saved tree is asked for to its deepest folder at once, not a
    /// level at a time, though its rows show only what has been listed.
    #[test]
    fn a_restored_tree_is_asked_for_at_once_down_to_its_deepest_folder() {
        let mut tree = tree(&["/r"]);
        for path in ["/r", "/r/a", "/r/a/b", "/r/a/b/c", "/r/z/y"] {
            tree.expand(Path::new(path));
        }
        // `/r/z` is collapsed, so `/r/z/y` is not asked for.
        assert_eq!(asked(&tree), paths(&["/r", "/r/a", "/r/a/b", "/r/a/b/c"]));
        assert_eq!(outline(&tree), ["/r"]);
        list(&mut tree, "/r/a/b", folders(&["/r/a/b/c"]));
        assert_eq!(asked(&tree), paths(&["/r", "/r/a", "/r/a/b/c"]));
        assert_eq!(outline(&tree), ["/r"]);
        list(&mut tree, "/r", folders(&["/r/a", "/r/z"]));
        assert_eq!(outline(&tree), ["/r", "  a", "  z"]);
    }

    /// A folder gone from disk is not asked for; a stale listing does not
    /// say it is gone.
    #[test]
    fn a_folder_missing_from_its_parent_s_fresh_listing_is_not_asked_for() {
        let mut tree = tree(&["/r"]);
        for path in ["/r", "/r/a", "/r/a/x"] {
            tree.expand(Path::new(path));
        }
        list(&mut tree, "/r", folders(&["/r/b"]));
        assert!(asked(&tree).is_empty());
        tree.refresh();
        assert_eq!(asked(&tree), paths(&["/r", "/r/a", "/r/a/x"]));
    }

    #[test]
    fn collapsing_keeps_the_listing_and_what_is_expanded_below() {
        let mut tree = listed_tree();
        tree.collapse(Path::new("/r"));
        assert!(!tree.is_expanded(Path::new("/r")));
        assert_eq!(outline(&tree), ["/r"]);
        assert_eq!(row(&tree, "/r").subfolders, Subfolders::Present);
        tree.expand(Path::new("/r"));
        assert!(asked(&tree).is_empty());
        assert_eq!(outline(&tree), ["/r", "  a", "    x", "  b"]);
    }

    #[test]
    fn a_row_tells_what_is_known_of_its_subfolders() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        list(
            &mut tree,
            "/r",
            folders(&["/r/empty", "/r/full", "/r/locked"]),
        );
        list(&mut tree, "/r/empty", folders(&[]));
        list(&mut tree, "/r/full", folders(&["/r/full/x"]));
        list(&mut tree, "/r/locked", denied());
        assert_eq!(row(&tree, "/r").subfolders, Subfolders::Present);
        assert_eq!(row(&tree, "/r/empty").subfolders, Subfolders::Empty);
        assert_eq!(row(&tree, "/r/full").subfolders, Subfolders::Present);
        assert_eq!(
            row(&tree, "/r/locked").subfolders,
            Subfolders::Unreadable(ErrorKind::PermissionDenied)
        );
    }

    /// An unreadable folder has nothing to show, and is not asked for
    /// again until a refresh.
    #[test]
    fn an_unreadable_folder_shows_nothing_below_it() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        list(&mut tree, "/r", denied());
        assert_eq!(outline(&tree), ["/r"]);
        assert!(asked(&tree).is_empty());
    }

    #[test]
    fn a_refresh_keeps_the_rows_and_asks_for_every_expanded_folder_again() {
        let mut tree = listed_tree();
        // Expanded but not shown: below a collapsed folder.
        list(&mut tree, "/r/b", folders(&["/r/b/y"]));
        tree.expand(Path::new("/r/b/y"));
        let before = format!("{:?}", tree.rows());
        tree.refresh();
        assert_eq!(format!("{:?}", tree.rows()), before);
        assert_eq!(asked(&tree), paths(&["/r", "/r/a"]));
        list(&mut tree, "/r", folders(&["/r/a", "/r/b"]));
        assert_eq!(asked(&tree), paths(&["/r/a"]));
        list(&mut tree, "/r/a", folders(&["/r/a/x"]));
        assert!(asked(&tree).is_empty());
    }

    /// A listing under way when the tree is refreshed may predate the
    /// change the refresh is for.
    #[test]
    fn a_listing_asked_for_before_a_refresh_is_shown_and_asked_for_again() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        let early = tree.to_list().remove(0);
        assert_eq!(early.path(), Path::new("/r"));
        tree.refresh();
        let late = tree.to_list().remove(0);
        assert_ne!(early, late);
        tree.listed(&early, folders(&["/r/a"]));
        assert_eq!(outline(&tree), ["/r", "  a"]);
        assert_eq!(tree.to_list(), std::slice::from_ref(&late));
        tree.listed(&late, folders(&["/r/a", "/r/b"]));
        assert_eq!(outline(&tree), ["/r", "  a", "  b"]);
        assert!(asked(&tree).is_empty());
    }

    #[test]
    fn a_listing_asked_for_before_a_refresh_does_not_replace_a_later_one() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        let early = tree.to_list().remove(0);
        tree.refresh();
        let late = tree.to_list().remove(0);
        tree.listed(&late, folders(&["/r/b"]));
        tree.listed(&early, folders(&["/r/a"]));
        assert_eq!(outline(&tree), ["/r", "  b"]);
        assert!(asked(&tree).is_empty());
    }

    #[test]
    fn a_folder_gone_from_a_fresh_listing_is_gone_from_the_rows_and_from_what_is_saved() {
        let mut tree = listed_tree();
        tree.refresh();
        list(&mut tree, "/r", folders(&["/r/b"]));
        assert_eq!(outline(&tree), ["/r", "  b"]);
        assert_eq!(tree.expanded(), [Path::new("/r")]);
        assert!(tree.is_expanded(Path::new("/r/a")));
    }

    #[test]
    fn a_folder_gone_from_a_stale_listing_is_still_saved() {
        let mut tree = listed_tree();
        list(&mut tree, "/r", folders(&["/r/b"]));
        assert_eq!(tree.expanded(), [Path::new("/r")]);
        // Stale, the listing no longer says whether `/r/a` is there.
        tree.refresh();
        assert_eq!(tree.expanded(), [Path::new("/r"), Path::new("/r/a")]);
    }

    #[test]
    fn a_restored_folder_below_an_unlisted_one_is_still_saved() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        tree.expand(Path::new("/r/a"));
        tree.expand(Path::new("/r/a/x"));
        assert_eq!(
            tree.expanded(),
            [Path::new("/r"), Path::new("/r/a"), Path::new("/r/a/x")]
        );
        list(&mut tree, "/r", denied());
        assert_eq!(
            tree.expanded(),
            [Path::new("/r"), Path::new("/r/a"), Path::new("/r/a/x")]
        );
    }

    #[test]
    fn a_folder_below_a_collapsed_one_or_no_root_is_not_saved() {
        let mut tree = listed_tree();
        tree.expand(Path::new("/elsewhere"));
        tree.expand(Path::new("/r/a/x"));
        tree.collapse(Path::new("/r/a"));
        assert_eq!(tree.expanded(), [Path::new("/r")]);
        tree.collapse(Path::new("/r"));
        assert!(tree.expanded().is_empty());
        tree.expand(Path::new("/r"));
        tree.expand(Path::new("/r/a"));
        assert_eq!(
            tree.expanded(),
            [Path::new("/r"), Path::new("/r/a"), Path::new("/r/a/x")]
        );
    }

    /// A path that merely begins with the same letters as a root is not
    /// below it.
    #[test]
    fn a_folder_is_below_a_root_part_by_part() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        tree.expand(Path::new("/rx"));
        assert_eq!(tree.expanded(), [Path::new("/r")]);
    }

    #[test]
    fn the_selection_is_kept_across_a_refresh_and_new_roots() {
        let mut tree = listed_tree();
        tree.select(Path::new("/r/a/x"));
        assert_eq!(tree.selected(), Some(Path::new("/r/a/x")));
        tree.refresh();
        assert_eq!(tree.selected(), Some(Path::new("/r/a/x")));
        tree.set_roots(Vec::new());
        assert_eq!(tree.selected(), Some(Path::new("/r/a/x")));
        tree.select(Path::new("/r/b"));
        assert_eq!(tree.selected(), Some(Path::new("/r/b")));
    }

    /// A card taken out and put back, or another card at the same path,
    /// may not hold what the tree last listed there.
    #[test]
    fn new_roots_keep_what_is_expanded_and_the_listings_of_the_roots_that_stay() {
        let mut tree = listed_tree();
        tree.set_roots(vec![
            Root::new(Place::Volume, "/r", "/r"),
            Root::new(Place::Volume, "Card", "/card"),
        ]);
        tree.expand(Path::new("/card"));
        list(&mut tree, "/card", folders(&["/card/DCIM"]));
        tree.expand(Path::new("/card/DCIM"));
        list(&mut tree, "/card/DCIM", folders(&["/card/DCIM/100"]));
        let card = ["Card", "  DCIM", "    100"];
        assert_eq!(outline(&tree)[4..], card);

        // The root that stays keeps its rows, and is listed again, as a
        // root going refreshes the tree.
        tree.set_roots(vec![Root::new(Place::Volume, "/r", "/r")]);
        assert_eq!(outline(&tree), ["/r", "  a", "    x", "  b"]);
        assert_eq!(asked(&tree), paths(&["/r", "/r/a"]));
        assert!(tree.is_expanded(Path::new("/card/DCIM")));

        tree.set_roots(vec![
            Root::new(Place::Volume, "/r", "/r"),
            Root::new(Place::Volume, "Card", "/card"),
        ]);
        assert_eq!(tree.roots().len(), 2);
        assert_eq!(outline(&tree)[4..], ["Card"]);
        assert!(row(&tree, "/card").expanded);
        assert_eq!(row(&tree, "/card").subfolders, Subfolders::Unknown);
        assert_eq!(asked(&tree), paths(&["/card", "/card/DCIM", "/r", "/r/a"]));
        list(&mut tree, "/card", folders(&["/card/DCIM"]));
        list(&mut tree, "/card/DCIM", folders(&["/card/DCIM/101"]));
        assert_eq!(outline(&tree)[4..], ["Card", "  DCIM", "    101"]);
    }

    /// A card taken out while it was being listed.
    #[test]
    fn a_listing_whose_root_has_gone_is_dropped() {
        let mut tree = tree(&["/card"]);
        tree.expand(Path::new("/card"));
        let request = tree.to_list().remove(0);
        tree.set_roots(Vec::new());
        tree.listed(&request, folders(&["/card/DCIM"]));
        tree.set_roots(vec![Root::new(Place::Volume, "/card", "/card")]);
        assert_eq!(outline(&tree), ["/card"]);
        assert_eq!(asked(&tree), paths(&["/card"]));
    }

    /// Another card of the same name, mounted where the one taken out
    /// was, is asked for anew, and a listing of the first one handed back
    /// late is not taken for its own.
    #[test]
    fn a_listing_of_a_card_taken_out_is_not_taken_for_the_next_one_s() {
        let card = || Root::new(Place::Volume, "EOS_DIGITAL", "/card");
        let mut tree = FolderTree::new(vec![card()]);
        tree.expand(Path::new("/card"));
        let first = tree.to_list().remove(0);
        tree.set_roots(Vec::new());
        tree.set_roots(vec![card()]);
        let second = tree.to_list().remove(0);
        assert_ne!(second, first);
        tree.listed(&first, folders(&["/card/DCIM"]));
        assert_eq!(asked(&tree), paths(&["/card"]));
        tree.listed(&second, folders(&["/card/DCIM", "/card/MISC"]));
        assert!(asked(&tree).is_empty());
        assert_eq!(outline(&tree), ["EOS_DIGITAL", "  DCIM", "  MISC"]);
    }

    /// Roots that only come, as a card is put in, leave the listings of
    /// the others fresh.
    #[test]
    fn a_root_that_comes_leaves_the_others_listed() {
        let mut tree = tree(&["/r"]);
        tree.expand(Path::new("/r"));
        list(&mut tree, "/r", folders(&["/r/a"]));
        tree.set_roots(vec![
            Root::new(Place::Home, "/r", "/r"),
            Root::new(Place::Volume, "/card", "/card"),
        ]);
        assert!(asked(&tree).is_empty());
    }

    /// The same folder under two roots, as the home folder is under `C:`
    /// on Windows, is shown under both and listed once.
    #[test]
    fn a_folder_shown_twice_is_listed_once() {
        let mut tree = tree(&["/r/a", "/r"]);
        tree.expand(Path::new("/r"));
        list(&mut tree, "/r", folders(&["/r/a"]));
        tree.expand(Path::new("/r/a"));
        assert_eq!(asked(&tree), paths(&["/r/a"]));
        list(&mut tree, "/r/a", folders(&["/r/a/x"]));
        assert_eq!(outline(&tree), ["/r/a", "  x", "/r", "  a", "    x"]);
    }
}
