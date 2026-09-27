//! `PpmSink`: binary P6 raster frames and the in-crate stroke and fill
//! rasterizers.
//!
//! The sink R-0006 exists for: ffmpeg has an `svg_pipe` demuxer but no SVG
//! *decoder* without librsvg, so the command motoreel documented never
//! worked on a stock build. P6 is decoded by every ffmpeg ever shipped.
//!
//! Ink lands where [`crate::SvgSink`] puts it (SPEC-0006 §2.3): the two
//! sinks share no code, only a documented mapping — the only thing they
//! could share is four lines of arithmetic whose two spellings, an SVG
//! attribute and an `f64` expression, have no common form.
//!
//! Byte-determinism is SPEC-0003's discipline unchanged: no clock, no
//! environment, no randomness, no map iteration, and no `mul_add` on the
//! write path (§2.7). Paths keep it (SPEC-0012 §2.10): they are flattened
//! to chords by a count that is a pure function of the control points, and
//! a fill's coverage is a closed form of each pixel centre and the edge
//! set, with no transcendental and nothing carried between pixels.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::path::{bernstein, Seg, Subpath};
use crate::prim::{Align, Fill, Prim2, Pt2, Rgb, Style};
use crate::sink::FrameSink;

/// A point in pixel space: x right, y **down**, pixel centres at `+0.5`.
type Px = (f64, f64);

/// Writes one `frame_%05d.ppm` per frame into a directory (RFC-012 §3.3).
///
/// Stroke widths are in image (view) units exactly as for
/// [`crate::SvgSink`], so swapping one sink for the other reproduces the
/// same picture — the defaults are deliberately identical.
pub struct PpmSink {
    dir: PathBuf,
    /// The centred image-space window this sink renders (R-0007 AC4).
    view: (f64, f64),
    /// Frozen at construction: ASCII, no float formatting (§2.2).
    header: Vec<u8>,
    canvas: Canvas,
}

impl PpmSink {
    /// Sink into `dir` (created now, parents included): 1920×1080 pixels
    /// over the default 3.2 × 1.8 centred view window, on black.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        PpmSink::with_view(dir, (1920, 1080), (3.2, 1.8))
    }

    /// Sink into `dir` with an explicit raster size (px) and centred
    /// image-space view window — the pair [`crate::SvgSink::with_view`]
    /// takes, mapped identically (§2.3).
    ///
    /// Zero raster dimensions, a non-finite/non-positive view, or a frame
    /// whose `w·h·3` byte count does not fit in `usize` are
    /// [`io::ErrorKind::InvalidInput`], rejected **before** the directory
    /// is created: invalid input has no side effects. Allocation failure is
    /// [`io::ErrorKind::OutOfMemory`] rather than an abort inside `Vec`.
    pub fn with_view(
        dir: impl AsRef<Path>,
        size: (u32, u32),
        view: (f64, f64),
    ) -> io::Result<Self> {
        if size.0 == 0 || size.1 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "raster size must be non-zero in both dimensions",
            ));
        }
        let view_valid = view.0.is_finite() && view.1.is_finite() && view.0 > 0.0 && view.1 > 0.0;
        if !view_valid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "view window must be finite and positive",
            ));
        }
        // A caller-supplied (100_000, 100_000) would otherwise abort the
        // process inside `Vec`; constitution §6 forbids unchecked failures
        // in library code.
        let bytes = u64::from(size.0) * u64::from(size.1) * 3;
        let Ok(bytes) = usize::try_from(bytes) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame too large",
            ));
        };

        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "frame buffer"))?;
        pixels.resize(bytes, 0);

        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        Ok(PpmSink {
            dir,
            view,
            header: format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes(),
            canvas: Canvas {
                dims: size,
                // `min` is what SVG's `xMidYMid meet` does; two independent
                // scales would skew every angle (§2.3).
                scale: (f64::from(size.0) / view.0).min(f64::from(size.1) / view.1),
                background: Rgb::BLACK,
                pixels,
                coverage: Vec::new(),
                segments: Vec::new(),
                #[cfg(feature = "text")]
                fonts: None,
                failed: None,
            },
        })
    }

    /// Composite alpha against `background` instead of black. PPM has no
    /// alpha channel, so the background is explicit rather than implied.
    ///
    /// A consuming builder, matching `Object::with_style`/`with_track` —
    /// not a fourth constructor.
    pub fn with_background(mut self, background: Rgb) -> Self {
        self.canvas.background = background;
        self
    }

    /// The faces this sink sets text in (R-0009).
    ///
    /// A `Prim2::Text` names its face by index into this registry. Without
    /// one, a text primitive is an **error** from [`FrameSink::frame`] —
    /// not a blank space, and not a `'?'`. That is the whole point: the
    /// engine drew «b?ceps» for months because the quiet answer was
    /// always available.
    #[cfg(feature = "text")]
    #[must_use]
    pub fn with_fonts(mut self, fonts: motoreel_typeset::Fonts) -> Self {
        self.canvas.fonts = Some(fonts);
        self
    }
}

impl FrameSink for PpmSink {
    fn view(&self) -> Option<(f64, f64)> {
        Some(self.view)
    }

    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()> {
        self.canvas.clear();
        for prim in prims {
            self.canvas.draw(prim); // draw order = slice order (painter's)
        }
        // A frame that could not set its text is not a frame. Writing it
        // anyway is how «b?ceps» reached 45 published pieces.
        if let Some(why) = self.canvas.failed.take() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame {index}: {why}"),
            ));
        }
        let path = self.dir.join(format!("frame_{index:05}.ppm"));
        let mut file = fs::File::create(path)?;
        file.write_all(&self.header)?;
        file.write_all(self.canvas.pixels())
    }
}

/// Pixel canvas: the pinned mapping plus the coverage rasterizer.
///
/// A private type with exactly one consumer, so it stays in this module —
/// a module per single abstraction is the premature abstraction the
/// constitution forbids. The R-0007 promotion path is SPEC-0006 §2.12.
struct Canvas {
    dims: (u32, u32),
    /// Pixels per image unit — `min(W/vw, H/vh)` (§2.3).
    scale: f64,
    background: Rgb,
    /// Row-major RGB, top row first: exactly `w·h·3` bytes.
    pixels: Vec<u8>,
    /// Scratch tile for the primitive in flight: a stroke's coverage, or
    /// a fill's nearest signed boundary distance (SPEC-0012 §2.6).
    coverage: Vec<f64>,
    /// Scratch segment list in pixel space — a stroke's segments or a
    /// fill's edges (capacity caches only).
    segments: Vec<(Px, Px)>,
    /// The faces text is set in; `None` until [`PpmSink::with_fonts`].
    #[cfg(feature = "text")]
    fonts: Option<motoreel_typeset::Fonts>,
    /// The first text failure of the frame in flight.
    ///
    /// `draw` has no return value — the painter's loop is infallible by
    /// design — so a failure is parked here and raised by `frame`, which
    /// does have an error channel. First one wins: a frame whose face is
    /// missing a glyph would otherwise report the same thing once per
    /// occurrence, and the first is the one you fix.
    failed: Option<String>,
}

impl Canvas {
    /// Refill every pixel with the background triple (§2.6): no frame
    /// inherits a pixel from its predecessor.
    fn clear(&mut self) {
        let bg = [self.background.r, self.background.g, self.background.b];
        // `as_chunks_mut` over `chunks_exact_mut(3)`: the buffer is
        // exactly `w·h·3` bytes, so the remainder is provably empty and
        // the array form says so in the type.
        for px in self.pixels.as_chunks_mut::<3>().0 {
            *px = bg;
        }
    }

    /// Route one primitive to its rasterizer.
    ///
    /// Text routes **first**: the stroke guards in `draw_stroke` key off
    /// `Style::width`, which text does not use, and would wrongly reject a
    /// label whose width is 0 (SPEC-0007 §2.13 edit 1). All six variants
    /// are listed by name — still no `_` arm, so a seventh is a compile
    /// error.
    fn draw(&mut self, prim: &Prim2) {
        match prim {
            Prim2::Text {
                at,
                text,
                size,
                align,
                face,
                style,
            } => self.draw_text(*at, text, *size, *align, *face, *style),
            // Fill first, then stroke, each composited once per pixel:
            // SVG's `paint-order: normal` (SPEC-0012 §2.7). A fill-only
            // path is one whose stroke the guards in `draw_stroke` reject.
            Prim2::Path { subpaths, style } => {
                if let Some(fill) = style.fill {
                    self.draw_fill(subpaths, fill);
                }
                self.draw_stroke(prim);
            }
            Prim2::Point { .. }
            | Prim2::Segment { .. }
            | Prim2::Polyline { .. }
            | Prim2::Edges { .. } => self.draw_stroke(prim),
        }
    }

    /// Set one run in real type (R-0009).
    ///
    /// Measured and placed by `motoreel-typeset`, which hands back
    /// coverage; this composites it. Proportional advances, real kerning,
    /// and subpixel horizontal placement — none of which the 5 × 7 bitmap
    /// face it replaces could do, and none of which matters as much as
    /// the fact that it can write `é`.
    #[cfg(feature = "text")]
    fn draw_text(
        &mut self,
        at: Pt2,
        text: &str,
        size: f64,
        align: Align,
        face: usize,
        style: Style,
    ) {
        use motoreel_typeset::{measure, place, rasterize, FaceId};

        let alpha = unit(style.alpha);
        // `size` is author data carried verbatim (SPEC-0007 §2.3); this is
        // the sink's own guard, the counterpart of the stroke path's.
        if !(size.is_finite() && size > 0.0) || alpha == 0.0 || text.is_empty() {
            return;
        }
        let Some(fonts) = self.fonts.as_ref() else {
            self.fail(format!(
                "no faces registered; {text:?} cannot be set. \
                 Build the sink with PpmSink::with_fonts"
            ));
            return;
        };

        let run = match measure(text, FaceId(face), size * self.scale, fonts) {
            Ok(r) => r,
            Err(e) => {
                self.fail(format!("{e}"));
                return;
            }
        };
        let (px, py) = to_pixel(at, self.scale, self.dims);
        let layout = place(&run, (px, 0.0), Canvas::align_of(align));

        // The typesetter works y-up from the baseline; `to_pixel` gives a
        // y-down row. Placing at y = 0 and subtracting is what bridges
        // them, and keeps the f64 handed to the rasterizer small.
        let mut spans: Vec<(i64, i64, f64)> = Vec::new();
        if let Err(e) = rasterize(&layout, fonts, |x, y, c| spans.push((x, y, c))) {
            self.fail(format!("{e}"));
            return;
        }
        let base = py.floor() as i64;
        for (x, y, c) in spans {
            self.blend_pixel(x, base - y, style.stroke, alpha * c);
        }
    }

    /// Without the `text` feature there is no typesetter, so a text
    /// primitive is refused rather than silently skipped.
    #[cfg(not(feature = "text"))]
    fn draw_text(
        &mut self,
        _at: Pt2,
        text: &str,
        _size: f64,
        _align: Align,
        _face: usize,
        _style: Style,
    ) {
        self.fail(format!(
            "built without the `text` feature; {text:?} cannot be set"
        ));
    }

    /// Park the first failure of the frame; `frame` raises it.
    fn fail(&mut self, why: String) {
        if self.failed.is_none() {
            self.failed = Some(why);
        }
    }

    /// One pixel of coverage, composited `src_over`.
    ///
    /// Clipping here rather than in the caller means an off-canvas run
    /// costs one comparison per pixel and nothing else — the same bargain
    /// the block blitter it replaces struck.
    #[cfg(feature = "text")]
    fn blend_pixel(&mut self, x: i64, y: i64, src: Rgb, alpha: f64) {
        let (w, h) = self.dims;
        if x < 0 || y < 0 || x >= i64::from(w) || y >= i64::from(h) {
            return;
        }
        // Non-negative and in range, so the arithmetic is exact in usize.
        let i = (y as usize * w as usize + x as usize) * 3;
        for (c, chan) in [src.r, src.g, src.b].into_iter().enumerate() {
            let under = f64::from(self.pixels[i + c]);
            let over = f64::from(chan);
            self.pixels[i + c] = alpha.mul_add(over - under, under).round() as u8;
        }
    }

    /// motoreel's alignment, in the typesetter's vocabulary. Two closed
    /// sets of three, so the mapping is total and a fourth variant on
    /// either side is a compile error.
    #[cfg(feature = "text")]
    fn align_of(align: Align) -> motoreel_typeset::Align {
        match align {
            Align::Left => motoreel_typeset::Align::Left,
            Align::Center => motoreel_typeset::Align::Center,
            Align::Right => motoreel_typeset::Align::Right,
        }
    }

    /// Rasterize one stroke primitive: union coverage of its round-capped
    /// segments, composited src-over in a single pass (§2.5).
    ///
    /// SPEC-0006's `draw` body, extracted verbatim so `draw` could become a
    /// router — not one expression edited or reordered, which is why the
    /// stroke golden cannot move (SPEC-0007 §2.13 edit 6).
    fn draw_stroke(&mut self, prim: &Prim2) {
        let style = style_of(prim);
        let radius = 0.5 * self.scale * style.width;
        let alpha = unit(style.alpha);
        // `stroke-width="0"` paints nothing in SVG, and must here too:
        // without this the ramp would paint a phantom 50 % hairline. The
        // `is_finite()` half rejects NaN *and* an infinite radius, which
        // would make `pad` infinite and the tile the whole canvas.
        if !(radius.is_finite() && radius > 0.0) || alpha == 0.0 {
            return;
        }

        let (scale, dims) = (self.scale, self.dims);
        self.segments.clear();
        push_segments(prim, |p| to_pixel(p, scale, dims), &mut self.segments);
        // The finite contract is SPEC-0002's; the debug_assert is the
        // tripwire, the retain keeps `floor`/`ceil` → u32 honest (§2.9).
        debug_assert!(self.segments.iter().all(|&(a, b)| finite(a) && finite(b)));
        self.segments.retain(|&(a, b)| finite(a) && finite(b));

        let pad = radius + 0.5; // the AA ramp's reach beyond the boundary
        let Some(tile) = Tile::around(&self.segments, pad, dims) else {
            return;
        };
        self.coverage.clear();
        self.coverage.resize(tile.area(), 0.0);

        for &(a, b) in &self.segments {
            let Some(band) = tile.intersect(Tile::around(&[(a, b)], pad, dims)) else {
                continue;
            };
            for y in band.y0..band.y1 {
                for x in band.x0..band.x1 {
                    let centre = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let c = coverage(distance_to_segment(centre, a, b), radius);
                    let slot = &mut self.coverage[tile.offset(x, y)];
                    if c > *slot {
                        *slot = c; // union by max — no accumulation (§2.5)
                    }
                }
            }
        }

        for y in tile.y0..tile.y1 {
            for x in tile.x0..tile.x1 {
                let c = self.coverage[tile.offset(x, y)];
                if c > 0.0 {
                    let i = (y as usize * dims.0 as usize + x as usize) * 3;
                    src_over(&mut self.pixels[i..i + 3], style.stroke, c * alpha);
                }
            }
        }
    }

    /// Fill one path by the nonzero winding rule, anti-aliased by the
    /// stroke's own one-pixel ramp applied to the signed distance to the
    /// outline (SPEC-0012 §2.6, realizing R-0012 OQ-2).
    ///
    /// A pixel whose centre lies within 0.5 px of a *boundary* edge takes
    /// `unit(0.5 + s)`, with `s` the signed distance to the nearest one
    /// (see [`signed_distance`]); every other pixel is 1 inside and 0
    /// outside. Coverage is therefore a closed form of each pixel centre
    /// and the edge set, and it agrees with the stroke on where the
    /// boundary is. Composited once per pixel, row-major.
    ///
    /// This builds the edge set; the band pass is [`nearest_boundary`],
    /// and the interior pass with the composite is [`composite_fill`].
    fn draw_fill(&mut self, subpaths: &[Subpath<Pt2>], fill: Fill) {
        // A non-finite alpha paints nothing, +∞ included (SPEC-0012
        // §2.13), which `unit` alone would read as opaque. The SVG sink
        // writes `fill="none"` for exactly the same set.
        let alpha = unit(fill.alpha);
        if alpha == 0.0 || !fill.alpha.is_finite() {
            return;
        }
        let (scale, dims) = (self.scale, self.dims);
        // The scratch buffers, named for what a fill keeps in them.
        let (edges, nearest) = (&mut self.segments, &mut self.coverage);
        edges.clear();
        for sub in subpaths {
            // Every subpath is closed for fill: SVG's implicit close.
            push_chords(sub, |p| to_pixel(p, scale, dims), true, edges);
        }
        // One non-finite end would open its loop and corrupt the winding
        // across whole rows, so such a fill paints nothing (finding 7).
        // Eval's finite invariant means this is never taken in practice.
        if !edges.iter().all(|&(a, b)| finite(a) && finite(b)) {
            return;
        }
        // A zero-length edge adds nothing to any winding number, and its
        // normal would be NaN (finding 6).
        edges.retain(|&(a, b)| a != b);
        let Some(tile) = Tile::around(edges, 0.5, dims) else {
            return;
        };
        nearest_boundary(edges, tile, dims, nearest);
        composite_fill(
            edges,
            nearest,
            tile,
            fill.colour,
            alpha,
            &mut self.pixels,
            dims,
        );
    }

    fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// Image space (y-up, centred) → pixel space (y-down, centres at `+0.5`).
/// The closed form of `SvgSink`'s `scale(1 -1)` ∘ viewBox ∘ `xMidYMid meet`
/// chain — §2.3 derives it; the two sinks agree because of this function.
fn to_pixel(p: Pt2, scale: f64, dims: (u32, u32)) -> Px {
    (
        f64::from(dims.0) / 2.0 + scale * p.x,
        f64::from(dims.1) / 2.0 - scale * p.y,
    )
}

/// Every primitive is a union of round-capped segments (§2.4); a path's
/// are its flattened chords (SPEC-0012 §2.7). Exhaustive with no `_` arm
/// on purpose: R-0007's new variant must be a compile error, never a
/// silently unrendered label.
fn push_segments(prim: &Prim2, map: impl Fn(Pt2) -> Px, out: &mut Vec<(Px, Px)>) {
    match prim {
        Prim2::Point { at, .. } => out.push((map(*at), map(*at))),
        Prim2::Segment { a, b, .. } => out.push((map(*a), map(*b))),
        // `< 2` points yields no segments — the degenerate case needs no branch
        Prim2::Polyline { points, .. } => {
            out.extend(points.windows(2).map(|w| (map(w[0]), map(w[1]))));
        }
        Prim2::Edges { segments, .. } => {
            out.extend(segments.iter().map(|(a, b)| (map(*a), map(*b))));
        }
        // Text has no centre-lines; `draw` routes it before here. An empty
        // arm, not a wildcard, so a seventh variant is still a compile error.
        Prim2::Text { .. } => {}
        // Only a closed subpath strokes its closing chord (§2.7).
        Prim2::Path { subpaths, .. } => {
            for sub in subpaths {
                push_chords(sub, &map, sub.closed, out);
            }
        }
    }
}

/// Chord-count ceiling per cubic (SPEC-0012 §2.5). It bounds memory
/// against absurd off-canvas geometry, and is reached only when
/// `L > 1.4 × 10⁵` px, which lies outside the tolerance claim.
const FLATTEN_MAX: u32 = 1024;

/// The chords of one subpath in pixel space, appended to `out`; with
/// `close`, also the chord from its last point back to its start.
fn push_chords(sub: &Subpath<Pt2>, map: impl Fn(Pt2) -> Px, close: bool, out: &mut Vec<(Px, Px)>) {
    let poly = flatten(sub, map);
    out.extend(poly.windows(2).map(|w| (w[0], w[1])));
    if close {
        out.push((poly[poly.len() - 1], poly[0])); // `poly` holds at least the start
    }
}

/// One subpath as a pixel-space polyline (SPEC-0012 §2.5): the mapped
/// start, each line's end, and each cubic's points at `t = i/n` for the
/// [`chords`] count `n`, ending on its mapped end point verbatim.
///
/// `to_pixel` is affine, so mapping the control points first is exact:
/// the pixel-space cubic *is* the image of the image-space one.
fn flatten(sub: &Subpath<Pt2>, map: impl Fn(Pt2) -> Px) -> Vec<Px> {
    let mut from = map(sub.start);
    let mut poly = vec![from];
    for seg in &sub.segs {
        let end = match *seg {
            Seg::Line(end) => map(end),
            Seg::Cubic(h1, h2, end) => {
                let q = [from, map(h1), map(h2), map(end)];
                let n = chords(q[0], q[1], q[2], q[3]);
                poly.extend((1..n).map(|i| on_cubic(q, f64::from(i) / f64::from(n))));
                q[3]
            }
        };
        poly.push(end);
        from = end;
    }
    poly
}

/// How many uniform chords keep a cubic within ε = 0.1 px of its curve
/// (SPEC-0012 §2.5, R-0012 AC6).
///
/// With `L` the larger second difference of the control points,
/// `‖B''‖ ≤ 6L`, and a chord spanning `1/n` of the parameter strays at
/// most `6L / 8n²` from the curve. Requiring that to be `≤ ε` gives
/// `n ≥ √(3L / 4ε) = √(7.5·L)`. The count is a bound, not a search: a pure
/// function of the four points, so it adds no nondeterminism.
fn chords(p0: Px, p1: Px, p2: Px, p3: Px) -> u32 {
    let l = second_difference(p0, p1, p2).max(second_difference(p1, p2, p3));
    // `as` saturates — NaN to 0, ∞ to `u32::MAX` — so the clamp is total.
    ((7.5 * l).sqrt().ceil() as u32).clamp(1, FLATTEN_MAX)
}

/// `‖a − 2b + c‖`, as `sqrt(dx·dx + dy·dy)`: no `hypot` (§2.10).
fn second_difference(a: Px, b: Px, c: Px) -> f64 {
    let (dx, dy) = (a.0 - 2.0 * b.0 + c.0, a.1 - 2.0 * b.1 + c.1);
    (dx * dx + dy * dy).sqrt()
}

/// The point at `t` on the pixel-space cubic `q`, in Bernstein form.
fn on_cubic(q: [Px; 4], t: f64) -> Px {
    let w = bernstein(t);
    (
        w[0] * q[0].0 + w[1] * q[1].0 + w[2] * q[2].0 + w[3] * q[3].0,
        w[0] * q[0].1 + w[1] * q[1].1 + w[2] * q[2].1 + w[3] * q[3].1,
    )
}

/// The band pass of a fill (SPEC-0012 §2.6): refill `nearest` over `tile`
/// with the signed distance of the nearest boundary edge within 0.5 px of
/// each pixel centre, and +∞ where there is none.
///
/// Each edge visits only its own band — its box grown by 0.5 px — as
/// `draw_stroke` does. The winner is [`nearer`]'s order, not the visiting
/// order, so each slot is a closed form of its centre and the edge set.
fn nearest_boundary(edges: &[(Px, Px)], tile: Tile, dims: (u32, u32), nearest: &mut Vec<f64>) {
    nearest.clear();
    nearest.resize(tile.area(), f64::INFINITY);
    for &(a, b) in edges {
        let Some(band) = tile.intersect(Tile::around(&[(a, b)], 0.5, dims)) else {
            continue;
        };
        for y in band.y0..band.y1 {
            for x in band.x0..band.x1 {
                let centre = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let Some(s) = signed_distance(edges, a, b, centre) else {
                    continue;
                };
                let slot = &mut nearest[tile.offset(x, y)];
                if nearer(s, *slot) {
                    *slot = s;
                }
            }
        }
    }
}

/// The interior pass of a fill and its one composite per pixel, row-major
/// (SPEC-0012 §2.6): a finite `nearest` distance `s` takes the ramp
/// `unit(0.5 + s)`; every other pixel is 1 inside and 0 outside.
///
/// The inside test sees only the edges whose half-open y-range holds the
/// row's centre line: no other edge can cross the row, so the winding
/// number is the full sum. The list is a filter with exact set equality,
/// not a scanline walk — nothing is carried along the row (SPEC-0006
/// §2.7, point 3).
fn composite_fill(
    edges: &[(Px, Px)],
    nearest: &[f64],
    tile: Tile,
    colour: Rgb,
    alpha: f64,
    pixels: &mut [u8],
    dims: (u32, u32),
) {
    let mut row = Vec::new();
    for y in tile.y0..tile.y1 {
        let cy = f64::from(y) + 0.5;
        row.clear();
        row.extend(edges.iter().copied().filter(|&(a, b)| spans(a, b, cy)));
        for x in tile.x0..tile.x1 {
            let s = nearest[tile.offset(x, y)];
            let cov = if s.is_finite() {
                unit(0.5 + s)
            } else if inside(&row, (f64::from(x) + 0.5, cy)) {
                1.0
            } else {
                0.0
            };
            if cov > 0.0 {
                let i = (y as usize * dims.0 as usize + x as usize) * 3;
                src_over(&mut pixels[i..i + 3], colour, cov * alpha);
            }
        }
    }
}

/// How far either side of an edge the boundary test probes: δ = 2⁻²⁰ px
/// (SPEC-0012 §2.6, finding 5). A power of two, far above coordinate
/// rounding on any canvas up to 2¹⁶ px, and small enough that a probe
/// near a self-intersection rarely lands across another edge.
const PROBE_OFFSET: f64 = 1.0 / 1_048_576.0;

/// The signed distance from pixel centre `c` to edge `a`–`b`, if it is
/// under 0.5 px and the edge is a *boundary* at its foot point; `None`
/// otherwise (SPEC-0012 §2.6).
///
/// The edge is a boundary where the inside test differs at `±δ` along its
/// unit normal from the foot point. Internal chords — a pentagram's, or a
/// same-orientation nested square's — have the inside on both sides, so
/// they draw no ramp and leave no seam (R-0012 AC3). The inside half-plane
/// is the side that is inside; `s = +d` when `c` lies **strictly** in it,
/// and `−d` otherwise, a centre on the edge's own line included. The
/// strict reading is load-bearing: the inclusive one breaks AC5's corner
/// bound (1.559 against 1.0 px²).
fn signed_distance(edges: &[(Px, Px)], a: Px, b: Px, c: Px) -> Option<f64> {
    let f = foot(a, b, c);
    let d = ((c.0 - f.0) * (c.0 - f.0) + (c.1 - f.1) * (c.1 - f.1)).sqrt();
    if d >= 0.5 {
        return None;
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (ox, oy) = (PROBE_OFFSET * -dy / len, PROBE_OFFSET * dx / len);
    let plus_inside = inside(edges, (f.0 + ox, f.1 + oy));
    if plus_inside == inside(edges, (f.0 - ox, f.1 - oy)) {
        return None; // the same on both sides: not a boundary here
    }
    // Positive when `c` is on the `+normal` side of the edge's line.
    let side = dx * (c.1 - a.1) - (c.0 - a.0) * dy;
    let strictly_inside = if plus_inside { side > 0.0 } else { side < 0.0 };
    Some(if strictly_inside { d } else { -d })
}

/// The point of edge `a`–`b` nearest `c` (SPEC-0012 §2.6). Where the
/// projection falls outside the edge, the end itself is returned
/// **verbatim**: `a + 1·(b − a)` is not `b` in floating point, and a probe
/// at a corner must land exactly on the vertex (finding 2). Requires
/// `a ≠ b`.
fn foot(a: Px, b: Px, c: Px) -> Px {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = ((c.0 - a.0) * dx + (c.1 - a.1) * dy) / (dx * dx + dy * dy);
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        (a.0 + t * dx, a.1 + t * dy)
    }
}

/// Whether `p` is inside `edges` by the nonzero rule (R-0012 AC3).
fn inside(edges: &[(Px, Px)], p: Px) -> bool {
    edges.iter().map(|&(a, b)| crossing(a, b, p)).sum::<i32>() != 0
}

/// One edge's contribution to the winding number at `p`: its signed
/// crossing of the rightward ray from `p`, by Sunday's half-open rule, so
/// a vertex on the ray is counted once (SPEC-0012 §2.6).
///
/// The side test is the rounded cross product: not the exact orientation
/// predicate, but the same bits on every run (finding 15).
fn crossing(a: Px, b: Px, p: Px) -> i32 {
    let left = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
    if a.1 <= p.1 && p.1 < b.1 && left > 0.0 {
        1
    } else if b.1 <= p.1 && p.1 < a.1 && left < 0.0 {
        -1
    } else {
        0
    }
}

/// Whether edge `a`–`b`'s half-open y-range holds `y` — exactly the edges
/// [`crossing`] can count on the line through `y`.
fn spans(a: Px, b: Px, y: f64) -> bool {
    (a.1 <= y && y < b.1) || (b.1 <= y && y < a.1)
}

/// Whether signed distance `s` beats `best` in SPEC-0012 §2.6's order:
/// `(d, −s)` lexicographically with `d = |s|`, so the nearer edge wins, and
/// at equal distance the inside sign (owner decision; it bounds hole
/// corners at 1.54 px² instead of 2.24). The order, not the visiting order,
/// picks the winner.
fn nearer(s: f64, best: f64) -> bool {
    s.abs() < best.abs() || (s.abs() == best.abs() && s > best)
}

/// Distance in pixels from `p` to segment `a`–`b`. A degenerate segment
/// (`a == b`) gives the distance to the point — which is what makes a
/// `Point` a disc and a round cap a cap (§2.4).
fn distance_to_segment(p: Px, a: Px, b: Px) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (p.0 - a.0, p.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (apx - t * abx, apy - t * aby);
    (dx * dx + dy * dy).sqrt() // IEEE-exact; no libm, no `mul_add` (§2.7)
}

/// Coverage of a pixel whose centre lies `d` px from the centre-line of a
/// stroke of radius `r` px: the pinned one-pixel ramp — 1 inside, 0.5 on
/// the boundary, 0 outside (§2.5).
fn coverage(d: f64, r: f64) -> f64 {
    unit(r + 0.5 - d)
}

/// Source-over in 8-bit sRGB — SVG's default `color-interpolation` — with
/// one pinned rounding (`f64::round`, ties away from zero).
fn src_over(dst: &mut [u8], src: Rgb, a: f64) {
    for (slot, s) in dst.iter_mut().zip([src.r, src.g, src.b]) {
        let out = f64::from(s) * a + f64::from(*slot) * (1.0 - a);
        *slot = out.round() as u8; // `out ∈ [0, 255]`; the cast saturates
    }
}

/// The style every `Prim2` variant carries (SPEC-0002 §2.2).
fn style_of(prim: &Prim2) -> Style {
    match prim {
        Prim2::Point { style, .. }
        | Prim2::Segment { style, .. }
        | Prim2::Polyline { style, .. }
        | Prim2::Edges { style, .. }
        | Prim2::Text { style, .. }
        | Prim2::Path { style, .. } => *style,
    }
}

/// Both coordinates finite — the guard that keeps `floor`/`ceil` → `u32`
/// from saturating into a nonsense tile (§2.9).
fn finite(p: Px) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// Total clamp to `[0, 1]`; NaN maps to 0, so a style violating its
/// documented contract paints nothing rather than poisoning the frame
/// (§2.5). **Not `x.clamp(0.0, 1.0)`** — `f64::clamp` returns NaN for NaN,
/// which would let a NaN alpha reach `src_over` and paint garbage. The
/// comparison order is what routes NaN to the final `else`; see SPEC-0006
/// §3's clippy note before changing it.
fn unit(x: f64) -> f64 {
    if x > 1.0 {
        1.0
    } else if x > 0.0 {
        x
    } else {
        0.0
    }
}

/// A half-open pixel rectangle clamped to the canvas — the region ink can
/// reach. `None` when empty (entirely off-canvas, or no segments).
#[derive(Clone, Copy)]
struct Tile {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Tile {
    /// The clamped bounding box of `segments` grown by `pad`.
    fn around(segments: &[(Px, Px)], pad: f64, dims: (u32, u32)) -> Option<Tile> {
        let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for &(a, b) in segments {
            for p in [a, b] {
                lo = (lo.0.min(p.0), lo.1.min(p.1));
                hi = (hi.0.max(p.0), hi.1.max(p.1));
            }
        }
        if segments.is_empty() {
            return None;
        }
        // Every input is finite (the caller's `retain`), so these casts
        // cannot saturate into a nonsense tile.
        let x0 = (lo.0 - pad).floor().max(0.0) as u32;
        let y0 = (lo.1 - pad).floor().max(0.0) as u32;
        let x1 = ((hi.0 + pad).ceil().max(0.0) as u32 + 1).min(dims.0);
        let y1 = ((hi.1 + pad).ceil().max(0.0) as u32 + 1).min(dims.1);
        (x0 < x1 && y0 < y1).then_some(Tile { x0, y0, x1, y1 })
    }
    fn intersect(self, other: Option<Tile>) -> Option<Tile> {
        let o = other?;
        let t = Tile {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        };
        (t.x0 < t.x1 && t.y0 < t.y1).then_some(t)
    }

    fn area(self) -> usize {
        (self.x1 - self.x0) as usize * (self.y1 - self.y0) as usize
    }

    /// Row-major offset **within the tile**, not the canvas.
    fn offset(self, x: u32, y: u32) -> usize {
        (y - self.y0) as usize * (self.x1 - self.x0) as usize + (x - self.x0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sum a horizontal stroke's cross-section coverages over every pixel
    /// row that can be lit: the quantity AC4's identity is about.
    fn cross_section_sum(y0: f64, r: f64) -> f64 {
        let lo = (y0 - r - 1.0).floor() as i64;
        let hi = (y0 + r + 1.0).ceil() as i64;
        (lo..=hi)
            .map(|j| coverage((j as f64 + 0.5 - y0).abs(), r))
            .sum()
    }

    // ---- AC2(a): the mapping is exact, not approximate (§2.8) ----

    #[test]
    fn to_pixel_is_the_pinned_closed_form_bit_exactly() {
        // 1920x1080 over 3.2x1.8: both ratios are 600 exactly.
        let s = 600.0_f64;
        for (x, y) in [(0.0, 0.0), (1.0, 0.5), (-1.6, 0.9), (0.37, -0.21)] {
            let got = to_pixel(Pt2 { x, y }, s, (1920, 1080));
            assert_eq!(got.0, 1920.0 / 2.0 + s * x, "x at ({x}, {y})");
            assert_eq!(got.1, 1080.0 / 2.0 - s * y, "y at ({x}, {y})");
        }
    }

    #[test]
    fn to_pixel_letterboxes_by_min_not_by_two_scales() {
        // 200x100 over 3.2x1.8: 200/3.2 = 62.5, 100/1.8 = 55.55... -> min.
        let s = (200.0_f64 / 3.2).min(100.0 / 1.8);
        assert_eq!(s, 100.0 / 1.8, "min must pick the height-limited scale");
        let got = to_pixel(Pt2 { x: 1.6, y: 0.9 }, s, (200, 100));
        assert_eq!(got.0, 100.0 + s * 1.6);
        assert_eq!(got.1, 50.0 - s * 0.9);
        // The view's own corner lands inside the raster horizontally:
        // that gap is the letterbox band AC2(d) checks at frame level.
        assert!(got.0 < 200.0, "letterbox band expected on the x axis");
    }

    // ---- AC4: the exact width identity, and the sub-pixel regime ----

    #[test]
    fn coverage_sums_to_exactly_twice_the_radius() {
        // Measured claim (SPEC-0006 Section 2.8): max |sum - 2r| = 0 over
        // 1000 sub-pixel offsets. Asserted with `==`, no tolerance.
        for r in [0.5_f64, 0.75, 1.0, 1.5, 2.0, 3.0, 6.0] {
            for i in 0..1000 {
                let y0 = 10.0 + i as f64 / 1000.0;
                assert_eq!(
                    cross_section_sum(y0, r),
                    2.0 * r,
                    "r = {r}, y0 = {y0}: the identity is exact for r >= 0.5"
                );
            }
        }
    }

    #[test]
    fn sub_pixel_strokes_over_ink_and_that_is_documented() {
        // Below r = 0.5 the plateau disappears and the identity fails
        // *upward* - the pinned regime, so it stays behaviour not surprise.
        let r = 0.05_f64;
        let worst = (0..1000)
            .map(|i| cross_section_sum(10.0 + i as f64 / 1000.0, r))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            worst > 2.0 * r,
            "sub-pixel strokes must over-ink: {worst} vs {}",
            2.0 * r
        );
        assert!(worst <= 5.5 * (2.0 * r), "over-ink is bounded at ~5.5x");
    }

    #[test]
    fn coverage_is_the_one_pixel_ramp() {
        let r = 2.0;
        assert_eq!(coverage(0.0, r), 1.0, "deep inside");
        assert_eq!(coverage(r - 0.5, r), 1.0, "last full pixel");
        assert_eq!(coverage(r, r), 0.5, "exactly on the boundary");
        assert_eq!(coverage(r + 0.5, r), 0.0, "just outside");
        assert_eq!(coverage(r + 9.0, r), 0.0, "far outside");
    }

    #[test]
    fn lit_extent_is_two_r_plus_one_not_proportional() {
        // The tempting "double the width, double the lit width" is FALSE.
        for (r, expect) in [(1.0_f64, 3_usize), (2.0, 5)] {
            let y0 = 10.5; // centred on a pixel centre
            let lit = (0..24)
                .filter(|&j| coverage((j as f64 + 0.5 - y0).abs(), r) > 0.0)
                .count();
            assert_eq!(lit, expect, "r = {r} must light {expect} px");
        }
    }

    // ---- unit(): the NaN route that `f64::clamp` would break ----

    #[test]
    fn unit_clamps_totally_and_sends_nan_to_zero() {
        assert_eq!(unit(0.25), 0.25);
        assert_eq!(unit(1.5), 1.0);
        assert_eq!(unit(-1.0), 0.0);
        assert_eq!(unit(f64::NAN), 0.0, "f64::clamp would return NaN here");
        assert_eq!(unit(f64::INFINITY), 1.0);
        assert_eq!(unit(f64::NEG_INFINITY), 0.0);
    }

    // ---- distance_to_segment: caps and degenerate segments ----

    #[test]
    fn distance_to_a_degenerate_segment_is_the_point_distance() {
        let a = (10.0, 10.0);
        assert_eq!(distance_to_segment((13.0, 14.0), a, a), 5.0);
    }

    #[test]
    fn distance_beyond_an_endpoint_is_measured_to_the_cap() {
        let (a, b) = ((10.0, 10.0), (20.0, 10.0));
        assert_eq!(distance_to_segment((15.0, 13.0), a, b), 3.0, "interior");
        assert_eq!(distance_to_segment((25.0, 10.0), a, b), 5.0, "past b");
        assert_eq!(distance_to_segment((6.0, 10.0), a, b), 4.0, "before a");
    }

    // ---- src_over: the pinned rounding ----

    #[test]
    fn src_over_composites_in_srgb_with_pinned_rounding() {
        let mut dst = [0u8, 0, 0];
        src_over(&mut dst, Rgb::WHITE, 0.5);
        assert_eq!(dst, [128, 128, 128], "white at half alpha over black");
        let mut opaque = [7u8, 7, 7];
        src_over(&mut opaque, Rgb::WHITE, 1.0);
        assert_eq!(opaque, [255, 255, 255], "full coverage replaces");
    }

    // ---- push_segments: the union-of-segments decomposition ----

    #[test]
    fn every_variant_decomposes_into_round_capped_segments() {
        let id = |p: Pt2| (p.x, p.y);
        let style = Style::default();
        let p = |x: f64, y: f64| Pt2 { x, y };

        let mut out = Vec::new();
        push_segments(
            &Prim2::Point {
                at: p(1.0, 2.0),
                style,
            },
            id,
            &mut out,
        );
        assert_eq!(out, vec![((1.0, 2.0), (1.0, 2.0))], "a point is degenerate");

        out.clear();
        push_segments(
            &Prim2::Polyline {
                points: vec![p(0.0, 0.0)],
                style,
            },
            id,
            &mut out,
        );
        assert!(out.is_empty(), "a 1-point polyline yields no segments");

        out.clear();
        push_segments(
            &Prim2::Polyline {
                points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
                style,
            },
            id,
            &mut out,
        );
        assert_eq!(out.len(), 2, "n points yield n-1 segments");
    }

    // ==== R-0012: private flattening and fill details (test plan §4) ====

    /// B(t) in Bernstein form, test-local, so the check does not reuse the
    /// code under test.
    fn bez_ref(p: [Px; 4], t: f64) -> Px {
        let u = 1.0 - t;
        let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (
            b0 * p[0].0 + b1 * p[1].0 + b2 * p[2].0 + b3 * p[3].0,
            b0 * p[0].1 + b1 * p[1].1 + b2 * p[2].1 + b3 * p[3].1,
        )
    }

    // R-0012 AC6 / SPEC-0012 §2.5 and §6 — the meaningful test: over 3000
    // random cubics plus a true cusp, the parametric distance between each
    // uniform chord and its piece of curve is ≤ 0.1 px. (Asserting that
    // `chords` equals its own formula would only restate it.)
    #[test]
    fn chord_deviation_is_within_a_tenth_of_a_pixel_on_random_cubics_and_a_cusp() {
        let mut state = 0x0012_u64;
        let mut unit = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut cubics: Vec<[Px; 4]> = (0..3000)
            .map(|_| std::array::from_fn(|_| (600.0 * unit() - 300.0, 600.0 * unit() - 300.0)))
            .collect();
        cubics.push([(-80.0, -80.0), (80.0, 80.0), (-80.0, 80.0), (80.0, -80.0)]);
        let mut worst = 0.0_f64;
        for p in cubics {
            let n = chords(p[0], p[1], p[2], p[3]);
            for i in 0..n {
                let (t0, t1) = (f64::from(i) / f64::from(n), f64::from(i + 1) / f64::from(n));
                let (a, b) = (bez_ref(p, t0), bez_ref(p, t1));
                for k in 0..=64 {
                    let u = f64::from(k) / 64.0;
                    let on_curve = bez_ref(p, t0 + u * (t1 - t0));
                    let (cx, cy) = (a.0 + u * (b.0 - a.0), a.1 + u * (b.1 - a.1));
                    let d = ((on_curve.0 - cx) * (on_curve.0 - cx)
                        + (on_curve.1 - cy) * (on_curve.1 - cy))
                        .sqrt();
                    worst = worst.max(d);
                }
            }
        }
        assert!(worst <= 0.1, "worst chord deviation {worst} px");
    }

    // R-0012 AC6 / §2.5 — the count's edges: 1 for a flat cubic, clamped at
    // FLATTEN_MAX beyond the claim, and 33 for §2.5's own worked example (a
    // 300 px quarter circle, L ≈ 138).
    #[test]
    fn chord_count_is_one_when_flat_and_clamps_at_flatten_max() {
        assert_eq!(chords((0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)), 1);
        assert_eq!(chords((5.0, 5.0), (5.0, 5.0), (5.0, 5.0), (5.0, 5.0)), 1);
        assert_eq!(
            chords((0.0, 0.0), (1e6, 0.0), (0.0, 0.0), (1e6, 0.0)),
            FLATTEN_MAX
        );
        let k = 0.552_284_749_830_793_6 * 300.0;
        assert_eq!(
            chords((300.0, 0.0), (300.0, k), (k, 300.0), (0.0, 300.0)),
            33
        );
    }

    // R-0012 AC5 / §2.6, finding 2 — when t clamps, the foot point is the
    // endpoint itself. Here `a + 1·(b − a)` is 0.8999999999999999, not 0.9,
    // so a foot computed by the formula would miss the vertex.
    #[test]
    fn the_foot_point_is_the_endpoint_verbatim_when_t_clamps() {
        let (a, b) = ((0.2, 0.0), (0.9, 0.0));
        let one: f64 = 1.0;
        assert_ne!(
            a.0 + one * (b.0 - a.0),
            b.0,
            "the case must be the hard one"
        );
        assert_eq!(foot(a, b, (5.0, 1.0)), b);
        assert_eq!(foot(a, b, (-5.0, 1.0)), a);
        assert_eq!(foot(a, b, (0.55, 3.0)), (0.2 + 0.5 * (0.9 - 0.2), 0.0));
    }

    // R-0012 AC2 / §2.6, finding 7 — a non-finite edge makes the fill paint
    // nothing: dropping one edge would open the loop and corrupt the winding
    // across whole rows. Called below `draw`, since eval never emits it.
    #[test]
    fn a_non_finite_edge_makes_the_fill_paint_nothing() {
        let dir = std::env::temp_dir().join("motoreel-r0012-nan-fill");
        let mut sink = PpmSink::with_view(&dir, (16, 16), (16.0, 16.0)).expect("sink");
        let q = |x: f64, y: f64| Pt2 { x, y };
        let bad = Subpath::new(q(-4.0, -4.0))
            .line_to(q(4.0, -4.0))
            .line_to(q(f64::NAN, 4.0))
            .line_to(q(-4.0, 4.0))
            .close();
        sink.canvas.draw_fill(&[bad], Fill::solid(Rgb::WHITE, 1.0));
        assert!(
            sink.canvas.pixels().iter().all(|&b| b == 0),
            "nothing painted"
        );
    }
}
