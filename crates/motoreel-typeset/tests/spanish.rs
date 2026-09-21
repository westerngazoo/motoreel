//! R-0009 AC2 and AC3 — the acceptance gate.
//!
//! The defect this requirement exists for is not "a font table is small".
//! It is that the engine rendered «b?ceps» for months and **nothing
//! objected**, because a `'?'` is a perfectly good glyph. So the gate is
//! in two halves: the language it must be able to write, and the promise
//! that what it cannot write is an error.

use motoreel_typeset::{measure, place, rasterize, Align, Error, Face, FaceId, Fonts, Node};

/// The exact strings the engine got wrong, taken from a rendered frame of
/// the reel-09 template on 2026-09-20.
const RENDERED_WRONG: &[(&str, &str)] = &[
    ("¿POR QUÉ TANTO?", "?POR QU? TANTO?"),
    ("bíceps", "b?ceps"),
    ("DE TERCER GÉNERO", "DE TERCER G?NERO"),
    ("cambió", "cambi?"),
];

/// Everything else a published piece has needed.
const ALSO_NEEDED: &[&str] = &["ángulo", "tensión", "30°", "mecánica", "repetición"];

/// A face with Spanish coverage. Deliberately **not** a skip: the owner's
/// Mac and the Ubuntu CI runner both have one, so a machine that fails to
/// find a face is a machine this gate should not pass quietly on.
fn spanish_face() -> Fonts {
    const CANDIDATES: &[&str] = &[
        // the brand, when the content repo sits alongside
        "../fisicobuenfisico/brand/fonts/ComicNeue-Bold.ttf",
        "../../../fisicobuenfisico/brand/fonts/ComicNeue-Bold.ttf",
        // macOS
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Debian/Ubuntu
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(face) = Face::load(bytes, *path) else {
            continue;
        };
        if face.missing("áéíóúüñÁÉÍÓÚÜÑ¿¡°").is_empty() {
            let mut fonts = Fonts::new();
            fonts.add(face);
            return fonts;
        }
    }
    panic!("no face with Spanish coverage found; tried {CANDIDATES:?}");
}

fn glyph_count(node: &Node) -> usize {
    match node {
        Node::Glyph(_) => 1,
        Node::Row(items) | Node::Stack(motoreel_typeset::Stack { items, .. }) => {
            items.iter().map(glyph_count).sum()
        }
        Node::Glue(_) | Node::Rule(_) => 0,
    }
}

/// AC2. Every string the engine mangled now measures to exactly as many
/// glyphs as it has characters — which is the claim «no substitution»
/// reduces to, since a substitution keeps the count but loses the
/// identity, and a drop loses the count.
#[test]
fn the_strings_it_got_wrong_now_set() {
    let fonts = spanish_face();
    for (right, wrong) in RENDERED_WRONG {
        let node = measure(right, FaceId(0), 64.0, &fonts)
            .unwrap_or_else(|e| panic!("{right:?} (the engine drew {wrong:?}): {e}"));
        assert_eq!(
            glyph_count(&node),
            right.chars().count(),
            "{right:?} lost or gained a glyph"
        );
        assert!(
            node.ink().width() > 0.0,
            "{right:?} measured to no ink at all"
        );
    }
    for s in ALSO_NEEDED {
        let node = measure(s, FaceId(0), 64.0, &fonts).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        assert_eq!(glyph_count(&node), s.chars().count(), "{s:?}");
    }
}

/// AC2, the part a glyph count cannot see: an accented letter must be a
/// *different glyph* from its unaccented twin. A face that mapped every
/// accent onto the base letter would pass the count and still be wrong.
#[test]
fn an_accent_is_not_the_same_glyph_as_the_bare_letter() {
    let fonts = spanish_face();
    let one = |s: &str| match measure(s, FaceId(0), 64.0, &fonts).unwrap() {
        Node::Row(items) => match items[0] {
            Node::Glyph(g) => g.id,
            _ => unreachable!("measure builds rows of glyphs"),
        },
        _ => unreachable!("measure returns a row"),
    };
    for (accented, bare) in [('í', 'i'), ('É', 'E'), ('ñ', 'n'), ('ü', 'u')] {
        assert_ne!(
            one(&accented.to_string()),
            one(&bare.to_string()),
            "{accented} resolved to the same glyph as {bare}"
        );
    }
    // And the one that started it: '?' must not be what 'í' becomes.
    assert_ne!(one("í"), one("?"), "the old substitution, still happening");
}

/// AC3. The inversion of R-0007 AC8: a character the face cannot draw is
/// an error that names it, not a silent `'?'`.
#[test]
fn a_character_the_face_cannot_draw_is_an_error() {
    let fonts = spanish_face();
    let face = fonts.get(FaceId(0)).unwrap();
    // Pick something genuinely absent rather than assuming: U+10FFFD is a
    // private-use code point no text face ships.
    let absent = '\u{10FFFD}';
    assert!(
        !face.has(absent),
        "the probe character must really be absent"
    );

    match measure(&format!("x{absent}y"), FaceId(0), 32.0, &fonts) {
        Err(Error::Missing { ch, face: f, name }) => {
            assert_eq!(ch, absent);
            assert_eq!(f, FaceId(0));
            assert!(!name.is_empty(), "the error must name the face");
        }
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!(
            "a missing glyph was accepted — this is exactly the R-0007 AC8 \
             behaviour R-0009 exists to remove"
        ),
    }
}

/// AC3's other half: `missing` reports *all* of them, so a face is fixed
/// once rather than one render at a time.
#[test]
fn missing_reports_every_absent_character_without_duplicates() {
    let fonts = spanish_face();
    let face = fonts.get(FaceId(0)).unwrap();
    let probe = "a\u{10FFFD}b\u{10FFFC}c\u{10FFFD}";
    assert_eq!(face.missing(probe), vec!['\u{10FFFD}', '\u{10FFFC}']);
    assert!(face.missing("bíceps ángulo 30°").is_empty());
}

/// AC4. The size of a run is available without a pixel being drawn — the
/// question a layout gate asks, answered as arithmetic.
#[test]
fn a_run_can_be_measured_without_rasterizing() {
    let fonts = spanish_face();
    let run = measure("¿POR QUÉ TANTO?", FaceId(0), 76.0, &fonts).unwrap();
    let layout = place(&run, (540.0, 1800.0), Align::Center);
    let (l, b, r, t) = layout.ink_bounds().expect("the title has ink");

    assert!(r > l && t > b, "a real box: {l}..{r} by {b}..{t}");
    // Centring puts the *advance* box on the anchor, so the ink lands
    // within a side bearing of it — not exactly on it. Asserting the ink
    // is centred is a plausible-looking test that is simply wrong about
    // how type is set.
    assert!(
        (((l + r) / 2.0) - 540.0).abs() < 76.0 / 8.0,
        "within a bearing of the anchor, got {l}..{r}"
    );
    let advance = run.metrics().width;
    assert!(
        r - l <= advance + 1.0,
        "ink {:.1} wider than its advance {advance:.1}",
        r - l
    );
    // The gate's actual question.
    assert!(l >= 0.0 && r <= 1080.0, "the title must fit 1080 wide");
}

/// AC6. Same input, same output — twice in a row, byte for byte.
#[test]
fn rasterizing_is_deterministic() {
    let fonts = spanish_face();
    let run = measure("tensión 30°", FaceId(0), 48.0, &fonts).unwrap();
    let layout = place(&run, (100.5, 200.25), Align::Left);
    let draw = || {
        let mut out: Vec<(i64, i64, f64)> = Vec::new();
        rasterize(&layout, &fonts, |x, y, a| out.push((x, y, a))).unwrap();
        out
    };
    let first = draw();
    assert!(!first.is_empty(), "accented text must put ink down");
    assert_eq!(first, draw());
}

/// The ink a run covers must land inside the box `ink_bounds` promised.
/// If it does not, every layout decision made from that box is a guess —
/// which is the failure mode the Python gate exists to catch by brute
/// force.
#[test]
fn the_ink_lands_inside_the_box_that_was_promised() {
    let fonts = spanish_face();
    let run = measure("ángulo", FaceId(0), 64.0, &fonts).unwrap();
    let layout = place(&run, (0.0, 0.0), Align::Left);
    let (l, b, r, t) = layout.ink_bounds().unwrap();
    let mut strays = Vec::new();
    rasterize(&layout, &fonts, |x, y, _| {
        let (x, y) = (x as f64, y as f64);
        // One pixel of slack: the box is in font units, the coverage is
        // on a pixel grid, and a glyph's edge rounds outward.
        if x < l - 1.0 || x > r + 1.0 || y < b - 1.0 || y > t + 1.0 {
            strays.push((x, y));
        }
    })
    .unwrap();
    assert!(
        strays.is_empty(),
        "{} pixels outside {l}..{r} by {b}..{t}: {:?}",
        strays.len(),
        &strays[..strays.len().min(8)]
    );
}
