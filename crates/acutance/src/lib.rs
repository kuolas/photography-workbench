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
//! Sharpness is measured as acutance: how abruptly brightness changes at
//! an edge. A plane's [`EdgeMap`] holds, for every pixel, the strength of
//! the edge it sits on, less the plane's noise floor, so that grain is not
//! taken for detail. An [`Area`] of the frame scores the mean of its
//! strongest edges ([`EdgeMap::score`]); the [`Score`] carries the
//! plane's fidelity, and scores of different fidelity are not compared.
//! [`EdgeMap::cells`] scores every cell of a grid laid over the frame, and
//! [`Cells::best`] says where the frame is sharpest.
//!
//! Making the edge map is the costly step, and it runs on the caller's
//! thread, on one core: over a 6000 × 4000 plane it took about 70 ms in a
//! release build on an Apple M5, 25 ms to blur, 30 ms for the Sobel
//! operator and 12 ms for the noise floor (the ignored test `timing_…`
//! measures it again). An embedded preview, a fraction of those pixels,
//! takes a fraction of the time.
//!
//! The crate has no dependencies, so its types implement no serde traits.
//!
//! # Examples
//!
//! Tell whether a photo is sharp where the camera focused, on a preview a
//! quarter as wide as the camera's image:
//!
//! ```
//! use acutance::{Area, EdgeMap, Fidelity, Plane, Size};
//!
//! # fn main() {
//! #     example().unwrap();
//! # }
//! # fn example() -> Option<()> {
//! # let rgb = vec![128; 1500 * 1000 * 3];
//! // The preview's pixels, as a JPEG decoder gives them.
//! let frame = Size { width: 6000, height: 4000 };
//! let plane = Plane::from_rgb8(1500, 1000, &rgb, frame)?;
//! let edges = EdgeMap::of(&plane);
//!
//! // The focus area, in the camera's pixels.
//! let focus = Area::centred(3000, 2000, Size { width: 500, height: 500 }, frame);
//! let score = edges.score(&focus);
//! assert_eq!(score.fidelity(), Fidelity::Coarse);
//! println!("{:.2} at the focus area, read on a {} plane", score.value(), score.fidelity());
//! # Some(())
//! # }
//! ```

mod area;
mod edges;
mod plane;
mod score;

pub use area::Area;
pub use edges::EdgeMap;
pub use plane::{Fidelity, Plane, Size};
pub use score::{Cells, Score};
