//! The label vocabulary and anchor resolution (R-0007).
//!
//! A label names something on screen: a short plain-text run pinned to a
//! world point, to a moving body's pose, or to a fixed image position.
//! Labels are projected, culled and emitted like any other primitive, and
//! render in both sinks.

use garust::{pga, Motor3};

use crate::camera::Projection;
use crate::object::Object;
use crate::prim::{Align, Pt2, Style};
use crate::scene::ObjectId;

/// A plain-text label anchored to the scene (R-0007).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    /// Author text, carried to the sink verbatim.
    ///
    /// Until R-0009 this was "normalized to printable ASCII at eval",
    /// which is how the engine came to render «¿POR QUÉ TANTO?» as
    /// «?POR QU? TANTO?» without anything objecting — a `'?'` is a
    /// perfectly good glyph. What the face cannot draw is now an error
    /// from the sink, naming the character.
    pub text: String,
    /// What the label is pinned to.
    pub anchor: Anchor,
    /// Image-space nudge, applied after projection.
    pub offset: Pt2,
    /// Em height in image units. Contract: finite and > 0.
    pub size: f64,
    /// Horizontal placement of the anchor relative to the run.
    pub align: Align,
    /// Which registered face to set this in — an index into the registry
    /// the sink was built with. `0` is the default and is what a single-
    /// face render uses.
    pub face: usize,
    /// Fill colour and opacity; `width` is unused for text.
    pub style: Style,
}

/// What a label is pinned to.
#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// A world point, projected through the camera exactly as geometry is.
    Point(pga::Point),
    /// A point in `object`'s model space, carried by that object's track —
    /// the anchor that makes a label ride a moving body.
    Pose {
        /// Which object's track to ride.
        object: ObjectId,
        /// The anchor point, in that object's model space.
        at: pga::Point,
    },
    /// A fixed image-space position derived from [`crate::Scene::view`];
    /// independent of camera and of time.
    Screen(ScreenAnchor),
}

/// One of nine fixed image-space positions — R-0007 AC4's 3 × 3 grid.
/// Image space is `y`-up, so `Top` is `+y`.
///
/// **The anchor is on the text baseline, and the run starts at it.** Placing
/// a run is three decisions, not one: the grid point, the [`Align`], and an
/// `offset`. In particular `TopCentre` alone is *not* a centred title — it
/// pins the baseline to the top edge and hangs the run to the right. The
/// title an explainer wants is all three:
///
/// ```
/// # use motoreel::{Align, Anchor, Label, Pt2, ScreenAnchor};
/// Label::new("Angular momentum", Anchor::Screen(ScreenAnchor::TopCentre))
///     .with_align(Align::Center)
///     .with_offset(Pt2 { x: 0.0, y: -0.25 });
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenAnchor {
    /// Top-left corner of the view window.
    TopLeft,
    /// Midpoint of the top edge.
    TopCentre,
    /// Top-right corner.
    TopRight,
    /// Midpoint of the left edge.
    MidLeft,
    /// The view centre — exactly `(+0.0, +0.0)` for every view.
    Centre,
    /// Midpoint of the right edge.
    MidRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Midpoint of the bottom edge.
    BottomCentre,
    /// Bottom-right corner.
    BottomRight,
}

impl Label {
    /// A label with the default size (0.08 image units), [`Align::Left`]
    /// and [`Style::default()`].
    pub fn new(text: impl Into<String>, anchor: Anchor) -> Self {
        Label {
            text: text.into(),
            anchor,
            offset: Pt2 { x: 0.0, y: 0.0 },
            size: 0.08,
            align: Align::Left,
            face: 0,
            style: Style::default(),
        }
    }

    /// Nudge the label in image space, after projection.
    #[must_use]
    pub fn with_offset(mut self, offset: Pt2) -> Self {
        self.offset = offset;
        self
    }

    /// Set the em height in image units.
    #[must_use]
    pub fn with_size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }

    /// Set the horizontal placement of the anchor.
    #[must_use]
    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Set the fill colour and opacity.
    #[must_use]
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set which registered face this label is drawn in.
    #[must_use]
    pub fn with_face(mut self, face: usize) -> Self {
        self.face = face;
        self
    }
}

impl Anchor {
    /// Resolve to an image-space position at time `t`; `None` culls the
    /// whole label (SPEC-0002 §2.5's rule, applied to a label's one vertex).
    ///
    /// `view` is `camera.pose.inverse()`, composed once per `eval`; `window`
    /// is `Scene::view` and is read only by [`Anchor::Screen`].
    pub(crate) fn resolve(
        &self,
        objects: &[Object],
        view: &Motor3,
        projection: Projection,
        window: (f64, f64),
        t: f64,
    ) -> Option<Pt2> {
        match self {
            Anchor::Point(p) => projection.project(&p.transform(view)),
            Anchor::Pose { object, at } => {
                // A stale id is reachable: `Scene`'s fields are public and
                // ids are not scene-scoped. Cull, never panic.
                let obj = objects.get(object.index())?;
                // Character-for-character `project_shape`'s two-step, so a
                // pose-anchored label and a `Shape::Point` on the same
                // track agree bit-for-bit (AC3).
                let to_view = view.compose(&obj.track.eval(t));
                projection.project(&at.transform(&to_view))
            }
            Anchor::Screen(a) => {
                let p = screen_point(*a, window);
                // A view violating its own contract makes the five
                // corner/edge points non-finite; the centre is a literal
                // and survives. Culling here keeps SPEC-0002 §2.2's
                // invariant unconditional (AC4b).
                (p.x.is_finite() && p.y.is_finite()).then_some(p)
            }
        }
    }
}

/// Image space is y-up, so `Top` is `+y`. Halving is exact, and the middle
/// row and column are the literal `0.0` — never arithmetic on `window`, so
/// `Centre` is `(+0.0, +0.0)` for every view.
pub(crate) fn screen_point(anchor: ScreenAnchor, window: (f64, f64)) -> Pt2 {
    let (hw, hh) = (window.0 / 2.0, window.1 / 2.0);
    match anchor {
        ScreenAnchor::TopLeft => Pt2 { x: -hw, y: hh },
        ScreenAnchor::TopCentre => Pt2 { x: 0.0, y: hh },
        ScreenAnchor::TopRight => Pt2 { x: hw, y: hh },
        ScreenAnchor::MidLeft => Pt2 { x: -hw, y: 0.0 },
        ScreenAnchor::Centre => Pt2 { x: 0.0, y: 0.0 },
        ScreenAnchor::MidRight => Pt2 { x: hw, y: 0.0 },
        ScreenAnchor::BottomLeft => Pt2 { x: -hw, y: -hh },
        ScreenAnchor::BottomCentre => Pt2 { x: 0.0, y: -hh },
        ScreenAnchor::BottomRight => Pt2 { x: hw, y: -hh },
    }
}
