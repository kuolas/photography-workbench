# ADR020: Sharpness measured as acutance

- Status: Accepted
- Date: 2026-11-02
- Issue: #36

## Context

Rating sharpness, where the photographer focused, needs a measure of
how sharp a picture is, over the whole frame and over an area of it,
and a way to show where it is sharp while looking at the photo (focus
peaking). The measure has to say whether a photo is sharp at its focus
point or sharper somewhere else, the sign of a missed focus.

What there is to measure differs from one input to another:

- A photo's embedded previews, JPEGs made in the camera, which the app
  decodes to show the photo: 8-bit, gamma-encoded, and for some bodies
  much smaller than the sensor's image, so fine detail that is sharp on
  the sensor may be lost in them.
- The raw sensor data, and the developed image demosaiced from it, as
  raw-files' capabilities of those names give them
  ([ROADMAP](../ROADMAP.md)): one sample per pixel, or a linear image in
  float RGB, unclipped, at the sensor's full size.

Acutance, how abruptly brightness changes at an edge, is what the eye
reads as sharpness, and edge detectors measure it on any of these
inputs once they are reduced to one plane of luminance. Noise and grain
are edges too, of one pixel, and a measure that counts them would score
a high-ISO frame as sharp everywhere.

[ADR013](adr013-a-workspace-of-small-crates.md) puts every new engine
capability in a GUI-free crate before any widget shows it.

## Decision

We will measure sharpness as acutance, in a crate of its own,
**`acutance`**, which depends on no other crate: neither a decoder nor
the GUI, nor anything else of the workspace.

- **One input type.** Every input becomes a plane of luminance samples,
  `f32` in `[0, 1]`, with the size of the frame it shows: an embedded
  preview's 8-bit RGB, the sensor's samples and the developed image's
  float RGB, each through a constructor of the same type. Areas are
  given in the frame's coordinates, so an area measures the same place
  on a small preview as on the full image; a plane whose shape differs
  from its frame's by more than 3 %, such as a preview not turned
  upright or one with black bars, is refused, since its areas would
  land on the wrong pixels.
- **Scores carry their fidelity.** A plane at least as wide as its
  frame is of full fidelity, one at least half as wide reduced, one
  narrower coarse. Every score says the fidelity of the plane it was
  read on, and scores of different fidelity are not compared. Nor are
  scores of planes made differently: a plane in linear light has weaker
  edges in its shadows and midtones than a gamma-encoded one, and so
  lower scores for the same sharpness, at the same fidelity.
- **The measure.** The plane is blurred with a 3 × 3 binomial kernel;
  the Sobel operator gives each pixel's gradient, its magnitude divided
  by the largest the operator can give; the plane's median magnitude,
  its noise floor, is subtracted, and what falls below it is set to
  zero. An area scores the mean of the strongest 5 % of its values, so
  a small sharp subject is not marked down by the smooth background
  around it. Focus peaking marks the strongest 10 %, 5 % or 2 % of the
  plane's pixels.
- **The CPU is the baseline, the GPU a speedup.** The measure runs on
  the CPU first: every machine has one, it is tested with no device,
  and it is the reference a faster path's results are checked against.
  On the CPU the edge map of a 6000 × 4000 plane took about 70 ms on
  one core of an Apple M5 in a release build, and an embedded preview
  takes a fraction of that. A compute shader on the graphics card is
  the speedup over it: every card the app supports, of the Vulkan,
  DirectX 12 or Metal class, runs compute shaders. The CPU path stays
  as the fallback wherever no such device can be had. Where the shader
  lives and how it reaches the device, given that the boundary check
  keeps GPU crates out of the engine crates, is a decision of its own.

Rejected:

- **Scoring only on the sensor's data.** Every photo the app shows has
  an embedded preview, decoded anyway to show it, while the sensor's
  data must be decoded and developed first. A score read from the
  preview, marked with its fidelity, tells a missed focus, and the same
  measure reads the sensor's data wherever the app has it.
- **The GPU alone.** A shader needs a graphics device that tests and
  some machines do not have, and its results need a reference to be
  checked against; the CPU path is both.

## Consequences

- The measure is tested on planes the tests make up, steps, blurs and
  noise, with no file and no display.
- Each input comes in through a constructor of the plane, and the
  measure, the scores and focus peaking are the same for all of them: a
  new kind of input adds a constructor and nothing else.
- A score read on a developed image, in linear light, cannot be set
  beside one read on its embedded preview until the two are brought to
  one scale, by encoding the linear plane or by scoring each kind
  apart.
- A score is only as good as its plane: on a coarse preview a slightly
  soft photo may score as sharp, and the fidelity beside the score is
  the only warning. Scores read from different previews of the same
  photo cannot be compared.
- The edge map is made on the caller's thread; the app has to run it
  off the UI thread, as it decodes the previews.
- The measure is defined by the CPU path: a GPU path must give the
  same edge map, scores and peaks, to within rounding, and is tested
  against it.
- The noise floor assumes that most of a frame is smooth. A frame full
  of fine texture, such as foliage edge to edge, raises its own floor
  and scores lower than its sharpness deserves.
