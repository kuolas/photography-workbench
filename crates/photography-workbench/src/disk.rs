// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading the disk off the window's thread: the roots of the folder tree
//! and a folder's subfolders, which `catalog` reads where its caller
//! decides.
//!
//! A read can take a while, on a network share or a drive that has to
//! wake up, and a window that waited for it would stop answering. So
//! [`Disk::start`] hands each [`Job`] to a thread of its own, and the
//! window takes the answers, each a [`Done`], from [`Disk::finished`] at
//! the start of a frame. A thread asks for a frame when it is done, so
//! its answer shows without the pointer having to move.
//!
//! The roots are watched rather than read when asked for: memory cards
//! and drives come and go, and no system tells a portable app at once.
//! [`Disk::watch_roots`] starts a thread of their own, which reads them
//! every [`ROOTS_EVERY`] and wakes the window only when they change, so
//! an idle window draws nothing.

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use catalog::folders::{self, Folder, ListRequest, Root};

/// How often the roots are read while they are watched. No system tells
/// a portable app at once that a card was put in, so the app looks for
/// it itself.
const ROOTS_EVERY: Duration = Duration::from_secs(2);

/// What to read from disk.
#[derive(Debug)]
pub enum Job {
    /// The subfolders of the folder the request names.
    Subfolders(ListRequest),
}

/// What a [`Job`] read: its answer, with the request it answers; or the
/// roots, as they are watched.
#[derive(Debug)]
pub enum Done {
    /// The roots of the tree: the home folder and the mounted volumes,
    /// as first read, and then each time they change.
    Roots(Vec<Root>),
    /// The subfolders of the folder the request names, or why it could
    /// not be listed.
    Subfolders(ListRequest, io::Result<Vec<Folder>>),
}

/// Where the roots of the tree are read from.
type RootsSource = Arc<dyn Fn() -> Vec<Root> + Send + Sync>;

/// The disk, read off the window's thread: jobs go in with
/// [`start`](Self::start), the roots are watched from
/// [`watch_roots`](Self::watch_roots) on, and the answers come out of
/// [`finished`](Self::finished) in the order they were done.
pub struct Disk {
    /// Where the answers are sent.
    sender: Sender<Done>,
    /// Where they are taken from.
    receiver: Receiver<Done>,
    /// What reads the roots: [`folders::roots`], or a stand-in in the
    /// tests.
    roots: RootsSource,
    /// Where the jobs are done, and how the roots are watched.
    run: Run,
}

/// Where a [`Disk`] does its jobs, and how it watches the roots.
enum Run {
    /// Each job on a short-lived thread of its own, and the roots read
    /// every `every` by a long-lived thread of their own.
    Threads {
        /// How long the watcher waits between two reads of the roots.
        every: Duration,
    },
    /// Each job at once, on the thread that starts it, and the roots read
    /// again each time the answers are taken, so a test sees a change of
    /// them on the next frame without waiting.
    #[cfg(test)]
    Inline {
        /// The roots sent last; `None` until they are watched.
        sent: Option<Vec<Root>>,
    },
}

impl Default for Disk {
    /// The real disk: the roots read by [`folders::roots`] every
    /// [`ROOTS_EVERY`] once they are watched, each job on a thread of its
    /// own.
    fn default() -> Self {
        Self::threaded(Arc::new(folders::roots), ROOTS_EVERY)
    }
}

impl Disk {
    /// The real disk, as [`Disk::default`].
    pub fn new() -> Self {
        Self::default()
    }

    /// A disk reading its roots from `roots` every `every` once they are
    /// watched, each job on a thread of its own.
    fn threaded(roots: RootsSource, every: Duration) -> Self {
        Self::with(roots, Run::Threads { every })
    }

    /// A disk whose roots are those `roots` holds, running each job at
    /// once on the thread that starts it and looking at the roots each
    /// time the answers are taken, so a test sees its answers on the next
    /// frame and can change what is mounted.
    #[cfg(test)]
    pub fn inline(roots: Arc<Mutex<Vec<Root>>>) -> Self {
        let read = move || roots.lock().unwrap_or_else(PoisonError::into_inner).clone();
        Self::with(Arc::new(read), Run::Inline { sent: None })
    }

    /// A disk reading its roots from `roots`, doing its jobs as `run`
    /// says.
    fn with(roots: RootsSource, run: Run) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            roots,
            run,
        }
    }

    /// Start `job`. Its answer comes out of [`finished`](Self::finished)
    /// once it is done, and `ctx` is asked for a frame then.
    ///
    /// Each job has a short-lived thread of its own, rather than a place
    /// in a queue: a listing that hangs on a network share that has gone
    /// must not hold up the others. The caller keeps at most one job per
    /// folder under way at a time, so there are never more threads than
    /// folders being read.
    pub fn start(&self, ctx: &egui::Context, job: Job) {
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let name = match job {
            Job::Subfolders(_) => "disk-subfolders",
        };
        let work = move || {
            let done = match job {
                Job::Subfolders(request) => {
                    let result = folders::subfolders(request.path());
                    Done::Subfolders(request, result)
                }
            };
            // The receiver goes only with the disk, when the window does,
            // and then nobody is waiting for the answer.
            let _ = sender.send(done);
            ctx.request_repaint();
        };
        #[cfg(test)]
        if matches!(self.run, Run::Inline { .. }) {
            work();
            return;
        }
        // Kept here as well as handed to the thread, so it can still be
        // done if the thread cannot be made.
        let slot = Arc::new(Mutex::new(Some(work)));
        let for_thread = Arc::clone(&slot);
        let spawned = thread::Builder::new().name(name.to_owned()).spawn(move || {
            if let Some(work) = take(&for_thread) {
                work();
            }
        });
        // With no thread to be had, the system is short of memory or of
        // threads. The job is done here and now, holding up the window,
        // rather than never: its folder would otherwise wait for an
        // answer that never comes, and never be listed again.
        if spawned.is_err()
            && let Some(work) = take(&slot)
        {
            work();
        }
    }

    /// Watch the roots from now on. They come out of
    /// [`finished`](Self::finished) as first read, and again each time
    /// they change, and `ctx` is asked for a frame then; while they stay
    /// the same, nothing comes and no frame is asked for. Each call starts
    /// another watcher, so the caller makes one.
    pub fn watch_roots(&mut self, ctx: &egui::Context) {
        let roots = Arc::clone(&self.roots);
        let sender = self.sender.clone();
        match &mut self.run {
            Run::Threads { every } => {
                let (every, ctx) = (*every, ctx.clone());
                let (for_thread, sender_for_thread) = (Arc::clone(&roots), sender.clone());
                let spawned = thread::Builder::new()
                    .name("disk-roots".to_owned())
                    .spawn(move || watch(&*for_thread, every, &sender_for_thread, &ctx));
                // With no thread to be had, the system is short of memory
                // or of threads. The roots are read once, here, holding up
                // the window, so the tree has something to show rather than
                // nothing; a card put in later shows only once the app is
                // started again, which the log says.
                if let Err(error) = spawned {
                    log::warn!(
                        "the memory cards and drives could not be watched ({error}); \
                         those mounted from now on show once the app is started again"
                    );
                    let _ = sender.send(Done::Roots(roots()));
                }
            }
            #[cfg(test)]
            Run::Inline { sent } => {
                let read = roots();
                *sent = Some(read.clone());
                let _ = sender.send(Done::Roots(read));
            }
        }
    }

    /// The answers done since the last call, in the order they were done.
    /// Waits for none.
    pub fn finished(&mut self) -> impl Iterator<Item = Done> + '_ {
        // An inline disk looks at its roots here, as its watcher would
        // have every so often, and sends them on when they changed.
        #[cfg(test)]
        if let Run::Inline { sent: Some(sent) } = &mut self.run {
            let read = (self.roots)();
            if *sent != read {
                sent.clone_from(&read);
                let _ = self.sender.send(Done::Roots(read));
            }
        }
        self.receiver.try_iter()
    }

    /// The next answer, waiting up to `timeout` for it.
    #[cfg(test)]
    fn wait(&self, timeout: Duration) -> Option<Done> {
        self.receiver.recv_timeout(timeout).ok()
    }
}

/// Watch the roots `roots` reads: read them every `every`, and send them
/// to `sender`, asking `ctx` for a frame, the first time and then each
/// time they differ from those sent last, so an idle window is never
/// woken. There is no way out of it but the app ending, as cards and
/// drives come and go for as long as it runs; it stops only when it
/// cannot send, as the window has gone.
fn watch(
    roots: &dyn Fn() -> Vec<Root>,
    every: Duration,
    sender: &Sender<Done>,
    ctx: &egui::Context,
) {
    let mut sent = None;
    loop {
        let read = roots();
        if sent.as_ref() != Some(&read) {
            if sender.send(Done::Roots(read.clone())).is_err() {
                return;
            }
            ctx.request_repaint();
            sent = Some(read);
        }
        thread::sleep(every);
    }
}

/// What `slot` holds, taking it out, so it is done once.
fn take<T>(slot: &Mutex<Option<T>>) -> Option<T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner).take()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use catalog::folders::{FolderTree, Place};

    use super::*;

    /// A request for the subfolders of `dir`, the only root of a tree.
    fn request(dir: &Path) -> ListRequest {
        let mut tree = FolderTree::new(vec![Root::new(Place::Home, "Home", dir)]);
        tree.expand(dir);
        tree.to_list().remove(0)
    }

    /// A job on its own thread lists the folder, and its answer arrives
    /// with the request it answers.
    #[test]
    fn a_folder_is_listed_on_a_thread_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("DCIM")).unwrap();
        let mut disk = Disk::new();
        let request = request(dir.path());
        disk.start(&egui::Context::default(), Job::Subfolders(request.clone()));
        let done = disk.wait(Duration::from_secs(5));
        let Some(Done::Subfolders(answered, Ok(folders))) = done else {
            panic!("no listing: {done:?}");
        };
        assert_eq!(answered, request);
        assert_eq!(folders, [Folder::new(dir.path().join("DCIM"))]);
        assert!(disk.finished().next().is_none());
    }

    /// The roots' watcher sends them as first read, then nothing while
    /// they stay the same, however many times it reads them, and then
    /// the roots as they changed.
    #[test]
    fn the_roots_are_sent_first_and_then_only_when_they_change() {
        let mounted = Arc::new(Mutex::new(Vec::new()));
        let read = {
            let mounted = Arc::clone(&mounted);
            move || mounted.lock().unwrap().clone()
        };
        let every = Duration::from_millis(5);
        let mut disk = Disk::threaded(Arc::new(read), every);
        disk.watch_roots(&egui::Context::default());
        let first = disk.wait(Duration::from_secs(5));
        assert!(
            matches!(&first, Some(Done::Roots(roots)) if roots.is_empty()),
            "{first:?}"
        );
        let unchanged = disk.wait(every * 20);
        assert!(unchanged.is_none(), "sent again unchanged: {unchanged:?}");

        let card = Root::new(Place::Volume, "Card", "/card");
        mounted.lock().unwrap().push(card.clone());
        let changed = disk.wait(Duration::from_secs(5));
        assert!(
            matches!(&changed, Some(Done::Roots(roots)) if roots == std::slice::from_ref(&card)),
            "{changed:?}"
        );
        // With the disk gone, the watcher stops at its next send.
        drop(disk);
        mounted.lock().unwrap().clear();
    }

    /// An inline disk sends the roots as soon as they are watched, and
    /// again the next time its answers are taken once they changed, and
    /// not while they stay the same.
    #[test]
    fn an_inline_disk_sends_the_roots_when_they_change() {
        let card = Root::new(Place::Volume, "Card", "/card");
        let roots = Arc::new(Mutex::new(vec![card.clone()]));
        let mut disk = Disk::inline(Arc::clone(&roots));
        disk.watch_roots(&egui::Context::default());
        let done: Vec<_> = disk.finished().collect();
        assert!(
            matches!(&done[..], [Done::Roots(first)] if first == std::slice::from_ref(&card)),
            "{done:?}"
        );
        assert!(disk.finished().next().is_none());
        roots.lock().unwrap().clear();
        let done: Vec<_> = disk.finished().collect();
        assert!(
            matches!(&done[..], [Done::Roots(second)] if second.is_empty()),
            "{done:?}"
        );
        assert!(disk.finished().next().is_none());
    }
}
