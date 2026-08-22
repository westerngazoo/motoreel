//! Frame sinks and the render walk: scenes become numbered frames.

use std::io;

use crate::prim::Prim2;
use crate::scene::Scene;

/// Per-frame consumer of projected primitives (RFC-012 §3.3).
pub trait FrameSink {
    /// Consume frame `index`'s primitives; any write failure propagates.
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;
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
        let frames = (self.duration * fps).ceil() as usize;
        for index in 0..frames {
            let t = index as f64 / fps; // derived, never accumulated
            sink.frame(index, &self.eval(t))?;
        }
        Ok(())
    }
}
