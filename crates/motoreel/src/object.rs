//! Scene content: drawable geometry with a style and a motor track.

use garust::{pga, Motor3};

use crate::path::Subpath;
use crate::prim::{Pt2, Style};
use crate::track::Track;

/// Drawable model-space geometry as typed PGA points.
///
/// R-0002's list, plus `Edges` (R-0004) and `Path` (R-0012); derived
/// incidence shapes (`JoinLine`, `MeetPoint`) arrive with M3.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A single point.
    Point(pga::Point),
    /// A straight stroke between two points.
    Segment(pga::Point, pga::Point),
    /// An open polyline through the points in order; close a figure by
    /// repeating its first point. Fewer than two points passes through
    /// unvalidated — drawing nothing (or a dot) is the sink's honest
    /// rendering of empty geometry.
    Polyline(Vec<pga::Point>),
    /// A wireframe as disjoint endpoint pairs — one object, one track,
    /// one style. A solid's edge list generally has no Euler path (every
    /// vertex of a box has odd degree), so a polyline would visibly
    /// retrace edges.
    Edges(Vec<(pga::Point, pga::Point)>),
    /// Straight and cubic pieces in model space, stroked and/or filled
    /// (SPEC-0012 §2.3). Posed by the object's track like every shape:
    /// its control points ride the motor, which is exact for a Bézier.
    Path(Vec<Subpath<pga::Point>>),
}

/// A scene entry: geometry, paint style, and the motor track posing it.
///
/// Plain data — all fields public, any combination valid.
#[derive(Clone, Debug)]
pub struct Object {
    /// Model-space geometry, posed by `track` at evaluation time.
    pub shape: Shape,
    /// Stroke and fill, carried to the emitted primitive unchanged (AC4).
    pub style: Style,
    /// Pose over time (SPEC-0001); a single key means a static object.
    pub track: Track,
}

impl Object {
    /// A point object with default style, holding the identity pose.
    pub fn point(p: pga::Point) -> Self {
        Object::with_shape(Shape::Point(p))
    }

    /// A segment object with default style, holding the identity pose.
    pub fn segment(a: pga::Point, b: pga::Point) -> Self {
        Object::with_shape(Shape::Segment(a, b))
    }

    /// A polyline object with default style, holding the identity pose.
    pub fn polyline(points: Vec<pga::Point>) -> Self {
        Object::with_shape(Shape::Polyline(points))
    }

    /// A wireframe object with default style, holding the identity pose.
    pub fn edges(segments: Vec<(pga::Point, pga::Point)>) -> Self {
        Object::with_shape(Shape::Edges(segments))
    }

    /// A path object with default style, holding the identity pose.
    pub fn path(subpaths: Vec<Subpath<pga::Point>>) -> Self {
        Object::with_shape(Shape::Path(subpaths))
    }

    /// A path authored in a plane — the output of [`crate::shapes`] —
    /// with each `(x, y)` lifted to the model's `z = 0` plane. Default
    /// style, identity pose.
    pub fn planar(subpaths: Vec<Subpath<Pt2>>) -> Self {
        let lift = |p: &Pt2| pga::Point::new(p.x, p.y, 0.0);
        Object::path(subpaths.iter().map(|s| s.map(lift)).collect())
    }

    /// Replace the style (builder).
    #[must_use]
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Replace the track (builder).
    #[must_use]
    pub fn with_track(mut self, track: Track) -> Self {
        self.track = track;
        self
    }

    /// Hold a fixed pose: replaces the track with `Track::hold(pose)`.
    #[must_use]
    pub fn at(self, pose: Motor3) -> Self {
        self.with_track(Track::hold(pose))
    }

    fn with_shape(shape: Shape) -> Self {
        Object {
            shape,
            style: Style::default(),
            track: Track::hold(Motor3::identity()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Object;
    use garust::{pga, Motor3};

    /// Constructors give the default style and an identity hold; `at`
    /// swaps in a fixed pose.
    #[test]
    fn constructors_default_to_identity_hold() {
        let obj = Object::point(pga::Point::new(1.0, 0.0, 0.0));
        assert_eq!(obj.track.eval(3.0), Motor3::identity());
        let posed = obj.at(Motor3::translator(1.0, 0.0, 0.0));
        assert_eq!(posed.track.eval(3.0), Motor3::translator(1.0, 0.0, 0.0));
    }
}
