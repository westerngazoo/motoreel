//! R-0009 — real type, end to end through the sinks.
#![cfg(feature = "text")]
//!
//!
//! The inverse of the two R-0007 AC8 tests this requirement amended away.
//! Those pinned the substitution: every unrenderable char becomes exactly
//! one `'?'`, character count preserved, nothing panics. It was total, it
//! was documented, and it is how the engine rendered «¿POR QUÉ TANTO?» as
//! «?POR QU? TANTO?» for months while everything reported success.
//!
//! What is pinned here instead: the text reaches the sink verbatim, a face
//! that can draw it does, and a face that cannot says so by name.

use std::fs;
use std::path::PathBuf;

use motoreel::{Align, Anchor, Label, PpmSink, Prim2, Rgb, Scene, ScreenAnchor, Style, SvgSink};
use motoreel_typeset::{Face, Fonts};

/// The exact strings a frame of the reel-09 template mangled on
/// 2026-09-20, paired with what it drew instead.
const MANGLED: &[(&str, &str)] = &[
    ("¿POR QUÉ TANTO?", "?POR QU? TANTO?"),
    ("bíceps", "b?ceps"),
    ("DE TERCER GÉNERO", "DE TERCER G?NERO"),
    ("cambió", "cambi?"),
];

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("motoreel-r0009").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// A face with Spanish coverage. Panicking when none is found is
/// deliberate: a machine with no usable font is one this gate should not
/// pass quietly on.
fn spanish() -> Fonts {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = fs::read(path) else { continue };
        let Ok(face) = Face::load(bytes, *path) else {
            continue;
        };
        if face.missing("áéíóúüñÁÉÍÓÚÜÑ¿¡°").is_empty() {
            let mut fonts = Fonts::new();
            fonts.add(face);
            return fonts;
        }
    }
    panic!("no face with Spanish coverage; tried {CANDIDATES:?}");
}

fn white() -> Style {
    Style {
        stroke: Rgb::WHITE,
        width: 0.0,
        alpha: 1.0,
    }
}

/// The one view every test here uses, so scene and sink agree — `render`
/// rejects a disagreement before writing (R-0007 AC4).
const VIEW: (f64, f64) = (3.2, 1.8);

fn one_label(text: &str) -> Scene {
    let mut scene = Scene::new(1.0);
    scene.view = VIEW;
    scene.add_label(
        Label::new(text, Anchor::Screen(ScreenAnchor::Centre))
            .with_size(0.4)
            .with_align(Align::Center)
            .with_style(white()),
    );
    scene
}

fn lit(frame: &[u8], dims: (u32, u32)) -> usize {
    let head = format!("P6\n{} {}\n255\n", dims.0, dims.1).len();
    frame[head..]
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|p| **p != [0, 0, 0])
        .count()
}

/// The substitution is gone from `eval`: what the author wrote is what the
/// primitive carries. This is the single change that unblocked the SVG
/// sink, which could always write UTF-8 and was being handed ASCII.
#[test]
fn eval_carries_the_text_verbatim() {
    for (right, wrong) in MANGLED {
        let prims = one_label(right).eval(0.0);
        let text = prims
            .iter()
            .find_map(|p| match p {
                Prim2::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .expect("a text primitive");
        assert_eq!(text, *right, "must reach the sink as authored");
        assert_ne!(text, *wrong, "the substitution is what R-0009 removed");
    }
}

/// And it reaches the raster sink as ink. A frame that merely *succeeded*
/// would have satisfied the old behaviour too; this counts pixels.
#[test]
fn accented_text_puts_ink_on_the_raster_sink() {
    let dims = (256, 144);
    for (i, (right, _)) in MANGLED.iter().enumerate() {
        let dir = tmp_dir(&format!("ink{i}"));
        let mut sink = PpmSink::with_view(&dir, dims, VIEW)
            .expect("sink")
            .with_fonts(spanish());
        one_label(right).render(1.0, &mut sink).expect("render");
        let frame = fs::read(dir.join("frame_00000.ppm")).expect("frame");
        assert!(lit(&frame, dims) > 0, "{right:?} drew nothing");
    }
}

/// An accented run must differ from its unaccented twin **in pixels**. A
/// face that mapped every accent onto the base letter would pass every
/// test above and still be wrong, and a substitution would too.
#[test]
fn an_accent_changes_the_pixels() {
    let dims = (256, 144);
    let render = |name: &str, text: &str| -> Vec<u8> {
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, dims, VIEW)
            .expect("sink")
            .with_fonts(spanish());
        one_label(text).render(1.0, &mut sink).expect("render");
        fs::read(dir.join("frame_00000.ppm")).expect("frame")
    };
    assert_ne!(
        render("acc", "bíceps"),
        render("bare", "biceps"),
        "«bíceps» and «biceps» must not rasterize alike"
    );
    assert_ne!(
        render("acc2", "bíceps"),
        render("sub", "b?ceps"),
        "and neither must «bíceps» and what the engine used to draw"
    );
}

/// R-0009 AC3 through the sink: a character the face cannot draw fails the
/// **frame**, and the message names the character. The old behaviour wrote
/// the frame and reported success.
#[test]
fn a_glyph_the_face_lacks_fails_the_frame_by_name() {
    let dir = tmp_dir("missing");
    let mut sink = PpmSink::with_view(&dir, (64, 36), (3.2, 1.8))
        .expect("sink")
        .with_fonts(spanish());
    // A private-use code point no text face ships.
    let err = one_label("x\u{10FFFD}y")
        .render(1.0, &mut sink)
        .expect_err("a missing glyph must fail the frame");
    let msg = err.to_string();
    assert!(
        msg.contains("10FFFD") || msg.contains('\u{10FFFD}'),
        "{msg}"
    );
    assert!(msg.contains("frame 0"), "and say which frame: {msg}");
    assert!(
        !dir.join("frame_00000.ppm").exists(),
        "a frame that could not set its text must not be written"
    );
}

/// The other silent failure: no registry at all. Drawing nothing would be
/// the same mistake in a new costume.
#[test]
fn a_sink_with_no_faces_refuses_text() {
    let dir = tmp_dir("nofonts");
    let mut sink = PpmSink::with_view(&dir, (64, 36), (3.2, 1.8)).expect("sink");
    let err = one_label("hola")
        .render(1.0, &mut sink)
        .expect_err("text with no registry must fail");
    assert!(err.to_string().contains("with_fonts"), "{err}");
}

/// A face index out of range is named too, rather than falling back to
/// face 0 — a fallback is how the wrong font ships.
#[test]
fn an_unregistered_face_index_is_an_error() {
    let dir = tmp_dir("badface");
    let mut sink = PpmSink::with_view(&dir, (64, 36), (3.2, 1.8))
        .expect("sink")
        .with_fonts(spanish());
    let mut scene = Scene::new(1.0);
    scene.add_label(
        Label::new("hola", Anchor::Screen(ScreenAnchor::Centre))
            .with_face(7)
            .with_style(white()),
    );
    let err = scene
        .render(1.0, &mut sink)
        .expect_err("face 7 is not registered");
    assert!(err.to_string().contains('7'), "{err}");
}

/// The SVG sink writes the text as authored and names the family it was
/// told. It could always emit UTF-8 — `escape_text` has been total over
/// `char` since R-0003 — and never got the chance.
#[test]
fn the_svg_sink_writes_the_text_and_the_family() {
    let dir = tmp_dir("svg");
    let mut sink = SvgSink::with_view(&dir, (256, 144), VIEW)
        .expect("sink")
        .with_families(["Bangers"]);
    one_label("¿POR QUÉ TANTO?")
        .render(1.0, &mut sink)
        .expect("render");
    let svg = fs::read_to_string(dir.join("frame_00000.svg")).expect("frame");
    assert!(svg.contains(">¿POR QUÉ TANTO?</text>"), "{svg}");
    assert!(svg.contains("font-family=\"Bangers\""), "{svg}");
    assert!(
        !svg.contains("?POR QU?"),
        "the old substitution must not survive anywhere: {svg}"
    );
}

/// Unnamed families still say `monospace`, which is what keeps R-0003's
/// golden bytes unchanged.
#[test]
fn an_unnamed_family_is_still_monospace() {
    let dir = tmp_dir("svg_default");
    let mut sink = SvgSink::with_view(&dir, (256, 144), VIEW).expect("sink");
    one_label("hola").render(1.0, &mut sink).expect("render");
    let svg = fs::read_to_string(dir.join("frame_00000.svg")).expect("frame");
    assert!(svg.contains("font-family=\"monospace\""), "{svg}");
}

/// R-0009 AC6. Real type is antialiased and subpixel-positioned, neither
/// of which the bitmap face was, so determinism is worth re-asserting
/// rather than assuming it survived.
#[test]
fn two_renders_of_accented_text_are_byte_identical() {
    let render = |name: &str| -> Vec<u8> {
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, (256, 144), VIEW)
            .expect("sink")
            .with_fonts(spanish());
        one_label("tensión 30° · ángulo")
            .render(1.0, &mut sink)
            .expect("render");
        fs::read(dir.join("frame_00000.ppm")).expect("frame")
    };
    assert_eq!(render("det_a"), render("det_b"));
}

/// Proportional type, which the 5 × 7 face could not be: its advance was
/// one cell for every glyph, and `PpmSink` hard-coded
/// `width = advance · text.len()`.
#[test]
fn the_raster_sink_sets_proportional_type() {
    let dims = (512, 288);
    let width = |name: &str, text: &str| -> usize {
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, dims, VIEW)
            .expect("sink")
            .with_fonts(spanish());
        let mut scene = Scene::new(1.0);
        scene.add_label(
            Label::new(text, Anchor::Screen(ScreenAnchor::Centre))
                .with_size(0.3)
                .with_style(white()),
        );
        scene.render(1.0, &mut sink).expect("render");
        let frame = fs::read(dir.join("frame_00000.ppm")).expect("frame");
        let head = format!("P6\n{} {}\n255\n", dims.0, dims.1).len();
        let cols: Vec<usize> = (0..dims.0 as usize)
            .filter(|&x| {
                (0..dims.1 as usize).any(|y| {
                    let i = head + (y * dims.0 as usize + x) * 3;
                    frame[i..i + 3] != [0, 0, 0]
                })
            })
            .collect();
        cols.last().copied().unwrap_or(0) - cols.first().copied().unwrap_or(0)
    };
    let narrow = width("iii", "iii");
    let wide = width("mmm", "mmm");
    assert!(
        wide > narrow * 2,
        "«mmm» must be far wider than «iii»; got {wide} and {narrow}"
    );
}
