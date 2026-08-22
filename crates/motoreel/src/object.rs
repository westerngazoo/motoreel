//! Scene content: drawable geometry with a style and a motor track.

use garust::{pga, Motor3};

use crate::prim::Style;
use crate::track::Track;

/// Drawable model-space geometry as typed PGA points.
///
/// Exactly R-0002's list; derived incidence shapes (`JoinLine`,
/// `MeetPoint`) arrive with M3 (R-0007).
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
}

/// A scene entry: geometry, stroke style, and the motor track posing it.
///
/// Plain data — all fields public, any combination valid.
#[derive(Clone, Debug)]
pub struct Object {
    /// Model-space geometry, posed by `track` at evaluation time.
    pub shape: Shape,
    /// Stroke style, carried to the emitted primitive unchanged (AC4).
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
