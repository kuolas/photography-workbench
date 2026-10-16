// SPDX-License-Identifier: GPL-3.0-or-later

//! previews: the photos turned into images to show, with no GUI, so it is
//! tested without a display.
//!
//! [`decode`] turns a photo's file, a JPEG or a RAW file's embedded
//! preview, into a small upright [`Image`] of the crate's own rather than
//! any GUI's, or says why it could not ([`Failure`]), on the caller's
//! thread. Decoding takes a while, so a window hands what it wants to a
//! [`Decoder`], which decodes on worker threads, the most wanted first,
//! and hands back each [`Answer`] when it is ready.

mod decode;
mod decoder;
mod failure;
#[cfg(test)]
mod fixtures;

pub use decode::{Decoded, Image, Size, decode};
pub use decoder::{Answer, Decoder};
pub use failure::Failure;
