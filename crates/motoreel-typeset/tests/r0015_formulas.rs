//! R-0015 — inline formulas. Red until SPEC-0015's code outline is signed:
//! run with `cargo test -p motoreel-typeset --features r0015`.
#![cfg(feature = "r0015")]

use motoreel_typeset::math::{parse, set, width, FormulaStyle, Piece};
use motoreel_typeset::outline::{outline, Seg};
use motoreel_typeset::{Face, FaceId, Fonts};

/// A mono face with ₁ ² − + √, found the way the R-0009 gate finds faces.
fn mono() -> Fonts {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Menlo.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ];
    for path in CANDIDATES {
        if let Some(face) = std::fs::read(path)
            .ok()
            .and_then(|b| Face::load(b, *path).ok())
        {
            if face.missing("₁²−+√e").is_empty() {
                let mut fonts = Fonts::new();
                fonts.add(face);
                return fonts;
            }
        }
    }
    panic!("no mono face with ₁²−+√ found; tried {CANDIDATES:?}");
}

/// The rotorf-sico rule: vectors (a, b, v, z, and e before an index) are italic.
fn vectors(c: char, next: Option<char>) -> bool {
    matches!(c, 'a' | 'b' | 'v' | 'z') || (c == 'e' && matches!(next, Some('₁' | '₂' | '₃')))
}

fn x_extent(cs: &[motoreel_typeset::outline::Contour]) -> (f64, f64) {
    let xs = cs.iter().flat_map(|c| {
        std::iter::once(c.start.0).chain(c.segs.iter().flat_map(|s| match *s {
            Seg::Line(p) => vec![p.0],
            Seg::Cubic(a, b, p) => vec![a.0, b.0, p.0],
        }))
    });
    xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
        (lo.min(x), hi.max(x))
    })
}

#[test]
fn ac1_reads_scripts_spaces_and_radicals() {
    let g = |t: &str, italic: bool| Piece::Glyph {
        text: t.into(),
        italic,
    };
    assert_eq!(
        parse("(e₁e₂)² = −1", vectors),
        vec![
            g("(", false),
            g("e", true),
            Piece::Sub("1".into()),
            g("e", true),
            Piece::Sub("2".into()),
            g(")", false),
            Piece::Sup("2".into()),
            Piece::Space,
            g("=", false),
            Piece::Space,
            g("−", false),
            g("1", false)
        ]
    );
    assert_eq!(
        parse("√−1 x", vectors),
        vec![
            Piece::Root(vec![g("−", false), g("1", false)]),
            Piece::Space,
            g("x", false)
        ]
    );
}

#[test]
fn ac2_scripts_are_smaller_and_shifted() {
    let fonts = mono();
    let st = FormulaStyle::new(FaceId(0), FaceId(0));
    let base = outline("1", FaceId(0), 100.0, 0.0, &fonts).unwrap();
    let sub = set(&[Piece::Sub("1".into())], &st, 100.0, &fonts).unwrap();
    assert!((sub.advance - 0.72 * base.advance).abs() < 1e-9);
    assert!(
        (sub.contours[0].start.1 - (0.72 * base.contours[0].start.1 - 22.0)).abs() < 1e-9,
        "lowered 0.22 em"
    );
    let sup = set(&[Piece::Sup("2".into())], &st, 100.0, &fonts).unwrap();
    let two = outline("2", FaceId(0), 62.0, 0.0, &fonts).unwrap();
    assert!(
        (sup.advance - (two.advance - 4.0)).abs() < 1e-9,
        "pulled 0.04 em back"
    );
    let floor = FormulaStyle {
        sup_min: 80.0,
        ..st
    };
    let big = set(&[Piece::Sup("2".into())], &floor, 100.0, &fonts).unwrap();
    assert!(
        (big.advance - (outline("2", FaceId(0), 80.0, 0.0, &fonts).unwrap().advance - 4.0)).abs()
            < 1e-9
    );
}

#[test]
fn ac3_the_minus_is_as_wide_as_the_plus() {
    let fonts = mono();
    let st = FormulaStyle::new(FaceId(0), FaceId(0));
    let minus = set(&parse("−", vectors), &st, 100.0, &fonts).unwrap();
    let plus = outline("+", FaceId(0), 100.0, 0.0, &fonts).unwrap();
    let (a, b) = (x_extent(&minus.contours), x_extent(&plus.contours));
    assert!(((a.1 - a.0) - (b.1 - b.0)).abs() < 1e-9 && ((a.0 + a.1) - (b.0 + b.1)).abs() < 1e-9);
    assert_eq!(
        minus.advance,
        outline("−", FaceId(0), 100.0, 0.0, &fonts).unwrap().advance
    );
}

#[test]
fn ac4_the_radical_has_its_bar() {
    let fonts = mono();
    let st = FormulaStyle::new(FaceId(0), FaceId(0));
    let root = set(&parse("√1", vectors), &st, 100.0, &fonts).unwrap();
    let sign = outline("√", FaceId(0), 100.0, 0.0, &fonts).unwrap();
    let one = outline("1", FaceId(0), 82.0, 0.0, &fonts).unwrap();
    assert!((root.advance - (sign.advance + 4.0 + one.advance + 8.0)).abs() < 1e-9);
    // one more contour than the √ and the 1: the bar, a rectangle 9.4 tall reaching the end
    assert_eq!(
        root.contours.len(),
        sign.contours.len() + one.contours.len() + 1
    );
    let bar = root.contours.last().unwrap();
    assert_eq!(bar.segs.len(), 4);
    assert!((x_extent(std::slice::from_ref(bar)).1 - root.advance).abs() < 1e-9);
}

#[test]
fn ac5_width_is_the_set_advance() {
    let fonts = mono();
    let st = FormulaStyle::new(FaceId(0), FaceId(0));
    for f in ["(e₁e₂)² = −1", "z i³ = −z i", "√−1"] {
        let p = parse(f, vectors);
        let a = set(&p, &st, 64.0, &fonts).unwrap();
        assert_eq!(width(&p, &st, 64.0, &fonts).unwrap(), a.advance, "«{f}»");
        assert_eq!(set(&p, &st, 64.0, &fonts).unwrap(), a, "deterministic");
    }
}
