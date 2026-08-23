//! Scene composition and the evaluation loop.

use garust::{pga, Motor3};

use crate::camera::{Camera, Projection};
use crate::object::{Object, Shape};
use crate::prim::{Prim2, Pt2, Style};

/// Opaque handle to an object in a scene (stable insertion index) — the
/// seam R-0007's derived shapes will reference; inert until then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId(usize);

/// A renderable description: objects in draw order, one camera, and a
/// duration in seconds (consumed by SPEC-0003's frame walk, not by
/// [`Scene::eval`]).
#[derive(Clone, Debug)]
pub struct Scene {
    /// Draw order = insertion order; later objects paint over earlier.
    pub objects: Vec<Object>,
    /// The eye (SPEC-0002 §2.4).
    pub camera: Camera,
    /// Scene length in seconds. Contract: finite and ≥ 0.
    pub duration: f64,
}

impl Scene {
    /// An empty scene of the given duration with the default camera.
    pub fn new(duration: f64) -> Self {
        Scene {
            objects: Vec::new(),
            camera: Camera::default(),
            duration,
        }
    }

    /// Append an object; returns its stable id (draw order = insertion).
    pub fn add(&mut self, object: Object) -> ObjectId {
        self.objects.push(object);
        ObjectId(self.objects.len() - 1)
    }

    /// Evaluate at time `t`: every track becomes a pose, geometry rides
    /// one composed motor into view space, surviving primitives are
    /// emitted in draw order. Total and deterministic; culled primitives
    /// are simply absent (SPEC-0002 §2.5).
    pub fn eval(&self, t: f64) -> Vec<Prim2> {
        let view = self.camera.pose.inverse();
        let mut prims = Vec::with_capacity(self.objects.len());
        for obj in &self.objects {
            let to_view = view.compose(&obj.track.eval(t)); // pose, then view
            if let Some(p) = project_shape(&obj.shape, &to_view, self.camera.projection, obj.style)
            {
                prims.push(p);
            }
        }
        prims
    }
}

/// One shape through the camera; `None` culls the whole primitive
/// (SPEC-0002 §2.5: every vertex must pass, or nothing is emitted).
fn project_shape(
    shape: &Shape,
    to_view: &Motor3,
    projection: Projection,
    style: Style,
) -> Option<Prim2> {
    let pt = |p: &pga::Point| projection.project(&p.transform(to_view));
    Some(match shape {
        Shape::Point(p) => Prim2::Point { at: pt(p)?, style },
        Shape::Segment(a, b) => Prim2::Segment {
            a: pt(a)?,
            b: pt(b)?,
            style,
        },
        Shape::Polyline(ps) => {
            let points = ps.iter().map(pt).collect::<Option<Vec<Pt2>>>()?;
            Prim2::Polyline { points, style }
        }
        Shape::Edges(es) => {
            let segments = es
                .iter()
                .map(|(a, b)| Some((pt(a)?, pt(b)?)))
                .collect::<Option<Vec<(Pt2, Pt2)>>>()?;
            Prim2::Edges { segments, style }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::Scene;
    use crate::object::Object;
    use garust::pga;

    /// Draw order is insertion order; ids are distinct stable indices.
    #[test]
    fn add_preserves_insertion_order_with_distinct_ids() {
        let mut scene = Scene::new(1.0);
        let a = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
        let b = scene.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)));
        assert_ne!(a, b);
        assert_eq!(scene.objects.len(), 2);
    }
}
