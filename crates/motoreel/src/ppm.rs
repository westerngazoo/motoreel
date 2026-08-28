//! `PpmSink`: binary P6 raster frames and the in-crate stroke rasterizer.
//!
//! The sink R-0006 exists for: ffmpeg has an `svg_pipe` demuxer but no SVG
//! *decoder* without librsvg, so the command motoreel documented never
//! worked on a stock build. P6 is decoded by every ffmpeg ever shipped.
//!
//! Ink lands where [`crate::SvgSink`] puts it (SPEC-0006 §2.3): the two
//! sinks share no code, only a documented mapping. Byte-determinism is
//! SPEC-0003's discipline unchanged — no clock, no environment, no map
//! iteration, and no `mul_add` on the write path.

use std::io;
use std::path::{Path, PathBuf};

use crate::prim::{Prim2, Pt2, Rgb, Style};
use crate::sink::FrameSink;

/// A point in pixel space: x right, y **down**, pixel centres at `+0.5`.
type Px = (f64, f64);

/// Writes one `frame_%05d.ppm` per frame into a directory (RFC-012 §3.3).
///
/// Stroke widths are in image (view) units exactly as for `SvgSink`, so
/// swapping one sink for the other reproduces the same picture.
pub struct PpmSink {
    #[allow(dead_code)]
    dir: PathBuf,
    /// Frozen at construction: ASCII, no float formatting (§2.2).
    #[allow(dead_code)]
    header: Vec<u8>,
    #[allow(dead_code)]
    canvas: Canvas,
}

impl PpmSink {
    /// Sink into `dir` (created now, parents included): 1920×1080 pixels
    /// over the default 3.2 × 1.8 centred view window, on black.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        PpmSink::with_view(dir, (1920, 1080), (3.2, 1.8))
    }

    /// Sink into `dir` with an explicit raster size (px) and centred
    /// image-space view window — the pair `SvgSink::with_view` takes,
    /// mapped identically (§2.3).
    ///
    /// Zero raster dimensions, a non-finite/non-positive view, or a frame
    /// whose `w·h·3` byte count does not fit in `usize` are
    /// [`io::ErrorKind::InvalidInput`], rejected before the directory is
    /// created. Allocation failure is [`io::ErrorKind::OutOfMemory`].
    pub fn with_view(
        _dir: impl AsRef<Path>,
        _size: (u32, u32),
        _view: (f64, f64),
    ) -> io::Result<Self> {
        unimplemented!("R-0006: PpmSink::with_view")
    }

    /// Composite alpha against `background` instead of black. PPM has no
    /// alpha channel, so the background is explicit rather than implied.
    pub fn with_background(self, _background: Rgb) -> Self {
        unimplemented!("R-0006: PpmSink::with_background")
    }
}

impl FrameSink for PpmSink {
    fn frame(&mut self, _index: usize, _prims: &[Prim2]) -> io::Result<()> {
        unimplemented!("R-0006: PpmSink::frame")
    }
}

/// Pixel canvas: the pinned mapping plus the coverage rasterizer.
#[allow(dead_code)]
struct Canvas {
    size: (u32, u32),
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

#[allow(dead_code)]
impl Canvas {
    /// Refill every pixel with the background triple (§2.6).
    fn clear(&mut self) {
        unimplemented!("R-0006: Canvas::clear")
    }

    /// Rasterize one primitive: union coverage of its round-capped
    /// segments, composited src-over in a single pass (§2.5).
    fn draw(&mut self, _prim: &Prim2) {
        unimplemented!("R-0006: Canvas::draw")
    }

    fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// Image space (y-up, centred) → pixel space (y-down, centres at `+0.5`).
/// The closed form of `SvgSink`'s `scale(1 -1)` ∘ viewBox ∘ `xMidYMid meet`
/// chain — §2.3 derives it; the two sinks agree because of this function.
#[allow(dead_code)]
fn to_pixel(_p: Pt2, _scale: f64, _size: (u32, u32)) -> Px {
    unimplemented!("R-0006: to_pixel")
}

/// Every primitive is a union of round-capped segments (§2.4). Exhaustive
/// with no `_` arm on purpose: R-0007's new variant must be a compile
/// error, never a silently unrendered label.
#[allow(dead_code)]
fn push_segments(_prim: &Prim2, _map: impl Fn(Pt2) -> Px, _out: &mut Vec<(Px, Px)>) {
    unimplemented!("R-0006: push_segments")
}

/// Distance in pixels from `p` to segment `a`–`b`. A degenerate segment
/// (`a == b`) gives the distance to the point — which is what makes a
/// `Point` a disc and a round cap a cap (§2.4).
#[allow(dead_code)]
fn distance_to_segment(_p: Px, _a: Px, _b: Px) -> f64 {
    unimplemented!("R-0006: distance_to_segment")
}

/// Coverage of a pixel whose centre lies `d` px from the centre-line of a
/// stroke of radius `r` px: the pinned one-pixel ramp — 1 inside, 0.5 on
/// the boundary, 0 outside (§2.5).
#[allow(dead_code)]
fn coverage(_d: f64, _r: f64) -> f64 {
    unimplemented!("R-0006: coverage")
}

/// Source-over in 8-bit sRGB — SVG's default `color-interpolation` — with
/// one pinned rounding (`f64::round`, ties away from zero).
#[allow(dead_code)]
fn src_over(_dst: &mut [u8], _src: Rgb, _a: f64) {
    unimplemented!("R-0006: src_over")
}

/// The style every `Prim2` variant carries (SPEC-0002 §2.2).
#[allow(dead_code)]
fn style_of(_prim: &Prim2) -> Style {
    unimplemented!("R-0006: style_of")
}

/// Both coordinates finite — the guard that keeps `floor`/`ceil` → `u32`
/// from saturating into a nonsense tile (§2.9).
#[allow(dead_code)]
fn finite(p: Px) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// Total clamp to `[0, 1]`; NaN maps to 0, so a style violating its
/// documented contract paints nothing rather than poisoning the frame
/// (§2.5). **Not `x.clamp(0.0, 1.0)`** — `f64::clamp` returns NaN for NaN.
#[allow(dead_code)]
fn unit(_x: f64) -> f64 {
    unimplemented!("R-0006: unit")
}

/// A half-open pixel rectangle clamped to the canvas — the region ink can
/// reach. `None` when empty (entirely off-canvas, or no segments).
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Tile {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

#[allow(dead_code)]
impl Tile {
    fn around(_segments: &[(Px, Px)], _pad: f64, _size: (u32, u32)) -> Option<Tile> {
        unimplemented!("R-0006: Tile::around")
    }
    fn intersect(self, _other: Option<Tile>) -> Option<Tile> {
        unimplemented!("R-0006: Tile::intersect")
    }
    fn area(self) -> usize {
        unimplemented!("R-0006: Tile::area")
    }
    fn offset(self, _x: u32, _y: u32) -> usize {
        unimplemented!("R-0006: Tile::offset")
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
        push_segments(&Prim2::Point { at: p(1.0, 2.0), style }, id, &mut out);
        assert_eq!(out, vec![((1.0, 2.0), (1.0, 2.0))], "a point is degenerate");

        out.clear();
        push_segments(
            &Prim2::Polyline { points: vec![p(0.0, 0.0)], style },
            id,
            &mut out,
        );
        assert!(out.is_empty(), "a 1-point polyline yields no segments");

        out.clear();
        push_segments(
            &Prim2::Polyline { points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], style },
            id,
            &mut out,
        );
        assert_eq!(out.len(), 2, "n points yield n-1 segments");
    }
}
