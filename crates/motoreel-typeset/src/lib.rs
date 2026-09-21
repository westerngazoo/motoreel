//! Real type for motoreel — R-0009.
//!
//! The engine could not write the language of the channel it feeds.
//! `motoreel`'s face is a 5 × 7 bitmap over printable ASCII, and
//! `label::to_ascii` turns everything else into `'?'` at eval, upstream
//! of both sinks. So a reel that says «bíceps» rendered «b?ceps», and a
//! title that said «¿POR QUÉ TANTO?» rendered «?POR QU? TANTO?», and
//! nothing objected — because a `'?'` is a perfectly good glyph.
//!
//! This crate replaces that with three things, in this order:
//!
//! 1. **Faces** ([`Face`], [`Fonts`]) — real fonts, real advances, real
//!    kerning, and a character with no glyph is an [`Error::Missing`]
//!    that names the character and the face.
//! 2. **A box model** ([`Node`], [`Metrics`]) — width, height above the
//!    baseline, depth below it. Stage 1 builds nothing but rows of
//!    glyphs; the tree is TeX-shaped from the start because designing "a
//!    line of text" now means rewriting it for the first fraction.
//! 3. **Measurement without pixels** ([`measure`], [`place`]) — a run's
//!    size and every glyph's position are arithmetic over that tree. The
//!    Python factory's collision gate has to render a frame and spy on
//!    the draw calls to learn how big a label is; here it is a function.
//!
//! [`rasterize`] is the only thing that touches pixels, and it emits
//! **coverage**, not colour: a sink decides what to do with
//! `(x, y, alpha)`.
//!
//! # Why its own crate
//!
//! `motoreel` depends on `garust` and nothing else, and its consumers
//! include lessons compiled to wasm. A font rasterizer in the core would
//! be inherited by all of them. Here, only the sink that rasterizes pays
//! for it (R-0009 AC7).
//!
//! # What this is not
//!
//! Not shaping: no bidi, no Indic reordering, no Arabic joining. Not a
//! TeX: mathematical *layout* — fractions, radicals, limits — is a later
//! requirement, and all this one promises is that the box model can hold
//! it. Not LaTeX: nothing here shells out.
//!
//! # Example
//!
//! ```no_run
//! use motoreel_typeset::{measure, place, rasterize, Align, Face, FaceId, Fonts};
//!
//! let mut fonts = Fonts::new();
//! let bytes = std::fs::read("brand/fonts/Bangers.ttf")?;
//! let display = fonts.add(Face::load(bytes, "Bangers")?);
//!
//! let run = measure("¿POR QUÉ TANTO?", display, 76.0, &fonts)?;
//! let layout = place(&run, (540.0, 1800.0), Align::Center);
//!
//! // Ask how big it is without drawing it — this is the gate's question.
//! if let Some((l, _, r, _)) = layout.ink_bounds() {
//!     assert!(l >= 0.0 && r <= 1080.0, "the title must fit the canvas");
//! }
//!
//! rasterize(&layout, &fonts, |x, y, alpha| {
//!     // blend `alpha` into the frame at (x, y)
//!     let _ = (x, y, alpha);
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

mod face;
mod measure;
mod node;
mod place;
mod raster;
mod scale;

pub use face::{Error, Face, Fonts, VMetrics};
pub use measure::measure;
pub use node::{FaceId, Glyph, Ink, Metrics, Node, Stack};
pub use place::{place, Align, Layout, Placed, PlacedRule};
pub use raster::rasterize;
