// SPDX-License-Identifier: GPL-3.0-or-later

//! catalog: the photos on disk, the folder tree of the home folder and
//! the mounted volumes ([`folders`]) and a folder's photo list and its
//! current photo ([`photos`]), with no GUI, so it is tested without a
//! display.
//!
//! It reads the disk but never starts a thread of its own: whoever calls
//! it decides where the waiting happens, which for the app is off the UI
//! thread.

mod entry;
pub mod folders;
mod natural;
pub mod photos;
