// SPDX-License-Identifier: GPL-3.0-or-later

//! previews: the photos turned into images to show, with no GUI, so it is
//! tested without a display.
//!
//! [`decode`] turns a photo's file, a JPEG or a RAW file's embedded
//! preview, into a small upright [`Image`] of the crate's own rather than
//! any GUI's, or says why it could not ([`Failure`]). It never starts a
//! thread of its own: decoding takes a while, so whoever calls it decides
//! where the waiting happens.

mod decode;
mod failure;
#[cfg(test)]
mod fixtures;

pub use decode::{Decoded, Image, Size, decode};
pub use failure::Failure;
