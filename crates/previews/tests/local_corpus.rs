// SPDX-License-Identifier: GPL-3.0-or-later

//! Every photo of a folder of one's own decoded, as a check against real
//! cameras' files, which the repository cannot hold. It runs only when
//! asked for, with `PW_LOCAL_CORPUS` naming the folder:
//!
//! ```sh
//! PW_LOCAL_CORPUS=/path/to/photos cargo test -p previews --release \
//!     --test local_corpus -- --ignored --nocapture
//! ```
//!
//! Every file under the folder, at any depth, whose extension is that of
//! a format raw-files reads or a JPEG's, and whose name does not start
//! with a dot, is decoded as a thumbnail, on as many
//! threads as the machine has cores, and the test fails listing those that
//! could not be. A second test decodes each RAW file's largest preview
//! whole, and fails listing those whose size it told before differs from
//! the size decoded.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::{env, fs, thread};

/// The longer side of the thumbnails decoded.
const LONGEST: u32 = 512;

/// Whether a file of this extension, in any case, is a photo: as the
/// app's photo list tells them, one of a format raw-files reads, or a
/// JPEG.
fn is_photo(extension: &str) -> bool {
    is_raw(extension)
        || ["jpg", "jpeg"]
            .iter()
            .any(|jpeg| extension.eq_ignore_ascii_case(jpeg))
}

/// Whether a file of this extension, in any case, is of a format
/// raw-files reads.
fn is_raw(extension: &str) -> bool {
    raw_files::Format::from_extension(extension).is_some()
}

/// The photos under `folder`, at any depth, into `photos`. A folder that
/// cannot be listed is a failure of the test: it would hide its photos.
fn find(folder: &Path, photos: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(folder).unwrap_or_else(|error| panic!("{folder:?}: {error}"));
    for entry in entries {
        let entry = entry.unwrap();
        // Left out as the app's photo list leaves them out: among them the
        // "._" files macOS writes beside photos on memory cards.
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            find(&path, photos);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(is_photo)
        {
            photos.push(path);
        }
    }
}

/// The photos of the folder `PW_LOCAL_CORPUS` names, sorted, or `None`
/// when it names none.
fn corpus() -> Option<Vec<PathBuf>> {
    let Some(folder) = env::var_os("PW_LOCAL_CORPUS") else {
        println!("skipped: PW_LOCAL_CORPUS names no folder");
        return None;
    };
    let mut photos = Vec::new();
    find(Path::new(&folder), &mut photos);
    photos.sort();
    Some(photos)
}

/// `check` run on every one of `photos`, on as many threads as the
/// machine has cores, and the test failed listing each photo it found
/// wrong with what it said, after a line saying how many it checked in
/// how long.
fn check_each(photos: &[PathBuf], what: &str, check: impl Fn(&Path) -> Result<(), String> + Sync) {
    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(path) = photos.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Err(failure) = check(path) {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("{}: {failure}", path.display()));
                    }
                }
            });
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();

    println!(
        "{what} {} of {} photos in {:.1} s on {threads} threads",
        photos.len() - failures.len(),
        photos.len(),
        start.elapsed().as_secs_f64(),
    );
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "needs PW_LOCAL_CORPUS, a folder of photos"]
fn every_photo_of_the_local_corpus_decodes() {
    let Some(photos) = corpus() else {
        return;
    };
    check_each(&photos, "decoded", |path| {
        previews::decode(path, LONGEST)
            .map(drop)
            .map_err(|failure| failure.to_string())
    });
}

/// The size a thumbnail's decode tells of the largest preview is the one
/// its frame header gives; decoded whole, the largest is measured by the
/// decoder itself. The two must agree, or 100 % would change as larger
/// decodes arrive.
#[test]
#[ignore = "needs PW_LOCAL_CORPUS, a folder of photos"]
fn every_raw_file_of_the_local_corpus_tells_its_largest_preview_s_true_size() {
    let Some(mut photos) = corpus() else {
        return;
    };
    photos.retain(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(is_raw)
    });
    check_each(&photos, "measured", |path| {
        let told = previews::decode(path, LONGEST).map_err(|failure| failure.to_string())?;
        let whole = previews::decode(path, u32::MAX).map_err(|failure| failure.to_string())?;
        if told.largest() == whole.full() && whole.largest() == whole.full() {
            Ok(())
        } else {
            Err(format!(
                "told {:?}, decoded {:?}",
                told.largest(),
                whole.full()
            ))
        }
    });
}
