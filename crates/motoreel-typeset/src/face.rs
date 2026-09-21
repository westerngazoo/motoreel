//! Loaded faces, and the one guarantee that matters: a character this
//! crate cannot draw is an **error**, never a `'?'`.
//!
//! R-0009 AC3 inverts R-0007 AC8 deliberately. The silent substitution
//! was the defect's own camouflage: the engine rendered «b?ceps» for
//! months and nothing in the pipeline objected, because a `'?'` is a
//! perfectly good glyph. Failing loudly costs a `Result` at every call
//! site and buys the one thing the quiet version could never give —
//! noticing.

use ab_glyph::{Font as _, FontVec, ScaleFont as _};

use crate::node::FaceId;
use crate::scale::px;

/// Why a face or a run could not be set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes handed to [`Face::load`] are not a font this crate reads.
    NotAFont,
    /// The face reports no units-per-em, so nothing can be scaled by it.
    NoUnitsPerEm,
    /// A character is not in the face's character map. **This is the
    /// point of the crate**: it names the character and the face instead
    /// of drawing a question mark and moving on.
    Missing {
        /// The character with no glyph.
        ch: char,
        /// The face that was asked for it.
        face: FaceId,
        /// That face's name, for an error a human can act on.
        name: String,
    },
    /// A [`FaceId`] with no face behind it in the registry.
    NoSuchFace(FaceId),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NotAFont => write!(f, "not a font this crate can read"),
            Error::NoUnitsPerEm => write!(f, "the face reports no units-per-em"),
            Error::Missing { ch, name, .. } => write!(
                f,
                "the face {name:?} has no glyph for {ch:?} (U+{:04X}); \
                 R-0009 AC3: this is an error, not a '?'",
                *ch as u32
            ),
            Error::NoSuchFace(FaceId(i)) => write!(f, "no face registered at index {i}"),
        }
    }
}

impl std::error::Error for Error {}

/// A face's vertical metrics at a given em size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VMetrics {
    /// Distance from the baseline up to the face's ascender.
    pub ascent: f64,
    /// Distance from the baseline down to the descender, non-negative.
    pub descent: f64,
    /// Recommended extra space between consecutive baselines.
    pub line_gap: f64,
}

/// One loaded font file.
pub struct Face {
    font: FontVec,
    name: String,
}

impl core::fmt::Debug for Face {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The font bytes are megabytes of tables; naming the face is
        // the whole of what a reader wants from this.
        f.debug_struct("Face")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Face {
    /// Load a face from font bytes, naming it for error messages.
    ///
    /// # Errors
    /// [`Error::NotAFont`] if the bytes do not parse, [`Error::NoUnitsPerEm`]
    /// if the face cannot be scaled.
    pub fn load(bytes: Vec<u8>, name: impl Into<String>) -> Result<Face, Error> {
        let font = FontVec::try_from_vec(bytes).map_err(|_| Error::NotAFont)?;
        if font.units_per_em().is_none() {
            return Err(Error::NoUnitsPerEm);
        }
        Ok(Face {
            font,
            name: name.into(),
        })
    }

    /// The name this face was registered under.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the face has a glyph for `ch`.
    ///
    /// Glyph 0 is `.notdef` by the OpenType specification, which is
    /// exactly what a face returns for a character it does not cover — so
    /// this is the check, not a heuristic.
    #[must_use]
    pub fn has(&self, ch: char) -> bool {
        self.font.glyph_id(ch).0 != 0
    }

    /// Every character of `text` this face cannot draw, in order, without
    /// duplicates.
    ///
    /// Reporting all of them at once is deliberate: fixing one missing
    /// glyph at a time, one render at a time, is how a catalogue-wide
    /// defect stays alive.
    #[must_use]
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out: Vec<char> = Vec::new();
        for ch in text.chars() {
            if !self.has(ch) && !out.contains(&ch) {
                out.push(ch);
            }
        }
        out
    }

    /// Vertical metrics at `size`.
    #[must_use]
    pub fn vmetrics(&self, size: f64) -> VMetrics {
        let s = self.font.as_scaled(px(size));
        VMetrics {
            ascent: f64::from(s.ascent()),
            // ab_glyph reports the descent as a negative offset; the box
            // model wants a non-negative distance (see `node::Metrics`).
            descent: f64::from(-s.descent()),
            line_gap: f64::from(s.line_gap()),
        }
    }

    pub(crate) fn font(&self) -> &FontVec {
        &self.font
    }
}

/// The faces a scene can set type in.
///
/// This crate never decides what "display" or "mono" mean — a consumer
/// registers faces in whatever order it likes and refers to them by the
/// [`FaceId`] it gets back. Keeping the naming out here is what lets the
/// brand change its mind about a face without this crate knowing.
#[derive(Debug, Default)]
pub struct Fonts {
    faces: Vec<Face>,
}

impl Fonts {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Fonts {
        Fonts { faces: Vec::new() }
    }

    /// Register a face and return its id.
    pub fn add(&mut self, face: Face) -> FaceId {
        self.faces.push(face);
        FaceId(self.faces.len() - 1)
    }

    /// How many faces are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// Whether no face is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// The face behind an id.
    ///
    /// # Errors
    /// [`Error::NoSuchFace`] if nothing is registered at that index.
    pub fn get(&self, id: FaceId) -> Result<&Face, Error> {
        self.faces.get(id.0).ok_or(Error::NoSuchFace(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubbish_bytes_are_not_a_font() {
        assert_eq!(
            Face::load(vec![0, 1, 2, 3], "junk").unwrap_err(),
            Error::NotAFont
        );
    }

    #[test]
    fn an_empty_registry_has_no_faces() {
        let f = Fonts::new();
        assert!(f.is_empty());
        assert_eq!(f.get(FaceId(0)).unwrap_err(), Error::NoSuchFace(FaceId(0)));
    }

    /// The message has to name the character *and* the face, because the
    /// whole failure mode being fixed is "nobody could tell what broke".
    #[test]
    fn the_missing_error_says_what_and_where() {
        let e = Error::Missing {
            ch: 'í',
            face: FaceId(2),
            name: "Bangers".into(),
        };
        let s = e.to_string();
        assert!(s.contains('í'), "{s}");
        assert!(s.contains("Bangers"), "{s}");
        assert!(s.contains("U+00ED"), "{s}");
    }
}
