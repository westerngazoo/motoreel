//! R-0013 AC2 — the raster sink never paints past the ink.
#![cfg(feature = "text")]
//!
//! A text pixel is a blend of what was under it and the text's colour, so
//! each channel lies between the two. With coverage above 1 the blend
//! extrapolates: on black, a (200, 100, 50) stroke came out (255, 200, 100)
//! where two contours of a glyph overlap — the flashes reported on JetBrains
//! Mono 700 'g' and 'r' in the rotorf-sico reels.

use std::fs;
use std::path::PathBuf;

use motoreel::{Align, Anchor, Label, PpmSink, Rgb, Scene, ScreenAnchor, Style};
use motoreel_typeset::{Face, Fonts};

const VIEW: (f64, f64) = (3.2, 1.8);
const DIMS: (u32, u32) = (320, 180);
const INK: Rgb = Rgb {
    r: 200,
    g: 100,
    b: 50,
};

fn overlap_face() -> Fonts {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../motoreel-typeset/tests/fixtures/overlap.ttf"
    );
    let mut fonts = Fonts::new();
    fonts.add(
        Face::load(fs::read(path).expect("the committed fixture"), "Overlap").expect("a font"),
    );
    fonts
}

fn frame_of(text: &str, name: &str) -> Vec<u8> {
    let dir: PathBuf = std::env::temp_dir().join("motoreel-r0013").join(name);
    let _ = fs::remove_dir_all(&dir);
    let mut scene = Scene::new(1.0);
    scene.view = VIEW;
    scene.add_label(
        Label::new(text, Anchor::Screen(ScreenAnchor::Centre))
            .with_size(1.0)
            .with_align(Align::Center)
            .with_style(Style {
                stroke: INK,
                width: 0.0,
                alpha: 1.0,
                fill: None,
            }),
    );
    let mut sink = PpmSink::with_view(&dir, DIMS, VIEW)
        .expect("sink")
        .with_fonts(overlap_face());
    scene.render(1.0, &mut sink).expect("render");
    let bytes = fs::read(dir.join("frame_00000.ppm")).expect("frame");
    let head = format!("P6\n{} {}\n255\n", DIMS.0, DIMS.1).len();
    bytes[head..].to_vec()
}

/// AC2. On a black frame every channel of every pixel stays at or below the
/// ink's, and the glyph's inside is the ink exactly.
#[test]
fn ac2_no_pixel_is_brighter_than_the_ink() {
    let px = frame_of("X", "x");
    let ink = [INK.r, INK.g, INK.b];
    let mut solid = 0;
    for p in px.chunks_exact(3) {
        for c in 0..3 {
            assert!(p[c] <= ink[c], "pixel {p:?} is past the ink {ink:?}");
        }
        solid += usize::from(p == ink);
    }
    assert!(solid > 0, "the glyph has a solid inside");
}
