//! `SvgSink`: byte-deterministic SVG frame files.
//!
//! The writer is a fixed template — same input bits, same output bytes,
//! on every platform. Every `f64` is written with Rust's default
//! `Display` (shortest round-trip); colors are lowercase `#rrggbb`;
//! attribute and element order are pinned; no clock, environment,
//! randomness, or map iteration touches the write path.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use crate::prim::{Align, Prim2, Pt2, Style};
use crate::sink::FrameSink;

/// Writes one `frame_%05d.svg` per frame into a directory (RFC-012 §3.3).
///
/// Stroke widths and point radii are in image (viewBox) units: width
/// `0.01` in the default 1.8-tall window is ≈ 6 px at 1080p.
pub struct SvgSink {
    dir: PathBuf,
    header: String,
    buf: String,
    view: (f64, f64),
    /// CSS family name per face index. See [`SvgSink::with_families`].
    families: Vec<String>,
}

impl SvgSink {
    /// Sink into `dir` (created now, parents included): 1920×1080 raster
    /// over the default 3.2 × 1.8 centred view window.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        SvgSink::with_view(dir, (1920, 1080), (3.2, 1.8))
    }

    /// Sink into `dir` with an explicit raster size (px) and centred
    /// image-space view window (width, height).
    ///
    /// Zero raster dimensions or a non-finite/non-positive view window
    /// are [`io::ErrorKind::InvalidInput`], rejected before the directory
    /// is created (invalid input has no side effects).
    pub fn with_view(
        dir: impl AsRef<Path>,
        size: (u32, u32),
        view: (f64, f64),
    ) -> io::Result<Self> {
        if size.0 == 0 || size.1 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "raster size must be non-zero in both dimensions",
            ));
        }
        let view_valid = view.0.is_finite() && view.1.is_finite() && view.0 > 0.0 && view.1 > 0.0;
        if !view_valid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "view window must be finite and positive",
            ));
        }
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        // fmt::Write to String is infallible; `let _` acknowledges that.
        let mut header = String::new();
        let _ = writeln!(
            header,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" \
             height=\"{}\" viewBox=\"{} {} {} {}\">\n\
             <g transform=\"scale(1 -1)\">",
            size.0,
            size.1,
            -(view.0 / 2.0),
            -(view.1 / 2.0),
            view.0,
            view.1,
        );
        Ok(SvgSink {
            dir,
            header,
            buf: String::new(),
            view,
            families: Vec::new(),
        })
    }

    /// The CSS family names this sink writes into `font-family`, indexed
    /// the way `Prim2::Text::face` indexes them.
    ///
    /// Not a font registry: this sink emits `<text>` and the *consumer's*
    /// font engine resolves the face, so the right currency here is a
    /// family name, not a loaded file. The two sinks therefore need not
    /// agree on which file backs a name (R-0009 OQ-3).
    ///
    /// Left unset, every run says `monospace` — what this sink emitted
    /// before R-0009, which keeps R-0003's golden bytes intact.
    #[must_use]
    pub fn with_families<I, T>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.families = families.into_iter().map(Into::into).collect();
        self
    }

    /// The family for a face index, or `monospace` when none is named.
    fn family(&self, face: usize) -> &str {
        self.families
            .get(face)
            .map_or("monospace", std::string::String::as_str)
    }

    fn write_prim(&mut self, prim: &Prim2) {
        match prim {
            Prim2::Point { at, style } => {
                debug_assert_finite(at);
                let _ = writeln!(
                    self.buf,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" \
                     fill-opacity=\"{}\"/>",
                    at.x,
                    at.y,
                    style.width / 2.0,
                    hex(style),
                    style.alpha,
                );
            }
            Prim2::Segment { a, b, style } => {
                debug_assert_finite(a);
                debug_assert_finite(b);
                let _ = writeln!(
                    self.buf,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" \
                     stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\" \
                     stroke-linecap=\"round\"/>",
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    hex(style),
                    style.width,
                    style.alpha,
                );
            }
            Prim2::Polyline { points, style } => {
                self.buf.push_str("<polyline points=\"");
                for (i, p) in points.iter().enumerate() {
                    debug_assert_finite(p);
                    if i > 0 {
                        self.buf.push(' ');
                    }
                    let _ = write!(self.buf, "{},{}", p.x, p.y);
                }
                let _ = writeln!(
                    self.buf,
                    "\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
                     stroke-opacity=\"{}\" stroke-linecap=\"round\" \
                     stroke-linejoin=\"round\"/>",
                    hex(style),
                    style.width,
                    style.alpha,
                );
            }
            Prim2::Edges { segments, style } => {
                self.buf.push_str("<path d=\"");
                for (i, (a, b)) in segments.iter().enumerate() {
                    debug_assert_finite(a);
                    debug_assert_finite(b);
                    if i > 0 {
                        self.buf.push(' ');
                    }
                    let _ = write!(self.buf, "M {},{} L {},{}", a.x, a.y, b.x, b.y);
                }
                // No stroke-linejoin: M/L subpairs are disjoint, so joins
                // never occur and the attribute would be inert.
                let _ = writeln!(
                    self.buf,
                    "\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
                     stroke-opacity=\"{}\" stroke-linecap=\"round\"/>",
                    hex(style),
                    style.width,
                    style.alpha,
                );
            }
            Prim2::Text {
                at,
                text,
                size,
                align,
                face,
                style,
            } => {
                debug_assert_finite(at);
                // `transform="scale(1 -1)"` undoes the document group's
                // flip, so glyphs are upright and this element's own
                // coordinates are y-down: hence `-y`. Emitting a second
                // <g> instead would move R-0003's golden bytes (§2.7).
                //
                // Bound first: `write!` borrows `self.buf` mutably and
                // `family` borrows `self`.
                let family = self.family(*face).to_owned();
                let _ = write!(
                    self.buf,
                    "<text transform=\"scale(1 -1)\" x=\"{}\" y=\"{}\" \
                     font-family=\"{}\" font-size=\"{}\" \
                     text-anchor=\"{}\" xml:space=\"preserve\" \
                     fill=\"{}\" fill-opacity=\"{}\">",
                    at.x,
                    -at.y,
                    family,
                    size,
                    anchor_word(*align),
                    hex(style),
                    style.alpha,
                );
                escape_text(text, &mut self.buf);
                let _ = writeln!(self.buf, "</text>");
            }
        }
    }
}

/// SVG's `text-anchor` keyword for an [`Align`] — a closed set of literals,
/// so no attribute value can ever contain a metacharacter (§2.6).
fn anchor_word(align: Align) -> &'static str {
    match align {
        Align::Left => "start",
        Align::Center => "middle",
        Align::Right => "end",
    }
}

/// XML element-content escaping — the only escaping motoreel performs.
///
/// Three metacharacters exhaust the interesting cases in element
/// content: `"` and `'` are legal there and this string never enters an
/// attribute value.
///
/// The map is **total over `char`**, and always was — the fallback arm
/// passes everything else through as UTF-8, which is why this function
/// needed no change when R-0009 removed `Prim2::Text`'s ASCII invariant.
/// The SVG sink could have written «bíceps» all along; `to_ascii` upstream
/// never let it try.
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

impl FrameSink for SvgSink {
    fn view(&self) -> Option<(f64, f64)> {
        Some(self.view)
    }

    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()> {
        self.buf.clear();
        self.buf.push_str(&self.header);
        for p in prims {
            self.write_prim(p);
        }
        self.buf.push_str("</g>\n</svg>\n");
        std::fs::write(self.dir.join(format!("frame_{index:05}.svg")), &self.buf)
    }
}

/// Lowercase `#rrggbb`, always six digits.
fn hex(style: &Style) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        style.stroke.r, style.stroke.g, style.stroke.b
    )
}

/// Test-time tripwire on SPEC-0002's coordinate invariant (§2.8 item 7);
/// release builds write whatever arrives, deterministically.
fn debug_assert_finite(p: &Pt2) {
    debug_assert!(
        p.x.is_finite() && p.y.is_finite(),
        "Scene::eval's finite-coordinate invariant was violated"
    );
}
