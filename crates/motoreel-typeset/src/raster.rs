//! The only module that produces pixels — and it produces **coverage**,
//! not colour.
//!
//! A sink is handed `(x, y, alpha)` and decides what to do with it. That
//! keeps this crate ignorant of `Rgb`, of blending, and of whether the
//! target is a PPM buffer or something else, and it is why `motoreel`'s
//! own primitive vocabulary does not have to grow a colour model it
//! shares with this one.
//!
//! Coordinates are motoreel's: **x right, y up**, integers, absolute.

use ab_glyph::Font as _;

use crate::face::{Error, Fonts};
use crate::place::Layout;
use crate::scale::{grid, px, round, split};

/// Draw a laid-out run, calling `emit(x, y, alpha)` once per covered
/// pixel with `alpha` in `[0, 1]`.
///
/// Subpixel horizontal position is preserved: a glyph whose baseline
/// origin falls between two pixels is rasterized at that offset rather
/// than snapped, which is most of the difference between type that looks
/// set and type that looks placed.
///
/// # Errors
/// [`Error::NoSuchFace`] if a glyph names a face that is not registered.
pub fn rasterize<F>(layout: &Layout, fonts: &Fonts, mut emit: F) -> Result<(), Error>
where
    F: FnMut(i64, i64, f64),
{
    for g in &layout.glyphs {
        let face = fonts.get(g.face)?;
        // Split each coordinate into a whole-pixel part we add back as an
        // integer and a fractional part we hand to the rasterizer. Doing
        // it this way also keeps the f32 the rasterizer wants small, so a
        // glyph near the bottom of a 1920-tall frame is positioned with
        // the same precision as one at the top.
        let (ix, fx) = split(g.x);
        let (iy, fy) = split(g.y);
        let positioned =
            ab_glyph::GlyphId(g.id).with_scale_and_position(px(g.size), ab_glyph::point(fx, fy));
        let Some(outline) = face.font().outline_glyph(positioned) else {
            continue; // no contours — a space, and its advance is already spent
        };
        let b = outline.px_bounds();
        let x0 = ix + grid(b.min.x);
        // ab_glyph rasterizes into a y-down box; motoreel is y-up, so the
        // box's top edge (`min.y`, negative above the baseline) becomes
        // the largest y and rows count downward from it.
        let top = iy - grid(b.min.y);
        outline.draw(|dx, dy, c| {
            if c > 0.0 {
                emit(x0 + i64::from(dx), top - i64::from(dy), f64::from(c));
            }
        });
    }

    for r in &layout.rules {
        // A rule is exact: no antialiasing, no outline, just the pixels
        // inside it. Half-covered edges on a fraction bar read as a
        // blurry line, which is worse than a hard one.
        let x0 = round(r.x);
        let x1 = round(r.x + r.metrics.width);
        let y0 = round(r.y - r.metrics.depth);
        let y1 = round(r.y + r.metrics.height);
        for y in y0..y1 {
            for x in x0..x1 {
                emit(x, y, 1.0);
            }
        }
    }

    Ok(())
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
    use crate::node::Metrics;
    use crate::place::{Layout, PlacedRule};

    #[test]
    fn a_rule_fills_exactly_its_box() {
        let layout = Layout {
            glyphs: vec![],
            rules: vec![PlacedRule {
                x: 10.0,
                y: 100.0,
                metrics: Metrics {
                    width: 4.0,
                    height: 2.0,
                    depth: 0.0,
                },
            }],
        };
        let mut hit = Vec::new();
        rasterize(&layout, &Fonts::new(), |x, y, a| hit.push((x, y, a))).unwrap();
        assert_eq!(hit.len(), 8, "4 wide by 2 tall");
        assert!(hit.iter().all(|&(_, _, a)| a == 1.0), "no soft edges");
        assert!(hit
            .iter()
            .all(|&(x, y, _)| (10..14).contains(&x) && (100..102).contains(&y)));
    }

    #[test]
    fn an_empty_layout_emits_nothing() {
        let mut n = 0;
        rasterize(&Layout::default(), &Fonts::new(), |_, _, _| n += 1).unwrap();
        assert_eq!(n, 0);
    }
}
