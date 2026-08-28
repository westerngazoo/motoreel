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
    /// Author text; normalized to printable ASCII at eval (§2.5).
    pub text: String,
    /// What the label is pinned to.
    pub anchor: Anchor,
    /// Image-space nudge, applied after projection.
    pub offset: Pt2,
    /// Em height in image units. Contract: finite and > 0.
    pub size: f64,
    /// Horizontal placement of the anchor relative to the run.
    pub align: Align,
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

    /// Whether every character renders as authored — `false` when §2.5's
    /// `'?'` substitution would fire. Pure; `eval` stays total.
    pub fn is_ascii_renderable(&self) -> bool {
        unimplemented!("R-0007: Label::is_ascii_renderable")
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
        _objects: &[Object],
        _view: &Motor3,
        _projection: Projection,
        _window: (f64, f64),
        _t: f64,
    ) -> Option<Pt2> {
        unimplemented!("R-0007: Anchor::resolve")
    }
}

/// Image space is y-up, so `Top` is `+y`. Halving is exact, and the middle
/// row and column are the literal `0.0` — never arithmetic on `window`, so
/// `Centre` is `(+0.0, +0.0)` for every view.
pub(crate) fn screen_point(_anchor: ScreenAnchor, _window: (f64, f64)) -> Pt2 {
    unimplemented!("R-0007: screen_point")
}

/// Printable ASCII — the alphabet the face covers and the SVG escaper
/// assumes.
pub(crate) fn is_renderable(_c: char) -> bool {
    unimplemented!("R-0007: is_renderable")
}

/// R-0007 AC8, total: every unrenderable `char` becomes exactly one `'?'`,
/// so the run's length — and therefore its alignment and advance — is
/// unchanged. Never drops, never panics (§2.5).
pub(crate) fn to_ascii(_text: &str) -> String {
    unimplemented!("R-0007: to_ascii")
}
