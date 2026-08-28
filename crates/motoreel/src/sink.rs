//! Frame sinks and the render walk: scenes become numbered frames.

use std::io;

use crate::prim::Prim2;
use crate::scene::Scene;

/// Per-frame consumer of projected primitives (RFC-012 §3.3).
pub trait FrameSink {
    /// Consume frame `index`'s primitives; any write failure propagates.
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;

    /// The image-space view window this sink renders, if it has one.
    ///
    /// Read **only** by [`Scene::render`], to reject a scene whose `view`
    /// disagrees — screen-anchored labels would otherwise land off-frame
    /// with no diagnostic. Never an input to [`Scene::eval`]: a frame stays
    /// a pure function of `(scene, t)`, so no golden moves. Defaulted, so a
    /// sink with no view window (a recorder, a counter) opts out by saying
    /// nothing.
    fn view(&self) -> Option<(f64, f64)> {
        None
    }
}

impl Scene {
    /// Walk `ceil(duration · fps)` frames at `t_i = i / fps`, feeding each
    /// evaluated frame to `sink`; stops at the first sink error.
    ///
    /// Times are derived, never accumulated — no float drift at frame
    /// 100 000. Every `t_i` lies in `[0, duration)`; the pose at exactly
    /// `duration` is never sampled (the last frame displays for its full
    /// `1/fps`). Non-finite or non-positive `fps`, and a `duration`
    /// outside its documented contract (finite, ≥ 0), return
    /// [`io::ErrorKind::InvalidInput`] before any sink call.
    pub fn render<S: FrameSink + ?Sized>(&self, fps: f64, sink: &mut S) -> io::Result<()> {
        if !(fps.is_finite() && fps > 0.0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fps must be finite and > 0",
            ));
        }
        if !(self.duration.is_finite() && self.duration >= 0.0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "duration must be finite and >= 0",
            ));
        }
        // Validate before side effects — SPEC-0003 §2.4's rule, third
        // clause. `!=` on (f64, f64): a NaN `self.view` never compares
        // equal, so a view violating its own contract is rejected here too.
        if let Some(v) = sink.view() {
            if v != self.view {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "scene.view must match the sink's view window",
                ));
            }
        }

        let frames = (self.duration * fps).ceil() as usize;
        for index in 0..frames {
            let t = index as f64 / fps; // derived, never accumulated
            sink.frame(index, &self.eval(t))?;
        }
        Ok(())
    }
}
