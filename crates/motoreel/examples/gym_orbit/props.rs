//! The gym behind the toy: the power rack and the cable machine, modelled
//! in Blender (`tools/blender/props.py`), read from OBJ (R-0020) and posed
//! by motors like every other part.
//!
//! Both stand at `x < −0.95`, behind the platform from every azimuth of
//! the orbit (15° to 165°), so neither ever covers the lifter: the rack on
//! his left, the cable machine on his right, both facing the way he does.

use std::f64::consts::FRAC_1_SQRT_2;

use garust::Motor3;
use motoreel::{Axes, Material, Obj, Object, Rgb};

use crate::palette::{acero, goma, tinta_cuerpo, tinta_objeto};

const RACK: &str = include_str!("../../../../assets/props/power_rack.obj");
const CABLE: &str = include_str!("../../../../assets/props/cable_machine.obj");

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

fn ramp(shadow: Rgb, base: Rgb, light: Rgb) -> Material {
    Material::new(&[shadow, base, light]).unwrap_or_else(|e| panic!("a ramp is valid: {e}"))
}

/// The OBJ's material names (`tools/blender/props.py`) as the house's
/// materials. The names `skin_*` and `inner_*` set the layer themselves.
fn bind(name: &str) -> Option<Material> {
    Some(match name {
        "ink" => tinta_objeto(),
        "skin_ink" => tinta_cuerpo(),
        "steel" => acero(),
        "chrome" => ramp(rgb(150, 158, 178), rgb(205, 212, 228), rgb(246, 248, 255)),
        "pad" => ramp(rgb(96, 14, 24), rgb(176, 32, 44), rgb(232, 74, 80)),
        "rubber" => goma(),
        "skin_shroud" => ramp(rgb(46, 56, 92), rgb(74, 88, 136), rgb(112, 128, 180)),
        "inner_stack" => ramp(rgb(186, 132, 10), rgb(255, 210, 63), rgb(255, 232, 120)),
        _ => return None,
    })
}

/// Every part of the prop in `text`, a quarter turn about +y (Blender's
/// front, +z here, turned to +x) and then moved to `(x, z)` on the floor.
fn place(text: &str, x: f64, z: f64) -> Vec<Object> {
    let pose = Motor3::translator(x, 0.0, z).compose(&Motor3::from_unit_quaternion(
        FRAC_1_SQRT_2,
        0.0,
        FRAC_1_SQRT_2,
        0.0,
    ));
    Obj::parse(text, Axes::ZUp)
        .and_then(|obj| obj.meshes(bind))
        .unwrap_or_else(|e| panic!("a committed prop loads: {e}"))
        .into_iter()
        .map(|(_, mesh)| Object::mesh(mesh).at(pose))
        .collect()
}

/// The rack on the lifter's left, the cable machine on his right.
pub fn gym() -> Vec<Object> {
    let mut out = place(RACK, -1.75, -1.15);
    out.extend(place(CABLE, -1.7, 1.15));
    out
}
