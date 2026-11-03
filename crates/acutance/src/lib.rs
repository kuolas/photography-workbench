// SPDX-License-Identifier: GPL-3.0-or-later

//! acutance: how sharp a picture is, and where, with no decoder and no
//! GUI, so it is tested without a display.
//!
//! Every input goes through one type, a [`Plane`] of luminance samples in
//! `[0, 1]`, as `f32`: a decoded embedded preview's 8-bit RGB
//! ([`Plane::from_rgb8`]), a demosaiced image's linear float RGB
//! ([`Plane::from_rgb_f32`]), or one sample per pixel, such as a sensor's
//! green ([`Plane::from_samples`]). A plane knows the size of the frame it
//! shows, so an area of the frame lands on the same place in a small
//! preview as in the full image, and its [`Fidelity`] says how much of the
//! frame's detail it holds.
//!
//! The crate has no dependencies, so its types implement no serde traits.

mod plane;

pub use plane::{Fidelity, Plane, Size};
