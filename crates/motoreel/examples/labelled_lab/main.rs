//! A labelled lab: what R-0007 is for.
//!
//! Three labels, one per anchor kind, on a screw motion — a title pinned
//! to the frame, a caption pinned to the frame, and a name that **rides
//! the moving body**. This is the shape an explainer takes: the geometry
//! shows what happens, the labels say what it is.
//!
//! ```bash
//! cargo run --release --example labelled_lab
//! ffmpeg -framerate 60 -i out/lab/frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p lab.mp4
//! ```

use std::f64::consts::TAU;

use garust::{Motor3, Pga3};
use motoreel::{
    Align, Anchor, Camera, Label, Object, PpmSink, Pt2, Rgb, Scene, ScreenAnchor, Style, Track,
};

const WHITE: Rgb = Rgb::WHITE;
const ORANGE: Rgb = Rgb {
    r: 0xff,
    g: 0x4d,
    b: 0x00,
};
const TEAL: Rgb = Rgb {
    r: 0x00,
    g: 0xb4,
    b: 0xd8,
};

fn text(stroke: Rgb) -> Style {
    // `width` is unused for text; 0 states that rather than carrying a
    // stroke radius that means nothing here.
    Style {
        stroke,
        width: 0.0,
        alpha: 1.0,
    }
}

fn main() -> std::io::Result<()> {
    let duration = 4.0;

    // A full screw: one turn about z while rising 2. Authored as
    // quarter-turn keys, because rotor(TAU) is antipodal.
    let screw = Track::keys((0..=4).map(|k| {
        let s = f64::from(k) / 4.0;
        let pose =
            Motor3::translator(0.0, 0.0, 2.0 * s) * Motor3::rotor(TAU * s, Pga3::basis(0b0011));
        (duration * s, pose)
    }))
    .expect("quarter-turn keys are strictly increasing");

    let mut scene = Scene::new(duration);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 6.0), 2.0);

    let square = scene.add(
        Object::polyline(vec![
            flat(-0.5, -0.5),
            flat(0.5, -0.5),
            flat(0.5, 0.5),
            flat(-0.5, 0.5),
            flat(-0.5, -0.5),
        ])
        .with_style(Style {
            stroke: WHITE,
            width: 0.02,
            alpha: 1.0,
        })
        .with_track(screw),
    );

    // 1. A title, pinned to the frame. Centring a title is three
    //    decisions: the grid point, the alignment, and an offset that
    //    drops the baseline below the top edge.
    scene.add_label(
        Label::new("SCREW MOTION", Anchor::Screen(ScreenAnchor::TopCentre))
            .with_align(Align::Center)
            .with_offset(Pt2 { x: 0.0, y: -0.22 })
            .with_size(0.12)
            .with_style(text(WHITE)),
    );

    // 2. A caption, bottom-left, out of the way of the action.
    scene.add_label(
        Label::new(
            "one turn about z while rising 2",
            Anchor::Screen(ScreenAnchor::BottomLeft),
        )
        .with_offset(Pt2 { x: 0.06, y: 0.10 })
        .with_size(0.07)
        .with_style(text(TEAL)),
    );

    // 3. The one that earns the requirement: a name riding the body.
    //    The anchor is a point in the square's own model space, so it is
    //    carried by the square's track — no per-frame bookkeeping.
    scene.add_label(
        Label::new(
            "body",
            Anchor::Pose {
                object: square,
                at: flat(0.0, 0.75),
            },
        )
        .with_align(Align::Center)
        .with_size(0.09)
        .with_style(text(ORANGE)),
    );

    let mut sink = PpmSink::new("out/lab/")?;
    scene.render(60.0, &mut sink)?;
    println!("240 frames in out/lab/ — see the header for the ffmpeg line.");
    Ok(())
}

fn flat(x: f64, y: f64) -> garust::pga::Point {
    garust::pga::Point::new(x, y, 0.0)
}
