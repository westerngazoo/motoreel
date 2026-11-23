//! The toy in three layers, read from `assets/anatomy/` (R-0021): skin,
//! muscle and bone, each part named `segment.part`. A segment's parts ride
//! one motor track, so the three layers curl together. Only the forearms
//! and hands move here (about the elbows of `joints.tsv`); in a reel the
//! tracks come from physics, one per segment.

use garust::{pga, Motor3, Pga3};
use motoreel::{Axes, Ease, Layer, Material, Mesh, Obj, Object, Peel, Rgb, Style, Track};

use crate::palette::{
    acero, amarillo, azul, biceps, hueso, pupila, short, tenis, tinta_cuerpo, tinta_objeto,
};

const SKIN: &str = include_str!("../../../../assets/anatomy/skin.obj");
const MUSCLE: &str = include_str!("../../../../assets/anatomy/muscle.obj");
const BONE: &str = include_str!("../../../../assets/anatomy/bone.obj");
const JOINTS: &str = include_str!("../../../../assets/anatomy/joints.tsv");
const DUMBBELL: &str = include_str!("../../../../assets/props/dumbbell.obj");

/// The elbow's flexion from straight (degrees) at each key (seconds): two
/// reps, then up to 90° and held there, the forearm's longest lever.
const CURL: [(f64, f64); 7] = [
    (0.0, 10.0),
    (1.4, 110.0),
    (2.8, 10.0),
    (4.2, 110.0),
    (5.6, 10.0),
    (7.2, 90.0),
    (9.0, 90.0),
];

/// What changes from frame to frame besides the tracks.
pub struct Moment {
    /// The skin's peel, 0 to 1; `None` once the skin is gone.
    pub peel: Option<f64>,
    /// How far the biceps is lit, 0 to 1.
    pub biceps: f64,
    /// How far the forearm bones are lit, 0 to 1.
    pub forearm: f64,
    /// The lever overlay's opacity.
    pub lever: f64,
}

/// The parsed layers, the dumbbell and the elbows.
pub struct Body {
    skin: Obj,
    muscle: Obj,
    bone: Obj,
    dumbbell: Obj,
    elbows: Vec<(String, [f64; 3])>,
    hands: Vec<(String, [f64; 3])>,
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// Muscle at rest: a dusty rose, glass under the x-ray.
fn musculo() -> Material {
    Material::new(&[rgb(122, 40, 62), rgb(198, 86, 106), rgb(238, 146, 156)])
        .unwrap_or_else(|e| panic!("a ramp is valid: {e}"))
        .with_layer(Layer::Skin)
}

fn parse(text: &str) -> Obj {
    Obj::parse(text, Axes::ZUp).unwrap_or_else(|e| panic!("a committed asset loads: {e}"))
}

/// The joint of `segment` in `joints.tsv`.
fn joint(segment: &str) -> [f64; 3] {
    JOINTS
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|c| c[0] == segment)
        .map(|c| {
            let n = |k: usize| {
                c[k].parse::<f64>()
                    .unwrap_or_else(|e| panic!("{segment}: {e}"))
            };
            [n(2), n(3), n(4)]
        })
        .unwrap_or_else(|| panic!("no joint {segment}"))
}

/// A right-handed turn of `degrees` about the z axis through `e`: elbow
/// flexion, carrying the hanging forearm forward and up.
fn about(e: [f64; 3], degrees: f64) -> Motor3 {
    Motor3::translator(e[0], e[1], e[2])
        .compose(&Motor3::rotor(degrees.to_radians(), Pga3::basis(0b0011)))
        .compose(&Motor3::translator(-e[0], -e[1], -e[2]))
}

/// A segment's name and its side's suffix: `forearm_r` → `r`.
fn side(segment: &str) -> &str {
    segment.rsplit('_').next().unwrap_or_default()
}

impl Body {
    pub fn load() -> Body {
        let sides = ["r", "l"];
        Body {
            skin: parse(SKIN),
            muscle: parse(MUSCLE),
            bone: parse(BONE),
            dumbbell: parse(DUMBBELL),
            elbows: sides
                .iter()
                .map(|s| (s.to_string(), joint(&format!("forearm_{s}"))))
                .collect(),
            hands: sides
                .iter()
                .map(|s| {
                    let w = joint(&format!("hand_{s}"));
                    (
                        s.to_string(),
                        [w[0], w[1] - 0.03, w[2] + 0.005 * w[2].signum()],
                    )
                })
                .collect(),
        }
    }

    fn elbow(&self, side: &str) -> [f64; 3] {
        self.elbows
            .iter()
            .find(|(s, _)| s == side)
            .map_or([0.0; 3], |(_, e)| *e)
    }

    fn hand(&self, side: &str) -> [f64; 3] {
        self.hands
            .iter()
            .find(|(s, _)| s == side)
            .map_or([0.0; 3], |(_, h)| *h)
    }

    /// The curl on `side`, then `after`.
    fn curl(&self, side: &str, after: Motor3) -> Track {
        let e = self.elbow(side);
        Track::keys(CURL.map(|(t, deg)| (t, about(e, deg).compose(&after))))
            .unwrap_or_else(|err| panic!("the curl's keys are valid: {err}"))
            .ease(Ease::SmoothStep)
    }

    /// The track a part of `segment` rides.
    fn track(&self, segment: &str) -> Track {
        if segment.starts_with("forearm_") || segment.starts_with("hand_") {
            self.curl(side(segment), Motor3::identity())
        } else {
            Track::hold(Motor3::identity())
        }
    }

    /// Every part of `obj`, bound by `bind`, each mapped by `shape` and
    /// riding its segment's track.
    fn layer(
        &self,
        obj: &Obj,
        bind: impl Fn(&str) -> Option<Material>,
        shape: impl Fn(&str, Mesh) -> Mesh,
    ) -> Vec<Object> {
        obj.meshes(bind)
            .unwrap_or_else(|e| panic!("a committed layer binds: {e}"))
            .into_iter()
            .map(|(name, mesh)| {
                let segment = name.split('.').next().unwrap_or_default();
                Object::mesh(shape(&name, mesh)).with_track(self.track(segment))
            })
            .collect()
    }

    /// The figure's objects at `m`: skin (peeled), muscle, bone, and a
    /// dumbbell in each hand. One depth group.
    pub fn objects(&self, m: &Moment) -> Vec<Object> {
        let load = biceps();
        let mut out = Vec::new();
        if let Some(u) = m.peel {
            // Facets lift 6 cm, head first; ink hulls only fade (a
            // reversed face would lift inward).
            let lift = Peel::new(0.06, [0.0, 1.0, 0.0], (1.85, 0.0), 0.6)
                .unwrap_or_else(|e| panic!("valid: {e}"));
            let fade = Peel::new(0.0, [0.0, 1.0, 0.0], (1.85, 0.0), 0.6)
                .unwrap_or_else(|e| panic!("valid: {e}"));
            out.extend(self.layer(&self.skin, skin_material, |name, mesh| {
                if u == 0.0 {
                    return mesh;
                }
                let peel = if name.ends_with("_ink") { &fade } else { &lift };
                peel.apply(&mesh, u)
                    .unwrap_or_else(|e| panic!("a peel is valid: {e}"))
            }));
        }
        out.extend(self.layer(
            &self.muscle,
            |name| {
                Some(if name == "muscle_biceps" {
                    musculo().mix(&load, m.biceps)
                } else {
                    musculo()
                })
            },
            |_, mesh| mesh,
        ));
        out.extend(self.layer(
            &self.bone,
            |name| match name {
                "inner_bone" => Some(hueso()),
                "inner_forearm" => Some(hueso().mix(&load, m.forearm)),
                _ => None,
            },
            |_, mesh| mesh,
        ));
        for s in ["r", "l"] {
            let h = self.hand(s);
            let place =
                Motor3::translator(h[0], h[1], h[2]).compose(&Motor3::from_unit_quaternion(
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                ));
            let track = self.curl(s, place);
            for (_, mesh) in self
                .dumbbell
                .meshes(|name| match name {
                    "ink" => Some(tinta_objeto()),
                    "steel" | "chrome" => Some(acero()),
                    _ => None,
                })
                .unwrap_or_else(|e| panic!("the dumbbell binds: {e}"))
            {
                out.push(Object::mesh(mesh).with_track(track.clone()));
            }
        }
        out
    }

    /// The forearm as a lever at `t`: elbow to hand, riding the curl, and
    /// the dumbbell's weight hanging from the hand. Paths, drawn on top.
    pub fn lever(&self, t: f64, alpha: f64) -> Vec<Object> {
        if alpha <= 0.0 {
            return Vec::new();
        }
        let style = |width: f64| Style {
            stroke: biceps().tones()[1],
            width,
            alpha,
            fill: None,
        };
        let point = |p: [f64; 3]| pga::Point::new(p[0], p[1], p[2]);
        let mut out = Vec::new();
        for s in ["r", "l"] {
            let (e, h) = (self.elbow(s), self.hand(s));
            let fore = self.curl(s, Motor3::identity());
            out.push(
                Object::segment(point(e), point(h))
                    .with_style(style(7.0))
                    .with_track(fore.clone()),
            );
            let (x, y, z) = point(h).transform(&fore.eval(t)).to_euclidean();
            let tip = [x, y - 0.28, z];
            out.push(Object::segment(point([x, y, z]), point(tip)).with_style(style(5.0)));
            out.push(
                Object::polyline(vec![
                    point([tip[0] - 0.035, tip[1] + 0.05, tip[2]]),
                    point(tip),
                    point([tip[0] + 0.035, tip[1] + 0.05, tip[2]]),
                ])
                .with_style(style(5.0)),
            );
        }
        out
    }
}

/// The skin's names (`tools/blender/anatomy.py`) as the toy's materials.
fn skin_material(name: &str) -> Option<Material> {
    Some(match name {
        "skin_blue" => azul(),
        "skin_shorts" => short(),
        "skin_shoe" | "skin_eye" => tenis(),
        "skin_band" => amarillo(),
        "skin_pupil" => pupila(),
        "skin_ink" => tinta_cuerpo(),
        _ => return None,
    })
}
