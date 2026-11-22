//! The fbf toy's palette tokens, copied **by value** from rotorf-sico's
//! `brand/palette.toml` (the spike's copy, PR #8). motoreel stays
//! brand-agnostic and cannot read that repo; this is an example's copy,
//! not a second source of truth. Each constant is named after its token.
//! Ramps run shadow → base → light, as a [`Material`] wants them.

use motoreel::{Layer, Material, Rgb};

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// `monito-marino`: the navy background.
pub const MARINO: Rgb = rgb(16, 22, 51);
/// `monito-marino-oscuro`: the vignette.
pub const MARINO_OSCURO: Rgb = rgb(7, 10, 26);
/// `monito-tinta`: ink outline and the figure's shadow.
pub const TINTA: Rgb = rgb(10, 12, 30);
/// `monito-calido`: the warm spotlight.
pub const CALIDO: Rgb = rgb(255, 207, 138);
/// `monito-blanco`: eyes, the freeze flash.
pub const BLANCO: Rgb = rgb(246, 248, 255);
/// `monito-cian`: the x-ray glass and the sweep line.
pub const CIAN: Rgb = rgb(34, 211, 238);

/// A three-tone ramp in `layer`.
fn ramp(shadow: Rgb, base: Rgb, light: Rgb, layer: Layer) -> Material {
    Material::new(&[shadow, base, light])
        .unwrap_or_else(|e| panic!("a three-tone ramp is valid: {e}"))
        .with_layer(layer)
}

/// `monito-azul-sombra`, `monito-azul`, `monito-azul-luz`: the blue toy's skin.
pub fn azul() -> Material {
    ramp(
        rgb(18, 110, 168),
        rgb(38, 190, 230),
        rgb(160, 240, 255),
        Layer::Skin,
    )
}

/// `monito-short-sombra`, `monito-short`, `monito-short-luz`.
pub fn short() -> Material {
    ramp(
        rgb(16, 24, 64),
        rgb(30, 46, 110),
        rgb(70, 90, 170),
        Layer::Skin,
    )
}

/// `monito-tenis-sombra`, `monito-blanco`, `monito-tenis-luz`.
pub fn tenis() -> Material {
    ramp(rgb(170, 178, 205), BLANCO, rgb(255, 255, 255), Layer::Skin)
}

/// `toy-amarillo` (the headband and the sole).
pub fn amarillo() -> Material {
    ramp(
        rgb(186, 132, 10),
        rgb(255, 210, 63),
        rgb(255, 232, 120),
        Layer::Skin,
    )
}

/// `monito-tinta` as a surface (pupils).
pub fn pupila() -> Material {
    ramp(TINTA, TINTA, rgb(37, 44, 79), Layer::Skin)
}

/// The ink outline of a body part (glass under x-ray, like its skin).
pub fn tinta_cuerpo() -> Material {
    ramp(TINTA, TINTA, TINTA, Layer::Skin)
}

/// The ink outline of a prop (opaque in every finish).
pub fn tinta_objeto() -> Material {
    ramp(TINTA, TINTA, TINTA, Layer::Solid)
}

/// The dumbbell: `monito-manga`, `steel-dim`, `monito-manga-luz`.
pub fn acero() -> Material {
    ramp(
        rgb(120, 128, 150),
        rgb(168, 171, 179),
        rgb(205, 212, 228),
        Layer::Solid,
    )
}

/// The platform's wood: `monito-madera-frente`, `monito-madera`, `monito-madera-luz`.
pub fn madera() -> Material {
    ramp(
        rgb(122, 70, 32),
        rgb(222, 158, 88),
        rgb(240, 184, 116),
        Layer::Solid,
    )
}

/// The rubber strips beside the wood: `monito-tinta` to `monito-short-sombra`.
pub fn goma() -> Material {
    ramp(TINTA, rgb(16, 24, 64), rgb(37, 44, 79), Layer::Solid)
}

/// Bone in the x-ray: `monito-azul-luz`, `monito-hielo`, `monito-blanco`.
pub fn hueso() -> Material {
    ramp(rgb(160, 240, 255), rgb(222, 246, 255), BLANCO, Layer::Inner)
}

/// The biceps line in the x-ray: `monito-disco-25-sombra`, `toy-rojo`, `monito-naranja`.
pub fn biceps() -> Material {
    ramp(
        rgb(150, 20, 28),
        rgb(255, 59, 59),
        rgb(255, 138, 31),
        Layer::Inner,
    )
}
