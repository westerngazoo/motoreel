//! `PpmSink`: binary P6 raster frames and the in-crate stroke rasterizer.
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
//! write path (§2.7).

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::prim::{Prim2, Pt2, Rgb, Style};
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
    /// Scratch coverage tile for the primitive in flight.
    coverage: Vec<f64>,
    /// Scratch segment list in pixel space (capacity caches only).
    segments: Vec<(Px, Px)>,
}

impl Canvas {
    /// Refill every pixel with the background triple (§2.6): no frame
    /// inherits a pixel from its predecessor.
    fn clear(&mut self) {
        let bg = [self.background.r, self.background.g, self.background.b];
        for px in self.pixels.chunks_exact_mut(3) {
            px.copy_from_slice(&bg);
        }
    }

    /// Route one primitive to its rasterizer.
    ///
    /// Text routes **first**: the stroke guards in `draw_stroke` key off
    /// `Style::width`, which text does not use, and would wrongly reject a
    /// label whose width is 0 (SPEC-0007 §2.13 edit 1). All five variants
    /// are listed by name — still no `_` arm, so a sixth is a compile error.
    fn draw(&mut self, prim: &Prim2) {
        match prim {
            Prim2::Text {
                at,
                text,
                size,
                align,
                style,
            } => self.draw_text(*at, text, *size, *align, *style),
            Prim2::Point { .. }
            | Prim2::Segment { .. }
            | Prim2::Polyline { .. }
            | Prim2::Edges { .. } => self.draw_stroke(prim),
        }
    }

    /// Blit one ASCII run from the embedded face (SPEC-0007 §2.8, §2.9).
    fn draw_text(
        &mut self,
        _at: Pt2,
        _text: &str,
        _size: f64,
        _align: crate::prim::Align,
        _style: Style,
    ) {
        unimplemented!("R-0007: Canvas::draw_text")
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

/// Every primitive is a union of round-capped segments (§2.4). Exhaustive
/// with no `_` arm on purpose: R-0007's new variant must be a compile
/// error, never a silently unrendered label.
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
        // arm, not a wildcard, so a sixth variant is still a compile error.
        Prim2::Text { .. } => {}
    }
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
        | Prim2::Text { style, .. } => *style,
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

    /// Clip an integer destination rectangle to the canvas — the glyph
    /// run's counterpart to `around`, which clips a padded float box.
    /// `around` is deliberately not refactored onto this: that would be
    /// churn in landed logic for symmetry alone (SPEC-0007 §2.13 edit 4).
    fn clip(x0: i64, y0: i64, x1: i64, y1: i64, dims: (u32, u32)) -> Option<Tile> {
        let cx0 = x0.clamp(0, i64::from(dims.0)) as u32;
        let cy0 = y0.clamp(0, i64::from(dims.1)) as u32;
        let cx1 = x1.clamp(0, i64::from(dims.0)) as u32;
        let cy1 = y1.clamp(0, i64::from(dims.1)) as u32;
        (cx0 < cx1 && cy0 < cy1).then_some(Tile {
            x0: cx0,
            y0: cy0,
            x1: cx1,
            y1: cy1,
        })
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
}
