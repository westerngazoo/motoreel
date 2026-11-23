//! Fewer primitives for a canvas that pays per draw call (R-0022 AC6,
//! SPEC-0022 §2.4): consecutive opaque paths of one style become one path.
//!
//! **Opt-in, and it changes the list.** Nothing in the render path calls
//! this: `Scene::eval_with` keeps one primitive per face, byte for byte,
//! because the goldens and the reels depend on it. A consumer that draws
//! each primitive with its own call (a Flutter `CustomPainter`, a 2D web
//! canvas) calls it on the frame's list and draws a quarter to a half of
//! the paths (the fitAI spike's skin-only toy went from 1 639 to 429 and
//! its paint recording from 20 to 6.8 ms; the three-layer toy goes from
//! about 2 600 to 1 100). Only runs that are **next to each other**
//! merge, so nothing is drawn earlier than before and the painter's order
//! holds.
//!
//! **It is not pixel-identical.** A sink fills a path's subpaths as one
//! nonzero union and strokes them once, while separate paths composite
//! one over the other. Inside a run, where every piece is the same opaque
//! colour, that is the same colour either way; antialiased pixels that two
//! pieces of a run both touch get their coverage combined once instead of
//! composited twice, and change. R-0022's acceptance test measures it on
//! the toy: under 1 % of the pixels, on edges, plus a few seam pixels
//! inside a run by at most 2 levels.

use crate::prim::{Fill, Paint, Prim2, Style};

/// `prims` with every run of consecutive [`Prim2::Path`]s that share one
/// opaque style merged into a single path, the run's subpaths in order.
///
/// A style is opaque when its fill is a flat colour at alpha `1` and its
/// stroke is either opaque (`alpha == 1`) or not drawn (`width == 0` or
/// `alpha == 0`). Translucent paths are never merged: compositing them
/// once instead of one over the other would change their colour.
/// Gradient fills, points, segments, polylines, edges and text pass
/// through unchanged, and break a run.
#[must_use]
pub fn coalesce(prims: Vec<Prim2>) -> Vec<Prim2> {
    let mut out: Vec<Prim2> = Vec::with_capacity(prims.len());
    for prim in prims {
        let joins = matches!(
            (&prim, out.last()),
            (Prim2::Path { style, .. }, Some(Prim2::Path { style: last, .. }))
                if style == last && opaque(style)
        );
        if !joins {
            out.push(prim);
        } else if let (Prim2::Path { subpaths, .. }, Some(Prim2::Path { subpaths: run, .. })) =
            (prim, out.last_mut())
        {
            run.extend(subpaths);
        }
    }
    out
}

/// Whether painting `style` twice over the same pixel gives the colour
/// painting it once does.
fn opaque(style: &Style) -> bool {
    let fill = matches!(
        style.fill,
        Some(Fill {
            paint: Paint::Flat(_),
            alpha,
        }) if alpha == 1.0
    );
    let stroke = style.alpha == 1.0 || style.width == 0.0 || style.alpha == 0.0;
    fill && stroke
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Subpath;
    use crate::prim::{Pt2, Rgb};

    fn square(x: f64) -> Vec<Subpath<Pt2>> {
        crate::shapes::polygon(&[
            Pt2 { x, y: 0.0 },
            Pt2 { x: x + 1.0, y: 0.0 },
            Pt2 { x: x + 1.0, y: 1.0 },
            Pt2 { x, y: 1.0 },
        ])
    }

    fn path(x: f64, colour: Rgb, alpha: f64) -> Prim2 {
        Prim2::Path {
            subpaths: square(x),
            style: Style {
                stroke: colour,
                width: 2.0,
                alpha: 1.0,
                fill: Some(Fill::solid(colour, alpha)),
            },
        }
    }

    const RED: Rgb = Rgb { r: 200, g: 0, b: 0 };

    #[test]
    fn a_run_of_one_opaque_style_becomes_one_path() {
        let merged = coalesce(vec![
            path(0.0, RED, 1.0),
            path(1.0, RED, 1.0),
            path(2.0, RED, 1.0),
        ]);
        let Prim2::Path { style, .. } = path(0.0, RED, 1.0) else {
            unreachable!("built as a path");
        };
        let subpaths = [0.0, 1.0, 2.0].into_iter().flat_map(square).collect();
        assert_eq!(merged, vec![Prim2::Path { subpaths, style }]);
    }

    #[test]
    fn only_neighbours_merge_so_the_order_holds() {
        let list = vec![
            path(0.0, RED, 1.0),
            path(1.0, Rgb::WHITE, 1.0),
            path(2.0, RED, 1.0),
        ];
        assert_eq!(coalesce(list.clone()), list);
    }

    #[test]
    fn translucent_paths_are_left_alone() {
        let list = vec![path(0.0, RED, 0.5), path(1.0, RED, 0.5)];
        assert_eq!(coalesce(list.clone()), list);
    }

    #[test]
    fn a_translucent_stroke_is_left_alone() {
        let mut list = vec![path(0.0, RED, 1.0), path(1.0, RED, 1.0)];
        for p in &mut list {
            if let Prim2::Path { style, .. } = p {
                style.alpha = 0.3;
            }
        }
        assert_eq!(coalesce(list.clone()), list);
    }

    #[test]
    fn other_primitives_break_a_run() {
        let dot = Prim2::Point {
            at: Pt2 { x: 0.0, y: 0.0 },
            style: Style::default(),
        };
        let list = vec![path(0.0, RED, 1.0), dot, path(1.0, RED, 1.0)];
        assert_eq!(coalesce(list.clone()), list);
    }
}
