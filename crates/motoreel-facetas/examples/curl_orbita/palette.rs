//! The fbf toy's palette tokens, copied by value from rotorf-sico's
//! `brand/palette.toml` (motoreel cannot read that repo). Each constant is
//! named after its token; a spike copy, not a second source of truth.

use motoreel::Rgb;
use motoreel_facetas::Tones;

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
/// `monito-cian`: the x-ray glass and sweep line.
pub const CIAN: Rgb = rgb(34, 211, 238);

/// `monito-azul`, `-luz`, `-sombra`: the blue toy.
pub const AZUL: Tones = Tones {
    light: rgb(160, 240, 255),
    base: rgb(38, 190, 230),
    shadow: rgb(18, 110, 168),
};
/// `monito-short`, `-luz`, `-sombra`.
pub const SHORT: Tones = Tones {
    light: rgb(70, 90, 170),
    base: rgb(30, 46, 110),
    shadow: rgb(16, 24, 64),
};
/// `monito-tenis-luz`, `monito-blanco`, `monito-tenis-sombra`.
pub const TENIS: Tones = Tones {
    light: rgb(255, 255, 255),
    base: BLANCO,
    shadow: rgb(170, 178, 205),
};
/// `toy-amarillo` (the blue toy's headband and sole), flat.
pub const AMARILLO: Tones = Tones {
    light: rgb(255, 232, 120),
    base: rgb(255, 210, 63),
    shadow: rgb(186, 132, 10),
};
/// `monito-tinta` as a surface (pupils).
pub const PUPILA: Tones = Tones {
    light: rgb(37, 44, 79),
    base: TINTA,
    shadow: TINTA,
};
/// The dumbbell: `monito-manga-luz`, `steel-dim`, `monito-manga` (as in `curl_tecnica_dibujo.rs`).
pub const ACERO: Tones = Tones {
    light: rgb(205, 212, 228),
    base: rgb(168, 171, 179),
    shadow: rgb(120, 128, 150),
};
/// The platform's wood: `monito-madera-luz`, `monito-madera`, `monito-madera-frente`.
pub const MADERA: Tones = Tones {
    light: rgb(240, 184, 116),
    base: rgb(222, 158, 88),
    shadow: rgb(122, 70, 32),
};
/// Bone in the x-ray: `monito-blanco`, `monito-hielo`, `monito-azul-luz`.
pub const HUESO: Tones = Tones {
    light: BLANCO,
    base: rgb(222, 246, 255),
    shadow: rgb(160, 240, 255),
};
/// The biceps line in the x-ray: `monito-naranja`, `toy-rojo`, `monito-disco-25-sombra`.
pub const BICEPS: Tones = Tones {
    light: rgb(255, 138, 31),
    base: rgb(255, 59, 59),
    shadow: rgb(150, 20, 28),
};
