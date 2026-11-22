//! R-0017 — interchangeable finish («acabado»): the acceptance suite.
//!
//! One `mod` per acceptance criterion of
//! `requirements/0017-interchangeable-finish.md`. AC5's oracles are
//! rotorf-sico's rf-marca helpers, copied verbatim below (motoreel cannot
//! depend on rf-marca), so «byte for byte» is checked against the real
//! code, not a paraphrase of it.

use std::fs;
use std::path::{Path, PathBuf};

use motoreel::shapes::polygon;
use motoreel::{
    lambert, quantize, Faceted, FacetedError, Facing, Fill, Finish, FrameCtx, FrameSink, Layer,
    Light, LightError, Material, MaterialError, PpmSink, Prim2, Pt2, Rgb, Seg, Style, Subpath,
    Surface, SurfaceError, SvgSink,
};

// --- The oracles: rotorf-sico `brand/rf-marca/src/{lienzo,poly}.rs` at
// --- 162ce661c69e60785d14936bd48ce727ad2ff6eb, token for token: `pub` added, rustfmt ----
// --- re-wrapped the lines, and `Semilla::bits` is a test addition. --------

mod rf_marca {
    #![allow(clippy::all)]
    use motoreel::shapes::polygon;
    use motoreel::Pt2;
    use motoreel::{Fill, Prim2, Rgb, Style, Subpath};

    pub const ANCHO: u32 = 1080;
    pub const ALTO: u32 = 1920;
    pub const ESCALA: f64 = 600.0;
    pub type P = (f64, f64);

    pub fn p(x: f64, y: f64) -> Pt2 {
        Pt2 {
            x: (x - ANCHO as f64 / 2.0) / ESCALA,
            y: (ALTO as f64 / 2.0 - y) / ESCALA,
        }
    }

    pub fn u(px: f64) -> f64 {
        px / ESCALA
    }

    pub const VELA: [f64; 3] = [-0.6, -0.6, 0.52];

    fn camino(pts: &[P]) -> Vec<Subpath<Pt2>> {
        let q: Vec<Pt2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        polygon(&q)
    }

    pub fn faceta(pts: &[P], c: Rgb) -> Prim2 {
        Prim2::Path {
            subpaths: camino(pts),
            style: Style {
                stroke: c,
                width: u(1.0),
                alpha: 1.0,
                fill: Some(Fill::solid(c, 1.0)),
            },
        }
    }

    pub fn faceta_con_junta(pts: &[P], c: Rgb, junta: Rgb, ancho: f64, alfa: f64) -> Prim2 {
        Prim2::Path {
            subpaths: camino(pts),
            style: Style {
                stroke: junta,
                width: u(ancho),
                alpha: alfa,
                fill: Some(Fill::solid(c, alfa)),
            },
        }
    }

    pub fn velo(pts: &[P], c: Rgb, alfa: f64) -> Prim2 {
        Prim2::Path {
            subpaths: camino(pts),
            style: Style {
                stroke: c,
                width: 0.0,
                alpha: 0.0,
                fill: Some(Fill::solid(c, alfa)),
            },
        }
    }

    pub fn lambert(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
        let uu = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let vv = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [
            uu[1] * vv[2] - uu[2] * vv[1],
            uu[2] * vv[0] - uu[0] * vv[2],
            uu[0] * vv[1] - uu[1] * vv[0],
        ];
        if n[2] < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let ll = (VELA[0] * VELA[0] + VELA[1] * VELA[1] + VELA[2] * VELA[2]).sqrt();
        (n[0] * VELA[0] + n[1] * VELA[1] + n[2] * VELA[2]) / (nl * ll)
    }

    pub fn cuantizar(valores: &[f64], bandas: usize) -> Vec<usize> {
        let mut orden = valores.to_vec();
        orden.sort_by(f64::total_cmp);
        let cortes: Vec<f64> = (1..bandas)
            .map(|k| orden[orden.len() * k / bandas])
            .collect();
        valores
            .iter()
            .map(|v| cortes.iter().filter(|&&c| *v >= c).count())
            .collect()
    }

    pub struct Semilla(u64);

    impl Semilla {
        pub fn new(s: u64) -> Semilla {
            Semilla(s)
        }
        fn siguiente(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        pub fn entre(&mut self, a: f64, b: f64) -> f64 {
            a + (b - a) * (self.siguiente() >> 11) as f64 / (1u64 << 53) as f64
        }
        pub fn bits(&mut self) -> u64 {
            self.siguiente()
        }
    }
}

use rf_marca::{Semilla, P, VELA};

// --- Shared helpers ---------------------------------------------------------

fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn pt(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

const CTX: FrameCtx = FrameCtx { t: 0.0, seed: 7 };

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

fn vela() -> Light {
    Light::new(VELA).expect("the candle is a valid light")
}

fn one(c: Rgb) -> Material {
    Material::new(&[c]).expect("valid material")
}

/// rf-marca's outline for `pts`, as `Faceted` receives it.
fn outline(pts: &[P]) -> Vec<Subpath<Pt2>> {
    let q: Vec<Pt2> = pts.iter().map(|&(x, y)| rf_marca::p(x, y)).collect();
    polygon(&q)
}

fn surface(pts: &[P], facing: Facing, id: u64, material: Material) -> Surface {
    Surface::new(outline(pts), facing, 0.0, id, material).expect("valid surface")
}

/// A random convex-ish polygon of 3 to 7 corners on the 1080 × 1920 canvas.
fn random_polygon(rng: &mut Semilla) -> Vec<P> {
    let n = 3 + (rng.bits() % 5) as usize;
    let (cx, cy, r) = (
        rng.entre(100.0, 980.0),
        rng.entre(100.0, 1820.0),
        rng.entre(5.0, 90.0),
    );
    // Corners on a circle at sorted random positions (no trig: a rational
    // parametrisation of the circle).
    let mut ts: Vec<f64> = (0..n).map(|_| rng.entre(-3.0, 3.0)).collect();
    ts.sort_by(f64::total_cmp);
    ts.iter()
        .map(|&t| {
            let d = 1.0 + t * t;
            (cx + r * (1.0 - t * t) / d, cy + r * 2.0 * t / d)
        })
        .collect()
}

fn random_rgb(rng: &mut Semilla) -> Rgb {
    let b = rng.bits();
    rgb(b as u8, (b >> 8) as u8, (b >> 16) as u8)
}

/// rf-marca's triangle normal, written as `lambert` computes it.
fn cross_of(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let uu = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let vv = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        uu[1] * vv[2] - uu[2] * vv[1],
        uu[2] * vv[0] - uu[0] * vv[2],
        uu[0] * vv[1] - uu[1] * vv[0],
    ]
}

// --- AC1: the surface --------------------------------------------------------

mod ac1 {
    use super::*;

    fn square() -> Vec<Subpath<Pt2>> {
        polygon(&[pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 1.0), pt(0.0, 1.0)])
    }

    #[test]
    fn a_valid_surface_keeps_every_field() {
        let m = one(Rgb::WHITE);
        let s = Surface::new(square(), Facing::Normal([0.0, 1.0, 2.0]), 3.5, 42, m)
            .expect("valid")
            .with_frame([pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)])
            .expect("valid frame");
        assert_eq!(s.outline(), square().as_slice());
        assert_eq!(s.facing(), Facing::Normal([0.0, 1.0, 2.0]));
        assert_eq!((s.depth(), s.id()), (3.5, 42));
        assert_eq!(*s.material(), m);
        assert_eq!(s.frame_point(1.0, 1.0), Some(pt(1.0, 1.0)));
        let flat = Surface::new(square(), Facing::Viewer, 0.0, 0, m).expect("valid");
        assert_eq!(flat.facing().normal(), [0.0, 0.0, 1.0]);
        assert_eq!(flat.frame(), None);
    }

    #[test]
    fn an_empty_or_non_finite_outline_is_refused() {
        let m = one(Rgb::WHITE);
        let f = Facing::Viewer;
        assert_eq!(
            Surface::new(Vec::new(), f, 0.0, 0, m),
            Err(SurfaceError::Outline)
        );
        let bare = vec![Subpath::new(pt(0.0, 0.0))];
        assert_eq!(Surface::new(bare, f, 0.0, 0, m), Err(SurfaceError::Outline));
        let handle = vec![Subpath::new(pt(0.0, 0.0)).cubic_to(
            pt(f64::NAN, 0.0),
            pt(1.0, 1.0),
            pt(1.0, 0.0),
        )];
        assert_eq!(
            Surface::new(handle, f, 0.0, 0, m),
            Err(SurfaceError::Outline)
        );
    }

    #[test]
    fn a_zero_or_non_finite_normal_is_refused() {
        let m = one(Rgb::WHITE);
        for n in [[0.0; 3], [f64::INFINITY, 0.0, 1.0], [0.0, f64::NAN, 1.0]] {
            assert_eq!(
                Surface::new(square(), Facing::Normal(n), 0.0, 0, m),
                Err(SurfaceError::Facing),
                "{n:?}"
            );
        }
    }

    #[test]
    fn a_non_finite_depth_or_frame_is_refused() {
        let m = one(Rgb::WHITE);
        assert_eq!(
            Surface::new(square(), Facing::Viewer, f64::NAN, 0, m),
            Err(SurfaceError::Depth)
        );
        let s = Surface::new(square(), Facing::Viewer, 0.0, 0, m).expect("valid");
        assert_eq!(
            s.with_frame([pt(0.0, 0.0), pt(f64::INFINITY, 0.0), pt(0.0, 1.0)]),
            Err(SurfaceError::Frame)
        );
    }
}

// --- AC2: the material -------------------------------------------------------

mod ac2 {
    use super::*;

    #[test]
    fn valid_materials_hold_their_fields() {
        let ramp = [rgb(1, 2, 3), rgb(4, 5, 6), rgb(7, 8, 9)];
        let m = Material::new(&ramp)
            .and_then(|m| m.with_opacity(0.0))
            .and_then(|m| m.with_seam(Rgb::BLACK, 0.0))
            .expect("valid")
            .with_layer(Layer::Skin);
        assert_eq!(m.tones(), &ramp);
        assert_eq!(m.opacity(), 0.0);
        assert_eq!(
            m.seam().map(|s| (s.colour, s.width)),
            Some((Rgb::BLACK, 0.0))
        );
        assert_eq!(m.layer(), Layer::Skin);
        let eight = [Rgb::WHITE; 8];
        assert_eq!(Material::new(&eight).map(|m| m.tones().len()), Ok(8));
        let plain = one(Rgb::WHITE);
        assert_eq!((plain.opacity(), plain.layer()), (1.0, Layer::Solid));
    }

    #[test]
    fn every_bad_field_is_named() {
        assert_eq!(Material::new(&[]), Err(MaterialError::NoTones));
        assert_eq!(
            Material::new(&[Rgb::WHITE; 9]),
            Err(MaterialError::TooManyTones(9))
        );
        let m = one(Rgb::WHITE);
        for a in [-0.1, 1.1, f64::INFINITY] {
            assert_eq!(m.with_opacity(a), Err(MaterialError::Opacity(a)));
        }
        assert!(matches!(
            m.with_opacity(f64::NAN),
            Err(MaterialError::Opacity(a)) if a.is_nan()
        ));
        for w in [-1.0, f64::INFINITY] {
            assert_eq!(m.with_seam(Rgb::BLACK, w), Err(MaterialError::SeamWidth(w)));
        }
    }

    /// Unused ramp slots never make two equal ramps differ.
    #[test]
    fn equality_compares_the_ramp_not_its_storage() {
        let a = Material::new(&[Rgb::WHITE, Rgb::BLACK]).expect("valid");
        let b = Material::new(&[Rgb::WHITE, Rgb::BLACK]).expect("valid");
        assert_eq!(a, b);
        assert_ne!(a, Material::new(&[Rgb::WHITE]).expect("valid"));
    }
}

// --- AC3: the light ----------------------------------------------------------

mod ac3 {
    use super::*;

    #[test]
    fn the_candle_round_trips_exactly() {
        assert_eq!(vela().toward(), VELA);
    }

    #[test]
    fn a_zero_or_non_finite_light_is_refused() {
        for d in [
            [0.0; 3],
            [f64::NAN, 0.0, 1.0],
            [0.0, 0.0, f64::NEG_INFINITY],
        ] {
            assert_eq!(Light::new(d), Err(LightError::Direction));
        }
    }
}

// --- AC4: the trait ----------------------------------------------------------

mod ac4 {
    use super::*;

    #[test]
    fn the_trait_is_object_safe() {
        let boxed: Box<dyn Finish> = Box::new(Faceted::new(3, 0.01).expect("valid"));
        assert!(boxed.paint(&[], &vela(), &CTX).is_empty());
    }

    /// Shuffling the batch shuffles the output the same way, and each
    /// surface keeps its primitive: tones come from the batch's
    /// population, not from positions.
    #[test]
    fn the_output_keeps_the_batch_order() {
        let mut rng = Semilla::new(4);
        let ramp: Vec<Rgb> = (0..4).map(|_| random_rgb(&mut rng)).collect();
        let m = Material::new(&ramp).expect("valid");
        let batch: Vec<Surface> = (0..20)
            .map(|i| {
                let n = [
                    rng.entre(-1.0, 1.0),
                    rng.entre(-1.0, 1.0),
                    rng.entre(0.1, 1.0),
                ];
                surface(&random_polygon(&mut rng), Facing::Normal(n), i, m)
            })
            .collect();
        let finish = Faceted::new(4, rf_marca::u(1.0)).expect("valid");
        let straight = finish.paint(&batch, &vela(), &CTX);
        let order: Vec<usize> = (0..20).map(|k| (k * 7) % 20).collect();
        let shuffled: Vec<Surface> = order.iter().map(|&k| batch[k].clone()).collect();
        let painted = finish.paint(&shuffled, &vela(), &CTX);
        for (slot, &k) in order.iter().enumerate() {
            assert_eq!(painted[slot], straight[k], "surface {k}");
        }
    }
}

// --- AC5: `Faceted` is rf-marca's low poly, byte for byte ---------------------

mod ac5 {
    use super::*;

    const STROKE: f64 = 1.0 / 600.0; // rf-marca's `u(1.0)`

    fn finish() -> Faceted {
        Faceted::new(4, rf_marca::u(1.0)).expect("valid")
    }

    #[test]
    fn faceta_is_an_opaque_surface() {
        assert_eq!(rf_marca::u(1.0), STROKE);
        let mut rng = Semilla::new(11);
        for i in 0..50 {
            let (pts, c) = (random_polygon(&mut rng), random_rgb(&mut rng));
            let s = surface(&pts, Facing::Viewer, i, one(c));
            let ours = finish().paint(&[s], &vela(), &CTX);
            assert_eq!(ours, vec![rf_marca::faceta(&pts, c)], "polygon {i}");
        }
    }

    #[test]
    fn faceta_con_junta_is_a_surface_with_a_seam() {
        let mut rng = Semilla::new(12);
        for i in 0..50 {
            let (pts, c, junta) = (
                random_polygon(&mut rng),
                random_rgb(&mut rng),
                random_rgb(&mut rng),
            );
            let (ancho, alfa) = (rng.entre(0.5, 6.0), rng.entre(0.0, 1.0));
            let alfa = if i % 5 == 0 { 1.0 } else { alfa };
            let m = one(c)
                .with_seam(junta, rf_marca::u(ancho))
                .and_then(|m| m.with_opacity(alfa))
                .expect("valid");
            let ours = finish().paint(&[surface(&pts, Facing::Viewer, i, m)], &vela(), &CTX);
            let theirs = rf_marca::faceta_con_junta(&pts, c, junta, ancho, alfa);
            assert_eq!(ours, vec![theirs], "polygon {i}");
        }
    }

    #[test]
    fn velo_is_a_translucent_surface() {
        let mut rng = Semilla::new(13);
        for i in 0..50 {
            let (pts, c) = (random_polygon(&mut rng), random_rgb(&mut rng));
            let alfa = rng.entre(0.0, 0.999);
            let m = one(c).with_opacity(alfa).expect("valid");
            let ours = finish().paint(&[surface(&pts, Facing::Viewer, i, m)], &vela(), &CTX);
            assert_eq!(ours, vec![rf_marca::velo(&pts, c, alfa)], "polygon {i}");
        }
    }

    /// The light value, bit for bit, on 1000 random canvas triangles.
    #[test]
    fn lambert_is_rf_marcas_to_the_bit() {
        let mut rng = Semilla::new(14);
        for i in 0..1000 {
            let mut v = || {
                [
                    rng.entre(0.0, 1080.0),
                    rng.entre(0.0, 1920.0),
                    rng.entre(-120.0, 120.0),
                ]
            };
            let (a, b, c) = (v(), v(), v());
            let theirs = rf_marca::lambert(a, b, c);
            let ours = lambert(cross_of(a, b, c), VELA);
            assert_eq!(ours.to_bits(), theirs.to_bits(), "triangle {i}");
        }
    }

    #[test]
    fn quantize_is_cuantizar() {
        let mut rng = Semilla::new(15);
        for bands in 1..=8 {
            let values: Vec<f64> = (0..1000).map(|_| rng.entre(-1.0, 1.0)).collect();
            assert_eq!(
                quantize(&values, bands),
                rf_marca::cuantizar(&values, bands)
            );
        }
        assert!(quantize(&[], 4).is_empty());
    }

    /// Bands over a batch of 1000 random normals are `cuantizar` over
    /// rf-marca's own light values.
    #[test]
    fn batch_bands_are_cuantizar_of_lambert() {
        let mut rng = Semilla::new(16);
        let m = one(Rgb::WHITE);
        let mut lit = Vec::new();
        let mut batch = Vec::new();
        for i in 0..1000 {
            let mut v = || {
                [
                    rng.entre(0.0, 900.0),
                    rng.entre(0.0, 900.0),
                    rng.entre(-80.0, 80.0),
                ]
            };
            let (a, b, c) = (v(), v(), v());
            lit.push(rf_marca::lambert(a, b, c));
            let tri: Vec<P> = [a, b, c].iter().map(|q| (q[0], q[1])).collect();
            let s = Surface::new(outline(&tri), Facing::Normal(cross_of(a, b, c)), 0.0, i, m);
            batch.push(s.expect("a random triangle does not collapse"));
        }
        let ours: Vec<usize> = finish()
            .bands_of(&batch, &vela())
            .into_iter()
            .map(|b| b.expect("a solid surface has a band"))
            .collect();
        assert_eq!(ours, rf_marca::cuantizar(&lit, 4));
    }

    /// A whole figure the way fbf's `marca.rs` draws one: light per face,
    /// four bands over the figure, `faceta` in the band's colour.
    #[test]
    fn a_whole_figure_is_painted_as_rf_marca_paints_it() {
        let mut rng = Semilla::new(17);
        let palette: Vec<Rgb> = (0..4).map(|_| random_rgb(&mut rng)).collect();
        let m = Material::new(&palette).expect("valid");
        let quads: Vec<[[f64; 3]; 4]> = (0..40)
            .map(|_| {
                let (x, y) = (rng.entre(100.0, 900.0), rng.entre(100.0, 1700.0));
                let z = [0, 1, 2, 3].map(|_| rng.entre(0.0, 60.0));
                [
                    [x, y, z[0]],
                    [x + 60.0, y, z[1]],
                    [x + 60.0, y + 60.0, z[2]],
                    [x, y + 60.0, z[3]],
                ]
            })
            .collect();
        let luz: Vec<f64> = quads
            .iter()
            .map(|q| rf_marca::lambert(q[0], q[1], q[2]))
            .collect();
        let theirs: Vec<Prim2> = quads
            .iter()
            .zip(rf_marca::cuantizar(&luz, 4))
            .map(|(q, k)| {
                let pts: Vec<P> = q.iter().map(|v| (v[0], v[1])).collect();
                rf_marca::faceta(&pts, palette[k])
            })
            .collect();
        let batch: Vec<Surface> = quads
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let pts: Vec<P> = q.iter().map(|v| (v[0], v[1])).collect();
                surface(
                    &pts,
                    Facing::Normal(cross_of(q[0], q[1], q[2])),
                    i as u64,
                    m,
                )
            })
            .collect();
        assert_eq!(finish().paint(&batch, &vela(), &CTX), theirs);
    }

    #[test]
    fn a_bad_faceted_is_refused() {
        assert_eq!(Faceted::new(0, 0.01), Err(FacetedError::Bands));
        assert_eq!(Faceted::new(3, -0.01), Err(FacetedError::Stroke(-0.01)));
    }
}

// --- AC6: nothing already rendered moves --------------------------------------

mod ac6 {
    use super::*;
    use garust::Motor3;
    use motoreel::{Look, Object, Scene};

    /// A scene with no meshes evaluates to the same primitives with or
    /// without a finish: the trait is additive.
    #[test]
    fn eval_with_a_look_changes_nothing_without_meshes() {
        let mut scene = Scene::new(1.0);
        scene.add(
            Object::planar(polygon(&[pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)]))
                .at(Motor3::translator(0.1, 0.2, 0.0)),
        );
        scene.add(Object::segment(
            garust::pga::Point::new(0.0, 0.0, 0.0),
            garust::pga::Point::new(1.0, 1.0, 0.0),
        ));
        let finish = Faceted::new(3, 0.01).expect("valid");
        let look = Look {
            finish: &finish,
            light: vela(),
            seed: 1,
        };
        assert_eq!(scene.eval_with(0.5, &look), scene.eval(0.5));
    }
}

// --- AC7: the trait carries the planned finishes ------------------------------

mod ac7 {
    use super::*;

    /// A stroke-placing finish (the shape of `Pintura`): marks at
    /// `(seed, id)`-seeded frame coordinates, so they ride the surface.
    struct Strokes;

    impl Finish for Strokes {
        fn paint(&self, batch: &[Surface], _: &Light, ctx: &FrameCtx) -> Vec<Prim2> {
            let mut out = Vec::new();
            for s in batch {
                let mut rng = Semilla::new(ctx.seed ^ s.id().wrapping_mul(0x9E37_79B9_7F4A_7C15));
                let count = 3 + rng.bits() % 5;
                for _ in 0..count {
                    let (u, v) = (rng.entre(0.0, 1.0), rng.entre(0.0, 1.0));
                    let (Some(a), Some(b)) = (s.frame_point(u, v), s.frame_point(u + 0.1, v))
                    else {
                        continue;
                    };
                    out.push(Prim2::Segment {
                        a,
                        b,
                        style: Style::default(),
                    });
                }
            }
            out
        }
    }

    /// A two-tone finish (the shape of `Cel`, Gooch et al. 1998): cool to
    /// warm with `n·l`.
    struct TwoTone;

    impl Finish for TwoTone {
        fn paint(&self, batch: &[Surface], light: &Light, _: &FrameCtx) -> Vec<Prim2> {
            batch
                .iter()
                .map(|s| {
                    let k = (1.0 + lambert(s.facing().normal(), light.toward())) / 2.0;
                    let mix = |a: u8, b: u8| {
                        (f64::from(a) + (f64::from(b) - f64::from(a)) * k).round() as u8
                    };
                    let c = rgb(mix(40, 250), mix(60, 180), mix(200, 60));
                    Prim2::Path {
                        subpaths: s.outline().to_vec(),
                        style: Style {
                            fill: Some(Fill::solid(c, 1.0)),
                            width: 0.0,
                            ..Style::default()
                        },
                    }
                })
                .collect()
        }
    }

    /// A two-layer finish (the shape of `RayosX`): skin translucent,
    /// inner parts opaque, both read from the material's layer.
    struct TwoLayer;

    impl Finish for TwoLayer {
        fn paint(&self, batch: &[Surface], _: &Light, _: &FrameCtx) -> Vec<Prim2> {
            batch
                .iter()
                .map(|s| {
                    let alpha = match s.material().layer() {
                        Layer::Skin => 0.3,
                        Layer::Inner | Layer::Solid => 1.0,
                    };
                    Prim2::Path {
                        subpaths: s.outline().to_vec(),
                        style: Style {
                            fill: Some(Fill::solid(s.material().tones()[0], alpha)),
                            width: 0.0,
                            ..Style::default()
                        },
                    }
                })
                .collect()
        }
    }

    fn translated(s: &Surface, dx: f64) -> Surface {
        let shift = |p: &Pt2| pt(p.x + dx, p.y);
        let outline = s.outline().iter().map(|sp| sp.map(shift)).collect();
        let frame = s.frame().expect("framed").map(|p| shift(&p));
        Surface::new(outline, s.facing(), s.depth(), s.id(), *s.material())
            .and_then(|s| s.with_frame(frame))
            .expect("valid")
    }

    #[test]
    fn marks_ride_the_surface_frame() {
        let tri = [(100.0, 100.0), (300.0, 120.0), (180.0, 330.0)];
        let q: Vec<Pt2> = tri.iter().map(|&(x, y)| rf_marca::p(x, y)).collect();
        let s = surface(&tri, Facing::Viewer, 9, one(Rgb::WHITE))
            .with_frame([q[0], q[1], q[2]])
            .expect("valid frame");
        let at_t0 = Strokes.paint(std::slice::from_ref(&s), &vela(), &CTX);
        let later = FrameCtx { t: 1.0, ..CTX };
        let at_t1 = Strokes.paint(&[translated(&s, 0.25)], &vela(), &later);
        assert!(!at_t0.is_empty());
        assert_eq!(at_t0.len(), at_t1.len(), "the same marks");
        for (a, b) in at_t0.iter().zip(&at_t1) {
            let (Prim2::Segment { a: a0, b: b0, .. }, Prim2::Segment { a: a1, b: b1, .. }) = (a, b)
            else {
                panic!("marks are segments");
            };
            for (p, q) in [(a0, a1), (b0, b1)] {
                assert!((q.x - p.x - 0.25).abs() < 1e-12 && (q.y - p.y).abs() < 1e-12);
            }
        }
        let other_seed = FrameCtx { seed: 8, ..CTX };
        assert_ne!(Strokes.paint(&[s], &vela(), &other_seed), at_t0);
    }

    #[test]
    fn two_tone_warms_monotonically_with_the_light() {
        let m = one(Rgb::WHITE);
        let tri = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
        let batch: Vec<Surface> = (0..=20)
            .map(|k| {
                // Normals swept from away from the candle to toward it.
                let a = f64::from(k) / 20.0;
                let n = [-0.6 * a + 0.6 * (1.0 - a), -0.6 * a + 0.6 * (1.0 - a), 0.52];
                surface(&tri, Facing::Normal(n), k as u64, m)
            })
            .collect();
        let reds: Vec<u8> = TwoTone
            .paint(&batch, &vela(), &CTX)
            .iter()
            .map(|p| match p {
                Prim2::Path { style, .. } => match style.fill.map(|f| f.paint) {
                    Some(motoreel::Paint::Flat(c)) => c.r,
                    _ => panic!("a flat fill"),
                },
                _ => panic!("a path"),
            })
            .collect();
        assert!(reds.windows(2).all(|w| w[0] <= w[1]), "{reds:?}");
        assert!(reds[0] < reds[20]);
    }

    #[test]
    fn the_interior_shows_only_under_the_two_layer_finish() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
        let skin = one(rgb(30, 190, 230)).with_layer(Layer::Skin);
        let bone = one(rgb(240, 240, 255)).with_layer(Layer::Inner);
        let batch = [
            surface(&tri, Facing::Viewer, 0, bone),
            surface(&tri, Facing::Viewer, 1, skin),
        ];
        let faceted = Faceted::new(2, 0.01)
            .expect("valid")
            .paint(&batch, &vela(), &CTX);
        assert_eq!(faceted.len(), 1, "the bone is hidden");
        let layered = TwoLayer.paint(&batch, &vela(), &CTX);
        let alphas: Vec<f64> = layered
            .iter()
            .filter_map(|p| match p {
                Prim2::Path { style, .. } => style.fill.map(|f| f.alpha),
                _ => None,
            })
            .collect();
        assert_eq!(alphas, vec![1.0, 0.3], "bone opaque, skin translucent");
    }
}

// --- AC8: determinism and the goldens -----------------------------------------

mod ac8 {
    use super::*;

    const SIZE: (u32, u32) = (160, 90);
    const VIEW: (f64, f64) = (3.2, 1.8);

    /// The five-face figure: a square table and four bevels around it,
    /// with hand-set normals (trig-free), under the candle in 4 bands.
    fn figure() -> Vec<Prim2> {
        let palette = [
            rgb(18, 110, 168),
            rgb(38, 150, 200),
            rgb(38, 190, 230),
            rgb(160, 240, 255),
        ];
        let m = Material::new(&palette).expect("valid");
        let (a, b) = (0.35, 0.8);
        let faces: [(Vec<Pt2>, [f64; 3]); 5] = [
            (
                vec![pt(-a, -a), pt(a, -a), pt(a, a), pt(-a, a)],
                [0.0, 0.0, 1.0],
            ),
            (
                vec![pt(-b, b), pt(-a, a), pt(a, a), pt(b, b)],
                [0.0, 0.7, 0.7],
            ),
            (
                vec![pt(-b, -b), pt(-a, -a), pt(-a, a), pt(-b, b)],
                [-0.5, 0.0, 0.86],
            ),
            (
                vec![pt(b, -b), pt(a, -a), pt(-a, -a), pt(-b, -b)],
                [0.0, -0.7, 0.7],
            ),
            (
                vec![pt(b, b), pt(a, a), pt(a, -a), pt(b, -b)],
                [0.5, 0.0, 0.86],
            ),
        ];
        let batch: Vec<Surface> = faces
            .iter()
            .enumerate()
            .map(|(i, (pts, n))| {
                Surface::new(polygon(pts), Facing::Normal(*n), 0.0, i as u64, m).expect("valid")
            })
            .collect();
        // The candle in a y-up view: rf-marca's canvas y points down.
        let light = Light::new([-0.6, 0.6, 0.52]).expect("valid");
        Faceted::new(4, 0.02)
            .expect("valid")
            .paint(&batch, &light, &CTX)
    }

    fn render(name: &str, svg: bool) -> Vec<u8> {
        let dir = tmp_dir(name);
        let prims = figure();
        let file = if svg {
            let mut sink = SvgSink::with_view(&dir, SIZE, VIEW).expect("svg sink");
            sink.frame(0, &prims).expect("frame");
            "frame_00000.svg"
        } else {
            let mut sink = PpmSink::with_view(&dir, SIZE, VIEW).expect("ppm sink");
            sink.frame(0, &prims).expect("frame");
            "frame_00000.ppm"
        };
        fs::read(dir.join(file)).expect("read frame")
    }

    fn check_golden(fixture: &str, got: &[u8]) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(fixture);
        if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
            fs::write(&path, got).expect("bless the golden fixture");
            return;
        }
        let want = fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing: MOTOREEL_BLESS=1 cargo test -p motoreel --test r0017_finish",
                path.display()
            )
        });
        assert!(want == got, "{fixture} drifted");
    }

    #[test]
    fn two_renders_are_byte_identical() {
        for svg in [false, true] {
            assert!(
                render(&format!("r0017_twice_a_{svg}"), svg)
                    == render(&format!("r0017_twice_b_{svg}"), svg)
            );
        }
    }

    #[test]
    fn renders_on_several_threads_are_byte_identical() {
        let reference = render("r0017_threads_ref", false);
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|i| s.spawn(move || render(&format!("r0017_threads_{i}"), false)))
                .collect();
            for h in handles {
                assert!(h.join().expect("render thread") == reference);
            }
        });
    }

    #[test]
    fn the_figure_uses_four_bands() {
        let fills: std::collections::BTreeSet<(u8, u8, u8)> = figure()
            .iter()
            .filter_map(|p| match p {
                Prim2::Path { style, .. } => match style.fill.map(|f| f.paint) {
                    Some(motoreel::Paint::Flat(c)) => Some((c.r, c.g, c.b)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 4, "{fills:?}");
    }

    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        check_golden("r0017_facetas.ppm", &render("r0017_ppm_golden", false));
    }

    #[test]
    fn the_svg_golden_matches_byte_for_byte() {
        check_golden("r0017_facetas.svg", &render("r0017_svg_golden", true));
    }

    /// Only `+ − × ÷`, `sqrt` and comparisons: no transcendental and no
    /// fused multiply-add in the finish and surface code.
    #[test]
    fn the_finish_code_is_trig_free() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for file in ["finish.rs", "faceted.rs"] {
            let code = fs::read_to_string(src.join(file)).expect("read source");
            for banned in [
                ".sin(", ".cos(", ".tan(", ".atan", ".exp(", ".ln(", ".powf(", "mul_add",
            ] {
                assert!(!code.contains(banned), "{file} uses {banned}");
            }
        }
    }
}

// --- AC9: the kernel stays thin -------------------------------------------------

mod ac9 {
    use super::*;

    /// The library's dependencies are still garust and the optional
    /// typesetter: the finish adds none.
    #[test]
    fn no_new_dependency() {
        let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("read the manifest");
        let deps = manifest
            .split("[dependencies]")
            .nth(1)
            .and_then(|s| s.split("\n[").next())
            .expect("a [dependencies] table");
        let names: Vec<&str> = deps
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && l.contains('='))
            .filter_map(|l| l.split('=').next().map(str::trim))
            .collect();
        assert_eq!(names, vec!["garust", "motoreel-typeset"]);
    }

    /// The finish is in the core, not behind a feature: it is reachable
    /// from a plain `use motoreel::…` and its outline is a core `Seg`.
    #[test]
    fn the_finish_lives_in_the_core() {
        let s = surface(
            &[(0.0, 0.0), (9.0, 0.0), (0.0, 9.0)],
            Facing::Viewer,
            0,
            one(Rgb::WHITE),
        );
        assert!(matches!(s.outline()[0].segs[0], Seg::Line(_)));
    }
}
