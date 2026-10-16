// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding on worker threads, so that a window never waits for a photo:
//! [`Decoder`] takes what its caller wants decoded, decodes the most
//! wanted first, and hands back each [`Answer`] when it is ready.
//!
//! The caller names what each request is for by a slot of its own, such
//! as a cell of a grid, and wants it again as often as it likes: a slot
//! has at most one request queued, and wanting what is already queued,
//! under way or answered and not yet taken costs nothing, so a window can
//! want every cell it shows on every frame. The caller ranks requests by
//! priority. Among requests of one priority, what was first wanted last
//! comes first, as a photographer's latest move is what they are waiting
//! for; wanting a request again keeps its place, so a window wanting the
//! same cells every frame has them decoded in the order it first wanted
//! them.
//!
//! The workers start with the first request, not with the decoder, and
//! stop when it is dropped, each after the decode it is in, without the
//! decoder waiting for them. A decode can hang, on a network share that
//! has gone, and hold its worker until the system gives up on the share.
//! The answers of the decodes under way when the decoder is cleared are
//! never handed out, so a worker held by one is replaced while it is
//! held, and leaves once it is free: what is wanted after a clear is
//! decoded even when every worker hangs on what was wanted before.

use std::collections::HashMap;
use std::hash::Hash;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;

use crate::decode::{Decoded, decode};
use crate::failure::Failure;

/// The most workers a decoder starts by default. Past a few, decoding
/// waits on the disk more than on the cores, and a window needs a core
/// of its own.
const MAX_WORKERS: usize = 4;

/// What decodes a photo: [`decode`], or a stand-in in the tests.
type Decode = Arc<dyn Fn(&Path, u32) -> Result<Decoded, Failure> + Send + Sync>;

/// What tells the caller that an answer is ready.
type Wake = Arc<dyn Fn() + Send + Sync>;

/// A photo decoded for a slot, or why it could not be.
///
/// It carries what was asked, the path and the longest side, so that a
/// caller that wanted something else for the slot since can tell an
/// answer to an earlier request from the one it is waiting for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer<S> {
    /// The slot the photo was wanted for.
    pub slot: S,
    /// The photo's file.
    pub path: PathBuf,
    /// The longest side asked for.
    pub longest: u32,
    /// The photo decoded, or why it could not be.
    pub result: Result<Decoded, Failure>,
}

/// Photos decoded on worker threads, the most wanted first.
///
/// `S` is the caller's name for what a request is for, such as the index
/// of a grid's cell: each slot has at most one request queued.
///
/// Each worker decodes one photo at a time, and decoding one may take as
/// much memory as `decode` allows a photo.
pub struct Decoder<S> {
    shared: Arc<Shared<S>>,
    run: Run,
}

/// Where a [`Decoder`] decodes.
enum Run {
    /// On worker threads.
    Threads,
    /// On the caller's thread, inside [`Decoder::answers`].
    Inline,
}

/// What the decoder and its workers share.
struct Shared<S> {
    state: Mutex<State<S>>,
    /// Signalled when a request is queued, or the decoder is dropped.
    wanted: Condvar,
    decode: Decode,
    wake: Wake,
}

/// The requests, the answers and the workers, under the lock.
struct State<S> {
    /// The requests waiting for a worker, one a slot.
    queue: HashMap<S, Queued>,
    /// The requests being decoded, each with the generation it was taken
    /// in.
    running: Vec<(Request<S>, u64)>,
    /// The answers not yet handed out, all of the current generation.
    answers: Vec<Answer<S>>,
    /// Raised by [`Decoder::clear`]: the answer of a request taken in an
    /// earlier generation is never handed out.
    generation: u64,
    /// The number of the last request queued, so that a later one has a
    /// greater number.
    wanted: u64,
    /// How many workers the decoder keeps for the current generation,
    /// fixed when it is made: it starts workers until this many are free
    /// of decodes of an earlier generation.
    workers: usize,
    /// How many workers are running, held or not.
    live: usize,
    /// Whether the decoder was dropped, and the workers are to stop.
    stopped: bool,
}

/// What is wanted for a slot.
#[derive(Clone, PartialEq, Eq)]
struct Request<S> {
    slot: S,
    path: PathBuf,
    longest: u32,
}

/// A request waiting for a worker, without its slot, which keys it.
struct Queued {
    path: PathBuf,
    longest: u32,
    priority: u32,
    /// When it was queued: the greater, the later.
    wanted: u64,
}

impl<S: Clone + Eq + Hash + Send + 'static> Decoder<S> {
    /// A decoder on as many workers as the machine has cores, less one
    /// for the window, and at least one and at most four. `wake` is
    /// called, from a worker, each time an answer is ready.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let cores = thread::available_parallelism().map_or(1, |cores| cores.get());
        Self::with_workers(workers_for(cores), wake)
    }

    /// A decoder on `workers` workers, at least one. `wake` is called,
    /// from a worker, each time an answer is ready.
    pub fn with_workers(workers: usize, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with(Run::Threads, workers, Arc::new(decode), Arc::new(wake))
    }

    /// A decoder with no threads: what is wanted is decoded on the
    /// caller's thread inside [`answers`](Self::answers), in the order
    /// workers would take it, so a test sees its answers at once.
    pub fn inline() -> Self {
        Self::with(Run::Inline, 0, Arc::new(decode), Arc::new(|| {}))
    }

    /// A decoder running as `run` says, on `workers` workers, at least
    /// one, decoding with `decode`.
    fn with(run: Run, workers: usize, decode: Decode, wake: Wake) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    queue: HashMap::new(),
                    running: Vec::new(),
                    answers: Vec::new(),
                    generation: 0,
                    wanted: 0,
                    workers: workers.max(1),
                    live: 0,
                    stopped: false,
                }),
                wanted: Condvar::new(),
                decode,
                wake,
            }),
            run,
        }
    }

    /// Wants the photo at `path` for `slot`, its longest side at most
    /// `longest`, before anything of a lower `priority`, and before
    /// anything of the same priority first wanted earlier.
    ///
    /// It replaces what was queued for the slot. Wanting again what is
    /// queued for it only sets its priority: its place among requests of
    /// one priority is the one it took when first wanted. What is being
    /// decoded for the slot already, or decoded and not yet taken from
    /// [`answers`](Self::answers), is not queued again, so a caller can
    /// want the same things every frame; once taken, the answer is the
    /// caller's to keep, and wanting it again decodes it again. Something
    /// else wanted for a slot whose decode is under way is queued all the
    /// same, and the two may be decoded at once: the answers carry the
    /// path and the longest side, which tell them apart.
    pub fn want(&mut self, slot: S, path: impl AsRef<Path>, longest: u32, priority: u32) {
        let path = path.as_ref();
        let queued = {
            let mut state = lock(&self.shared.state);
            state.want(slot, path, longest, priority)
        };
        if queued {
            self.shared.wanted.notify_one();
        }
        self.start_workers();
    }

    /// Drops the queued requests of the slots `keep` does not keep, such
    /// as cells scrolled out of view. Those under way are decoded all the
    /// same, and answered.
    pub fn retain(&mut self, mut keep: impl FnMut(&S) -> bool) {
        lock(&self.shared.state).queue.retain(|slot, _| keep(slot));
    }

    /// Drops every queued request and every answer not yet handed out,
    /// and the answers of the decodes under way are never handed out:
    /// for when what was wanted no longer is, as when another folder is
    /// chosen. The decodes under way run to their end, and the workers
    /// they hold are replaced on the next request.
    pub fn clear(&mut self) {
        let mut state = lock(&self.shared.state);
        state.queue.clear();
        state.answers.clear();
        state.generation += 1;
    }

    /// The answers ready since the last call, in the order they were
    /// ready. An inline decoder decodes everything wanted first.
    pub fn answers(&mut self) -> Vec<Answer<S>> {
        if let Run::Inline = self.run {
            loop {
                // Taken in a statement of its own, so the lock is
                // released before the decode.
                let next = lock(&self.shared.state).take();
                let Some(request) = next else { break };
                let result = decode_caught(&self.shared.decode, &request);
                lock(&self.shared.state)
                    .answers
                    .push(request.answer(result));
            }
        }
        mem::take(&mut lock(&self.shared.state).answers)
    }

    /// How many requests are queued or being decoded, not counting the
    /// decodes under way whose answers [`clear`](Self::clear) dropped:
    /// how many answers are still to come.
    pub fn pending(&self) -> usize {
        let state = lock(&self.shared.state);
        let running = state
            .running
            .iter()
            .filter(|(_, taken)| *taken == state.generation)
            .count();
        state.queue.len() + running
    }

    /// Starts as many workers as it takes to have the decoder's number of
    /// them free of decodes whose answers were dropped, and no more than
    /// twice that number in all.
    ///
    /// The bound is for a network share that has gone, which holds every
    /// decode of a photo on it: the workers held are replaced once, but
    /// past that more would most likely be held by the same share. They
    /// are free again, and leave, when the system gives up on it. A worker
    /// the system will not start is tried again on the next request.
    fn start_workers(&mut self) {
        if let Run::Inline = self.run {
            return;
        }
        loop {
            {
                let mut state = lock(&self.shared.state);
                if state.free() >= state.workers || state.live >= 2 * state.workers {
                    return;
                }
                state.live += 1;
            }
            let shared = Arc::clone(&self.shared);
            let spawned = thread::Builder::new()
                .name("previews-decode".to_owned())
                .spawn(move || work(&shared));
            if spawned.is_err() {
                let mut state = lock(&self.shared.state);
                state.live = state.live.saturating_sub(1);
                return;
            }
        }
    }
}

/// Stops the workers: what is queued is dropped, and so are the answers
/// not taken, and each worker stops once the decode it is in ends, its
/// answer dropped. Nothing waits for them, so a decode stuck on a network
/// share that has gone does not hold up the caller, a window closing.
impl<S> Drop for Decoder<S> {
    fn drop(&mut self) {
        {
            let mut state = lock(&self.shared.state);
            state.stopped = true;
            state.queue.clear();
            state.answers.clear();
        }
        self.shared.wanted.notify_all();
    }
}

impl<S: Clone + Eq + Hash> State<S> {
    /// Wants what [`Decoder::want`] wants; whether it queued something a
    /// worker can take.
    fn want(&mut self, slot: S, path: &Path, longest: u32, priority: u32) -> bool {
        let generation = self.generation;
        let same = |s: &S, p: &Path, l: u32| *s == slot && p == path && l == longest;
        if self.running.iter().any(|(running, taken)| {
            *taken == generation && same(&running.slot, &running.path, running.longest)
        }) || self
            .answers
            .iter()
            .any(|answer| same(&answer.slot, &answer.path, answer.longest))
        {
            // Under way or answered already: whatever else was queued for
            // the slot was wanted before this, and is wanted no longer.
            self.queue.remove(&slot);
            return false;
        }
        if let Some(queued) = self.queue.get_mut(&slot)
            && queued.path == path
            && queued.longest == longest
        {
            queued.priority = priority;
            return false;
        }
        self.wanted += 1;
        let queued = Queued {
            path: path.to_path_buf(),
            longest,
            priority,
            wanted: self.wanted,
        };
        self.queue.insert(slot, queued);
        true
    }

    /// Takes the most wanted request off the queue: the highest priority,
    /// and of those the latest queued.
    fn take(&mut self) -> Option<Request<S>> {
        let slot = self
            .queue
            .iter()
            .max_by_key(|(_, queued)| (queued.priority, queued.wanted))
            .map(|(slot, _)| slot.clone())?;
        let queued = self.queue.remove(&slot)?;
        Some(Request {
            slot,
            path: queued.path,
            longest: queued.longest,
        })
    }

    /// How many of the running workers are free of decodes whose answers
    /// [`Decoder::clear`] dropped.
    fn free(&self) -> usize {
        let held = self
            .running
            .iter()
            .filter(|(_, taken)| *taken != self.generation)
            .count();
        self.live.saturating_sub(held)
    }
}

impl<S> Request<S> {
    /// The answer to this request.
    fn answer(self, result: Result<Decoded, Failure>) -> Answer<S> {
        Answer {
            slot: self.slot,
            path: self.path,
            longest: self.longest,
            result,
        }
    }
}

/// A worker: it takes the most wanted request, decodes it and answers,
/// until the decoder is dropped, or until it ends a decode whose answer
/// was dropped and the decoder has workers enough without it.
fn work<S: Clone + Eq + Hash>(shared: &Shared<S>) {
    // Counts the worker out if it unwinds; declared first, so the
    // state's lock is released before it is dropped.
    let mut leaving = Leaving {
        shared,
        counted_out: false,
    };
    loop {
        let (request, generation) = {
            let mut state = lock(&shared.state);
            loop {
                if state.stopped {
                    return leaving.count_out(&mut state);
                }
                if let Some(request) = state.take() {
                    let generation = state.generation;
                    state.running.push((request.clone(), generation));
                    break (request, generation);
                }
                state = shared
                    .wanted
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let result = decode_caught(&shared.decode, &request);
        {
            let mut state = lock(&shared.state);
            if let Some(at) = state
                .running
                .iter()
                .position(|(running, taken)| *taken == generation && *running == request)
            {
                state.running.swap_remove(at);
            }
            if state.stopped {
                return leaving.count_out(&mut state);
            }
            if generation != state.generation {
                // A worker started in this one's place while it was held
                // leaves it one too many. Counted out under the lock, so
                // that another worker deciding the same counts it gone.
                if state.free() > state.workers {
                    return leaving.count_out(&mut state);
                }
                continue;
            }
            state.answers.push(request.answer(result));
        }
        // With the lock released, so the caller can take the answer at
        // once. The caller's code: a panic in it costs nothing here, and
        // the answer waits for the next look at the answers.
        let _ = panic::catch_unwind(AssertUnwindSafe(|| (shared.wake)()));
    }
}

/// A worker's way out of the running ones: counted out under the lock
/// when it decides to leave, or when dropped if it unwinds instead.
struct Leaving<'a, S> {
    shared: &'a Shared<S>,
    counted_out: bool,
}

impl<S> Leaving<'_, S> {
    /// Counts the worker out of `state`, the shared state locked.
    fn count_out(&mut self, state: &mut State<S>) {
        state.live = state.live.saturating_sub(1);
        self.counted_out = true;
    }
}

impl<S> Drop for Leaving<'_, S> {
    fn drop(&mut self) {
        if !self.counted_out {
            let mut state = lock(&self.shared.state);
            state.live = state.live.saturating_sub(1);
        }
    }
}

/// `request` decoded by `decode`, a panic taken for a damaged file.
///
/// A decoder that panics has a bug, met on this file's bytes: the file
/// is the one thing known to be wrong, and it costs that file alone,
/// never a worker and with it the decoding of every photo after it.
fn decode_caught<S>(decode: &Decode, request: &Request<S>) -> Result<Decoded, Failure> {
    panic::catch_unwind(AssertUnwindSafe(|| decode(&request.path, request.longest)))
        .unwrap_or(Err(Failure::Damaged))
}

/// The state, even if a thread panicked holding it: nothing is left half
/// done under the lock, so what it holds is whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How many workers to start on a machine of `cores` cores: one for each
/// but the window's, at least one and at most [`MAX_WORKERS`].
fn workers_for(cores: usize) -> usize {
    cores.saturating_sub(1).clamp(1, MAX_WORKERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{self, GREEN, RED};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
    use std::time::Duration;

    /// How long a test waits for what a worker does before it fails: a
    /// safety net, never a pause; everything comes long before.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// What a stand-in decode answers: a photo of no preview, whatever
    /// the path; the answer's path tells the requests apart.
    const ANSWERED: Result<Decoded, Failure> = Err(Failure::NoPreview);

    /// A stand-in decode that answers [`ANSWERED`] at once, and panics on
    /// a path named `panic`.
    fn instant() -> Decode {
        Arc::new(|path, _| {
            assert!(path != Path::new("panic"), "a bug in a decoder");
            ANSWERED
        })
    }

    /// A stand-in decode held by the test: it tells `started` the path of
    /// each photo it starts on, then waits for a go on `go` before it
    /// answers [`ANSWERED`].
    struct Held {
        started: Receiver<PathBuf>,
        go: Sender<()>,
    }

    impl Held {
        /// The decode, and its handle.
        fn new() -> (Decode, Held) {
            let (started_tx, started) = mpsc::channel();
            let (go, go_rx) = mpsc::channel::<()>();
            let go_rx = Mutex::new(go_rx);
            let decode: Decode = Arc::new(move |path, _| {
                let _ = started_tx.send(path.to_path_buf());
                let _ = lock(&go_rx).recv_timeout(PATIENCE);
                ANSWERED
            });
            (decode, Held { started, go })
        }

        /// The path of the next photo started.
        fn next_started(&self) -> PathBuf {
            self.started.recv_timeout(PATIENCE).unwrap()
        }

        /// Lets one held decode answer.
        fn go(&self) {
            self.go.send(()).unwrap();
        }
    }

    /// A wake that counts its calls and sends one tick for each.
    fn counted() -> (Wake, Arc<AtomicUsize>, Receiver<()>) {
        let count = Arc::new(AtomicUsize::new(0));
        let (tick, ticks) = mpsc::channel();
        let counting = Arc::clone(&count);
        let wake: Wake = Arc::new(move || {
            counting.fetch_add(1, Ordering::SeqCst);
            let _ = tick.send(());
        });
        (wake, count, ticks)
    }

    /// A decoder of one worker decoding with `decode`, and its wake's
    /// count and ticks.
    fn one_worker<S>(decode: Decode) -> (Decoder<S>, Arc<AtomicUsize>, Receiver<()>)
    where
        S: Clone + Eq + Hash + Send + 'static,
    {
        workers(1, decode)
    }

    /// A decoder of `n` workers decoding with `decode`, and its wake's
    /// count and ticks.
    fn workers<S>(n: usize, decode: Decode) -> (Decoder<S>, Arc<AtomicUsize>, Receiver<()>)
    where
        S: Clone + Eq + Hash + Send + 'static,
    {
        let (wake, count, ticks) = counted();
        (Decoder::with(Run::Threads, n, decode, wake), count, ticks)
    }

    /// How many workers of `decoder` are running.
    fn live<S>(decoder: &Decoder<S>) -> usize {
        lock(&decoder.shared.state).live
    }

    /// Waits until `decoder` has `n` workers running, or fails. A worker
    /// leaves on its own thread and tells no one, so this looks again and
    /// again, never longer than the test's patience.
    fn until_live<S>(decoder: &Decoder<S>, n: usize) {
        let start = std::time::Instant::now();
        while live(decoder) != n {
            assert!(
                start.elapsed() < PATIENCE,
                "{} workers, not {n}",
                live(decoder)
            );
            thread::yield_now();
        }
    }

    /// An inline decoder decoding with [`instant`].
    fn inline() -> Decoder<u32> {
        Decoder::with(Run::Inline, 0, instant(), Arc::new(|| {}))
    }

    /// The answers that `ticks` says `n` of are ready, waited for.
    fn wait_for(decoder: &mut Decoder<u32>, ticks: &Receiver<()>, n: usize) -> Vec<Answer<u32>> {
        for _ in 0..n {
            ticks.recv_timeout(PATIENCE).unwrap();
        }
        decoder.answers()
    }

    /// The paths of `answers`, in order, as text.
    fn paths(answers: &[Answer<u32>]) -> Vec<&str> {
        answers
            .iter()
            .map(|answer| answer.path.to_str().unwrap())
            .collect()
    }

    #[test]
    fn the_most_wanted_is_decoded_first_and_the_latest_among_equals() {
        let mut decoder = inline();
        decoder.want(1, "a", 256, 1);
        decoder.want(2, "b", 256, 2);
        decoder.want(3, "c", 256, 1);
        decoder.want(4, "d", 256, 0);
        assert_eq!(decoder.pending(), 4);
        assert_eq!(paths(&decoder.answers()), ["b", "c", "a", "d"]);
        assert_eq!(decoder.pending(), 0);
        assert!(decoder.answers().is_empty());
    }

    #[test]
    fn wanting_for_a_slot_replaces_what_was_queued_for_it() {
        let mut decoder = inline();
        decoder.want(1, "a", 256, 0);
        decoder.want(1, "b", 512, 0);
        assert_eq!(decoder.pending(), 1);
        let answers = decoder.answers();
        assert_eq!(paths(&answers), ["b"]);
        assert_eq!((answers[0].slot, answers[0].longest), (1, 512));
    }

    /// Wanting again what is queued keeps the place it took when first
    /// wanted, so a window wanting the same cells every frame has them in
    /// the order it first wanted them; only its priority changes. Another
    /// request for the slot takes a new place.
    #[test]
    fn wanting_what_is_queued_again_keeps_its_place_and_sets_its_priority() {
        let mut decoder = inline();
        decoder.want(1, "a", 256, 0);
        decoder.want(2, "b", 256, 0);
        decoder.want(1, "a", 256, 0);
        assert_eq!(paths(&decoder.answers()), ["b", "a"]);
        decoder.want(1, "a", 256, 0);
        decoder.want(2, "b", 256, 0);
        decoder.want(1, "a", 256, 1);
        assert_eq!(paths(&decoder.answers()), ["a", "b"]);
        decoder.want(1, "a", 256, 0);
        decoder.want(2, "b", 256, 0);
        decoder.want(1, "c", 256, 0);
        assert_eq!(paths(&decoder.answers()), ["c", "b"]);
    }

    #[test]
    fn retain_drops_the_queued_requests_of_the_slots_not_kept() {
        let mut decoder = inline();
        for slot in 0..6 {
            decoder.want(slot, format!("p{slot}"), 256, 0);
        }
        decoder.retain(|slot| slot % 2 == 0);
        assert_eq!(decoder.pending(), 3);
        assert_eq!(paths(&decoder.answers()), ["p4", "p2", "p0"]);
    }

    #[test]
    fn clear_drops_what_is_queued() {
        let mut decoder = inline();
        decoder.want(1, "a", 256, 0);
        decoder.want(2, "b", 256, 0);
        decoder.clear();
        assert_eq!(decoder.pending(), 0);
        assert!(decoder.answers().is_empty());
    }

    /// One worker held on its first photo takes the others in order once
    /// it is let go.
    #[test]
    fn workers_take_the_most_wanted_first() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(0, "first", 256, 0);
        assert_eq!(held.next_started(), Path::new("first"));
        decoder.want(1, "x", 256, 1);
        decoder.want(2, "y", 256, 2);
        decoder.want(3, "z", 256, 1);
        assert_eq!(decoder.pending(), 4);
        for _ in 0..4 {
            held.go();
        }
        let mut answers = Vec::new();
        for _ in 0..4 {
            answers.extend(wait_for(&mut decoder, &ticks, 1));
        }
        assert_eq!(paths(&answers), ["first", "y", "z", "x"]);
        assert!(answers.iter().all(|answer| answer.result == ANSWERED));
        assert_eq!(decoder.pending(), 0);
    }

    /// What is being decoded for a slot already is not queued again
    /// however often it is wanted, even above everything else, so the
    /// next photo started is another.
    #[test]
    fn wanting_what_is_under_way_queues_nothing() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        for _ in 0..3 {
            decoder.want(1, "a", 256, 9);
        }
        assert_eq!(decoder.pending(), 1);
        decoder.want(2, "b", 256, 0);
        held.go();
        assert_eq!(held.next_started(), Path::new("b"));
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 2)), ["a", "b"]);
    }

    /// Something else wanted for a slot whose decode is under way is
    /// queued, and both are answered, told apart by their paths.
    #[test]
    fn another_request_for_a_slot_under_way_is_queued() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        decoder.want(1, "b", 256, 0);
        assert_eq!(decoder.pending(), 2);
        held.go();
        held.go();
        let answers = wait_for(&mut decoder, &ticks, 2);
        assert_eq!(paths(&answers), ["a", "b"]);
        assert!(answers.iter().all(|answer| answer.slot == 1));
    }

    /// Wanting again for a slot what is under way for it drops what was
    /// queued for the slot since: the latest want is what the slot wants.
    #[test]
    fn wanting_again_what_is_under_way_drops_what_was_queued_since() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        decoder.want(1, "b", 256, 0);
        decoder.want(1, "a", 256, 0);
        assert_eq!(decoder.pending(), 1);
        decoder.want(2, "c", 256, 0);
        held.go();
        assert_eq!(held.next_started(), Path::new("c"));
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 2)), ["a", "c"]);
    }

    /// An answer ready and not yet taken is not decoded again when its
    /// request is wanted again, even above everything else, as a window
    /// does on the frame before it takes the answers.
    #[test]
    fn wanting_what_is_answered_and_not_taken_queues_nothing() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        held.go();
        ticks.recv_timeout(PATIENCE).unwrap();
        decoder.want(1, "a", 256, 9);
        assert_eq!(decoder.pending(), 0);
        decoder.want(2, "b", 256, 0);
        assert_eq!(held.next_started(), Path::new("b"));
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 1)), ["a", "b"]);
    }

    /// Two workers both held by decodes a clear dropped the answers of,
    /// as on a network share that has gone: two more are started for
    /// what is wanted next, and once the held ones are let go they leave.
    #[test]
    fn what_is_wanted_after_a_clear_is_decoded_while_every_worker_hangs() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = workers(2, decode);
        decoder.want(1, "a", 256, 0);
        decoder.want(2, "b", 256, 0);
        let mut started = [held.next_started(), held.next_started()];
        started.sort();
        assert_eq!(started, [Path::new("a"), Path::new("b")]);
        decoder.clear();
        decoder.want(1, "c", 256, 0);
        decoder.want(2, "d", 256, 0);
        let mut started = [held.next_started(), held.next_started()];
        started.sort();
        assert_eq!(started, [Path::new("c"), Path::new("d")]);
        assert_eq!(live(&decoder), 4);
        for _ in 0..4 {
            held.go();
        }
        let mut answers = wait_for(&mut decoder, &ticks, 2);
        answers.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(paths(&answers), ["c", "d"]);
        until_live(&decoder, 2);
    }

    /// Clears over a share that holds every decode replace the held
    /// workers once at most, so they cannot pile up: a second clear
    /// starts no third worker, and what is wanted waits for one to be
    /// free.
    #[test]
    fn the_workers_held_after_clears_are_replaced_once_at_most() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        held.next_started();
        decoder.clear();
        decoder.want(1, "b", 256, 0);
        assert_eq!(held.next_started(), Path::new("b"));
        decoder.clear();
        decoder.want(1, "c", 256, 0);
        assert_eq!(live(&decoder), 2);
        assert_eq!(decoder.pending(), 1);
        assert_eq!(held.started.try_recv(), Err(mpsc::TryRecvError::Empty));
        // One held decode let go frees its worker, which takes "c".
        held.go();
        assert_eq!(held.next_started(), Path::new("c"));
        held.go();
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 1)), ["c"]);
        until_live(&decoder, 1);
    }

    #[test]
    fn retain_lets_a_decode_under_way_answer() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        decoder.want(2, "b", 256, 0);
        decoder.retain(|_| false);
        assert_eq!(decoder.pending(), 1);
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 1)), ["a"]);
        assert_eq!(decoder.pending(), 0);
    }

    /// The answer of a decode under way when the decoder is cleared is
    /// never handed out, nor does it wake the caller; an answer ready but
    /// not taken is dropped too.
    #[test]
    fn clear_drops_the_answers_of_decodes_under_way() {
        let (decode, held) = Held::new();
        let (mut decoder, count, ticks) = one_worker(decode);
        decoder.want(1, "ready", 256, 0);
        held.next_started();
        held.go();
        ticks.recv_timeout(PATIENCE).unwrap();
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        decoder.clear();
        assert_eq!(decoder.pending(), 0);
        // Wanted again after the clear, it is queued again: the decode
        // under way will not answer it.
        decoder.want(1, "a", 256, 0);
        assert_eq!(decoder.pending(), 1);
        held.go();
        assert_eq!(held.next_started(), Path::new("a"));
        held.go();
        assert_eq!(paths(&wait_for(&mut decoder, &ticks, 1)), ["a"]);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_caller_is_woken_once_for_each_answer() {
        let (mut decoder, count, ticks) = one_worker(instant());
        for slot in 0..3 {
            decoder.want(slot, format!("p{slot}"), 256, 0);
        }
        assert_eq!(wait_for(&mut decoder, &ticks, 3).len(), 3);
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    /// A decode that panics answers that the file is damaged, and the one
    /// worker goes on to the next photo.
    #[test]
    fn a_decode_that_panics_answers_damaged_and_the_worker_goes_on() {
        let (mut decoder, _, ticks) = one_worker(instant());
        decoder.want(1, "panic", 256, 1);
        decoder.want(2, "after", 256, 0);
        let answers = wait_for(&mut decoder, &ticks, 2);
        assert_eq!(paths(&answers), ["panic", "after"]);
        assert_eq!(answers[0].result, Err(Failure::Damaged));
        assert_eq!(answers[1].result, ANSWERED);
        let mut decoder = inline();
        decoder.want(1, "panic", 256, 0);
        assert_eq!(decoder.answers()[0].result, Err(Failure::Damaged));
    }

    /// Dropping the decoder returns while a decode is held, drops what
    /// is queued, and the worker stops once its decode ends: the stand-in
    /// decode, its last holder gone, then starts nothing more.
    #[test]
    fn dropping_the_decoder_waits_for_no_decode_and_drops_the_queue() {
        let (decode, held) = Held::new();
        let (mut decoder, count, _) = one_worker(decode);
        decoder.want(1, "a", 256, 0);
        assert_eq!(held.next_started(), Path::new("a"));
        decoder.want(2, "b", 256, 0);
        let (dropped, done) = mpsc::channel();
        thread::spawn(move || {
            drop(decoder);
            dropped.send(()).unwrap();
        });
        done.recv_timeout(PATIENCE).unwrap();
        held.go();
        assert_eq!(
            held.started.recv_timeout(PATIENCE),
            Err(RecvTimeoutError::Disconnected)
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    /// The caller's wake panicking costs the worker nothing: the answer
    /// is there, and the worker goes on to the next photo.
    #[test]
    fn a_wake_that_panics_costs_nothing() {
        let calls = AtomicUsize::new(0);
        let (tick, ticks) = mpsc::channel();
        let wake: Wake = Arc::new(move || {
            assert!(calls.fetch_add(1, Ordering::SeqCst) > 0, "a bug in a wake");
            let _ = tick.send(());
        });
        let mut decoder = Decoder::with(Run::Threads, 1, instant(), wake);
        decoder.want(1, "a", 256, 1);
        decoder.want(2, "b", 256, 0);
        let answers = wait_for(&mut decoder, &ticks, 1);
        let mut answers = [answers, decoder.answers()].concat();
        answers.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(paths(&answers), ["a", "b"]);
    }

    /// The answers not taken are dropped with the decoder, even while a
    /// worker is held, so the images they hold are not kept.
    #[test]
    fn dropping_the_decoder_drops_the_answers_not_taken() {
        let (decode, held) = Held::new();
        let (mut decoder, _, ticks) = one_worker::<Arc<u32>>(decode);
        let slot = Arc::new(1);
        let answered = Arc::downgrade(&slot);
        decoder.want(slot, "a", 256, 0);
        held.next_started();
        held.go();
        ticks.recv_timeout(PATIENCE).unwrap();
        decoder.want(Arc::new(2), "held", 256, 0);
        assert_eq!(held.next_started(), Path::new("held"));
        drop(decoder);
        assert!(answered.upgrade().is_none());
        held.go();
    }

    #[test]
    fn workers_start_with_the_first_request() {
        let (mut decoder, _, ticks) = one_worker(instant());
        assert_eq!(lock(&decoder.shared.state).live, 0);
        decoder.want(1, "a", 256, 0);
        assert_eq!(lock(&decoder.shared.state).live, 1);
        wait_for(&mut decoder, &ticks, 1);
    }

    #[test]
    fn a_machine_keeps_a_core_for_the_window_and_starts_one_to_four_workers() {
        let workers: Vec<usize> = [1, 2, 3, 5, 16].into_iter().map(workers_for).collect();
        assert_eq!(workers, [1, 1, 2, 4, 4]);
    }

    /// The real decode, on real threads, over a JPEG on disk.
    #[test]
    fn a_photo_is_decoded_on_a_worker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.jpg");
        std::fs::write(&path, fixtures::jpeg(400, 300, RED, GREEN, None)).unwrap();
        let (tick, ticks) = mpsc::channel();
        let mut decoder = Decoder::new(move || {
            let _ = tick.send(());
        });
        decoder.want("cell", path.clone(), 100, 0);
        ticks.recv_timeout(PATIENCE).unwrap();
        let answers = decoder.answers();
        assert_eq!(answers.len(), 1);
        let answer = &answers[0];
        assert_eq!(
            (answer.slot, &answer.path, answer.longest),
            ("cell", &path, 100)
        );
        let decoded = answer.result.as_ref().unwrap();
        let image = decoded.image();
        assert_eq!((image.width(), image.height()), (100, 75));
    }
}
