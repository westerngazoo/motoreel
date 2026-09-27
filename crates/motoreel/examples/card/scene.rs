//! The explanatory card (R-0012 AC12, SPEC-0012 §2.12): a panel with a
//! hairline border; a friction circle, filled at low opacity and stroked,
//! with a point running once round its rim; and the shaded area under
//! `y = k/x` between two bounds — the work of an isothermal expansion.
//!
//! Shared verbatim by `tests/r0012_paths_fills.rs` via `#[path]`, so the
//! shipped demo and its tests cannot drift apart. motoreel items are named
//! through `motoreel::`, garust's through `garust::`.
//!
//! Every colour is passed in from here: the engine has no palette. These
//! are the Goosethropic brand tokens (`tokens.css`).

use std::f64::consts::TAU;

use garust::{Motor3, Pga3};
use motoreel::shapes::{circle, polygon, rounded_rect};
use motoreel::{Fill, Object, Pt2, Rgb, Scene, Style, Subpath, Track};

/// `--panel`: the card's face.
const PANEL: Rgb = Rgb {
    r: 0x14,
    g: 0x14,
    b: 0x17,
};
/// `--gold`: the quantity being shown.
const GOLD: Rgb = Rgb {
    r: 0xe9,
    g: 0xb2,
    b: 0x3f,
};
/// `--steel`: geometry.
const STEEL: Rgb = Rgb {
    r: 0xce,
    g: 0xd1,
    b: 0xd9,
};
/// `--red`: the one thing to look at.
const RED: Rgb = Rgb {
    r: 0xe0,
    g: 0x32,
    b: 0x2a,
};
/// `--ash-dim`: axes and guides.
const ASH_DIM: Rgb = Rgb {
    r: 0x4a,
    g: 0x4b,
    b: 0x51,
};
/// The opacity of `--line`: card borders.
const LINE_ALPHA: f64 = 0.10;
/// The opacity of `--gold-dim`: areas.
const DIM_ALPHA: f64 = 0.16;

/// One pixel at the documented 1080 × 1920 raster: 600 px per unit.
const PX: f64 = 1.0 / 600.0;
/// Clip length in seconds: one revolution of the point.
const DURATION: f64 = 3.0;

/// The friction circle's centre.
const FRICTION_CENTRE: Pt2 = Pt2 { x: 0.0, y: 0.5 };
/// The friction circle's radius.
const FRICTION_RADIUS: f64 = 0.6;

/// The chart's origin in image space; one chart unit is one image unit.
const CHART_ORIGIN: Pt2 = Pt2 { x: -0.62, y: -1.25 };
/// The curve is `y = K/x`, an isotherm.
const K: f64 = 0.12;
/// Where the curve is drawn: `x` in `[X_MIN, X_MAX]`.
const X_MIN: f64 = 0.12;
/// See [`X_MIN`].
const X_MAX: f64 = 1.24;
/// Where the area under it is shaded: `x` in `[X_A, X_B]`.
const X_A: f64 = 0.3;
/// See [`X_A`].
const X_B: f64 = 1.0;

/// Build the card: a portrait 1.8 × 3.2 view, 3 seconds.
pub fn scene() -> Scene {
    let mut scene = Scene::new(DURATION);
    scene.view = (1.8, 3.2); // must match the sink's view (finding 14)

    scene.add(
        Object::planar(rounded_rect(Pt2 { x: 0.0, y: 0.0 }, 1.64, 3.04, 0.04)).with_style(Style {
            stroke: Rgb::WHITE,
            width: PX,
            alpha: LINE_ALPHA,
            fill: Some(Fill::solid(PANEL, 1.0)),
        }),
    );
    scene.add(
        Object::planar(circle(FRICTION_CENTRE, FRICTION_RADIUS)).with_style(Style {
            stroke: STEEL,
            width: 2.0 * PX,
            alpha: 1.0,
            fill: Some(Fill::solid(GOLD, DIM_ALPHA)),
        }),
    );
    scene.add(Object::planar(guides()).with_style(Style {
        stroke: ASH_DIM,
        width: 1.5 * PX,
        alpha: 1.0,
        fill: None,
    }));

    let curve = hyperbola();
    scene.add(Object::planar(shaded_area(&curve)).with_style(Style {
        stroke: GOLD,
        width: 0.0, // fill only: the curve is stroked on top
        alpha: 1.0,
        fill: Some(Fill::solid(GOLD, DIM_ALPHA)),
    }));
    let mut line = Subpath::new(curve[0]);
    for &p in &curve[1..] {
        line = line.line_to(p);
    }
    scene.add(Object::planar(vec![line]).with_style(Style {
        stroke: GOLD,
        width: 3.0 * PX,
        alpha: 1.0,
        fill: None,
    }));

    // The point: a dot authored at (R, 0) about the model origin. Its
    // track turns it about that origin and carries it to the centre.
    let rim = Pt2 {
        x: FRICTION_RADIUS,
        y: 0.0,
    };
    scene.add(
        Object::planar(circle(rim, 0.03))
            .with_style(Style {
                stroke: RED,
                width: 0.0,
                alpha: 1.0,
                fill: Some(Fill::solid(RED, 1.0)),
            })
            .with_track(orbit()),
    );
    scene
}

/// One counter-clockwise revolution about the friction circle's centre.
///
/// Quarter-turn keys, as `Track::spin` places them: one slerp span folds to
/// the short way past a half turn, so a revolution needs at least three
/// keys. Each key is on the one-parameter rotation, so the point never
/// leaves the rim between them.
fn orbit() -> Track {
    let about = Motor3::translator(FRICTION_CENTRE.x, FRICTION_CENTRE.y, 0.0);
    Track::keys((0..=4).map(|k| {
        let s = f64::from(k) / 4.0; // derived, not accumulated
        (
            DURATION * s,
            about * Motor3::rotor(TAU * s, Pga3::basis(0b0011)),
        )
    }))
    .expect("static demo keys are strictly increasing and non-empty")
}

/// Crosshairs through the friction circle, and the chart's two axes: one
/// stroke-only path of four open subpaths.
fn guides() -> Vec<Subpath<Pt2>> {
    let (c, reach) = (FRICTION_CENTRE, FRICTION_RADIUS + 0.1);
    let o = CHART_ORIGIN;
    let segment = |a: Pt2, b: Pt2| Subpath::new(a).line_to(b);
    vec![
        segment(
            Pt2 {
                x: c.x - reach,
                y: c.y,
            },
            Pt2 {
                x: c.x + reach,
                y: c.y,
            },
        ),
        segment(
            Pt2 {
                x: c.x,
                y: c.y - reach,
            },
            Pt2 {
                x: c.x,
                y: c.y + reach,
            },
        ),
        segment(
            o,
            Pt2 {
                x: o.x + X_MAX + 0.04,
                y: o.y,
            },
        ),
        segment(
            o,
            Pt2 {
                x: o.x,
                y: o.y + K / X_MIN,
            },
        ),
    ]
}

/// `y = K/x` over `[X_MIN, X_MAX]` in image space, sampled geometrically
/// in `x` so the steep end is as smooth as the flat one. Trig-bearing
/// (`powf`) at scene construction, which R-0003's caveat allows.
fn hyperbola() -> Vec<Pt2> {
    const SAMPLES: u32 = 64;
    (0..=SAMPLES)
        .map(|i| {
            let x = X_MIN * (X_MAX / X_MIN).powf(f64::from(i) / f64::from(SAMPLES));
            chart(x, K / x)
        })
        .collect()
}

/// The region under `curve` between `X_A` and `X_B`, down to the axis:
/// counter-clockwise, along the axis and back over the curve.
fn shaded_area(curve: &[Pt2]) -> Vec<Subpath<Pt2>> {
    let (lo, hi) = (chart(X_A, 0.0).x, chart(X_B, 0.0).x);
    let top: Vec<Pt2> = curve
        .iter()
        .copied()
        .filter(|p| (lo..=hi).contains(&p.x))
        .collect();
    let (Some(first), Some(last)) = (top.first(), top.last()) else {
        return Vec::new();
    };
    let mut outline = vec![
        Pt2 {
            x: first.x,
            y: CHART_ORIGIN.y,
        },
        Pt2 {
            x: last.x,
            y: CHART_ORIGIN.y,
        },
    ];
    outline.extend(top.iter().rev());
    polygon(&outline)
}

/// Chart coordinates to image space.
fn chart(x: f64, y: f64) -> Pt2 {
    Pt2 {
        x: CHART_ORIGIN.x + x,
        y: CHART_ORIGIN.y + y,
    }
}
