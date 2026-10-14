//! R-0014 — glyph outlines. Red until SPEC-0014's code outline is signed:
//! run with `cargo test -p motoreel-typeset --features r0014`.
#![cfg(feature = "r0014")]

use motoreel_typeset::outline::{outline, Seg};
use motoreel_typeset::{measure, Error, Face, FaceId, Fonts};

/// A face the way the R-0009 gate finds one: a failure, not a skip.
fn face() -> Fonts {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for path in CANDIDATES {
        if let Some(face) = std::fs::read(path)
            .ok()
            .and_then(|b| Face::load(b, *path).ok())
        {
            let mut fonts = Fonts::new();
            fonts.add(face);
            return fonts;
        }
    }
    panic!("no face found; tried {CANDIDATES:?}");
}

#[test]
fn ac1_contours_are_closed_and_cubic() {
    let fonts = face();
    let o = outline("Rg", FaceId(0), 100.0, 0.0, &fonts).unwrap();
    assert!(o.contours.len() >= 3, "R has two contours, g at least one");
    for c in &o.contours {
        let end = match c.segs.last().expect("a contour has segments") {
            Seg::Line(p) | Seg::Cubic(_, _, p) => *p,
        };
        assert!(
            (end.0 - c.start.0).abs() < 1e-9 && (end.1 - c.start.1).abs() < 1e-9,
            "a contour closes"
        );
    }
    // y up: the R stands on the baseline and rises above it
    let top = o
        .contours
        .iter()
        .flat_map(|c| c.segs.iter())
        .map(|s| match s {
            Seg::Line(p) | Seg::Cubic(_, _, p) => p.1,
        });
    assert!(top.fold(f64::NEG_INFINITY, f64::max) > 50.0);
}

#[test]
fn ac2_advance_is_the_measured_width() {
    let fonts = face();
    for text in ["AV", "Física", "e₁e₂"] {
        let Ok(node) = measure(text, FaceId(0), 64.0, &fonts) else {
            continue;
        };
        let o = outline(text, FaceId(0), 64.0, 0.0, &fonts).unwrap();
        assert!((o.advance - node.metrics().width).abs() < 1e-9, "«{text}»");
    }
}

#[test]
fn ac3_spacing_goes_between_letters() {
    let fonts = face();
    let a = outline("abc", FaceId(0), 50.0, 0.0, &fonts).unwrap();
    let b = outline("abc", FaceId(0), 50.0, 7.0, &fonts).unwrap();
    assert!(
        (b.advance - a.advance - 14.0).abs() < 1e-9,
        "two gaps, none after the last letter"
    );
}

#[test]
fn ac4_missing_glyph_is_an_error() {
    let fonts = face();
    assert!(matches!(
        outline("a\u{E000}", FaceId(0), 50.0, 0.0, &fonts),
        Err(Error::Missing { .. })
    ));
}

#[test]
fn ac5_affine_maps_are_exact() {
    let fonts = face();
    let o = outline("S", FaceId(0), 80.0, 0.0, &fonts).unwrap();
    let (s, c) = 0.3f64.sin_cos();
    let turn = |p: (f64, f64)| (c * p.0 - s * p.1 + 5.0, s * p.0 + c * p.1 - 2.0);
    let back = |p: (f64, f64)| {
        (
            c * (p.0 - 5.0) + s * (p.1 + 2.0),
            -s * (p.0 - 5.0) + c * (p.1 + 2.0),
        )
    };
    for k in &o.contours {
        let r = k.map(turn).map(back);
        assert!((r.start.0 - k.start.0).abs() < 1e-9 && (r.start.1 - k.start.1).abs() < 1e-9);
        assert_eq!(r.segs.len(), k.segs.len());
    }
}
