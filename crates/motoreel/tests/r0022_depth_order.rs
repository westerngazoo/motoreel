//! R-0022 — depth-order performance («rendimiento del orden de
//! profundidad»): the acceptance suite, one `mod` per criterion of
//! `requirements/0022-depth-order-performance.md` that a test can check.
//!
//! AC1's differential test needs the previous order, which is crate-private
//! (`src/solid_reference.rs`), so it lives with it as a unit test:
//! `solid_reference::differential::*`, and the exact sandwich's own
//! proptest is `sandwich::tests::*`. AC4's speed-up is the bench's
//! (`cargo bench -p motoreel`), measured before and after on one machine.

use std::fs;
use std::path::{Path, PathBuf};

use garust::Motor3;
use motoreel::{
    coalesce, obj::layer_of, Axes, Camera, Faceted, FrameSink, Layer, Light, Look, Material, Mesh,
    Obj, Object, Paint, Peel, PpmSink, Prim2, Rgb, Scene,
};

// --- Shared helpers ---------------------------------------------------------

const SIZE: (u32, u32) = (540, 960);
const VIEW: (f64, f64) = (1080.0, 1920.0);

fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn ramp() -> Material {
    Material::new(&[
        Rgb {
            r: 18,
            g: 110,
            b: 168,
        },
        Rgb {
            r: 38,
            g: 190,
            b: 230,
        },
        Rgb {
            r: 160,
            g: 240,
            b: 255,
        },
    ])
    .expect("valid")
}

fn layer(text: &str) -> Vec<Mesh> {
    let bind = |name: &str| Some(ramp().with_layer(layer_of(name).unwrap_or(Layer::Solid)));
    Obj::parse(text, Axes::ZUp)
        .expect("a committed asset loads")
        .meshes(bind)
        .expect("binds")
        .into_iter()
        .map(|(_, mesh)| mesh)
        .collect()
}

/// The house toy in its three layers, the skin peeled by `u` (whole at 0).
fn toy(u: f64) -> Vec<Mesh> {
    let peel = Peel::new(0.06, [0.0, 1.0, 0.0], (1.85, 0.0), 0.6).expect("valid");
    let skin = layer(include_str!("../../../assets/anatomy/skin.obj"))
        .into_iter()
        .map(|m| match u {
            0.0 => m,
            _ => peel.apply(&m, u).expect("valid"),
        });
    skin.chain(layer(include_str!("../../../assets/anatomy/muscle.obj")))
        .chain(layer(include_str!("../../../assets/anatomy/bone.obj")))
        .collect()
}

/// A pinhole camera 4.6 m from the toy's middle at `azimuth` (radians),
/// 8° down, as the anatomy reel frames it.
fn camera(azimuth: f64) -> Camera {
    let about = |a: f64, x: f64, y: f64, z: f64| {
        let (s, c) = ((a / 2.0).sin(), (a / 2.0).cos());
        Motor3::from_unit_quaternion(c, x * s, y * s, z * s)
    };
    Camera::pinhole(
        Motor3::translator(0.02, 0.95, 0.0)
            .compose(&about(azimuth, 0.0, 1.0, 0.0))
            .compose(&about(-8f64.to_radians(), 1.0, 0.0, 0.0))
            .compose(&Motor3::translator(0.0, 0.0, 4.6)),
        2700.0,
    )
}

fn scene(meshes: &[Mesh], azimuth: f64) -> Scene {
    let mut scene = Scene::new(1.0);
    scene.view = VIEW;
    scene.camera = camera(azimuth);
    for mesh in meshes {
        scene.add(Object::mesh(mesh.clone()));
    }
    scene
}

fn faceted() -> Faceted {
    Faceted::new(3, 2.0).expect("valid")
}

fn frame(scene: &Scene) -> Vec<Prim2> {
    let finish = faceted();
    let look = Look {
        finish: &finish,
        light: Light::new(scene.camera.view_direction([0.55, 0.8, 0.45])).expect("valid"),
        seed: 0,
    };
    scene.eval_with(0.0, &look)
}

/// Eight azimuths all round, off the axes.
fn azimuths() -> impl Iterator<Item = f64> {
    (0..8).map(|k| (f64::from(k) + 0.3) * std::f64::consts::TAU / 8.0)
}

// --- AC3: determinism -------------------------------------------------------------

mod ac3 {
    use super::*;

    /// Two renders, and four threads each rendering every view, give the
    /// same primitives, compared as exact text (`-0.0` included).
    #[test]
    fn two_renders_and_four_threads_are_byte_identical() {
        for u in [0.0, 0.5] {
            let meshes = toy(u);
            let serial: Vec<String> = azimuths()
                .map(|a| format!("{:?}", frame(&scene(&meshes, a))))
                .collect();
            let again: Vec<String> = azimuths()
                .map(|a| format!("{:?}", frame(&scene(&meshes, a))))
                .collect();
            assert_eq!(serial, again, "two renders, peel {u}");
            let threaded: Vec<Vec<String>> = std::thread::scope(|s| {
                let workers: Vec<_> = (0..4)
                    .map(|_| {
                        s.spawn(|| {
                            azimuths()
                                .map(|a| format!("{:?}", frame(&scene(&meshes, a))))
                                .collect::<Vec<String>>()
                        })
                    })
                    .collect();
                workers
                    .into_iter()
                    .map(|w| w.join().expect("a render thread"))
                    .collect()
            });
            for run in threaded {
                assert_eq!(run, serial, "a thread, peel {u}");
            }
        }
    }
}

// --- AC6: coalescing, opt-in ------------------------------------------------------

mod ac6 {
    use super::*;

    /// Every path primitive as (style, subpath) pairs, in paint order.
    fn pieces(prims: &[Prim2]) -> Vec<String> {
        prims
            .iter()
            .flat_map(|p| match p {
                Prim2::Path { subpaths, style } => subpaths
                    .iter()
                    .map(|s| format!("{style:?} {s:?}"))
                    .collect::<Vec<_>>(),
                other => vec![format!("{other:?}")],
            })
            .collect()
    }

    /// The default render is one path per face: `eval_with` never merges.
    #[test]
    fn the_default_render_is_one_path_per_face() {
        let scene = scene(&toy(0.0), 0.7);
        let shown = scene
            .surfaces(0.0)
            .into_iter()
            .flatten()
            .filter(|s| s.material().layer() != Layer::Inner)
            .count();
        let prims = frame(&scene);
        assert_eq!(prims.len(), shown);
        assert!(prims.iter().all(|p| match p {
            Prim2::Path { subpaths, .. } => subpaths.len() == 1,
            _ => false,
        }));
    }

    /// Merging keeps every piece, in order, with its style; it only
    /// changes which primitive a piece belongs to. On the toy it leaves
    /// under half of the paths.
    #[test]
    fn coalescing_keeps_every_piece_in_order_and_cuts_the_paths() {
        for a in azimuths() {
            let prims = frame(&scene(&toy(0.0), a));
            let merged = coalesce(prims.clone());
            assert_eq!(pieces(&merged), pieces(&prims), "azimuth {a}");
            println!(
                "azimuth {a:.2}: {} paths from {}",
                merged.len(),
                prims.len()
            );
            assert!(
                merged.len() * 2 <= prims.len(),
                "azimuth {a}: {} paths from {}",
                merged.len(),
                prims.len()
            );
        }
    }

    /// A translucent run (the x-ray glass, a peeled facet fading) is never
    /// merged: compositing it once would change its colour.
    #[test]
    fn translucent_paths_are_never_merged() {
        let prims = frame(&scene(&toy(0.5), 0.7));
        let translucent = |p: &Prim2| match p {
            Prim2::Path { style, .. } => style.fill.is_some_and(|f| f.alpha < 1.0),
            _ => false,
        };
        let before = prims.iter().filter(|p| translucent(p)).count();
        assert!(before > 0, "the half peel has fading facets");
        let after = coalesce(prims).iter().filter(|p| translucent(p)).count();
        assert_eq!(after, before);
    }

    /// What merging does to the pixels: the union of a run is composited
    /// once, so only antialiased pixels two pieces of a run both touch can
    /// change. Under 1 % of the frame changes; a changed pixel lies on an
    /// edge of the default frame (a neighbour of another colour), or on a
    /// seam inside a run, where it moves a level or two (SPEC-0022 §2.4).
    #[test]
    fn merged_pixels_change_only_on_antialiased_edges() {
        for (k, a) in azimuths().enumerate() {
            let prims = frame(&scene(&toy(0.0), a));
            let flat = |p: &Prim2| match p {
                Prim2::Path { style, .. } => {
                    matches!(style.fill.map(|f| f.paint), Some(Paint::Flat(_)))
                }
                _ => false,
            };
            assert!(prims.iter().all(flat));
            let render = |name: &str, prims: &[Prim2]| {
                let dir = tmp_dir(&format!("r0022_ac6_{name}_{k}"));
                let mut sink = PpmSink::with_view(&dir, SIZE, VIEW).expect("ppm sink");
                sink.frame(0, prims).expect("frame");
                fs::read(dir.join("frame_00000.ppm")).expect("read frame")
            };
            let (one, merged) = (render("each", &prims), render("merged", &coalesce(prims)));
            assert_eq!(one.len(), merged.len());
            let header = one.len() - (SIZE.0 * SIZE.1 * 3) as usize;
            let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
            let px = |img: &[u8], x: usize, y: usize| {
                let k = header + 3 * (y * w + x);
                [img[k], img[k + 1], img[k + 2]]
            };
            let (mut changed, mut worst, mut seams) = (0, 0, 0);
            for y in 0..h {
                for x in 0..w {
                    let (p, q) = (px(&one, x, y), px(&merged, x, y));
                    let d = p
                        .iter()
                        .zip(q)
                        .map(|(a, b)| a.abs_diff(b))
                        .max()
                        .unwrap_or(0);
                    if d == 0 {
                        continue;
                    }
                    changed += 1;
                    worst = worst.max(d);
                    let edge = [(0, 1), (2, 1), (1, 0), (1, 2)].iter().any(|&(dx, dy)| {
                        let (nx, ny) = ((x + dx).wrapping_sub(1), (y + dy).wrapping_sub(1));
                        nx < w && ny < h && px(&one, nx, ny) != p
                    });
                    // Off an edge, only a seam inside a run: a level or two.
                    if !edge {
                        seams += 1;
                        assert!(d <= 2, "azimuth {a}: ({x}, {y}) off an edge by {d}");
                    }
                }
            }
            let total = w * h;
            println!(
                "azimuth {a:.2}: {changed} of {total} pixels changed, at most {worst} levels; \
                 {seams} off an edge"
            );
            assert!(
                changed * 100 <= total,
                "azimuth {a}: {changed} pixels changed"
            );
        }
    }
}
