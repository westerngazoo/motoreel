//! R-0013 — glyph coverage is a fraction of a pixel.
//!
//! The fixture face (`fixtures/overlap.ttf`, remade by `overlap.py`) has
//! two glyphs: 'X', two squares that overlap in the middle, and 'U', the
//! one rectangle that is their union. Real faces ship contours like 'X' at
//! stroke junctions; a renderer that sums coverage per contour sees 2
//! there, and a sink that blends with it extrapolates past the ink.

use std::collections::HashMap;

use motoreel_typeset::{measure, place, rasterize, Align, Face, FaceId, Fonts};

fn overlap_face() -> Fonts {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/overlap.ttf");
    let bytes = std::fs::read(path).expect("the committed fixture");
    let mut fonts = Fonts::new();
    fonts.add(Face::load(bytes, "Overlap").expect("a font"));
    fonts
}

/// Every pixel the rasterizer emits for `text`, coverage summed per pixel
/// (one glyph here, so nothing is summed across glyphs).
fn coverage(text: &str, fonts: &Fonts) -> HashMap<(i64, i64), f64> {
    let node = measure(text, FaceId(0), 64.0, fonts).expect("measures");
    let layout = place(&node, (0.0, 0.0), Align::Left);
    let mut out = HashMap::new();
    rasterize(&layout, fonts, |x, y, c| {
        *out.entry((x, y)).or_insert(0.0) += c;
    })
    .expect("rasterizes");
    out
}

/// AC1. No emitted coverage exceeds 1, and the inside of the glyph is
/// exactly 1: the clamp may not dim what was already right.
#[test]
fn ac1_coverage_never_exceeds_one() {
    let fonts = overlap_face();
    let x = coverage("X", &fonts);
    let peak = x.values().copied().fold(0.0, f64::max);
    assert!(
        peak <= 1.0,
        "coverage {peak} on one pixel: it is a fraction of the pixel"
    );
    assert!(
        x.values().any(|&c| c == 1.0),
        "a solid interior is fully covered"
    );
}

/// AC3. Where the contours do not overlap, nothing changes: 'X' and its
/// union 'U' agree exactly on every pixel outside the overlap's span.
#[test]
fn ac3_outside_the_overlap_nothing_changes() {
    let fonts = overlap_face();
    let (x, u) = (coverage("X", &fonts), coverage("U", &fonts));
    assert_eq!(
        x.keys().collect::<std::collections::BTreeSet<_>>(),
        u.keys().collect::<std::collections::BTreeSet<_>>(),
        "the same pixels are inked"
    );
    // the overlap spans x in [300, 600] font units of a 900-unit advance at
    // 64 px per 1000 units: pixels 19.2 to 38.4; stay a pixel clear of it
    for (&(px, py), &cu) in &u {
        if !(19..=39).contains(&px) {
            assert_eq!(x[&(px, py)], cu, "pixel ({px}, {py}) outside the overlap");
        }
    }
}
