//! What a face can and cannot draw — run it before adopting a font.
//!
//! The reel factory's brand was chosen against Pillow on a Mac, where
//! every face happened to have what it needed. A face that reaches the
//! engine has to be checked against the alphabet the pieces actually
//! use, and checked *before* it ships rather than by reading a `'?'` off
//! a rendered frame.
//!
//! ```text
//! cargo run -p motoreel-typeset --example cobertura -- path/to/Face.ttf ...
//! ```

use motoreel_typeset::{Face, Fonts};

/// Everything the published pieces have needed so far, plus the notation
/// the mathematics series will need. Grouped so a report says *what kind*
/// of thing is missing, not just which code point.
const ALFABETOS: &[(&str, &str)] = &[
    ("ascii", " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~"),
    ("español", "áéíóúüñÁÉÍÓÚÜÑ¿¡"),
    ("unidades", "°·×÷−±%‰"),
    ("griego", "αβγδθλμπρστφωΔΘΛΠΣΦΩ"),
    ("lógica", "∀∃¬∧∨→↔⊢⊨≡□◇"),
    ("matemáticas", "∫∑∏√∞≤≥≠≈∂∇∈∉⊂⊆∪∩"),
    ("flechas", "←↑→↓↦⇒⇔"),
];

fn main() {
    let rutas: Vec<String> = std::env::args().skip(1).collect();
    if rutas.is_empty() {
        eprintln!("uso: cobertura <fuente.ttf> [más fuentes...]");
        std::process::exit(2);
    }

    for ruta in &rutas {
        let bytes = match std::fs::read(ruta) {
            Ok(b) => b,
            Err(e) => {
                println!("{ruta}\n  no se pudo leer: {e}\n");
                continue;
            }
        };
        let mut fonts = Fonts::new();
        let face = match Face::load(bytes, ruta.clone()) {
            Ok(f) => f,
            Err(e) => {
                println!("{ruta}\n  {e}\n");
                continue;
            }
        };
        println!("{ruta}");
        let vm = face.vmetrics(100.0);
        println!(
            "  a 100: ascenso {:.1}  descenso {:.1}  interlínea {:.1}",
            vm.ascent, vm.descent, vm.line_gap
        );
        for (nombre, alfabeto) in ALFABETOS {
            let faltan = face.missing(alfabeto);
            let total = alfabeto.chars().count();
            if faltan.is_empty() {
                println!("  {nombre:<12} completo ({total})");
            } else {
                let muestra: String = faltan.iter().take(24).collect();
                println!(
                    "  {nombre:<12} faltan {} de {total}: {muestra}",
                    faltan.len()
                );
            }
        }
        println!();
        fonts.add(face);
    }
}
