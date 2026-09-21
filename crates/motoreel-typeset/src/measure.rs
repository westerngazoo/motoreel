//! Text to boxes. No pixels, no buffer, no sink — R-0009 AC4.
//!
//! The face is consulted here and nowhere after: what comes back is a
//! self-describing tree that a layout gate can interrogate for free. That
//! separation is the thing the Python factory never had — `choques.py`
//! has to render a frame and spy on the draw calls to find out how big a
//! label is. Here it is arithmetic.

use ab_glyph::{Font as _, ScaleFont as _};

use crate::face::{Error, Fonts};
use crate::node::{FaceId, Glyph, Metrics, Node};
use crate::scale::px;

/// Measure a run of text in one face at one size.
///
/// Returns a [`Node::Row`] of glyphs, with kerning folded into each
/// glyph's advance, so the row's width is the sum of its parts and stays
/// so under any later edit to the tree.
///
/// # Errors
/// [`Error::Missing`] naming the **first** character the face cannot
/// draw. Call [`crate::Face::missing`] first when you want all of them.
/// [`Error::NoSuchFace`] if `face` is not registered.
pub fn measure(text: &str, face: FaceId, size: f64, fonts: &Fonts) -> Result<Node, Error> {
    let f = fonts.get(face)?;
    let scaled = f.font().as_scaled(px(size));

    let chars: Vec<char> = text.chars().collect();
    let mut items: Vec<Node> = Vec::with_capacity(chars.len());

    for (i, &ch) in chars.iter().enumerate() {
        let id = f.font().glyph_id(ch);
        if id.0 == 0 {
            return Err(Error::Missing {
                ch,
                face,
                name: f.name().to_owned(),
            });
        }

        // Kerning belongs to the *pair*, and folding it into the left
        // glyph keeps `Row`'s width a plain sum. Attaching it to the
        // right glyph instead would make the first glyph's advance depend
        // on what precedes the row, which is exactly the kind of context
        // a self-describing tree must not carry.
        let next = chars.get(i + 1).map(|&c| f.font().glyph_id(c));
        let kern = next.map_or(0.0, |n| f64::from(scaled.kern(id, n)));
        let advance = f64::from(scaled.h_advance(id)) + kern;

        // `outline_glyph` returns `None` for a glyph with no contours —
        // a space, most often. That is ink of zero, not an error: the
        // advance still moves the pen.
        let positioned = id.with_scale_and_position(px(size), ab_glyph::point(0.0, 0.0));
        let (bearing, ink) =
            f.font()
                .outline_glyph(positioned)
                .map_or((0.0, Metrics::default()), |o| {
                    let b = o.px_bounds();
                    // ab_glyph's pixel space runs y **down** from the
                    // baseline, so what is above the baseline has a negative
                    // `min.y`. `Metrics` wants both as non-negative
                    // distances (see `node::Metrics`). `min.x` is the left
                    // side bearing and is signed: a `j` reaches left of the
                    // pen and reports it as negative.
                    (
                        f64::from(b.min.x),
                        Metrics {
                            width: f64::from(b.width()),
                            height: f64::from(-b.min.y).max(0.0),
                            depth: f64::from(b.max.y).max(0.0),
                        },
                    )
                });

        items.push(Node::Glyph(Glyph {
            face,
            id: id.0,
            size,
            advance,
            bearing,
            ink,
        }));
    }

    Ok(Node::Row(items))
}

#[cfg(test)]
mod tests {
    // Exactness is the assertion, not an accident: these boxes are built
    // from small decimals whose sums are exact in binary floating point,
    // and a test that tolerated drift would stop catching the arithmetic
    // it exists to pin down.
    #![allow(
        clippy::float_cmp,
        reason = "the box arithmetic here is exact by construction"
    )]

    use super::*;
    use crate::face::Face;

    /// A face with real coverage, loaded from the system. Tests that need
    /// one skip rather than fail when it is absent, so the suite stays
    /// green on a machine without it — but `tests/spanish.rs` is the
    /// acceptance gate and it does not skip.
    fn a_face() -> Option<Fonts> {
        let candidates = [
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        for p in candidates {
            if let Ok(bytes) = std::fs::read(p) {
                if let Ok(face) = Face::load(bytes, p) {
                    let mut fonts = Fonts::new();
                    fonts.add(face);
                    return Some(fonts);
                }
            }
        }
        None
    }

    #[test]
    fn an_empty_run_is_an_empty_row() {
        let Some(fonts) = a_face() else { return };
        let n = measure("", FaceId(0), 32.0, &fonts).unwrap();
        assert_eq!(n, Node::Row(vec![]));
        assert_eq!(n.metrics(), Metrics::default());
    }

    #[test]
    fn a_missing_face_is_an_error_not_a_panic() {
        let fonts = Fonts::new();
        assert_eq!(
            measure("x", FaceId(0), 10.0, &fonts).unwrap_err(),
            Error::NoSuchFace(FaceId(0))
        );
    }

    #[test]
    fn type_is_proportional_not_monospaced() {
        let Some(fonts) = a_face() else { return };
        let i = measure("i", FaceId(0), 64.0, &fonts)
            .unwrap()
            .metrics()
            .width;
        let m = measure("m", FaceId(0), 64.0, &fonts)
            .unwrap()
            .metrics()
            .width;
        assert!(
            m > i * 1.5,
            "an 'm' must be much wider than an 'i'; got {m} and {i}. \
             The old face advanced every glyph by one cell, which is the \
             assumption `width = advance * len()` baked into PpmSink."
        );
    }

    #[test]
    fn a_space_has_advance_but_no_ink() {
        let Some(fonts) = a_face() else { return };
        let n = measure(" ", FaceId(0), 64.0, &fonts).unwrap();
        assert!(n.metrics().width > 0.0, "a space still moves the pen");
        assert!(n.ink().is_empty(), "and covers nothing");
    }

    #[test]
    fn a_descender_reaches_below_the_baseline() {
        let Some(fonts) = a_face() else { return };
        let x = measure("x", FaceId(0), 64.0, &fonts).unwrap();
        let p = measure("p", FaceId(0), 64.0, &fonts).unwrap();
        assert_eq!(x.ink().depth, 0.0, "an 'x' sits on the baseline");
        assert!(p.ink().depth > 0.0, "a 'p' hangs below it");
        assert!(
            x.ink().left >= 0.0 || x.ink().left > -1.0,
            "an 'x' does not overhang much either way"
        );
    }

    #[test]
    fn width_grows_with_size() {
        let Some(fonts) = a_face() else { return };
        let small = measure("abc", FaceId(0), 20.0, &fonts)
            .unwrap()
            .metrics()
            .width;
        let big = measure("abc", FaceId(0), 40.0, &fonts)
            .unwrap()
            .metrics()
            .width;
        assert!((big / small - 2.0).abs() < 0.05, "{big} vs {small}");
    }
}
